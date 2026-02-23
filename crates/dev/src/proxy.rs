use std::collections::HashMap;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use http::{Request, Response, StatusCode};
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use tokio::sync::RwLock;

type Routes = Arc<RwLock<HashMap<String, u16>>>;
type HttpClient = Client<hyper_util::client::legacy::connect::HttpConnector, Full<Bytes>>;

/// Minimal reverse proxy: binds `proxy_port`, routes `<subdomain>.localhost` -> `127.0.0.1:<port>`.
pub struct ProxyRouter {
    port: u16,
    routes: Routes,
}

impl ProxyRouter {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register or update a route (used when port is resolved at startup).
    pub async fn update(&self, subdomain: String, port: u16) {
        self.routes.write().await.insert(subdomain, port);
    }

    pub async fn run(self: Arc<Self>) {
        let addr: SocketAddr = ([127, 0, 0, 1], self.port).into();
        let routes = self.routes.clone();

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("proxy failed to bind on port {}: {e}", self.port);
                return;
            }
        };

        tracing::info!("proxy listening on http://localhost:{}", self.port);

        // Single shared HTTP client — connection pool reused across all requests
        let client: HttpClient = Client::builder(TokioExecutor::new()).build_http();

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("proxy accept error: {e}");
                    continue;
                }
            };

            let routes = routes.clone();
            let client = client.clone();
            tokio::spawn(async move {
                let io = hyper_util::rt::TokioIo::new(stream);
                let svc = hyper::service::service_fn(move |req| {
                    handle(req, routes.clone(), client.clone())
                });
                if let Err(e) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, svc)
                    .await
                {
                    tracing::debug!("proxy connection error: {e}");
                }
            });
        }
    }
}

async fn handle(
    req: Request<Incoming>,
    routes: Routes,
    client: HttpClient,
) -> Result<Response<Full<Bytes>>, Infallible> {
    // Extract subdomain from Host header (e.g. "power.localhost:7080" -> "power")
    let host = req
        .headers()
        .get(http::header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let subdomain = host
        .split(':')
        .next()
        .unwrap_or("")
        .strip_suffix(".localhost")
        .unwrap_or("");

    let upstream_port = routes.read().await.get(subdomain).copied();

    let Some(port) = upstream_port else {
        let body = format!("no route for '{subdomain}.localhost'");
        return Ok(Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from(body)))
            .unwrap_or_default());
    };

    let uri = format!(
        "http://127.0.0.1:{}{}",
        port,
        req.uri()
            .path_and_query()
            .map(|p| p.as_str())
            .unwrap_or("/")
    );

    let (parts, body) = req.into_parts();
    let body_bytes = match body.collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => {
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Full::new(Bytes::from("failed to read request body")))
                .unwrap_or_default());
        }
    };

    let mut upstream_req = Request::builder().method(parts.method).uri(&uri);
    for (k, v) in &parts.headers {
        upstream_req = upstream_req.header(k, v);
    }
    let upstream_req = match upstream_req.body(Full::new(body_bytes)) {
        Ok(r) => r,
        Err(e) => {
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Full::new(Bytes::from(e.to_string())))
                .unwrap_or_default());
        }
    };

    match client.request(upstream_req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            let bytes = body
                .collect()
                .await
                .map(|b| b.to_bytes())
                .unwrap_or_default();
            let mut builder = Response::builder().status(parts.status);
            for (k, v) in &parts.headers {
                builder = builder.header(k, v);
            }
            Ok(builder.body(Full::new(bytes)).unwrap_or_default())
        }
        Err(e) => Ok(Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Full::new(Bytes::from(e.to_string())))
            .unwrap_or_default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_update_and_lookup() {
        let router = ProxyRouter::new(0);
        router.update("web".into(), 3000).await;
        router.update("api".into(), 4000).await;
        let routes = router.routes.read().await;
        assert_eq!(routes.get("web").copied(), Some(3000));
        assert_eq!(routes.get("api").copied(), Some(4000));
        assert_eq!(routes.get("missing"), None);
    }

    #[tokio::test]
    async fn test_update_overwrites() {
        let router = ProxyRouter::new(0);
        router.update("web".into(), 3000).await;
        router.update("web".into(), 3001).await;
        let routes = router.routes.read().await;
        assert_eq!(routes.get("web").copied(), Some(3001));
    }
}
