use std::fs::{self, File, OpenOptions};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context};
use fs2::FileExt;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind};
use tokio::time::{sleep, Instant};

use super::options::ServeOptions;

const READY_FILE_ENV: &str = "A3S_INTERNAL_WEB_READY_FILE";
pub(super) const INSTANCE_NONCE_ENV: &str = "A3S_INTERNAL_WEB_INSTANCE_NONCE";
pub(super) const INSTANCE_FILE_ENV: &str = "A3S_INTERNAL_WEB_INSTANCE_FILE";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const START_LOCK_TIMEOUT: Duration = Duration::from_secs(20);
const OBSERVED_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebInstanceRecord {
    pub schema_version: u32,
    pub pid: u32,
    pub nonce: String,
    pub address: SocketAddr,
    pub workspace: PathBuf,
    pub log_path: PathBuf,
    pub executable: PathBuf,
    pub started_at_ms: u128,
    pub api_only: bool,
    #[serde(default)]
    pub version: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebEndpoint {
    pub address: SocketAddr,
    pub workspace: PathBuf,
    pub version: Option<String>,
    pub pid: Option<u32>,
    pub managed: bool,
    pub api_only: Option<bool>,
}

impl WebEndpoint {
    fn from_managed(instance: &WebInstanceRecord) -> Self {
        Self {
            address: instance.address,
            workspace: instance.workspace.clone(),
            version: instance.version.clone(),
            pid: Some(instance.pid),
            managed: true,
            api_only: Some(instance.api_only),
        }
    }
}

pub(super) enum BackgroundStart {
    Started(WebInstanceRecord),
    Reused(WebInstanceRecord),
    Existing(WebEndpoint),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebInstanceStatus {
    pub running: bool,
    pub stale: bool,
    pub managed: bool,
    pub instance: Option<WebInstanceRecord>,
    pub observed: Option<WebEndpoint>,
}

pub(super) async fn start(
    args: &[String],
    options: &ServeOptions,
) -> anyhow::Result<BackgroundStart> {
    let executable = std::env::current_exe()
        .map_err(|error| anyhow::anyhow!("could not locate the current a3s executable: {error}"))?;
    let workspace = canonical_workspace(&options.workspace)?;
    let _start_lock = acquire_start_lock(&workspace).await?;
    let instance_path = instance_path(&workspace)?;
    if let Some(existing) = read_instance(&instance_path)? {
        if probe_instance(&existing).await {
            if options.replace {
                stop_owned_instance(&instance_path, &existing).await?;
            } else {
                return Ok(BackgroundStart::Reused(existing));
            }
        } else {
            quarantine_stale_instance(&instance_path)?;
        }
    }

    if let Some(existing) = discover_requested_instance(options).await? {
        if options.replace {
            replace_observed_instance(&existing, options, &executable).await?;
        } else {
            return Ok(BackgroundStart::Existing(existing));
        }
    }
    ensure_requested_port_available(options).await?;
    let _prepared_web_root = super::resolve_web_root(options).await?;

    let log_path = log_path(&workspace)?;
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            anyhow::anyhow!(
                "could not create A3S Web log directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| {
            anyhow::anyhow!("could not open A3S Web log {}: {error}", log_path.display())
        })?;
    let ready_path = ready_file_path();
    let _ = std::fs::remove_file(&ready_path);
    let nonce = random_nonce();

    let mut command = Command::new(executable);
    command
        .arg("web")
        .args(foreground_args(args))
        .env(READY_FILE_ENV, &ready_path)
        .env(INSTANCE_NONCE_ENV, &nonce)
        .env(INSTANCE_FILE_ENV, &instance_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log.try_clone()?))
        .stderr(Stdio::from(log));
    if options.offline {
        command.env("A3S_OFFLINE", "1");
    }
    if !options.allow_asset_download {
        command.env("A3S_NO_AUTO_INSTALL", "1");
    }
    configure_detached(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| anyhow::anyhow!("failed to start A3S Web in the background: {error}"))?;
    let pid = child.id();
    let ready = wait_until_ready(&mut child, &ready_path, &log_path).await;
    let _ = std::fs::remove_file(&ready_path);
    let address = ready?;

    let record = WebInstanceRecord {
        schema_version: 1,
        pid,
        nonce,
        address,
        workspace,
        log_path: log_path.clone(),
        executable: std::env::current_exe()?,
        started_at_ms: unix_millis(),
        api_only: options.api_only,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
    };
    wait_until_control_ready(&record, &mut child).await?;
    write_instance(&instance_path, &record)?;

    Ok(BackgroundStart::Started(record))
}

pub(crate) async fn status(workspace: &Path) -> anyhow::Result<WebInstanceStatus> {
    let workspace = canonical_workspace(workspace)?;
    let path = instance_path(&workspace)?;
    let Some(instance) = read_instance(&path)? else {
        let mut options = ServeOptions::parse(&[])?;
        options.workspace = workspace;
        let observed = discover_matching_instance(&options).await?;
        return Ok(WebInstanceStatus {
            running: observed.is_some(),
            stale: false,
            managed: false,
            instance: None,
            observed,
        });
    };
    let running = probe_instance(&instance).await;
    Ok(WebInstanceStatus {
        running,
        stale: !running,
        managed: true,
        instance: Some(instance),
        observed: None,
    })
}

pub(crate) async fn stop(workspace: &Path) -> anyhow::Result<Option<WebInstanceRecord>> {
    let workspace = canonical_workspace(workspace)?;
    let _start_lock = acquire_start_lock(&workspace).await?;
    let path = instance_path(&workspace)?;
    let Some(instance) = read_instance(&path)? else {
        let mut options = ServeOptions::parse(&[])?;
        options.workspace = workspace.clone();
        if let Some(existing) = discover_matching_instance(&options).await? {
            bail!(
                "A3S Web is running at http://{} but is not managed by this CLI state; no process \
                 was stopped. Stop the original foreground command instead",
                existing.address
            );
        }
        return Ok(None);
    };
    if !probe_instance(&instance).await {
        quarantine_stale_instance(&path)?;
        bail!(
            "the A3S Web instance record for {} is stale; it was quarantined without signaling any process",
            workspace.display()
        );
    }

    stop_owned_instance(&path, &instance).await?;
    Ok(Some(instance))
}

pub(super) async fn replace_managed(workspace: &Path) -> anyhow::Result<Option<WebInstanceRecord>> {
    let workspace = canonical_workspace(workspace)?;
    let _start_lock = acquire_start_lock(&workspace).await?;
    let path = instance_path(&workspace)?;
    let Some(instance) = read_instance(&path)? else {
        return Ok(None);
    };
    if !probe_instance(&instance).await {
        quarantine_stale_instance(&path)?;
        return Ok(None);
    }
    stop_owned_instance(&path, &instance).await?;
    Ok(Some(instance))
}

async fn stop_owned_instance(path: &Path, instance: &WebInstanceRecord) -> anyhow::Result<()> {
    let url = control_url(instance, "stop");
    let response = control_client()?
        .post(url)
        .send()
        .await
        .context("failed to request A3S Web shutdown")?;
    if !response.status().is_success() {
        bail!("A3S Web rejected the authenticated shutdown request");
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if !probe_instance(instance).await {
            remove_instance_if_owned(path, &instance.nonce);
            return Ok(());
        }
        sleep(POLL_INTERVAL).await;
    }
    bail!("A3S Web did not stop within 10 seconds; no force signal was sent")
}

pub(super) async fn replace_observed_instance(
    existing: &WebEndpoint,
    options: &ServeOptions,
    executable: &Path,
) -> anyhow::Result<()> {
    let pid = existing.pid.ok_or_else(|| {
        anyhow::anyhow!(
            "A3S Web {} did not report a process ID; no process was stopped",
            existing.address
        )
    })?;
    if pid == std::process::id() {
        bail!(
            "A3S Web {} reported the current process ID; no process was stopped",
            existing.address
        );
    }
    if options.addr.port() == 0 || existing.address.port() != options.addr.port() {
        bail!(
            "A3S Web {} does not match the requested port {}; no process was stopped",
            existing.address,
            options.addr.port()
        );
    }

    let expected_executable = executable.to_path_buf();
    let expected_port = options.addr.port();
    let identity =
        inspect_observed_process(pid, expected_executable.clone(), expected_port).await?;

    let confirmed = probe_web_endpoint(existing.address).await.ok_or_else(|| {
        anyhow::anyhow!(
            "A3S Web {} changed while replacement was being verified; no process was stopped",
            existing.address
        )
    })?;
    if confirmed.pid != Some(pid)
        || !same_workspace(&existing.workspace, &confirmed.workspace)
        || !same_workspace(&options.workspace, &confirmed.workspace)
    {
        bail!(
            "A3S Web {} changed while replacement was being verified; no process was stopped",
            existing.address
        );
    }

    signal_observed_process(pid, expected_executable, expected_port, identity).await?;
    wait_until_port_released(existing.address).await
}

#[derive(Clone, Copy)]
struct ObservedProcessIdentity {
    start_time: u64,
}

async fn inspect_observed_process(
    pid: u32,
    executable: PathBuf,
    port: u16,
) -> anyhow::Result<ObservedProcessIdentity> {
    tokio::task::spawn_blocking(move || inspect_observed_process_sync(pid, &executable, port))
        .await
        .context("A3S Web process inspection task failed")?
}

fn inspect_observed_process_sync(
    pid: u32,
    executable: &Path,
    port: u16,
) -> anyhow::Result<ObservedProcessIdentity> {
    let system = process_snapshot(pid);
    let process = system.process(Pid::from_u32(pid)).ok_or_else(|| {
        anyhow::anyhow!("A3S Web process {pid} is no longer running; no process was stopped")
    })?;
    verify_observed_process(process, executable, port)?;
    Ok(ObservedProcessIdentity {
        start_time: process.start_time(),
    })
}

async fn signal_observed_process(
    pid: u32,
    executable: PathBuf,
    port: u16,
    identity: ObservedProcessIdentity,
) -> anyhow::Result<()> {
    tokio::task::spawn_blocking(move || {
        let system = process_snapshot(pid);
        let Some(process) = system.process(Pid::from_u32(pid)) else {
            return Ok(());
        };
        verify_observed_process(process, &executable, port)?;
        if process.start_time() != identity.start_time {
            bail!("A3S Web process {pid} changed identity; no process was stopped");
        }
        match process.kill_with(Signal::Interrupt) {
            Some(true) => Ok(()),
            Some(false) => bail!(
                "could not interrupt verified A3S Web process {pid}; no force signal was sent"
            ),
            None if process.kill() => Ok(()),
            None => bail!("could not stop verified A3S Web process {pid}"),
        }
    })
    .await
    .context("A3S Web process stop task failed")?
}

fn process_snapshot(pid: u32) -> System {
    let pid = Pid::from_u32(pid);
    let pids = [pid];
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&pids),
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_exe(UpdateKind::Always)
            .without_tasks(),
    );
    system
}

fn verify_observed_process(
    process: &sysinfo::Process,
    executable: &Path,
    port: u16,
) -> anyhow::Result<()> {
    let pid = process.pid().as_u32();
    let Some(process_executable) = process.exe() else {
        bail!("could not verify the executable for A3S Web process {pid}; no process was stopped");
    };
    if !same_executable(process_executable, executable) {
        bail!(
            "A3S Web process {pid} does not use the current a3s executable; no process was stopped"
        );
    }
    if !command_declares_web_port(process.cmd(), port) {
        bail!("A3S Web process {pid} does not declare `web --port {port}`; no process was stopped");
    }
    Ok(())
}

fn same_executable(left: &Path, right: &Path) -> bool {
    let left = comparable_executable_path(left);
    let right = comparable_executable_path(right);
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

#[cfg(target_os = "linux")]
fn comparable_executable_path(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    const DELETED_SUFFIX: &[u8] = b" (deleted)";
    let bytes = path.as_os_str().as_bytes();
    let bytes = bytes.strip_suffix(DELETED_SUFFIX).unwrap_or(bytes);
    PathBuf::from(OsString::from_vec(bytes.to_vec()))
}

#[cfg(not(target_os = "linux"))]
fn comparable_executable_path(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn command_declares_web_port(command: &[std::ffi::OsString], expected_port: u16) -> bool {
    let Some(web_index) = command
        .iter()
        .position(|argument| argument.to_string_lossy() == "web")
    else {
        return false;
    };
    let mut declared_port = None;
    let mut index = web_index + 1;
    while index < command.len() {
        let argument = command[index].to_string_lossy();
        if argument == "--port" {
            let Some(value) = command.get(index + 1).and_then(|value| value.to_str()) else {
                return false;
            };
            let Ok(port) = value.parse::<u16>() else {
                return false;
            };
            declared_port = Some(port);
            index += 2;
            continue;
        }
        if let Some(value) = argument.strip_prefix("--port=") {
            let Ok(port) = value.parse::<u16>() else {
                return false;
            };
            declared_port = Some(port);
        }
        index += 1;
    }
    declared_port == Some(expected_port)
}

async fn wait_until_port_released(address: SocketAddr) -> anyhow::Result<()> {
    let deadline = Instant::now() + OBSERVED_SHUTDOWN_TIMEOUT;
    loop {
        match tokio::net::TcpListener::bind(address).await {
            Ok(listener) => {
                drop(listener);
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
                if Instant::now() >= deadline {
                    bail!(
                        "verified A3S Web process did not release {} within {} seconds",
                        address,
                        OBSERVED_SHUTDOWN_TIMEOUT.as_secs()
                    );
                }
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to verify that A3S Web released {address}"))
            }
        }
        sleep(POLL_INTERVAL).await;
    }
}

pub(crate) async fn open(workspace: &Path) -> anyhow::Result<WebEndpoint> {
    let status = status(workspace).await?;
    if status.running {
        if let Some(instance) = status.instance.as_ref() {
            return Ok(WebEndpoint::from_managed(instance));
        }
        if let Some(observed) = status.observed {
            return Ok(observed);
        }
    }
    bail!("A3S Web is not running for {}", workspace.display())
}

pub(crate) fn read_log_tail(path: &Path, lines: usize) -> anyhow::Result<String> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("could not read A3S Web log {}", path.display()))?;
    let lines = content.lines().rev().take(lines).collect::<Vec<_>>();
    let mut output = lines.into_iter().rev().collect::<Vec<_>>().join("\n");
    if !output.is_empty() {
        output.push('\n');
    }
    Ok(output)
}

pub(crate) fn remove_instance_if_owned(path: &Path, nonce: &str) {
    let owned = read_instance(path)
        .ok()
        .flatten()
        .is_some_and(|record| record.nonce == nonce);
    if owned {
        let _ = fs::remove_file(path);
    }
}

pub(super) fn notify_ready(address: SocketAddr) -> anyhow::Result<()> {
    let Some(path) = std::env::var_os(READY_FILE_ENV) else {
        return Ok(());
    };
    std::fs::write(&path, address.to_string()).map_err(|error| {
        anyhow::anyhow!(
            "could not report A3S Web background startup through {}: {error}",
            PathBuf::from(path).display()
        )
    })
}

async fn wait_until_ready(
    child: &mut Child,
    ready_path: &Path,
    log_path: &Path,
) -> anyhow::Result<SocketAddr> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        if let Ok(value) = std::fs::read_to_string(ready_path) {
            if let Ok(address) = value.trim().parse::<SocketAddr>() {
                return Ok(address);
            }
        }
        if let Some(status) = child.try_wait()? {
            anyhow::bail!(
                "A3S Web exited before becoming ready ({status}); see {}",
                log_path.display()
            );
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!(
                "A3S Web did not become ready within {} seconds; see {}",
                STARTUP_TIMEOUT.as_secs(),
                log_path.display()
            );
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn foreground_args(args: &[String]) -> impl Iterator<Item = &String> {
    args.iter()
        .filter(|argument| !matches!(argument.as_str(), "-d" | "--detach" | "--replace"))
}

fn log_path(workspace: &Path) -> anyhow::Result<PathBuf> {
    Ok(state_root()?
        .join("logs/web")
        .join(format!("{}.log", workspace_key(workspace))))
}

fn ready_file_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "a3s-web-ready-{}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    ))
}

fn instance_path(workspace: &Path) -> anyhow::Result<PathBuf> {
    Ok(state_root()?
        .join("web/instances")
        .join(format!("{}.json", workspace_key(workspace))))
}

async fn acquire_start_lock(workspace: &Path) -> anyhow::Result<StartLock> {
    let path = state_root()?
        .join("web/locks")
        .join(format!("{}.lock", workspace_key(workspace)));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create Web lock directory {}", parent.display()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("could not open Web lifecycle lock {}", path.display()))?;
    let deadline = Instant::now() + START_LOCK_TIMEOUT;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(StartLock { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    bail!(
                        "timed out waiting for another A3S Web lifecycle operation for {}",
                        workspace.display()
                    );
                }
                sleep(POLL_INTERVAL).await;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("could not lock Web lifecycle state {}", path.display())
                })
            }
        }
    }
}

struct StartLock {
    file: File,
}

impl Drop for StartLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn state_root() -> anyhow::Result<PathBuf> {
    if let Some(path) = std::env::var_os("A3S_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(absolute(PathBuf::from(path)));
    }
    if let Some(path) = std::env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        return Ok(absolute(PathBuf::from(path)).join("a3s"));
    }
    if let Some(home) = crate::user_paths::user_home_dir() {
        return Ok(home.join(".local/state/a3s"));
    }
    #[cfg(windows)]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(path).join("a3s/state"));
    }
    bail!("A3S_STATE_HOME is not set and no home directory is available")
}

fn absolute(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(&path))
            .unwrap_or(path)
    }
}

fn canonical_workspace(workspace: &Path) -> anyhow::Result<PathBuf> {
    fs::canonicalize(workspace)
        .with_context(|| format!("could not resolve workspace {}", workspace.display()))
}

fn workspace_key(workspace: &Path) -> String {
    let digest = Sha256::digest(workspace.to_string_lossy().as_bytes());
    format!("{digest:x}")
}

fn random_nonce() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn read_instance(path: &Path) -> anyhow::Result<Option<WebInstanceRecord>> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("could not read {}", path.display()))
        }
    };
    let record = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid A3S Web instance record {}", path.display()))?;
    Ok(Some(record))
}

fn write_instance(path: &Path, record: &WebInstanceRecord) -> anyhow::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let temp = path.with_extension(format!("{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec_pretty(record)?;
    fs::write(&temp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temp, fs::Permissions::from_mode(0o600))?;
    }
    fs::rename(&temp, path)?;
    Ok(())
}

fn quarantine_stale_instance(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let stale = path.with_extension(format!("stale-{}", unix_millis()));
    fs::rename(path, &stale).with_context(|| {
        format!(
            "could not quarantine stale A3S Web state {}",
            path.display()
        )
    })
}

pub(super) async fn discover_requested_instance(
    options: &ServeOptions,
) -> anyhow::Result<Option<WebEndpoint>> {
    if options.addr.port() == 0 {
        return Ok(None);
    }
    let Some(existing) = probe_web_endpoint(options.addr).await else {
        return Ok(None);
    };
    let workspace = canonical_workspace(&options.workspace)?;
    if same_workspace(&workspace, &existing.workspace) {
        return Ok(Some(existing));
    }
    bail!(
        "{} is already serving A3S Web for {}; no process was stopped. Use that workspace's Web \
         instance or select another port with --port 0",
        options.addr,
        existing.workspace.display()
    )
}

async fn ensure_requested_port_available(options: &ServeOptions) -> anyhow::Result<()> {
    if options.addr.port() == 0 {
        return Ok(());
    }
    match tokio::net::TcpListener::bind(options.addr).await {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            bail!(
                "{} is already in use by another application; no process was stopped. Stop that \
                 application or select an available port with --port 0",
                options.addr
            )
        }
        Err(error) => Err(error)
            .with_context(|| format!("failed to validate {} before A3S Web startup", options.addr)),
    }
}

async fn discover_matching_instance(options: &ServeOptions) -> anyhow::Result<Option<WebEndpoint>> {
    if options.addr.port() == 0 {
        return Ok(None);
    }
    let workspace = canonical_workspace(&options.workspace)?;
    Ok(probe_web_endpoint(options.addr)
        .await
        .filter(|existing| same_workspace(&workspace, &existing.workspace)))
}

async fn probe_web_endpoint(address: SocketAddr) -> Option<WebEndpoint> {
    let connect_address = connectable_address(address);
    let client = control_client().ok()?;
    let response = client
        .get(format!("http://{connect_address}/api/v1/health"))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let value = response.json::<serde_json::Value>().await.ok()?;
    let health = value.get("data").unwrap_or(&value);
    if health.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
        return None;
    }
    let service = health.get("service").and_then(serde_json::Value::as_str);
    let app = health.get("app").and_then(serde_json::Value::as_str);
    let identified =
        service == Some("a3s-code-web") || matches!(app, Some("a3s-code-web" | "书小安"));
    if !identified {
        return None;
    }
    let workspace = health
        .get("workspace")
        .and_then(serde_json::Value::as_str)
        .filter(|workspace| !workspace.trim().is_empty())
        .map(PathBuf::from)?;
    let pid = health
        .get("pid")
        .and_then(serde_json::Value::as_u64)
        .and_then(|pid| u32::try_from(pid).ok());
    let version = health
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);

    Some(WebEndpoint {
        address,
        workspace,
        version,
        pid,
        managed: false,
        api_only: None,
    })
}

fn connectable_address(address: SocketAddr) -> SocketAddr {
    if !address.ip().is_unspecified() {
        return address;
    }
    match address {
        SocketAddr::V4(address) => SocketAddr::from(([127, 0, 0, 1], address.port())),
        SocketAddr::V6(address) => SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], address.port())),
    }
}

fn same_workspace(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => absolute(left.to_path_buf()) == absolute(right.to_path_buf()),
    }
}

fn control_url(instance: &WebInstanceRecord, action: &str) -> String {
    format!(
        "http://{}/.a3s/web/{}/{}",
        instance.address, instance.nonce, action
    )
}

fn control_client() -> anyhow::Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .no_proxy()
        .build()
        .context("could not create the A3S Web control client")
}

async fn probe_instance(instance: &WebInstanceRecord) -> bool {
    let Ok(client) = control_client() else {
        return false;
    };
    let Ok(response) = client.get(control_url(instance, "status")).send().await else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    response
        .json::<serde_json::Value>()
        .await
        .ok()
        .is_some_and(|value| {
            value.get("pid").and_then(serde_json::Value::as_u64) == Some(instance.pid as u64)
                && value.get("nonce").and_then(serde_json::Value::as_str)
                    == Some(instance.nonce.as_str())
        })
}

async fn wait_until_control_ready(
    instance: &WebInstanceRecord,
    child: &mut Child,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        if probe_instance(instance).await {
            return Ok(());
        }
        if let Some(status) = child.try_wait()? {
            bail!("A3S Web exited before its control endpoint was ready ({status})");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("A3S Web control endpoint did not become ready in time");
        }
        sleep(POLL_INTERVAL).await;
    }
}

#[cfg(unix)]
fn configure_detached(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn configure_detached(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(not(any(unix, windows)))]
fn configure_detached(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn foreground_arguments_remove_parent_only_lifecycle_flags() {
        let args = vec![
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "-d".to_string(),
            "--replace".to_string(),
            "--port".to_string(),
            "29653".to_string(),
        ];

        assert_eq!(
            foreground_args(&args).cloned().collect::<Vec<_>>(),
            ["--host", "127.0.0.1", "--port", "29653"]
        );
    }

    #[test]
    fn observed_replacement_requires_web_with_the_matching_explicit_port() {
        let matching = ["a3s", "web", "--host", "127.0.0.1", "--port", "29653"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        assert!(command_declares_web_port(&matching, 29653));

        let environment_only = ["a3s", "web"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        assert!(!command_declares_web_port(&environment_only, 29653));

        let wrong_port = ["a3s", "web", "--port", "29654"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        assert!(!command_declares_web_port(&wrong_port, 29653));

        let wrong_command = ["a3s", "code", "--port", "29653"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        assert!(!command_declares_web_port(&wrong_command, 29653));
    }
}
