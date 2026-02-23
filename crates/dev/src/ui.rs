use std::convert::Infallible;
use std::sync::Arc;

use bytes::Bytes;
use http::{Method, Request, Response, StatusCode};
use http_body_util::{combinators::BoxBody, BodyExt, Full, StreamBody};
use hyper::body::Frame;
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::supervisor::Supervisor;

pub const DEFAULT_UI_PORT: u16 = 10350;

type BoxResp = Response<BoxBody<Bytes, Infallible>>;

pub async fn serve(sup: Arc<Supervisor>, port: u16) {
    let addr = format!("127.0.0.1:{port}");
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("UI server bind failed on {addr}: {e}");
            return;
        }
    };
    tracing::debug!("UI server at http://{addr}");

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("UI accept error: {e}");
                continue;
            }
        };
        let sup = sup.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let svc = hyper::service::service_fn(move |req| handle(req, sup.clone()));
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!("UI connection error: {e}");
            }
        });
    }
}

async fn handle(
    req: Request<hyper::body::Incoming>,
    sup: Arc<Supervisor>,
) -> Result<BoxResp, Infallible> {
    let path = req.uri().path().to_string();
    let query = req.uri().query().unwrap_or("").to_string();
    let method = req.method().clone();

    let resp = match (method, path.as_str()) {
        (Method::GET, "/") | (Method::GET, "/index.html") => {
            full_response("text/html; charset=utf-8", INDEX_HTML.as_bytes().to_vec())
        }
        (Method::GET, "/api/status") => {
            let rows = sup.status_rows().await;
            let body = serde_json::to_vec(&rows).unwrap_or_default();
            full_response("application/json", body)
        }
        (Method::GET, "/api/history") => {
            let service_filter: Option<String> = query
                .split('&')
                .find(|p| p.starts_with("service="))
                .map(|p| p["service=".len()..].to_string());
            let recent = sup.log_history(service_filter.as_deref(), 200);
            let body = serde_json::to_vec(&recent).unwrap_or_default();
            full_response("application/json", body)
        }
        (Method::GET, "/api/logs") => {
            // SSE stream
            let service_filter: Option<String> = query
                .split('&')
                .find(|p| p.starts_with("service="))
                .map(|p| p["service=".len()..].to_string());

            let rx = sup.subscribe_logs();
            let stream = BroadcastStream::new(rx).filter_map(move |item| match item {
                Ok(entry) => {
                    if service_filter.as_deref().is_none_or(|f| f == entry.service) {
                        let payload = serde_json::json!({
                            "service": entry.service,
                            "line": entry.line,
                        });
                        let data = format!("data: {}\n\n", payload);
                        Some(Ok::<_, Infallible>(Frame::data(Bytes::from(data))))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            });

            Response::builder()
                .header("content-type", "text/event-stream")
                .header("cache-control", "no-cache")
                .header("access-control-allow-origin", "*")
                .body(StreamBody::new(stream).map_err(|e| e).boxed())
                .unwrap_or_default()
        }
        (Method::POST, p) if p.starts_with("/api/restart/") => {
            let name = urldecode(&p["/api/restart/".len()..]);
            match sup.restart_service(&name).await {
                Ok(_) => full_response("application/json", b"{\"ok\":true}".to_vec()),
                Err(e) => {
                    let body = format!("{{\"error\":\"{}\"}}", e).into_bytes();
                    Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .header("content-type", "application/json")
                        .body(Full::new(Bytes::from(body)).map_err(|e| e).boxed())
                        .unwrap()
                }
            }
        }
        (Method::POST, p) if p.starts_with("/api/stop/") => {
            let name = urldecode(&p["/api/stop/".len()..]);
            sup.stop_service(&name).await;
            full_response("application/json", b"{\"ok\":true}".to_vec())
        }
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(
                Full::new(Bytes::from_static(b"not found"))
                    .map_err(|e| e)
                    .boxed(),
            )
            .unwrap(),
    };

    Ok(resp)
}

fn full_response(content_type: &str, body: Vec<u8>) -> BoxResp {
    Response::builder()
        .header("content-type", content_type)
        .header("access-control-allow-origin", "*")
        .body(Full::new(Bytes::from(body)).map_err(|e| e).boxed())
        .unwrap()
}

fn urldecode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b as char);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

static INDEX_HTML: &str = include_str!("ui.html");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_urldecode_plain() {
        assert_eq!(urldecode("hello"), "hello");
    }

    #[test]
    fn test_urldecode_space() {
        assert_eq!(urldecode("hello%20world"), "hello world");
    }

    #[test]
    fn test_urldecode_slash() {
        assert_eq!(urldecode("a%2Fb"), "a/b");
    }

    #[test]
    fn test_urldecode_plus() {
        assert_eq!(urldecode("a%2Bb"), "a+b");
    }

    #[test]
    fn test_urldecode_equals_and_ampersand() {
        assert_eq!(urldecode("a%3Db%26c"), "a=b&c");
    }

    #[test]
    fn test_urldecode_incomplete_sequence_passthrough() {
        // Incomplete %xx — pass through as-is
        assert_eq!(urldecode("a%2"), "a%2");
        assert_eq!(urldecode("a%"), "a%");
    }

    #[test]
    fn test_urldecode_invalid_hex_passthrough() {
        // Non-hex chars after % — pass through
        assert_eq!(urldecode("a%ZZb"), "a%ZZb");
    }

    #[test]
    fn test_urldecode_service_name() {
        assert_eq!(urldecode("my-service"), "my-service");
        assert_eq!(urldecode("svc%5F1"), "svc_1");
    }
}
