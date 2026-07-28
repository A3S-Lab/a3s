//! Network implementation for the Codex Responses WebSocket and HTTPS/SSE
//! transports.

use super::proxy::{self, ProxyRoute};
use super::{TransportError, TransportKind, WireClient, WireRequest, WireStream};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use futures::{SinkExt, StreamExt};
use reqwest::cookie::{CookieStore, Jar};
use reqwest::header::{HeaderMap, HeaderValue, RETRY_AFTER};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Error as WebSocketError, Message};
use tokio_tungstenite::{client_async_tls_with_config, Connector};
use tokio_util::sync::CancellationToken;

const WS_BETA_VALUE: &str = "responses_websockets=2026-02-06";

#[derive(Debug, Default)]
struct CloudflareCookieStore {
    jar: Jar,
}

impl CookieStore for CloudflareCookieStore {
    fn set_cookies(
        &self,
        cookie_headers: &mut dyn Iterator<Item = &HeaderValue>,
        url: &reqwest::Url,
    ) {
        if !is_chatgpt_https_url(url) {
            return;
        }
        let mut allowed = cookie_headers.filter(|header| {
            header
                .to_str()
                .ok()
                .and_then(|value| value.split_once('=').map(|(name, _)| name.trim()))
                .is_some_and(is_allowed_cloudflare_cookie)
        });
        self.jar.set_cookies(&mut allowed, url);
    }

    fn cookies(&self, url: &reqwest::Url) -> Option<HeaderValue> {
        if !is_chatgpt_https_url(url) {
            return None;
        }
        let raw = self.jar.cookies(url)?;
        let filtered = raw
            .to_str()
            .ok()?
            .split(';')
            .filter_map(|cookie| {
                let cookie = cookie.trim();
                let (name, _) = cookie.split_once('=')?;
                is_allowed_cloudflare_cookie(name.trim()).then_some(cookie)
            })
            .collect::<Vec<_>>()
            .join("; ");
        (!filtered.is_empty())
            .then(|| HeaderValue::from_str(&filtered).ok())
            .flatten()
    }
}

impl CloudflareCookieStore {
    fn cookie_snapshot(&self, url: &reqwest::Url) -> Option<Vec<u8>> {
        <Self as CookieStore>::cookies(self, url).map(|value| value.as_bytes().to_vec())
    }

    fn websocket_cookie_header(
        &self,
        websocket_url: &reqwest::Url,
    ) -> Option<tokio_tungstenite::tungstenite::http::HeaderValue> {
        let cookie_url = proxy_target_url(websocket_url).ok()?;
        let cookie = <Self as CookieStore>::cookies(self, &cookie_url)?;
        tokio_tungstenite::tungstenite::http::HeaderValue::from_bytes(cookie.as_bytes()).ok()
    }

    fn store_websocket_headers(
        &self,
        websocket_url: &reqwest::Url,
        headers: &tokio_tungstenite::tungstenite::http::HeaderMap,
    ) {
        let Ok(cookie_url) = proxy_target_url(websocket_url) else {
            return;
        };
        let converted = headers
            .get_all("set-cookie")
            .iter()
            .filter_map(|value| HeaderValue::from_bytes(value.as_bytes()).ok())
            .collect::<Vec<_>>();
        let mut values = converted.iter();
        <Self as CookieStore>::set_cookies(self, &mut values, &cookie_url);
    }

    fn store_websocket_error(&self, websocket_url: &reqwest::Url, error: &WebSocketError) {
        if let WebSocketError::Http(response) = error {
            self.store_websocket_headers(websocket_url, response.headers());
        }
    }
}

fn is_chatgpt_https_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url
            .host_str()
            .is_some_and(|host| host == "chatgpt.com" || host.ends_with(".chatgpt.com"))
}

fn is_allowed_cloudflare_cookie(name: &str) -> bool {
    matches!(
        name,
        "__cf_bm"
            | "__cflb"
            | "__cfruid"
            | "__cfseq"
            | "__cfwaitingroom"
            | "_cfuvid"
            | "cf_clearance"
            | "cf_ob_info"
            | "cf_use_ob"
    ) || name.starts_with("cf_chl_")
}

static CLOUDFLARE_COOKIES: LazyLock<Arc<CloudflareCookieStore>> =
    LazyLock::new(|| Arc::new(CloudflareCookieStore::default()));

pub(in crate::account_providers::codex) struct NetworkWireClient {
    tls_roots: super::super::tls::TlsRoots,
    tls_connector: Connector,
}

impl NetworkWireClient {
    pub(in crate::account_providers::codex) fn new() -> anyhow::Result<Self> {
        let tls_roots = super::super::tls::TlsRoots::load()?;
        let tls_connector = Connector::Rustls(Arc::new(tls_roots.rustls_client_config()?));
        Ok(Self {
            tls_roots,
            tls_connector,
        })
    }

    fn request_headers(request: &WireRequest) -> Result<HeaderMap, TransportError> {
        let mut headers = HeaderMap::new();
        for (name, value) in &request.headers {
            let name =
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
                    TransportError::protocol(format!("invalid header name: {error}"))
                })?;
            let value = HeaderValue::from_str(value).map_err(|error| {
                TransportError::protocol(format!("invalid header value: {error}"))
            })?;
            headers.insert(name, value);
        }
        Ok(headers)
    }

    fn http_client(&self, route: &ProxyRoute) -> Result<reqwest::Client, TransportError> {
        let mut builder = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&CLOUDFLARE_COOKIES))
            .connect_timeout(Duration::from_secs(20));
        builder = match route {
            ProxyRoute::Direct => builder.no_proxy(),
            ProxyRoute::Http(url) | ProxyRoute::Socks(url) => {
                builder.proxy(reqwest::Proxy::all(url.as_str()).map_err(|error| {
                    TransportError::protocol(format!("invalid Codex proxy URL: {error}"))
                })?)
            }
            ProxyRoute::Unsupported(scheme) => {
                return Err(TransportError::network(format!(
                    "unsupported Codex proxy scheme: {scheme}"
                )))
            }
        };
        builder = self
            .tls_roots
            .add_to_reqwest(builder)
            .map_err(|error| TransportError::protocol(error.to_string()))?;
        builder
            .build()
            .map_err(|error| TransportError::network(format!("build Codex HTTP client: {error}")))
    }
}

async fn resolve_route(url: &reqwest::Url) -> Result<ProxyRoute, TransportError> {
    let url = url.clone();
    tokio::task::spawn_blocking(move || proxy::resolve(&url))
        .await
        .map_err(|error| TransportError::network(format!("resolve Codex proxy route: {error}")))
}

fn proxy_target_url(websocket_url: &reqwest::Url) -> Result<reqwest::Url, TransportError> {
    let mut target = websocket_url.clone();
    let scheme = match target.scheme() {
        "wss" => "https",
        "ws" => "http",
        _ => return Ok(target),
    };
    target
        .set_scheme(scheme)
        .map_err(|_| TransportError::protocol("failed to normalize Codex proxy target"))?;
    Ok(target)
}

async fn connect_proxy_tunnel(
    route: &ProxyRoute,
    target: &reqwest::Url,
    cancel: &CancellationToken,
) -> Result<TcpStream, TransportError> {
    match route {
        ProxyRoute::Http(proxy_url) if proxy_url.scheme() == "http" => {
            connect_http_proxy(proxy_url, target, cancel).await
        }
        ProxyRoute::Http(proxy_url) => Err(TransportError::network(format!(
            "WebSocket tunneling through a {} proxy is unavailable; HTTPS fallback will be used",
            proxy_url.scheme()
        ))),
        ProxyRoute::Socks(proxy_url) => connect_socks_proxy(proxy_url, target, cancel).await,
        ProxyRoute::Unsupported(scheme) => Err(TransportError::network(format!(
            "unsupported Codex proxy scheme: {scheme}"
        ))),
        ProxyRoute::Direct => Err(TransportError::protocol(
            "direct Codex route unexpectedly requested a proxy tunnel",
        )),
    }
}

async fn connect_http_proxy(
    proxy: &reqwest::Url,
    target: &reqwest::Url,
    cancel: &CancellationToken,
) -> Result<TcpStream, TransportError> {
    let proxy_address = url_authority(proxy, 80)?;
    let target_authority = url_authority(target, 443)?;
    let mut socket = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(TransportError::cancelled()),
        result = TcpStream::connect(&proxy_address) => result.map_err(|error| {
            TransportError::network(format!("connect Codex HTTP proxy {proxy_address}: {error}"))
        })?,
    };

    let mut request = format!(
        "CONNECT {target_authority} HTTP/1.1\r\nHost: {target_authority}\r\nProxy-Connection: Keep-Alive\r\n"
    );
    if !proxy.username().is_empty() {
        let password = proxy.password().unwrap_or_default();
        let credentials = STANDARD.encode(format!("{}:{password}", proxy.username()));
        request.push_str(&format!("Proxy-Authorization: Basic {credentials}\r\n"));
    }
    request.push_str("\r\n");
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|error| TransportError::network(format!("write Codex proxy CONNECT: {error}")))?;

    let mut response = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        if response.len() > 64 * 1024 {
            return Err(TransportError::protocol(
                "Codex proxy CONNECT response headers exceeded 64 KiB",
            ));
        }
        let read = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TransportError::cancelled()),
            result = socket.read(&mut chunk) => result.map_err(|error| {
                TransportError::network(format!("read Codex proxy CONNECT: {error}"))
            })?,
        };
        if read == 0 {
            return Err(TransportError::stream_closed(
                "Codex proxy closed during CONNECT",
            ));
        }
        response.extend_from_slice(&chunk[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let status_line = String::from_utf8_lossy(&response)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| TransportError::protocol("invalid Codex proxy CONNECT response"))?;
    if status != 200 {
        return Err(TransportError::http(status, None, None));
    }
    Ok(socket)
}

async fn connect_socks_proxy(
    proxy: &reqwest::Url,
    target: &reqwest::Url,
    cancel: &CancellationToken,
) -> Result<TcpStream, TransportError> {
    let proxy_address = url_authority(proxy, 1080)?;
    let target_host = target
        .host_str()
        .ok_or_else(|| TransportError::protocol("Codex target has no host"))?;
    let target_port = target.port_or_known_default().unwrap_or(443);
    let connected = if proxy.username().is_empty() {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TransportError::cancelled()),
            result = tokio_socks::tcp::Socks5Stream::connect(
                proxy_address.as_str(),
                (target_host, target_port),
            ) => result,
        }
    } else {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TransportError::cancelled()),
            result = tokio_socks::tcp::Socks5Stream::connect_with_password(
                proxy_address.as_str(),
                (target_host, target_port),
                proxy.username(),
                proxy.password().unwrap_or_default(),
            ) => result,
        }
    };
    connected
        .map(tokio_socks::tcp::Socks5Stream::into_inner)
        .map_err(|error| TransportError::network(format!("connect Codex SOCKS proxy: {error}")))
}

fn url_authority(url: &reqwest::Url, default_port: u16) -> Result<String, TransportError> {
    let host = url
        .host_str()
        .ok_or_else(|| TransportError::protocol("Codex proxy URL has no host"))?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    Ok(format!("{host}:{}", url.port().unwrap_or(default_port)))
}

#[async_trait]
impl WireClient for NetworkWireClient {
    async fn open_websocket(
        &self,
        request: &WireRequest,
        cancel: CancellationToken,
    ) -> Result<WireStream, TransportError> {
        let ws_url = websocket_url(&request.endpoint)?;
        let mut handshake = ws_url.as_str().into_client_request().map_err(|error| {
            TransportError::protocol(format!("build WebSocket request: {error}"))
        })?;
        for (name, value) in &request.headers {
            let name =
                tokio_tungstenite::tungstenite::http::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|error| {
                        TransportError::protocol(format!("invalid WebSocket header: {error}"))
                    })?;
            let value = tokio_tungstenite::tungstenite::http::HeaderValue::from_str(value)
                .map_err(|error| {
                    TransportError::protocol(format!("invalid WebSocket header value: {error}"))
                })?;
            handshake.headers_mut().insert(name, value);
        }
        handshake.headers_mut().insert(
            "openai-beta",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(WS_BETA_VALUE),
        );
        if !handshake.headers().contains_key("cookie") {
            if let Some(cookie) = CLOUDFLARE_COOKIES.websocket_cookie_header(&ws_url) {
                handshake.headers_mut().insert("cookie", cookie);
            }
        }

        let route_url = proxy_target_url(&ws_url)?;
        let route = resolve_route(&route_url).await?;
        let connected = match route {
            ProxyRoute::Direct => tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(TransportError::cancelled()),
                result = tokio_tungstenite::connect_async_tls_with_config(
                    handshake,
                    None,
                    false,
                    Some(self.tls_connector.clone()),
                ) => result,
            },
            route => {
                let tunnel = connect_proxy_tunnel(&route, &ws_url, &cancel).await?;
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err(TransportError::cancelled()),
                    result = client_async_tls_with_config(
                        handshake,
                        tunnel,
                        None,
                        Some(self.tls_connector.clone()),
                    ) => result,
                }
            }
        };
        let (mut socket, response) = match connected {
            Ok(connected) => connected,
            Err(error) => {
                CLOUDFLARE_COOKIES.store_websocket_error(&ws_url, &error);
                return Err(map_websocket_error(error));
            }
        };
        CLOUDFLARE_COOKIES.store_websocket_headers(&ws_url, response.headers());

        let mut envelope =
            request.body.as_object().cloned().ok_or_else(|| {
                TransportError::protocol("Codex request body must be a JSON object")
            })?;
        envelope.insert(
            "type".to_string(),
            Value::String("response.create".to_string()),
        );
        let payload = serde_json::to_string(&Value::Object(envelope)).map_err(|error| {
            TransportError::protocol(format!("encode WebSocket request: {error}"))
        })?;
        socket
            .send(Message::Text(payload.into()))
            .await
            .map_err(map_websocket_error)?;

        let (tx, rx) = mpsc::channel(256);
        tokio::spawn(async move {
            let mut completed = false;
            loop {
                let next = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        let _ = socket.close(None).await;
                        let _ = tx.send(Err(TransportError::cancelled())).await;
                        return;
                    }
                    next = socket.next() => next,
                };
                match next {
                    Some(Ok(Message::Text(text))) => {
                        if forward_json_event(text.as_str(), &tx, &mut completed)
                            .await
                            .is_err()
                        {
                            return;
                        }
                        if completed {
                            return;
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => match std::str::from_utf8(bytes.as_ref()) {
                        Ok(text) => {
                            if forward_json_event(text, &tx, &mut completed).await.is_err() {
                                return;
                            }
                            if completed {
                                return;
                            }
                        }
                        Err(error) => {
                            let _ = tx
                                .send(Err(TransportError::protocol(format!(
                                    "Codex WebSocket sent non-UTF-8 data: {error}"
                                ))))
                                .await;
                            return;
                        }
                    },
                    Some(Ok(Message::Ping(payload))) => {
                        if let Err(error) = socket.send(Message::Pong(payload)).await {
                            let _ = tx.send(Err(map_websocket_error(error))).await;
                            return;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        if !completed {
                            let _ = tx
                                .send(Err(TransportError::stream_closed(
                                    "Codex WebSocket closed before response.completed",
                                )))
                                .await;
                        }
                        return;
                    }
                    Some(Ok(_)) => {}
                    Some(Err(error)) => {
                        let _ = tx.send(Err(map_websocket_error(error))).await;
                        return;
                    }
                }
            }
        });

        Ok(WireStream {
            kind: TransportKind::WebSocket,
            events: rx,
        })
    }

    async fn open_http_sse(
        &self,
        request: &WireRequest,
        cancel: CancellationToken,
    ) -> Result<WireStream, TransportError> {
        let headers = Self::request_headers(request)?;
        let endpoint = reqwest::Url::parse(&request.endpoint).map_err(|error| {
            TransportError::protocol(format!("invalid Codex endpoint: {error}"))
        })?;
        let route = resolve_route(&endpoint).await?;
        let http = self.http_client(&route)?;
        let mut retried_cloudflare_challenge = false;
        let mut previous_cookies = CLOUDFLARE_COOKIES.cookie_snapshot(&endpoint);
        let response = loop {
            let response = tokio::select! {
                biased;
                _ = cancel.cancelled() => return Err(TransportError::cancelled()),
                result = http
                    .post(&request.endpoint)
                    .headers(headers.clone())
                    .json(&request.body)
                    .send() => {
                    result.map_err(|error| TransportError::network(format!("Codex HTTPS request failed: {error}")))?
                }
            };
            let status = response.status().as_u16();
            let current_cookies = CLOUDFLARE_COOKIES.cookie_snapshot(&endpoint);
            if should_retry_cloudflare_challenge(
                status,
                retried_cloudflare_challenge,
                previous_cookies.as_deref(),
                current_cookies.as_deref(),
            ) {
                retried_cloudflare_challenge = true;
                previous_cookies = current_cookies;
                tracing::warn!("Retrying Codex HTTPS request once with updated Cloudflare cookies");
                continue;
            }
            break response;
        };
        let status = response.status().as_u16();
        let retry_after = parse_retry_after(response.headers());
        if !(200..300).contains(&status) {
            let body = response.text().await.unwrap_or_default();
            return Err(TransportError::http(status, Some(body), retry_after));
        }

        let (tx, rx) = mpsc::channel(256);
        let mut bytes = response.bytes_stream();
        tokio::spawn(async move {
            let mut buffer = Vec::new();
            let mut completed = false;
            loop {
                let next = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        let _ = tx.send(Err(TransportError::cancelled())).await;
                        return;
                    }
                    next = bytes.next() => next,
                };
                match next {
                    Some(Ok(chunk)) => {
                        buffer.extend_from_slice(&chunk);
                        while let Some((frame_end, separator_len)) = find_sse_frame(&buffer) {
                            let frame = buffer.drain(..frame_end).collect::<Vec<_>>();
                            buffer.drain(..separator_len);
                            if let Err(error) = forward_sse_frame(&frame, &tx, &mut completed).await
                            {
                                let _ = tx.send(Err(error)).await;
                                return;
                            }
                            if completed {
                                return;
                            }
                        }
                    }
                    Some(Err(error)) => {
                        let _ = tx
                            .send(Err(TransportError::network(format!(
                                "Codex HTTPS stream failed: {error}"
                            ))))
                            .await;
                        return;
                    }
                    None => {
                        if !completed {
                            let _ = tx
                                .send(Err(TransportError::stream_closed(
                                    "Codex HTTPS stream closed before response.completed",
                                )))
                                .await;
                        }
                        return;
                    }
                }
            }
        });

        Ok(WireStream {
            kind: TransportKind::HttpSse,
            events: rx,
        })
    }
}

fn should_retry_cloudflare_challenge(
    status: u16,
    already_retried: bool,
    previous_cookies: Option<&[u8]>,
    current_cookies: Option<&[u8]>,
) -> bool {
    status == 403
        && !already_retried
        && current_cookies.is_some()
        && current_cookies != previous_cookies
}

fn websocket_url(endpoint: &str) -> Result<reqwest::Url, TransportError> {
    let mut url = reqwest::Url::parse(endpoint)
        .map_err(|error| TransportError::protocol(format!("invalid Codex endpoint: {error}")))?;
    let scheme = match url.scheme() {
        "https" => "wss",
        "http" => "ws",
        "wss" | "ws" => return Ok(url),
        scheme => {
            return Err(TransportError::protocol(format!(
                "unsupported Codex endpoint scheme: {scheme}"
            )))
        }
    };
    url.set_scheme(scheme)
        .map_err(|_| TransportError::protocol("failed to set Codex WebSocket scheme"))?;
    Ok(url)
}

fn map_websocket_error(error: WebSocketError) -> TransportError {
    match error {
        WebSocketError::Http(response) => {
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after_value);
            let body = response
                .body()
                .as_ref()
                .and_then(|body| String::from_utf8(body.to_vec()).ok());
            TransportError::http(status, body, retry_after)
        }
        WebSocketError::ConnectionClosed | WebSocketError::AlreadyClosed => {
            TransportError::stream_closed("Codex WebSocket connection closed")
        }
        other => TransportError::network(format!("Codex WebSocket failed: {other}")),
    }
}

async fn forward_json_event(
    text: &str,
    tx: &mpsc::Sender<Result<Value, TransportError>>,
    completed: &mut bool,
) -> Result<(), ()> {
    let event = match serde_json::from_str::<Value>(text) {
        Ok(event) => event,
        Err(error) => {
            let _ = tx
                .send(Err(TransportError::protocol(format!(
                    "decode Codex WebSocket event: {error}"
                ))))
                .await;
            return Err(());
        }
    };
    *completed = event.get("type").and_then(Value::as_str) == Some("response.completed");
    tx.send(Ok(event)).await.map_err(|_| ())
}

fn find_sse_frame(buffer: &[u8]) -> Option<(usize, usize)> {
    buffer
        .windows(2)
        .position(|window| window == b"\n\n")
        .map(|position| (position, 2))
        .or_else(|| {
            buffer
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .map(|position| (position, 4))
        })
}

async fn forward_sse_frame(
    frame: &[u8],
    tx: &mpsc::Sender<Result<Value, TransportError>>,
    completed: &mut bool,
) -> Result<(), TransportError> {
    let text = std::str::from_utf8(frame)
        .map_err(|error| TransportError::protocol(format!("decode Codex SSE frame: {error}")))?;
    let data = text
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
        .collect::<Vec<_>>()
        .join("\n");
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }
    let event = serde_json::from_str::<Value>(&data)
        .map_err(|error| TransportError::protocol(format!("decode Codex SSE event: {error}")))?;
    *completed = event.get("type").and_then(Value::as_str) == Some("response.completed");
    tx.send(Ok(event))
        .await
        .map_err(|_| TransportError::stream_closed("Codex event consumer closed"))
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(parse_retry_after_value)
}

fn parse_retry_after_value(value: &str) -> Option<Duration> {
    let value = value.trim();
    value
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
        .or_else(|| {
            httpdate::parse_http_date(value)
                .ok()?
                .duration_since(std::time::SystemTime::now())
                .ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_https_endpoint_to_wss() {
        assert_eq!(
            websocket_url("https://chatgpt.com/backend-api/codex/responses")
                .unwrap()
                .as_str(),
            "wss://chatgpt.com/backend-api/codex/responses"
        );
    }

    #[test]
    fn cloudflare_allowlist_rejects_account_cookies() {
        assert!(is_allowed_cloudflare_cookie("cf_clearance"));
        assert!(is_allowed_cloudflare_cookie("cf_chl_rc_i"));
        assert!(!is_allowed_cloudflare_cookie("chatgpt_session"));
        assert!(!is_allowed_cloudflare_cookie(
            "__Secure-next-auth.session-token"
        ));
    }

    #[test]
    fn websocket_handshake_cookies_round_trip_through_the_allowlisted_store() {
        let store = CloudflareCookieStore::default();
        let url = reqwest::Url::parse("wss://chatgpt.com/backend-api/codex/responses").unwrap();
        let mut headers = tokio_tungstenite::tungstenite::http::HeaderMap::new();
        headers.append(
            "set-cookie",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(
                "_cfuvid=visitor; Path=/; Secure; HttpOnly",
            ),
        );
        headers.append(
            "set-cookie",
            tokio_tungstenite::tungstenite::http::HeaderValue::from_static(
                "chatgpt_session=secret; Path=/; Secure; HttpOnly",
            ),
        );

        store.store_websocket_headers(&url, &headers);

        let cookie = store
            .websocket_cookie_header(&url)
            .and_then(|value| value.to_str().ok().map(str::to_string));
        assert_eq!(cookie.as_deref(), Some("_cfuvid=visitor"));
    }

    #[test]
    fn cloudflare_challenge_retry_requires_a_new_cookie_and_runs_once() {
        assert!(should_retry_cloudflare_challenge(
            403,
            false,
            None,
            Some(b"_cfuvid=visitor"),
        ));
        assert!(!should_retry_cloudflare_challenge(
            403,
            true,
            None,
            Some(b"_cfuvid=visitor"),
        ));
        assert!(!should_retry_cloudflare_challenge(
            403,
            false,
            Some(b"_cfuvid=visitor"),
            Some(b"_cfuvid=visitor"),
        ));
        assert!(!should_retry_cloudflare_challenge(
            500,
            false,
            None,
            Some(b"_cfuvid=visitor"),
        ));
    }

    #[tokio::test]
    async fn parses_crlf_sse_frames() {
        let (tx, mut rx) = mpsc::channel(2);
        let mut completed = false;
        forward_sse_frame(
            b"event: message\r\ndata: {\"type\":\"response.completed\"}",
            &tx,
            &mut completed,
        )
        .await
        .unwrap();

        assert!(completed);
        assert_eq!(
            rx.recv().await.unwrap().unwrap()["type"],
            "response.completed"
        );
    }

    #[tokio::test]
    async fn http_connect_proxy_creates_a_bidirectional_tunnel() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 1024];
            loop {
                let read = socket.read(&mut chunk).await.unwrap();
                request.extend_from_slice(&chunk[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            socket
                .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                .await
                .unwrap();
            let mut payload = [0_u8; 4];
            socket.read_exact(&mut payload).await.unwrap();
            socket.write_all(b"pong").await.unwrap();
            (String::from_utf8_lossy(&request).into_owned(), payload)
        });
        let proxy = reqwest::Url::parse(&format!("http://user:pass@{address}")).unwrap();
        let target = reqwest::Url::parse("wss://chatgpt.com/backend-api/codex/responses").unwrap();

        let mut tunnel = connect_http_proxy(&proxy, &target, &CancellationToken::new())
            .await
            .unwrap();
        tunnel.write_all(b"ping").await.unwrap();
        let mut reply = [0_u8; 4];
        tunnel.read_exact(&mut reply).await.unwrap();
        let (request, payload) = server.await.unwrap();

        assert_eq!(&reply, b"pong");
        assert_eq!(&payload, b"ping");
        assert!(request.starts_with("CONNECT chatgpt.com:443 HTTP/1.1\r\n"));
        assert!(request.contains("Proxy-Authorization: Basic dXNlcjpwYXNz\r\n"));
    }

    #[test]
    fn retry_after_accepts_seconds_and_http_dates() {
        assert_eq!(parse_retry_after_value("7"), Some(Duration::from_secs(7)));
        let future = std::time::SystemTime::now() + Duration::from_secs(60);
        let parsed = parse_retry_after_value(&httpdate::fmt_http_date(future)).unwrap();
        assert!((58..=60).contains(&parsed.as_secs()));
    }
}
