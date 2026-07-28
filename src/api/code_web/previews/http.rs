use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{
    header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_SECURITY_POLICY, CONTENT_TYPE},
    HeaderName, HeaderValue, StatusCode,
};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use super::model::PreviewContentSource;
use super::registry::PreviewRegistry;

const MAX_PREVIEW_ASSET_BYTES: u64 = 64 * 1024 * 1024;
const PREVIEW_CSP: &str = "default-src 'self' data: blob: http: https:; script-src 'self' 'unsafe-inline' 'unsafe-eval' blob: http: https:; style-src 'self' 'unsafe-inline' data: http: https:; img-src 'self' data: blob: http: https:; font-src 'self' data: http: https:; media-src 'self' data: blob: http: https:; connect-src 'self' data: blob: http://localhost:* https://localhost:* ws://localhost:* wss://localhost:* http://127.0.0.1:* https://127.0.0.1:* ws://127.0.0.1:* wss://127.0.0.1:* http://[::1]:* https://[::1]:* ws://[::1]:* wss://[::1]:*; frame-src http: https:; object-src 'none'; base-uri 'none'; sandbox allow-scripts allow-forms allow-modals allow-popups allow-downloads";

pub(in crate::api) fn content_router(registry: Arc<PreviewRegistry>) -> Router {
    Router::new()
        .route("/preview/{token}", get(serve_root))
        .route("/preview/{token}/", get(serve_root))
        .route("/preview/{token}/{*path}", get(serve_asset))
        .with_state(registry)
}

async fn serve_root(
    State(registry): State<Arc<PreviewRegistry>>,
    AxumPath(token): AxumPath<String>,
) -> Response {
    serve_content(&registry, &token, "").await
}

async fn serve_asset(
    State(registry): State<Arc<PreviewRegistry>>,
    AxumPath((token, path)): AxumPath<(String, String)>,
) -> Response {
    serve_content(&registry, &token, &path).await
}

pub(super) async fn serve_content(
    registry: &PreviewRegistry,
    token: &str,
    request_path: &str,
) -> Response {
    let Some(source) = registry.content(token).await else {
        return text_response(StatusCode::NOT_FOUND, "preview session not found");
    };
    let path = match resolve_content_path(source, request_path).await {
        Ok(path) => path,
        Err(status) => return text_response(status, status.canonical_reason().unwrap_or("error")),
    };
    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return text_response(StatusCode::NOT_FOUND, "preview asset not found"),
    };
    if metadata.len() > MAX_PREVIEW_ASSET_BYTES {
        return text_response(StatusCode::PAYLOAD_TOO_LARGE, "preview asset is too large");
    }
    match tokio::fs::read(&path).await {
        Ok(body) => preview_response(&path, body),
        Err(_) => text_response(StatusCode::NOT_FOUND, "preview asset not found"),
    }
}

async fn resolve_content_path(
    source: PreviewContentSource,
    request_path: &str,
) -> Result<PathBuf, StatusCode> {
    match source {
        PreviewContentSource::File { path } => {
            if request_path.is_empty() {
                Ok(path)
            } else {
                Err(StatusCode::NOT_FOUND)
            }
        }
        PreviewContentSource::Directory { root, entry } => {
            if request_path.is_empty() {
                return Ok(entry);
            }
            let segments = safe_segments(request_path)?;
            let mut requested = root.clone();
            for segment in segments {
                requested.push(segment);
            }
            let mut canonical = tokio::fs::canonicalize(&requested)
                .await
                .map_err(|_| StatusCode::NOT_FOUND)?;
            if !canonical.starts_with(&root) {
                return Err(StatusCode::FORBIDDEN);
            }
            if canonical.is_dir() {
                canonical = tokio::fs::canonicalize(canonical.join("index.html"))
                    .await
                    .map_err(|_| StatusCode::NOT_FOUND)?;
                if !canonical.starts_with(&root) {
                    return Err(StatusCode::FORBIDDEN);
                }
            }
            Ok(canonical)
        }
    }
}

fn safe_segments(request_path: &str) -> Result<Vec<&str>, StatusCode> {
    let mut segments = Vec::new();
    for segment in request_path.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "."
            || segment == ".."
            || segment.starts_with('.')
            || segment.contains(['\\', '\0'])
            || is_sensitive_file(segment)
        {
            return Err(StatusCode::FORBIDDEN);
        }
        segments.push(segment);
    }
    Ok(segments)
}

fn is_sensitive_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "id_rsa" | "id_dsa" | "id_ecdsa" | "id_ed25519" | "credentials" | "credentials.json"
    ) || matches!(
        Path::new(&lower)
            .extension()
            .and_then(|value| value.to_str()),
        Some("pem" | "key" | "p12" | "pfx")
    )
}

fn preview_response(path: &Path, body: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static(content_type_for(path)),
    );
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("no-cache, no-store, must-revalidate"),
    );
    response
        .headers_mut()
        .insert(CONTENT_DISPOSITION, HeaderValue::from_static("inline"));
    response.headers_mut().insert(
        HeaderName::from_static("x-content-type-options"),
        HeaderValue::from_static("nosniff"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("referrer-policy"),
        HeaderValue::from_static("no-referrer"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), usb=(), serial=()"),
    );
    if is_html(path) {
        response.headers_mut().insert(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(PREVIEW_CSP),
        );
    }
    response
}

fn text_response(status: StatusCode, body: &'static str) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn is_html(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("html" | "htm")
    )
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "tsx" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "xml" => "application/xml",
        "txt" | "md" | "markdown" | "yaml" | "yml" | "toml" | "acl" | "log" | "rs" | "py"
        | "sh" | "c" | "h" | "cc" | "cpp" | "java" | "go" | "sql" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "webmanifest" => "application/manifest+json",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "csv" => "text/csv; charset=utf-8",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odt" => "application/vnd.oasis.opendocument.text",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        _ => "application/octet-stream",
    }
}
