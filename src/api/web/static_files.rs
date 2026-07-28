use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{
    header::{CACHE_CONTROL, CONTENT_TYPE},
    HeaderValue, StatusCode, Uri,
};
use axum::response::Response;

pub(in crate::api) async fn api_only_fallback() -> Response {
    response_with_status(
        StatusCode::NOT_FOUND,
        "text/plain; charset=utf-8",
        "A3S Code API is running. Web assets are disabled.",
    )
}

pub(in crate::api) async fn serve_static(uri: Uri, root: Arc<PathBuf>) -> Response {
    let Some(path) = static_path(&root, uri.path()) else {
        return response_with_status(
            StatusCode::BAD_REQUEST,
            "text/plain; charset=utf-8",
            "invalid static path",
        );
    };

    match tokio::fs::read(&path).await {
        Ok(body) => response_with_headers(StatusCode::OK, content_type_for(&path), body),
        Err(_) => response_with_status(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            "not found",
        ),
    }
}

fn static_path(root: &Path, request_path: &str) -> Option<PathBuf> {
    let trimmed = request_path.trim_start_matches('/');
    let mut candidate = root.to_path_buf();
    if !trimmed.is_empty() {
        for segment in trimmed.split('/') {
            if segment.is_empty() || segment == "." {
                continue;
            }
            if segment == ".." || segment.contains('\\') {
                return None;
            }
            candidate.push(segment);
        }
    }

    if candidate.is_dir() {
        return Some(candidate.join("index.html"));
    }
    if candidate.is_file() {
        return Some(candidate);
    }
    Some(root.join("index.html"))
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn response_with_status(
    status: StatusCode,
    content_type: &'static str,
    body: &'static str,
) -> Response {
    response_with_headers(status, content_type, body.as_bytes().to_vec())
}

fn response_with_headers(
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );
    response
}
