use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

mod support;

use support::{a3s_bin, FakeReleaseServer, TempWorkspace};

#[test]
fn cargo_style_install_downloads_verified_web_assets_once_and_reuses_them_offline() {
    let fixture = CargoWebFixture::new("cargo-web-assets");
    let archive_name = web_archive_name();
    let archive = web_release_archive("<!doctype html><title>A3S downloaded Web workspace</title>");
    let server = FakeReleaseServer::start("a3s", env!("CARGO_PKG_VERSION"), &archive_name, archive);

    let first = fixture.start(Some(server.api_base()), false, false, 0);
    assert_success(&first);
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let first_pid = output_value(&first_stdout, "Background PID:")
        .parse::<u32>()
        .expect("downloaded Web background PID");
    let mut first_guard = DaemonGuard::new(first_pid);
    let first_address = web_address(&first_stdout);
    let page = http_get(&first_address, "/");
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("A3S downloaded Web workspace"), "{page}");
    assert_eq!(
        fs::read_to_string(fixture.cached_index()).expect("read cached Web index"),
        "<!doctype html><title>A3S downloaded Web workspace</title>"
    );
    first_guard.stop();
    wait_until_stopped(&first_address);

    let first_requests = server.requests();
    assert_eq!(first_requests, expected_release_requests(&archive_name));

    let second = fixture.start(Some(server.api_base()), true, false, 0);
    assert_success(&second);
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    let second_pid = output_value(&second_stdout, "Background PID:")
        .parse::<u32>()
        .expect("cached Web background PID");
    let mut second_guard = DaemonGuard::new(second_pid);
    let second_address = web_address(&second_stdout);
    let page = http_get(&second_address, "/");
    assert!(page.contains("A3S downloaded Web workspace"), "{page}");
    assert_eq!(
        server.requests(),
        first_requests,
        "offline cache reuse unexpectedly contacted the release server"
    );
    second_guard.stop();
    wait_until_stopped(&second_address);
}

#[test]
fn github_api_rate_limit_does_not_block_deterministic_web_asset_downloads() {
    let fixture = CargoWebFixture::new("cargo-web-api-rate-limit");
    let archive_name = web_archive_name();
    let archive =
        web_release_archive("<!doctype html><title>A3S Web without release API discovery</title>");
    let server = FakeReleaseServer::start_with_forbidden_api(
        "a3s",
        env!("CARGO_PKG_VERSION"),
        &archive_name,
        archive,
    );

    let output = fixture.start(Some(server.api_base()), false, false, 0);

    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid = output_value(&stdout, "Background PID:")
        .parse::<u32>()
        .expect("downloaded Web background PID");
    let address = web_address(&stdout);
    let mut guard = DaemonGuard::new(pid);
    assert!(
        http_get(&address, "/").contains("without release API discovery"),
        "downloaded Web workspace was not served"
    );
    guard.stop();
    wait_until_stopped(&address);

    assert_eq!(
        server.requests(),
        expected_release_requests(&archive_name),
        "Web setup must not query the rate-limited GitHub Releases API"
    );
}

#[test]
fn downloaded_web_assets_reject_a_mismatched_release_digest() {
    let fixture = CargoWebFixture::new("cargo-web-bad-digest");
    let archive_name = web_archive_name();
    let server = FakeReleaseServer::start_with_digest(
        "a3s",
        env!("CARGO_PKG_VERSION"),
        &archive_name,
        web_release_archive("<!doctype html><title>Untrusted Web workspace</title>"),
        &"0".repeat(64),
    );

    let output = fixture.start(Some(server.api_base()), false, false, 0);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("checksum mismatch"), "{stderr}");
    assert!(
        !fixture.cached_index().exists(),
        "unverified Web assets entered the active cache"
    );
}

#[test]
fn offline_web_start_without_assets_performs_no_network_request() {
    let fixture = CargoWebFixture::new("cargo-web-offline");
    let archive_name = web_archive_name();
    let server = FakeReleaseServer::start(
        "a3s",
        env!("CARGO_PKG_VERSION"),
        &archive_name,
        web_release_archive("<!doctype html><title>Offline Web workspace</title>"),
    );

    let output = fixture.start(Some(server.api_base()), true, false, 0);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("offline"), "{stderr}");
    assert!(stderr.contains("--api-only"), "{stderr}");
    assert!(
        server.requests().is_empty(),
        "offline Web start used network"
    );
}

#[test]
fn disabled_automatic_setup_without_assets_performs_no_network_request() {
    let fixture = CargoWebFixture::new("cargo-web-no-auto-install");
    let archive_name = web_archive_name();
    let server = FakeReleaseServer::start(
        "a3s",
        env!("CARGO_PKG_VERSION"),
        &archive_name,
        web_release_archive("<!doctype html><title>Disabled Web workspace</title>"),
    );

    let output = fixture.start(Some(server.api_base()), false, true, 0);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("A3S_NO_AUTO_INSTALL=1"), "{stderr}");
    assert!(stderr.contains("--api-only"), "{stderr}");
    assert!(
        server.requests().is_empty(),
        "disabled automatic setup used network"
    );
}

#[test]
fn concurrent_workspaces_share_one_verified_web_asset_download() {
    let fixture = CargoWebFixture::new("cargo-web-concurrent-download");
    let second_workspace = fixture.root.join("workspace-two");
    fs::create_dir_all(&second_workspace).expect("create second Cargo-style workspace");
    let archive_name = web_archive_name();
    let server = FakeReleaseServer::start(
        "a3s",
        env!("CARGO_PKG_VERSION"),
        &archive_name,
        web_release_archive("<!doctype html><title>Shared Web workspace</title>"),
    );

    let spawn = |workspace: &Path| {
        let mut command =
            fixture.start_command(workspace, Some(server.api_base()), false, false, 0);
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn concurrent Cargo-style Web start")
    };
    let first = spawn(&fixture.workspace);
    let second = spawn(&second_workspace);
    let first = first.wait_with_output().expect("wait for first Web start");
    let second = second
        .wait_with_output()
        .expect("wait for second Web start");
    assert_success(&first);
    assert_success(&second);

    let mut daemons = [&first, &second]
        .into_iter()
        .map(|output| {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let pid = output_value(&stdout, "Background PID:")
                .parse::<u32>()
                .expect("concurrent Web background PID");
            (DaemonGuard::new(pid), web_address(&stdout))
        })
        .collect::<Vec<_>>();
    for (guard, address) in &mut daemons {
        guard.stop();
        wait_until_stopped(address);
    }

    assert_eq!(
        server.requests(),
        expected_release_requests(&archive_name),
        "concurrent workspaces downloaded the shared release more than once"
    );
}

#[test]
fn detached_web_checks_a_foreign_port_before_downloading_assets() {
    let fixture = CargoWebFixture::new("cargo-web-foreign-port");
    let archive_name = web_archive_name();
    let server = FakeReleaseServer::start(
        "a3s",
        env!("CARGO_PKG_VERSION"),
        &archive_name,
        web_release_archive("<!doctype html><title>Unused Web workspace</title>"),
    );
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind foreign Web port");
    let port = listener.local_addr().expect("foreign Web address").port();

    let output = fixture.start(Some(server.api_base()), false, false, port);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already in use"), "{stderr}");
    assert!(stderr.contains("no process was stopped"), "{stderr}");
    assert!(
        server.requests().is_empty(),
        "port conflict downloaded Web assets before failing"
    );
    assert!(listener.local_addr().is_ok(), "foreign listener was closed");
}

struct CargoWebFixture {
    _temp: TempWorkspace,
    root: PathBuf,
    workspace: PathBuf,
    binary: PathBuf,
    config: PathBuf,
    data_home: PathBuf,
    state_home: PathBuf,
    cache_home: PathBuf,
    runtime_home: PathBuf,
}

impl CargoWebFixture {
    fn new(name: &str) -> Self {
        let temp = TempWorkspace::new(name);
        let root = temp.path("root");
        let workspace = root.join("workspace");
        let binary = root
            .join("cargo/bin")
            .join(if cfg!(windows) { "a3s.exe" } else { "a3s" });
        let config = root.join("config.acl");
        let data_home = root.join("data");
        let state_home = root.join("state");
        let cache_home = root.join("cache");
        let runtime_home = root.join("runtime");
        fs::create_dir_all(&workspace).expect("create Cargo-style Web workspace");
        fs::create_dir_all(binary.parent().expect("Cargo-style binary parent"))
            .expect("create Cargo-style bin directory");
        fs::copy(a3s_bin(), &binary).expect("copy Cargo-style a3s binary");
        fs::write(&config, test_config()).expect("write Cargo-style Web config");
        Self {
            _temp: temp,
            root,
            workspace,
            binary,
            config,
            data_home,
            state_home,
            cache_home,
            runtime_home,
        }
    }

    fn start(
        &self,
        api_base: Option<&str>,
        offline: bool,
        no_auto_install: bool,
        port: u16,
    ) -> std::process::Output {
        self.start_command(&self.workspace, api_base, offline, no_auto_install, port)
            .output()
            .expect("start Cargo-style A3S Web")
    }

    fn start_command(
        &self,
        workspace: &Path,
        api_base: Option<&str>,
        offline: bool,
        no_auto_install: bool,
        port: u16,
    ) -> Command {
        let mut command = Command::new(&self.binary);
        command
            .arg("-C")
            .arg(workspace)
            .arg("--config")
            .arg(&self.config);
        if offline {
            command.arg("--offline");
        }
        command.args([
            "web",
            "start",
            "--detach",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
        ]);
        self.configure(&mut command, workspace);
        if no_auto_install {
            command.env("A3S_NO_AUTO_INSTALL", "1");
        }
        if let Some(api_base) = api_base {
            command.env("A3S_UPDATER_GITHUB_API_BASE", api_base);
            command.env("A3S_UPDATER_GITHUB_RELEASE_BASE", api_base);
        }
        command
    }

    fn configure(&self, command: &mut Command, workspace: &Path) {
        let workspace_name = workspace
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("workspace"));
        command
            .current_dir(workspace)
            .env("HOME", self.root.join("home"))
            .env("A3S_DATA_HOME", &self.data_home)
            .env("A3S_STATE_HOME", &self.state_home)
            .env("A3S_CACHE_HOME", &self.cache_home)
            .env("A3S_RUNTIME_HOME", &self.runtime_home)
            .env(
                "A3S_CODE_WEB_STATE_DIR",
                self.root.join("code-web-state").join(workspace_name),
            )
            .env_remove("A3S_CODE_WEB_DIR")
            .env_remove("A3S_NO_AUTO_INSTALL")
            .env_remove("A3S_OFFLINE")
            .env_remove("A3S_UPDATER_GITHUB_API_BASE")
            .env_remove("A3S_UPDATER_GITHUB_RELEASE_BASE");
    }

    fn cached_index(&self) -> PathBuf {
        self.data_home
            .join("web")
            .join(env!("CARGO_PKG_VERSION"))
            .join("index.html")
    }
}

fn web_archive_name() -> String {
    format!("a3s-web-v{}.tar.gz", env!("CARGO_PKG_VERSION"))
}

fn expected_release_requests(archive_name: &str) -> [String; 2] {
    let release_path = format!(
        "/A3S-Lab/a3s/releases/download/v{}",
        env!("CARGO_PKG_VERSION")
    );
    [
        format!("{release_path}/{archive_name}.sha256"),
        format!("{release_path}/{archive_name}"),
    ]
}

fn web_release_archive(index: &str) -> Vec<u8> {
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut archive = tar::Builder::new(encoder);
    let bytes = index.as_bytes();
    let mut header = tar::Header::new_gnu();
    header
        .set_path("web/index.html")
        .expect("set Web index archive path");
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append(&header, bytes)
        .expect("append Web index fixture");
    let encoder = archive.into_inner().expect("finish Web release tar");
    encoder.finish().expect("finish Web release gzip")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn output_value<'a>(output: &'a str, prefix: &str) -> &'a str {
    output
        .lines()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim))
        .unwrap_or_else(|| panic!("missing `{prefix}` in output:\n{output}"))
}

fn web_address(output: &str) -> String {
    output_value(output, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string()
}

fn http_get(address: &str, path: &str) -> String {
    let mut stream = TcpStream::connect(address).expect("connect to detached Web process");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("set Web read timeout");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("write Web request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read Web response");
    response
}

fn wait_until_stopped(address: &str) {
    for _ in 0..100 {
        if TcpStream::connect(address).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("detached Web process still listens on {address}");
}

struct DaemonGuard {
    pid: u32,
    active: bool,
}

impl DaemonGuard {
    fn new(pid: u32) -> Self {
        Self { pid, active: true }
    }

    fn stop(&mut self) {
        if !self.active {
            return;
        }
        stop_process(self.pid);
        self.active = false;
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(unix)]
fn stop_process(pid: u32) {
    let _ = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status();
}

#[cfg(windows)]
fn stop_process(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

fn test_config() -> &'static str {
    r#"default_model = "openai/test"
providers "openai" {
  apiKey = "test"
  baseUrl = "http://127.0.0.1:1"
  models "test" {
    name = "Test"
    toolCall = true
  }
}
memory { llmExtraction = false }
"#
}
