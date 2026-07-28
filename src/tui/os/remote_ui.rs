//! RemoteUI: surface the `view` (a sized embed widget) that OS's progressive
//! API returns for a task.
//!
//! A `view` is a partial, chrome-less OS surface meant for a *sized popup*
//! rather than a full browser tab. We can't embed a WebView in the terminal, so
//! we spawn the sibling `a3s-webview` helper. OS views seed the exported OS token
//! into localStorage; validated local report views use `--no-auth` and never
//! receive that credential. Plain links still go to the user's browser.

use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

static LOCAL_FILE_SERVER: OnceLock<std::io::Result<LocalFileServer>> = OnceLock::new();
const WEBVIEW_BIN_ENV: &str = "A3S_WEBVIEW_BIN";
const MAX_REGISTERED_LOCAL_FILES: usize = 128;
const MAX_LOCAL_VIEW_BYTES: u64 = 32 * 1024 * 1024;

#[derive(Debug, Default)]
struct LocalFileRegistry {
    files: HashMap<String, PathBuf>,
    order: VecDeque<String>,
}

/// A `viewUrl` (+ optional size / embeddable hint) extracted from a tool result.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ViewSpec {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// The API explicitly marked this view as a sized popup (or returned a size).
    pub embeddable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OpenedWith {
    Webview,
    Browser,
}

/// A live-preview window whose native helper remains owned by the TUI. Plain
/// RemoteUI popups are intentionally detached, but `/preview stop` must be able
/// to close the exact preview window without stopping the shared A3S Web
/// server. Browser fallbacks cannot be closed safely by the host.
pub(crate) enum LivePreviewWindow {
    Webview(Child),
    Browser,
}

impl LivePreviewWindow {
    pub(crate) fn opened_with(&self) -> OpenedWith {
        match self {
            Self::Webview(_) => OpenedWith::Webview,
            Self::Browser => OpenedWith::Browser,
        }
    }

    /// `Some(true)` means the native helper is still running, `Some(false)`
    /// means the user closed it, and `None` denotes an untracked browser tab.
    pub(crate) fn webview_running(&mut self) -> std::io::Result<Option<bool>> {
        match self {
            Self::Webview(child) => child.try_wait().map(|status| Some(status.is_none())),
            Self::Browser => Ok(None),
        }
    }
}

impl Drop for LivePreviewWindow {
    fn drop(&mut self) {
        if let Self::Webview(child) = self {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

/// Build a trusted local-file view after the caller has decided this path is
/// safe to surface. Generic tool output must still flow through `find_view_url`,
/// which intentionally rejects `file://` URLs.
pub(crate) fn local_file_view(path: &Path) -> std::io::Result<ViewSpec> {
    let path = path.canonicalize()?;
    let url = local_file_server()?.register(path)?;
    Ok(ViewSpec {
        url,
        width: Some(1200),
        height: Some(820),
        embeddable: true,
    })
}

/// Build a no-auth RemoteUI popup for a validated local image attachment.
pub(crate) fn local_image_view(
    path: &Path,
    pixel_width: u32,
    pixel_height: u32,
) -> std::io::Result<ViewSpec> {
    let path = path.canonicalize()?;
    let url = local_file_server()?.register(path)?;
    let (width, height) = image_window_size(pixel_width, pixel_height);
    Ok(ViewSpec {
        url,
        width: Some(width),
        height: Some(height),
        embeddable: true,
    })
}

fn image_window_size(pixel_width: u32, pixel_height: u32) -> (u32, u32) {
    const MIN_WIDTH: u32 = 360;
    const MIN_HEIGHT: u32 = 240;
    const MAX_WIDTH: u32 = 1200;
    const MAX_HEIGHT: u32 = 820;

    let pixel_width = pixel_width.max(1);
    let pixel_height = pixel_height.max(1);
    let scale = (MAX_WIDTH as f64 / pixel_width as f64)
        .min(MAX_HEIGHT as f64 / pixel_height as f64)
        .min(1.0);
    let width = (pixel_width as f64 * scale).round() as u32;
    let height = (pixel_height as f64 * scale).round() as u32;
    (
        width.clamp(MIN_WIDTH, MAX_WIDTH),
        height.clamp(MIN_HEIGHT, MAX_HEIGHT),
    )
}

#[derive(Debug)]
struct LocalFileServer {
    origin: String,
    files: std::sync::Arc<Mutex<LocalFileRegistry>>,
}

impl Clone for LocalFileServer {
    fn clone(&self) -> Self {
        Self {
            origin: self.origin.clone(),
            files: self.files.clone(),
        }
    }
}

impl LocalFileServer {
    fn start() -> std::io::Result<Self> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let port = listener.local_addr()?.port();
        let files = std::sync::Arc::new(Mutex::new(LocalFileRegistry::default()));
        let thread_files = files.clone();
        thread::Builder::new()
            .name("a3s-local-remoteui".to_string())
            .spawn(move || serve_local_files(listener, thread_files))
            .map_err(|err| std::io::Error::new(err.kind(), err.to_string()))?;
        Ok(Self {
            origin: format!("http://127.0.0.1:{port}"),
            files,
        })
    }

    fn register(&self, path: PathBuf) -> std::io::Result<String> {
        let mut registry = self
            .files
            .lock()
            .map_err(|_| std::io::Error::other("local RemoteUI file registry poisoned"))?;
        let id = loop {
            let candidate = format!("{:032x}", rand::random::<u128>());
            if !registry.files.contains_key(&candidate) {
                break candidate;
            }
        };
        registry.files.insert(id.clone(), path.clone());
        registry.order.push_back(id.clone());
        while registry.order.len() > MAX_REGISTERED_LOCAL_FILES {
            if let Some(expired) = registry.order.pop_front() {
                registry.files.remove(&expired);
            }
        }
        drop(registry);
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("index.html");
        Ok(format!(
            "{}/a3s-local-view/{}/{}",
            self.origin,
            id,
            percent_encode_file_url_path(name)
        ))
    }
}

fn local_file_server() -> std::io::Result<LocalFileServer> {
    match LOCAL_FILE_SERVER.get_or_init(LocalFileServer::start) {
        Ok(server) => Ok(server.clone()),
        Err(err) => Err(std::io::Error::new(err.kind(), err.to_string())),
    }
}

fn serve_local_files(listener: TcpListener, files: std::sync::Arc<Mutex<LocalFileRegistry>>) {
    for stream in listener.incoming().flatten() {
        let _ = handle_local_file_request(stream, &files);
    }
}

fn handle_local_file_request(
    mut stream: TcpStream,
    files: &std::sync::Arc<Mutex<LocalFileRegistry>>,
) -> std::io::Result<()> {
    let request = read_http_request_head(&mut stream)?;
    let path = request
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some("GET"), Some(path)) => Some(path),
                _ => None,
            }
        })
        .unwrap_or("/");
    let id = path
        .strip_prefix("/a3s-local-view/")
        .and_then(|rest| rest.split('/').next())
        .filter(|id| !id.is_empty());
    let Some(id) = id else {
        return write_local_file_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found",
        );
    };
    let file = files
        .lock()
        .ok()
        .and_then(|registry| registry.files.get(id).cloned());
    let Some(file) = file else {
        return write_local_file_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found",
        );
    };
    let Ok(metadata) = std::fs::metadata(&file) else {
        return write_local_file_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found",
        );
    };
    if !metadata.is_file() || metadata.len() > MAX_LOCAL_VIEW_BYTES {
        return write_local_file_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found",
        );
    }
    let Ok(bytes) = std::fs::read(&file) else {
        return write_local_file_response(
            &mut stream,
            404,
            "text/plain; charset=utf-8",
            b"not found",
        );
    };
    write_local_file_response(&mut stream, 200, content_type_for(&file), &bytes)
}

fn read_http_request_head(stream: &mut TcpStream) -> std::io::Result<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut request = Vec::with_capacity(1024);
    let mut buf = [0_u8; 1024];
    while request.len() < 8192 {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                request.extend_from_slice(&buf[..n]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(String::from_utf8_lossy(&request).into_owned())
}

fn write_local_file_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Error",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nContent-Security-Policy: default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:; script-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body)?;
    stream.flush()?;
    let _ = stream.shutdown(Shutdown::Write);
    Ok(())
}

fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}

/// Find a renderable view in a tool's JSON output. Prefers the current `view`
/// object `{ url, width, height }`; falls back to a legacy top-level `viewUrl`
/// (+ optional `viewSize` / `embeddable`). The capabilities API nests it under
/// `data` too, so we walk recursively and take the first match.
///
/// The progressive API returns a RELATIVE `url` (`/admin/…?embed=1`) by the OS's
/// "store relative, complete at the edge" convention — the TUI IS the edge, so we
/// absolutize it against `origin` (the signed-in OS origin). Without this every
/// capabilities view is silently dropped, since the webview needs an absolute URL.
pub(crate) fn find_view_url(output: &str, origin: Option<&str>) -> Option<ViewSpec> {
    // Tool stdout is usually one JSON doc, but a bash block may emit several
    // (e.g. a `list` then an `execute`). Scan every parseable JSON value and take
    // the LAST that carries a view — the freshest result the user just ran. This
    // also tolerates concatenated docs that a single `from_str` would reject.
    serde_json::Deserializer::from_str(output)
        .into_iter::<serde_json::Value>()
        .flatten()
        .filter_map(|v| find_in(&v, origin))
        .last()
}

/// Accept an absolute `http(s)://` url as-is, or complete a root-relative
/// `/path` against `origin`. Anything else (relative with no origin, `mailto:`,
/// …) yields `None` so we never hand the webview a URL it can't open.
fn absolutize(url: &str, origin: Option<&str>) -> Option<String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url.to_string())
    } else if url.starts_with('/') {
        origin.map(|o| format!("{}{}", o.trim_end_matches('/'), url))
    } else {
        None
    }
}

fn percent_encode_file_url_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for b in path.as_bytes() {
        let safe = matches!(
            b,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'_'
                | b'.'
                | b'~'
                | b'/'
                | b':'
        );
        if safe {
            out.push(*b as char);
        } else {
            out.push_str(&format!("%{:02X}", *b));
        }
    }
    out
}

fn find_in(value: &serde_json::Value, origin: Option<&str>) -> Option<ViewSpec> {
    match value {
        serde_json::Value::Object(obj) => {
            // Current OS shape: a `view` object `{ url, width, height }` — a
            // focused, chrome-less embed widget at a suggested size.
            if let Some(spec) = obj.get("view").and_then(|v| parse_view_object(v, origin)) {
                return Some(spec);
            }
            // Back-compat: a bare top-level `viewUrl` (+ optional `viewSize` /
            // `embeddable`), the shape the API returned before the `view` object.
            if let Some(spec) = parse_legacy_view_url(obj, origin) {
                return Some(spec);
            }
            obj.values().find_map(|v| find_in(v, origin))
        }
        serde_json::Value::Array(arr) => arr.iter().find_map(|v| find_in(v, origin)),
        _ => None,
    }
}

/// Read a JSON number (int or float) as a pixel dimension.
fn px(obj: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<u32> {
    obj.get(key)
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f.round() as u64)))
        .map(|n| n as u32)
}

/// Parse the current `view` object `{ url, width, height }`. The API only emits
/// it for sized popups, so a parsed `view` is always embeddable.
fn parse_view_object(v: &serde_json::Value, origin: Option<&str>) -> Option<ViewSpec> {
    let obj = v.as_object()?;
    let url = obj.get("url").and_then(|u| u.as_str())?;
    Some(ViewSpec {
        url: absolutize(url, origin)?,
        width: px(obj, "width"),
        height: px(obj, "height"),
        embeddable: true,
    })
}

/// Back-compat: the older top-level `viewUrl` string with an optional `viewSize`
/// `{width,height}` sibling and `embeddable` flag.
fn parse_legacy_view_url(
    obj: &serde_json::Map<String, serde_json::Value>,
    origin: Option<&str>,
) -> Option<ViewSpec> {
    let url = obj.get("viewUrl").and_then(|u| u.as_str())?;
    let url = absolutize(url, origin)?;
    let size = obj.get("viewSize").and_then(|s| s.as_object());
    let width = size.and_then(|s| px(s, "width"));
    let height = size.and_then(|s| px(s, "height"));
    let embeddable = obj
        .get("embeddable")
        .and_then(|e| e.as_bool())
        .unwrap_or(false)
        || width.is_some();
    Some(ViewSpec {
        url,
        width,
        height,
        embeddable,
    })
}

/// Locate the `a3s-webview` binary: prefer an explicit env override, then a
/// sibling of the running `a3s` executable (how it ships), then source-tree dev
/// builds, then PATH.
fn webview_binary_name() -> &'static str {
    if cfg!(windows) {
        "a3s-webview.exe"
    } else {
        "a3s-webview"
    }
}

fn executable_path(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        Some(path.to_path_buf())
    } else {
        None
    }
}

fn find_on_paths(name: &str, paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    paths
        .into_iter()
        .filter_map(|directory| executable_path(&directory.join(name)))
        .collect()
}

fn find_all_on_path(name: &str) -> Vec<PathBuf> {
    let Some(paths) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    find_on_paths(name, std::env::split_paths(&paths))
}

fn env_webview_override() -> Option<PathBuf> {
    let raw = std::env::var_os(WEBVIEW_BIN_ENV)?;
    if raw.is_empty() {
        None
    } else {
        Some(PathBuf::from(raw))
    }
}

fn dev_webview_candidates(manifest_dir: &Path, name: &str) -> Vec<PathBuf> {
    vec![
        manifest_dir.join("target/debug").join(name),
        manifest_dir.join("target/release").join(name),
        manifest_dir.join("crates/webview/target/debug").join(name),
        manifest_dir
            .join("crates/webview/target/release")
            .join(name),
    ]
}

fn find_dev_webviews(name: &str) -> Vec<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    dev_webview_candidates(manifest_dir, name)
        .into_iter()
        .filter_map(|path| executable_path(&path))
        .collect()
}

pub(crate) fn webview_helper_candidates() -> (bool, Vec<PathBuf>) {
    let name = webview_binary_name();
    if let Some(path) = env_webview_override() {
        // Honor an explicit override even before the file exists; spawn will
        // produce the concrete path error instead of silently using another bin.
        return (true, vec![path]);
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(sibling) = exe.parent().map(|d| d.join(name)) {
            if let Some(path) = executable_path(&sibling) {
                candidates.push(path);
            }
        }
    }
    for path in find_dev_webviews(name) {
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }
    for path in find_all_on_path(name) {
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }
    (false, candidates)
}

fn find_existing_webview() -> Option<PathBuf> {
    webview_helper_candidates().1.into_iter().next()
}

pub(crate) fn webview_helper_path_with(preferred: Option<&Path>) -> Option<PathBuf> {
    if let Some(override_path) = env_webview_override() {
        return executable_path(&override_path);
    }
    preferred
        .and_then(executable_path)
        .or_else(|| find_existing_webview().filter(|path| executable_path(path).is_some()))
}

fn resolve_webview_bin(preferred: Option<&Path>) -> PathBuf {
    env_webview_override()
        .or_else(|| preferred.map(Path::to_path_buf))
        .or_else(find_existing_webview)
        .unwrap_or_else(|| PathBuf::from(webview_binary_name()))
}

/// Build the `a3s-webview` argv for a view (url + optional size). Split out from
/// spawning so the spec→argv mapping is unit-testable.
fn webview_args(spec: &ViewSpec) -> Vec<String> {
    let local_view = is_local_file_view(spec);
    let title = if is_local_image_view(spec) {
        "A3S Code · Image preview"
    } else if is_local_report_view(spec) {
        "A3S Research Report"
    } else {
        "A3S RemoteUI"
    };
    let mut args = vec![
        "--url".to_string(),
        spec.url.clone(),
        "--title".to_string(),
        title.to_string(),
    ];
    if local_view {
        // Local content must never receive OS access or refresh tokens, even
        // though it is served over a loopback HTTP origin for the native view.
        args.push("--no-auth".to_string());
    }
    if let Some(w) = spec.width {
        args.push("--width".to_string());
        args.push(w.to_string());
    }
    if let Some(h) = spec.height {
        args.push("--height".to_string());
        args.push(h.to_string());
    }
    args
}

fn live_preview_webview_args(url: &str) -> Vec<String> {
    vec![
        "--url".to_string(),
        url.to_string(),
        "--title".to_string(),
        "A3S Live Preview".to_string(),
        // The preview shell and every artifact it embeds are local. Never
        // export OS access or refresh tokens into this window.
        "--no-auth".to_string(),
        "--width".to_string(),
        "1440".to_string(),
        "--height".to_string(),
        "960".to_string(),
    ]
}

pub(crate) fn is_local_file_view(spec: &ViewSpec) -> bool {
    local_view_requires_no_auth(&spec.url)
}

pub(crate) fn is_local_image_view(spec: &ViewSpec) -> bool {
    is_local_file_view(spec) && url_has_image_extension(&spec.url)
}

pub(crate) fn is_local_report_view(spec: &ViewSpec) -> bool {
    is_local_file_view(spec) && !is_local_image_view(spec)
}

fn url_has_image_extension(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    [".png", ".jpg", ".jpeg", ".gif", ".webp"]
        .iter()
        .any(|extension| path.to_ascii_lowercase().ends_with(extension))
}

fn local_view_requires_no_auth(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    let loopback = ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
        .into_iter()
        .find_map(|prefix| lower.strip_prefix(prefix));
    loopback
        .and_then(|rest| rest.find('/').map(|path_at| &rest[path_at..]))
        .is_some_and(|path| path.starts_with("/a3s-local-view/"))
}

/// Open a view's url in the native `a3s-webview` window (detached), falling back
/// to the system browser when the helper is not installed or cannot launch.
/// The webview inherits the process env so OS views can read `A3S_OS_TOKEN` for
/// auth; registered local files receive `--no-auth` and ignore it.
pub(crate) fn open_window(spec: &ViewSpec) -> std::io::Result<OpenedWith> {
    open_window_with(spec, None)
}

pub(crate) fn open_window_with(
    spec: &ViewSpec,
    preferred: Option<&Path>,
) -> std::io::Result<OpenedWith> {
    Command::new(resolve_webview_bin(preferred))
        .args(webview_args(spec))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_child| OpenedWith::Webview)
        .or_else(|webview_error| {
            open_in_browser(&spec.url)
                .map(|()| OpenedWith::Browser)
                .map_err(|browser_error| {
                    std::io::Error::new(
                        browser_error.kind(),
                        format!(
                            "a3s-webview failed: {webview_error}; browser fallback failed: {browser_error}"
                        ),
                    )
                })
        })
}

/// Open the persistent A3S live-preview shell. Unlike ordinary RemoteUI
/// popups, the native child is returned to the TUI so `/preview stop`, preview
/// replacement, and TUI shutdown can close precisely that window.
pub(crate) fn open_live_preview_window_with(
    url: &str,
    preferred: Option<&Path>,
) -> std::io::Result<LivePreviewWindow> {
    Command::new(resolve_webview_bin(preferred))
        .args(live_preview_webview_args(url))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(LivePreviewWindow::Webview)
        .or_else(|webview_error| {
            open_in_browser(url)
                .map(|()| LivePreviewWindow::Browser)
                .map_err(|browser_error| {
                    std::io::Error::new(
                        browser_error.kind(),
                        format!(
                            "a3s-webview failed: {webview_error}; browser fallback failed: {browser_error}"
                        ),
                    )
                })
        })
}

fn browser_open_command(url: &str) -> (&'static str, Vec<String>) {
    if cfg!(target_os = "macos") {
        ("open", vec![url.to_string()])
    } else if cfg!(windows) {
        (
            "cmd",
            vec![
                "/C".to_string(),
                "start".to_string(),
                String::new(),
                url.to_string(),
            ],
        )
    } else {
        ("xdg-open", vec![url.to_string()])
    }
}

fn open_in_browser(url: &str) -> std::io::Result<()> {
    let (program, args) = browser_open_command(url);
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_child| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_top_level_view_url() {
        let out = r#"{"success":true,"viewUrl":"https://os.x/p","data":{"items":[]}}"#;
        let s = find_view_url(out, None).unwrap();
        assert_eq!(s.url, "https://os.x/p");
        assert!(!s.embeddable); // no size / flag
    }

    #[test]
    fn finds_nested_view_url_with_size_marks_embeddable() {
        let out =
            r#"{"data":{"viewUrl":"https://os.x/embed","viewSize":{"width":720,"height":520}}}"#;
        let s = find_view_url(out, None).unwrap();
        assert_eq!((s.width, s.height), (Some(720), Some(520)));
        assert!(s.embeddable); // size present ⇒ embeddable
    }

    #[test]
    fn embeddable_flag_without_size() {
        let out = r#"{"viewUrl":"https://os.x/p","embeddable":true}"#;
        assert!(find_view_url(out, None).unwrap().embeddable);
    }

    #[test]
    fn finds_view_object_marks_embeddable() {
        let out = r#"{"success":true,"view":{"url":"https://os.x/p?embed=1","width":720,"height":520},"modules":[]}"#;
        let s = find_view_url(out, None).unwrap();
        assert_eq!(s.url, "https://os.x/p?embed=1");
        assert_eq!((s.width, s.height), (Some(720), Some(520)));
        assert!(s.embeddable); // a `view` object is always a sized popup
    }

    #[test]
    fn finds_nested_view_object() {
        let out = r#"{"data":{"view":{"url":"https://os.x/embed","width":400,"height":300}}}"#;
        assert_eq!(find_view_url(out, None).unwrap().width, Some(400));
    }

    #[test]
    fn view_object_takes_precedence_over_legacy_url() {
        let out = r#"{"viewUrl":"https://old/x","view":{"url":"https://new/y","width":300,"height":200}}"#;
        assert_eq!(find_view_url(out, None).unwrap().url, "https://new/y");
    }

    #[test]
    fn relative_view_url_is_absolutized_against_origin() {
        // The OS progressive API returns a RELATIVE url; the TUI (the edge)
        // completes it. This is the common real-world shape.
        let out = r#"{"success":true,"view":{"url":"/admin/kernel/assets?embed=1","width":1440,"height":900}}"#;
        let s = find_view_url(out, Some("https://os.example.com/")).unwrap();
        assert_eq!(s.url, "https://os.example.com/admin/kernel/assets?embed=1"); // trailing / trimmed
        assert!(s.embeddable);
    }

    #[test]
    fn last_view_wins_across_concatenated_json_docs() {
        // A bash block that ran `list` then `execute` emits two JSON docs; the
        // freshest (execute, with the view) must win.
        let out = r#"{"success":true,"modules":[]}
{"success":true,"view":{"url":"/admin/assets/a1?embed=1","width":1024,"height":768}}"#;
        let s = find_view_url(out, Some("https://os.x")).unwrap();
        assert_eq!(s.url, "https://os.x/admin/assets/a1?embed=1");
    }

    #[test]
    fn relative_view_url_without_origin_is_dropped() {
        // No signed-in origin ⇒ we can't complete it; better none than a broken url.
        let out = r#"{"view":{"url":"/admin/kernel/assets?embed=1","width":10,"height":10}}"#;
        assert!(find_view_url(out, None).is_none());
    }

    #[test]
    fn ignores_non_http_and_absent() {
        assert!(find_view_url(r#"{"viewUrl":"file:///x"}"#, None).is_none());
        assert!(find_view_url(
            r#"{"view":{"url":"file:///x","width":10,"height":10}}"#,
            None
        )
        .is_none());
        assert!(find_view_url(r#"{"data":{"items":[1,2]}}"#, None).is_none());
        assert!(find_view_url("not json", None).is_none());
    }

    #[test]
    fn trusted_local_file_view_uses_local_http_server() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-local-view-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("report with space.html");
        std::fs::write(&path, "<!doctype html>").unwrap();

        let spec = local_file_view(&path).unwrap();
        assert!(spec.url.starts_with("http://127.0.0.1:"), "{spec:?}");
        assert!(spec.url.contains("/a3s-local-view/"), "{spec:?}");
        assert!(spec.url.ends_with("report%20with%20space.html"), "{spec:?}");
        assert_eq!((spec.width, spec.height), (Some(1200), Some(820)));
        assert!(spec.embeddable);
        let response = fetch_local_test_url(&spec.url);
        assert!(response.contains("<!doctype html"), "{response}");
        assert!(
            response.contains("Content-Security-Policy: default-src 'none'"),
            "{response}"
        );
        assert!(response.contains("script-src 'none'"), "{response}");
        assert!(response.contains("connect-src 'none'"), "{response}");

        let args = webview_args(&spec);
        assert!(args.iter().any(|arg| arg == "--no-auth"), "{args:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn local_image_view_uses_preview_title_mime_and_bounded_size() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-local-image-view-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("clipboard image.png");
        std::fs::write(&path, b"png-test-body").unwrap();

        let spec = local_image_view(&path, 2400, 1600).unwrap();
        assert!(is_local_file_view(&spec));
        assert!(is_local_image_view(&spec));
        assert!(!is_local_report_view(&spec));
        assert_eq!((spec.width, spec.height), (Some(1200), Some(800)));

        let response = fetch_local_test_url(&spec.url);
        assert!(response.contains("Content-Type: image/png"), "{response}");
        assert!(response.contains("png-test-body"), "{response}");

        let args = webview_args(&spec);
        assert!(args.iter().any(|arg| arg == "--no-auth"), "{args:?}");
        assert!(
            args.iter().any(|arg| arg == "A3S Code · Image preview"),
            "{args:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_preview_window_keeps_small_images_clickable() {
        assert_eq!(image_window_size(32, 16), (360, 240));
        assert_eq!(image_window_size(800, 600), (800, 600));
    }

    fn fetch_local_test_url(url: &str) -> String {
        let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
        let (port, path) = rest.split_once('/').unwrap();
        let mut stream = TcpStream::connect(("127.0.0.1", port.parse::<u16>().unwrap())).unwrap();
        write!(
            stream,
            "GET /{path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    }

    #[test]
    fn webview_args_pass_url_and_size() {
        let spec = ViewSpec {
            url: "https://os.x/p?embed=1".into(),
            width: Some(720),
            height: Some(520),
            embeddable: true,
        };
        assert_eq!(
            webview_args(&spec),
            vec![
                "--url",
                "https://os.x/p?embed=1",
                "--title",
                "A3S RemoteUI",
                "--width",
                "720",
                "--height",
                "520"
            ]
        );
        let no_size = ViewSpec {
            url: "https://os.x/p".into(),
            width: None,
            height: None,
            embeddable: false,
        };
        assert_eq!(
            webview_args(&no_size),
            vec!["--url", "https://os.x/p", "--title", "A3S RemoteUI"]
        );

        let local = ViewSpec {
            url: "http://127.0.0.1:4321/a3s-local-view/id/index.html".into(),
            width: None,
            height: None,
            embeddable: true,
        };
        assert_eq!(
            webview_args(&local),
            vec![
                "--url",
                "http://127.0.0.1:4321/a3s-local-view/id/index.html",
                "--title",
                "A3S Research Report",
                "--no-auth"
            ]
        );

        let authenticated_loopback = ViewSpec {
            url: "http://127.0.0.1:4321/runtime/view".into(),
            width: None,
            height: None,
            embeddable: true,
        };
        assert_eq!(
            webview_args(&authenticated_loopback),
            vec![
                "--url",
                "http://127.0.0.1:4321/runtime/view",
                "--title",
                "A3S RemoteUI"
            ],
            "ordinary loopback OS/development views may still require auth"
        );
    }

    #[test]
    fn live_preview_webview_is_no_auth_and_uses_a_product_window() {
        let args =
            live_preview_webview_args("http://127.0.0.1:29653/?preview=site%2Findex.html#home");

        assert_eq!(
            args,
            vec![
                "--url",
                "http://127.0.0.1:29653/?preview=site%2Findex.html#home",
                "--title",
                "A3S Live Preview",
                "--no-auth",
                "--width",
                "1440",
                "--height",
                "960",
            ]
        );
    }

    #[test]
    fn browser_fallback_command_tracks_platform() {
        let (program, args) = browser_open_command("https://os.x/p?embed=1");
        if cfg!(target_os = "macos") {
            assert_eq!(program, "open");
            assert_eq!(args, vec!["https://os.x/p?embed=1"]);
        } else if cfg!(windows) {
            assert_eq!(program, "cmd");
            assert_eq!(args, vec!["/C", "start", "", "https://os.x/p?embed=1"]);
        } else {
            assert_eq!(program, "xdg-open");
            assert_eq!(args, vec!["https://os.x/p?embed=1"]);
        }
    }

    #[test]
    fn dev_webview_candidates_include_root_cli_and_webview_targets() {
        let root = Path::new("/repo");
        let candidates = dev_webview_candidates(root, "a3s-webview");

        assert!(candidates.contains(&PathBuf::from("/repo/target/debug/a3s-webview")));
        assert!(candidates.contains(&PathBuf::from(
            "/repo/crates/webview/target/release/a3s-webview"
        )));
    }

    #[test]
    fn path_webview_candidates_preserve_every_match_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let first_directory = temp.path().join("first");
        let missing_directory = temp.path().join("missing");
        let second_directory = temp.path().join("second");
        std::fs::create_dir_all(&first_directory).unwrap();
        std::fs::create_dir_all(&second_directory).unwrap();
        let first = first_directory.join("a3s-webview");
        let second = second_directory.join("a3s-webview");
        std::fs::write(&first, b"stale helper").unwrap();
        std::fs::write(&second, b"compatible helper").unwrap();

        let candidates = find_on_paths(
            "a3s-webview",
            [first_directory, missing_directory, second_directory],
        );

        assert_eq!(candidates, vec![first, second]);
    }

    #[test]
    fn webview_binary_name_tracks_platform() {
        if cfg!(windows) {
            assert_eq!(webview_binary_name(), "a3s-webview.exe");
        } else {
            assert_eq!(webview_binary_name(), "a3s-webview");
        }
    }

    #[test]
    fn managed_webview_path_is_used_when_no_override_is_configured() {
        let preferred = Path::new("/managed/a3s-webview");
        assert_eq!(
            resolve_webview_bin(Some(preferred)),
            env_webview_override().unwrap_or_else(|| preferred.to_path_buf())
        );
    }

    /// End-to-end: a progressive-API `execute` response carrying a `view` object
    /// parses into a ViewSpec whose url + size reach the a3s-webview argv — i.e.
    /// the view's url is what gets opened in the webview.
    #[test]
    fn progressive_api_view_flows_to_webview_args() {
        let resp = r#"{"success":true,
            "view":{"url":"/admin/kernel/assets?embed=1","width":900,"height":680},
            "data":{"items":[]}}"#;
        let spec =
            find_view_url(resp, Some("https://os.example.com")).expect("view object should parse");
        assert!(spec.embeddable); // a `view` is always a sized popup → auto-opens
        let args = webview_args(&spec);
        assert_eq!(args[0], "--url");
        assert_eq!(
            args[1],
            "https://os.example.com/admin/kernel/assets?embed=1"
        );
        assert!(args.contains(&"900".to_string()) && args.contains(&"680".to_string()));
    }
}
