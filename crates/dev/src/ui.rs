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
        (Method::GET, "/api/logs") => {
            // SSE stream
            let service_filter: Option<String> = query
                .split('&')
                .find(|p| p.starts_with("service="))
                .map(|p| p["service=".len()..].to_string());

            let rx = sup.subscribe_logs();
            let stream = BroadcastStream::new(rx).filter_map(move |item| {
                match item {
                    Ok(entry) => {
                        if service_filter
                            .as_deref()
                            .is_none_or(|f| f == entry.service)
                        {
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
                }
            });

            Response::builder()
                .header("content-type", "text/event-stream")
                .header("cache-control", "no-cache")
                .header("access-control-allow-origin", "*")
                .body(
                    StreamBody::new(stream)
                        .map_err(|e| e)
                        .boxed(),
                )
                .unwrap()
        }
        (Method::POST, p) if p.starts_with("/api/restart/") => {
            let name = urldecode(&p["/api/restart/".len()..]);
            match sup.restart_service(&name).await {
                Ok(_) => full_response(
                    "application/json",
                    b"{\"ok\":true}".to_vec(),
                ),
                Err(e) => {
                    let body = serde_json::to_vec(
                        &serde_json::json!({"error": e.to_string()})
                    ).unwrap_or_default();
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
            .body(Full::new(Bytes::from_static(b"not found")).map_err(|e| e).boxed())
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
    s.replace("%20", " ")
        .replace("%2F", "/")
        .replace("%2B", "+")
}

static INDEX_HTML: &str = include_str!("ui.html");
