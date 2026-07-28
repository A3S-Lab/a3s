#![allow(dead_code)]

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use sha2::{Digest, Sha256};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

pub struct TempWorkspace {
    root: PathBuf,
}

impl TempWorkspace {
    pub fn new(name: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("a3s-cli-{name}-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap_or_else(|error| {
            panic!(
                "failed to create temp workspace {}: {error}",
                root.display()
            )
        });
        Self { root }
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

impl Drop for TempWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub fn a3s_bin() -> &'static str {
    env!("CARGO_BIN_EXE_a3s")
}

pub fn configure_component_env(command: &mut std::process::Command, workspace: &TempWorkspace) {
    command
        .env("A3S_DATA_HOME", workspace.path("data"))
        .env("A3S_STATE_HOME", workspace.path("state"))
        .env("A3S_CACHE_HOME", workspace.path("cache"))
        .env("A3S_RUNTIME_HOME", workspace.path("runtime"))
        .env("HOME", workspace.path("home"))
        .env("PATH", "");
}

pub fn box_release_target() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "macos-arm64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "x86_64") => "linux-x86_64",
        _ => return None,
    })
}

pub fn make_executable(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|error| {
            panic!(
                "failed to create parent directory {}: {error}",
                parent.display()
            )
        });
    }
    std::fs::write(path, body)
        .unwrap_or_else(|error| panic!("failed to write executable {}: {error}", path.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(
            |error| panic!("failed to chmod executable {}: {error}", path.display()),
        );
    }
}

pub fn sh_quote(path: &Path) -> String {
    format!("'{}'", path.display().to_string().replace('\'', "'\\''"))
}

pub fn start_fake_box_release(
    workspace: &TempWorkspace,
    version: &str,
    installed_args_log: Option<&Path>,
) -> FakeReleaseServer {
    let target = box_release_target().expect("test host must support a Box release");
    let package_name = format!("a3s-box-v{version}-{target}");
    let package_root = workspace.path("release").join(&package_name);
    let installed_log_line = installed_args_log
        .map(|path| format!("printf '%s\\n' \"$@\" >> {}\n", sh_quote(path)))
        .unwrap_or_default();
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-box {version}\\n'\n  exit 0\nfi\n{installed_log_line}printf 'installed-box:%s\\n' \"$*\"\nexit 0\n"
    );
    make_executable(&package_root.join("a3s-box"), &script);

    let archive_name = format!("{package_name}.tar.gz");
    let archive_path = workspace.path(&archive_name);
    let status = std::process::Command::new("tar")
        .arg("czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(workspace.path("release"))
        .arg(&package_name)
        .status()
        .expect("failed to run tar for release fixture");
    assert!(status.success(), "failed to create release fixture");
    let archive = std::fs::read(&archive_path).expect("failed to read release fixture");
    FakeReleaseServer::start("Box", version, &archive_name, archive)
}

pub fn portable_release_target() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x86_64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "x86_64") => "linux-x86_64",
        ("windows", "x86_64") => "windows-x86_64",
        _ => return None,
    })
}

pub struct FakeReleaseServer {
    api_base: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeReleaseServer {
    pub fn start(repository: &str, version: &str, asset_name: &str, archive: Vec<u8>) -> Self {
        Self::start_internal(repository, version, asset_name, archive, None, false)
    }

    pub fn start_with_digest(
        repository: &str,
        version: &str,
        asset_name: &str,
        archive: Vec<u8>,
        advertised_digest: &str,
    ) -> Self {
        Self::start_internal(
            repository,
            version,
            asset_name,
            archive,
            Some(advertised_digest),
            false,
        )
    }

    pub fn start_with_forbidden_api(
        repository: &str,
        version: &str,
        asset_name: &str,
        archive: Vec<u8>,
    ) -> Self {
        Self::start_internal(repository, version, asset_name, archive, None, true)
    }

    fn start_internal(
        repository: &str,
        version: &str,
        asset_name: &str,
        archive: Vec<u8>,
        advertised_digest: Option<&str>,
        api_forbidden: bool,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind release server");
        listener
            .set_nonblocking(true)
            .expect("failed to configure release server");
        let address = listener.local_addr().unwrap();
        let api_base = format!("http://{address}");
        let digest = advertised_digest
            .map(str::to_string)
            .unwrap_or_else(|| format!("{:x}", Sha256::digest(&archive)));
        let checksum = format!("{digest}  {asset_name}\n").into_bytes();
        let release = serde_json::to_vec(&serde_json::json!({
            "tag_name": format!("v{version}"),
            "body": "fixture",
            "assets": [{
                "name": asset_name,
                "browser_download_url": format!("{api_base}/assets/{asset_name}"),
                "digest": format!("sha256:{digest}")
            }]
        }))
        .unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let thread_requests = Arc::clone(&requests);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let asset_path = format!("/assets/{asset_name}");
        let latest_path = format!("/repos/A3S-Lab/{repository}/releases/latest");
        let tag_path = format!("/repos/A3S-Lab/{repository}/releases/tags/v{version}");
        let direct_asset_path =
            format!("/A3S-Lab/{repository}/releases/download/v{version}/{asset_name}");
        let direct_checksum_path = format!("{direct_asset_path}.sha256");
        let response = Arc::new(FakeReleaseResponse {
            release,
            asset_path,
            latest_path,
            tag_path,
            direct_asset_path,
            direct_checksum_path,
            archive,
            checksum,
            api_forbidden,
            requests: Arc::clone(&thread_requests),
        });
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        // BSD-derived platforms may propagate O_NONBLOCK from
                        // the listener to accepted sockets. The fixture reads
                        // each request on a worker thread, so make that contract
                        // explicit; otherwise a transient WouldBlock closes the
                        // connection before reqwest receives the response.
                        stream
                            .set_nonblocking(false)
                            .expect("failed to configure release connection");
                        let response = Arc::clone(&response);
                        std::thread::spawn(move || {
                            serve_request(stream, &response);
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            api_base,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub fn api_base(&self) -> &str {
        &self.api_base
    }

    pub fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }
}

impl Drop for FakeReleaseServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct FakeReleaseResponse {
    release: Vec<u8>,
    asset_path: String,
    latest_path: String,
    tag_path: String,
    direct_asset_path: String,
    direct_checksum_path: String,
    archive: Vec<u8>,
    checksum: Vec<u8>,
    api_forbidden: bool,
    requests: Arc<Mutex<Vec<String>>>,
}

fn serve_request(mut stream: TcpStream, response: &FakeReleaseResponse) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buffer = [0_u8; 8192];
    let Ok(size) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..size]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();
    response.requests.lock().unwrap().push(path.clone());
    let (status, content_type, body) =
        if path == response.asset_path || path == response.direct_asset_path {
            ("200 OK", "application/gzip", response.archive.as_slice())
        } else if path == response.direct_checksum_path {
            ("200 OK", "text/plain", response.checksum.as_slice())
        } else if path == response.latest_path || path == response.tag_path {
            if response.api_forbidden {
                (
                    "403 Forbidden",
                    "application/json",
                    br#"{"message":"API rate limit exceeded"}"#.as_slice(),
                )
            } else {
                ("200 OK", "application/json", response.release.as_slice())
            }
        } else {
            ("404 Not Found", "text/plain", b"not found".as_slice())
        };
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = Vec::with_capacity(header.len() + body.len());
    response.extend_from_slice(header.as_bytes());
    response.extend_from_slice(body);
    if stream.write_all(&response).is_ok() {
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Write);
    }
}
