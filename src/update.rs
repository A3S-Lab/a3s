//! Self-update, shared by the TUI `/update` and the `a3s update` CLI command.
//!
//! Tries Homebrew (how a3s is usually installed) and **falls back to a direct
//! binary download** if brew or the tap is in any bad state — so an update can
//! never be blocked again by a stale tap clone or a broken `brew upgrade`.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(not(windows))]
use std::process::Stdio;
#[cfg(not(windows))]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(not(windows))]
use std::sync::{mpsc, Arc};
use std::time::Duration;
#[cfg(not(windows))]
use std::time::Instant;

use a3s_updater::{extract_tar_gz_archive, verify_sha256};

struct CommandOutput {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

trait CommandRunner {
    fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput>;
    fn output_bounded(
        &self,
        program: &OsStr,
        args: &[OsString],
        _timeout: Duration,
    ) -> Option<CommandOutput> {
        self.output(program, args)
    }
    fn status(&self, program: &OsStr, args: &[OsString]) -> bool;
}

struct RealCommandRunner;

impl CommandRunner for RealCommandRunner {
    fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput> {
        let out = Command::new(program).args(args).output().ok()?;
        Some(CommandOutput {
            success: out.status.success(),
            stdout: out.stdout,
            stderr: out.stderr,
        })
    }

    fn output_bounded(
        &self,
        program: &OsStr,
        args: &[OsString],
        timeout: Duration,
    ) -> Option<CommandOutput> {
        bounded_command_output(program, args, timeout)
    }

    fn status(&self, program: &OsStr, args: &[OsString]) -> bool {
        Command::new(program)
            .args(args)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn args(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

const BREW_TAP: &str = "a3s-lab/tap";
const BREW_TAP_URL: &str = "https://github.com/A3S-Lab/homebrew-tap";
const BREW_FORMULA: &str = "a3s-lab/tap/a3s";
const BREW_SHORT_FORMULA: &str = "a3s";
const WEBVIEW_FORMULA: &str = "a3s-lab/tap/a3s-webview";
const AGENT_ISLAND_BIN_ENV: &str = "A3S_AGENT_ISLAND_BIN";
const WEBVIEW_BIN_ENV: &str = "A3S_WEBVIEW_BIN";
const MAX_SELF_UPDATE_ARCHIVE_BYTES: usize = 512 * 1024 * 1024;
const AGENT_ISLAND_HELPER_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const AGENT_ISLAND_HELPER_TERMINATE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_AGENT_ISLAND_HELPER_PROBE_BYTES: u64 = 8 * 1024;
#[cfg(any(windows, test))]
const AGENT_ISLAND_HELPER_USAGE: &[u8] =
    b"usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>";
#[cfg(any(windows, test))]
const SYSTEM_AGENT_SNAPSHOT_MARKER: &[u8] = b"a3s.system_agent_snapshot.v1";
#[cfg(windows)]
const MAX_AGENT_ISLAND_HELPER_BINARY_BYTES: u64 = 128 * 1024 * 1024;
#[cfg(any(windows, test))]
const MIN_WINDOWS_PE_HEADER_OFFSET: usize = 0x40;
// A normal PE header follows a short DOS stub. Bound the pointer independently
// of the overall helper size before using it as a slice offset.
#[cfg(any(windows, test))]
const MAX_WINDOWS_PE_HEADER_OFFSET: usize = 1024 * 1024;
#[cfg(any(windows, test))]
const WINDOWS_PE_MACHINE_AMD64: u16 = 0x8664;
#[cfg(any(windows, test))]
const WINDOWS_PE_MACHINE_ARM64: u16 = 0xaa64;
const A3S_BINARY: &str = if cfg!(windows) { "a3s.exe" } else { "a3s" };
const WEBVIEW_BINARY: &str = if cfg!(windows) {
    "a3s-webview.exe"
} else {
    "a3s-webview"
};
const LATEST_RELEASE_API_ARGS: &[&str] = &[
    "-fsSL",
    "--connect-timeout",
    "5",
    "--max-time",
    "12",
    "https://api.github.com/repos/A3S-Lab/a3s/releases?per_page=100",
];

#[cfg(not(windows))]
fn spawn_probe_reader<R>(
    mut reader: R,
    exceeded: Arc<AtomicBool>,
) -> mpsc::Receiver<Option<Vec<u8>>>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let limit = usize::try_from(MAX_AGENT_ISLAND_HELPER_PROBE_BYTES).unwrap_or(usize::MAX);
        let mut retained = Vec::with_capacity(limit);
        let mut total = 0_u64;
        let mut chunk = [0_u8; 4096];
        loop {
            let read = match reader.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    let _ = sender.send(None);
                    return;
                }
            };
            total = total.saturating_add(read as u64);
            if total > MAX_AGENT_ISLAND_HELPER_PROBE_BYTES {
                exceeded.store(true, Ordering::Release);
            }
            let keep = limit.saturating_sub(retained.len()).min(read);
            retained.extend_from_slice(&chunk[..keep]);
        }
        let _ = sender.send(Some(retained));
    });
    receiver
}

#[cfg(unix)]
fn configure_probe_command(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(any(unix, windows)))]
fn configure_probe_command(_command: &mut Command) {}

#[cfg(unix)]
struct ProbeProcessTree {
    process_group: libc::pid_t,
}

#[cfg(unix)]
impl ProbeProcessTree {
    fn attach(child: &mut std::process::Child) -> Option<Self> {
        Some(Self {
            process_group: libc::pid_t::try_from(child.id()).ok()?,
        })
    }

    fn terminate(&self) {
        // SAFETY: the child was spawned into a new process group whose id is
        // its pid. A negative pid targets that group, including descendants
        // that inherited the capability probe's stdout or stderr handles.
        unsafe {
            libc::kill(-self.process_group, libc::SIGKILL);
        }
    }
}

#[cfg(not(any(unix, windows)))]
struct ProbeProcessTree;

#[cfg(not(any(unix, windows)))]
impl ProbeProcessTree {
    fn attach(_child: &mut std::process::Child) -> Option<Self> {
        Some(Self)
    }

    fn terminate(&self) {}
}

#[cfg(not(windows))]
fn terminate_probe_child(child: &mut std::process::Child, tree: &ProbeProcessTree) {
    tree.terminate();
    let _ = child.kill();
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) | Err(_) => return,
            Ok(None) if started.elapsed() < AGENT_ISLAND_HELPER_TERMINATE_TIMEOUT => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) => return,
        }
    }
}

#[cfg(not(windows))]
fn bounded_command_output(
    program: &OsStr,
    args: &[OsString],
    timeout: Duration,
) -> Option<CommandOutput> {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure_probe_command(&mut command);
    let mut child = command.spawn().ok()?;
    let tree = match ProbeProcessTree::attach(&mut child) {
        Some(tree) => tree,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            return None;
        }
    };
    let exceeded = Arc::new(AtomicBool::new(false));
    let stdout = spawn_probe_reader(child.stdout.take()?, Arc::clone(&exceeded));
    let stderr = spawn_probe_reader(child.stderr.take()?, Arc::clone(&exceeded));
    let started = Instant::now();
    let status = loop {
        if exceeded.load(Ordering::Acquire) {
            terminate_probe_child(&mut child, &tree);
            return None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(None) | Err(_) => {
                terminate_probe_child(&mut child, &tree);
                return None;
            }
        }
    };
    // A capability command has no reason to leave descendants running. Stop
    // its process tree so inherited pipe handles cannot outlive the bound.
    tree.terminate();
    let stdout = stdout
        .recv_timeout(AGENT_ISLAND_HELPER_TERMINATE_TIMEOUT)
        .ok()??;
    let stderr = stderr
        .recv_timeout(AGENT_ISLAND_HELPER_TERMINATE_TIMEOUT)
        .ok()??;
    if exceeded.load(Ordering::Acquire) {
        return None;
    }
    Some(CommandOutput {
        success: status.success(),
        stdout,
        stderr,
    })
}

#[cfg(windows)]
fn bounded_command_output(
    program: &OsStr,
    args: &[OsString],
    _timeout: Duration,
) -> Option<CommandOutput> {
    let expected_args = [OsString::from("--agent-island"), OsString::from("--help")];
    if args != expected_args {
        return None;
    }
    if !webview_binary_supports_agent_island(Path::new(program)).ok()? {
        return None;
    }
    Some(CommandOutput {
        success: false,
        stdout: Vec::new(),
        stderr: AGENT_ISLAND_HELPER_USAGE.to_vec(),
    })
}

/// Validate the Windows helper contract without executing the candidate.
///
/// The runtime island launcher shares this check so an incompatible or hostile
/// helper cannot escape a capability-probe timeout by leaving descendants.
#[cfg(windows)]
pub(crate) fn webview_binary_supports_agent_island(binary: &Path) -> std::io::Result<bool> {
    let binary = resolve_probe_binary(binary.as_os_str()).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("could not resolve native helper {}", binary.display()),
        )
    })?;
    let file = std::fs::File::open(&binary)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_AGENT_ISLAND_HELPER_BINARY_BYTES {
        return Ok(false);
    }
    let mut bytes = Vec::new();
    file.take(MAX_AGENT_ISLAND_HELPER_BINARY_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AGENT_ISLAND_HELPER_BINARY_BYTES {
        return Ok(false);
    }
    Ok(webview_binary_contains_agent_island_contract(&bytes))
}

#[cfg(any(windows, test))]
fn webview_binary_contains_agent_island_contract(bytes: &[u8]) -> bool {
    if !webview_binary_has_target_pe_header(bytes) {
        return false;
    }
    [AGENT_ISLAND_HELPER_USAGE, SYSTEM_AGENT_SNAPSHOT_MARKER]
        .into_iter()
        .all(|needle| {
            bytes
                .windows(needle.len())
                .any(|candidate| candidate == needle)
        })
}

#[cfg(any(windows, test))]
fn webview_binary_has_target_pe_header(bytes: &[u8]) -> bool {
    if bytes.get(..2) != Some(b"MZ") {
        return false;
    }
    let Some(pe_offset_bytes) = bytes.get(0x3c..0x40) else {
        return false;
    };
    let pe_offset = u32::from_le_bytes([
        pe_offset_bytes[0],
        pe_offset_bytes[1],
        pe_offset_bytes[2],
        pe_offset_bytes[3],
    ]);
    let Ok(pe_offset) = usize::try_from(pe_offset) else {
        return false;
    };
    if !(MIN_WINDOWS_PE_HEADER_OFFSET..=MAX_WINDOWS_PE_HEADER_OFFSET).contains(&pe_offset) {
        return false;
    }
    let Some(machine_offset) = pe_offset.checked_add(4) else {
        return false;
    };
    if bytes.get(pe_offset..machine_offset) != Some(b"PE\0\0") {
        return false;
    }
    let Some(machine_end) = machine_offset.checked_add(2) else {
        return false;
    };
    let Some(machine_bytes) = bytes.get(machine_offset..machine_end) else {
        return false;
    };
    let machine = u16::from_le_bytes([machine_bytes[0], machine_bytes[1]]);
    target_windows_pe_machine().is_some_and(|target| machine == target)
}

#[cfg(target_arch = "x86_64")]
#[cfg(any(windows, test))]
fn target_windows_pe_machine() -> Option<u16> {
    Some(WINDOWS_PE_MACHINE_AMD64)
}

#[cfg(target_arch = "aarch64")]
#[cfg(any(windows, test))]
fn target_windows_pe_machine() -> Option<u16> {
    Some(WINDOWS_PE_MACHINE_ARM64)
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
#[cfg(any(windows, test))]
fn target_windows_pe_machine() -> Option<u16> {
    None
}

#[cfg(windows)]
fn resolve_probe_binary(program: &OsStr) -> Option<PathBuf> {
    let candidate = PathBuf::from(program);
    if candidate.is_file() {
        return Some(candidate);
    }
    if candidate.components().count() != 1 {
        return None;
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|directory| directory.join(&candidate))
        .find(|path| path.is_file())
}

fn numeric_version_parts(s: &str) -> Vec<u32> {
    let trimmed = s.trim().trim_start_matches('v');
    let core = trimmed.split(['-', '+']).next().unwrap_or(trimmed);
    let mut parts = Vec::new();
    for part in core.split('.') {
        let digits = part
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if digits.is_empty() {
            break;
        }
        match digits.parse::<u32>() {
            Ok(n) => parts.push(n),
            Err(_) => break,
        }
    }
    parts
}

/// Compare stable numeric version components with optional `v` prefixes.
pub(crate) fn version_ge(a: &str, b: &str) -> bool {
    let mut av = numeric_version_parts(a);
    let mut bv = numeric_version_parts(b);
    if av.is_empty() || bv.is_empty() {
        return false;
    }
    let len = av.len().max(bv.len());
    av.resize(len, 0);
    bv.resize(len, 0);
    av >= bv
}

/// Latest release version tag from GitHub (no leading `v`), or `None` if the
/// release server is unreachable. Blocking — use [`fetch_latest_async`] from
/// cancellation-sensitive async flows.
///
/// The monorepo publishes several independently versioned products, so this
/// selects the newest published stable `vX.Y.Z` release instead of trusting
/// GitHub's repository-wide `releases/latest` pointer.
pub(crate) fn fetch_latest() -> Option<String> {
    fetch_latest_from_api()
}

fn fetch_latest_from_api() -> Option<String> {
    let out = Command::new("curl")
        .args(LATEST_RELEASE_API_ARGS)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    version_from_api_response(&out.stdout)
}

/// Async latest-release lookup whose `curl` process is terminated if the
/// caller is cancelled (for example, while the TUI is shutting down).
pub(crate) async fn fetch_latest_async() -> Option<String> {
    fetch_latest_from_api_async().await
}

async fn fetch_latest_from_api_async() -> Option<String> {
    let out = cancellable_curl_output(LATEST_RELEASE_API_ARGS).await?;
    if !out.status.success() {
        return None;
    }
    version_from_api_response(&out.stdout)
}

async fn cancellable_curl_output(args: &[&str]) -> Option<std::process::Output> {
    let mut command = tokio::process::Command::new("curl");
    command.args(args);
    cancellable_command_output(command).await
}

async fn cancellable_command_output(
    mut command: tokio::process::Command,
) -> Option<std::process::Output> {
    command.kill_on_drop(true);
    command.output().await.ok()
}

fn stable_release_version(tag: &str) -> Option<String> {
    let version = tag.strip_prefix('v')?;
    let parts = version.split('.').collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return None;
    }
    Some(version.to_string())
}

fn version_from_api_response(bytes: &[u8]) -> Option<String> {
    let releases = serde_json::from_slice::<Vec<serde_json::Value>>(bytes).ok()?;
    releases
        .iter()
        .filter(|release| {
            !release
                .get("draft")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true)
                && !release
                    .get("prerelease")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(true)
        })
        .filter_map(|release| release.get("tag_name")?.as_str())
        .filter_map(stable_release_version)
        .max_by(|left, right| numeric_version_parts(left).cmp(&numeric_version_parts(right)))
}

fn version_from_output(text: &str) -> Option<String> {
    for token in text.split(|c: char| {
        !(c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '+' || c == 'v')
    }) {
        let token = token.trim().trim_start_matches('v');
        let core = token.split(['-', '+']).next().unwrap_or(token);
        if numeric_version_parts(core).len() >= 2 {
            return Some(core.to_string());
        }
    }
    None
}

/// Version reported by the running executable. Falls back to the package
/// version only if the self-probe fails.
pub(crate) fn current_version() -> String {
    let runner = RealCommandRunner;
    std::env::current_exe()
        .ok()
        .and_then(|exe| binary_version(&runner, exe.as_os_str()))
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// GitHub release target triple for this platform, or `None` if unsupported
/// (e.g. Windows) — those fall back to a manual download.
pub(crate) fn release_target() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        _ => return None,
    })
}

/// Whether an in-place self-update is possible on this platform.
pub(crate) fn can_self_update() -> bool {
    release_target().is_some()
}

fn brew_manages_formula(runner: &impl CommandRunner, formula: &str) -> bool {
    runner
        .output(OsStr::new("brew"), &args(&["list", "--versions", formula]))
        .map(|o| o.success && !o.stdout.is_empty())
        .unwrap_or(false)
}

fn managed_brew_formula(runner: &impl CommandRunner) -> Option<&'static str> {
    if brew_manages_formula(runner, BREW_SHORT_FORMULA)
        || brew_manages_formula(runner, BREW_FORMULA)
    {
        Some(BREW_FORMULA)
    } else {
        None
    }
}

fn brew_has_version(runner: &impl CommandRunner, formula: &str, v: &str) -> bool {
    runner
        .output(OsStr::new("brew"), &args(&["list", "--versions", formula]))
        .map(|o| o.success && String::from_utf8_lossy(&o.stdout).contains(v))
        .unwrap_or(false)
}

fn brew_prefix_bin(runner: &impl CommandRunner, formula: &str) -> Option<PathBuf> {
    let out = runner.output(OsStr::new("brew"), &args(&["--prefix", formula]))?;
    if !out.success {
        return None;
    }
    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!prefix.is_empty()).then(|| PathBuf::from(prefix).join("bin").join("a3s"))
}

fn binary_version(runner: &impl CommandRunner, bin: impl AsRef<OsStr>) -> Option<String> {
    let out = runner.output(bin.as_ref(), &[OsString::from("--version")])?;
    if !out.success {
        return None;
    }
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    version_from_output(&text)
}

fn verify_binary_version(
    runner: &impl CommandRunner,
    bin: impl AsRef<OsStr>,
    latest: &str,
) -> Option<String> {
    let version = binary_version(runner, bin)?;
    version_ge(&version, latest).then_some(version)
}

fn verify_brew_binary(
    runner: &impl CommandRunner,
    formula: &str,
    current_exe: &Path,
    latest: &str,
) -> Option<PathBuf> {
    let path_bin = PathBuf::from("a3s");
    if verify_binary_version(runner, path_bin.as_os_str(), latest).is_some() {
        return Some(path_bin);
    }
    let prefix_bin = brew_prefix_bin(runner, formula)?;
    verify_binary_version(runner, prefix_bin.as_os_str(), latest)?;

    eprintln!("\n⚠  Homebrew has a3s {latest}, but `a3s` on PATH is still older — relinking…");
    let _ = runner.status(OsStr::new("brew"), &args(&["link", "--overwrite", formula]));
    if verify_binary_version(runner, path_bin.as_os_str(), latest).is_some() {
        return Some(path_bin);
    }

    if current_exe != prefix_bin {
        eprintln!(
            "⚠  Homebrew link is still shadowed — repairing {} from {}…",
            current_exe.display(),
            prefix_bin.display()
        );
        if swap_binary_and_verify(runner, &prefix_bin, current_exe, latest).is_ok() {
            if verify_binary_version(runner, path_bin.as_os_str(), latest).is_some() {
                return Some(path_bin);
            }
            return Some(current_exe.to_path_buf());
        }
    }

    eprintln!(
        "⚠  Homebrew binary is current at {}, but the active `a3s` command is still shadowed",
        prefix_bin.display()
    );
    None
}

fn sibling_webview_helper(current_exe: &Path) -> Option<PathBuf> {
    let sibling = current_exe.parent()?.join(WEBVIEW_BINARY);
    sibling.is_file().then_some(sibling)
}

pub(crate) fn webview_supports_agent_island_output(stdout: &[u8], stderr: &[u8]) -> bool {
    let stdout = String::from_utf8_lossy(stdout);
    let stderr = String::from_utf8_lossy(stderr);
    let contract = format!("{stdout}\n{stderr}");
    contract.contains("usage: a3s-webview --agent-island")
        && contract.contains("--snapshot")
        && contract.contains("--lock-file")
}

fn webview_supports_agent_island(runner: &impl CommandRunner, binary: &Path) -> bool {
    runner
        .output_bounded(
            binary.as_os_str(),
            &[OsString::from("--agent-island"), OsString::from("--help")],
            AGENT_ISLAND_HELPER_PROBE_TIMEOUT,
        )
        .is_some_and(|output| webview_supports_agent_island_output(&output.stdout, &output.stderr))
}

fn path_webview_helper(runner: &impl CommandRunner) -> Option<PathBuf> {
    let binary = PathBuf::from(WEBVIEW_BINARY);
    webview_supports_agent_island(runner, &binary).then_some(binary)
}

fn webview_helper_path(runner: &impl CommandRunner, current_exe: &Path) -> Option<PathBuf> {
    sibling_webview_helper(current_exe)
        .filter(|binary| webview_supports_agent_island(runner, binary))
        .or_else(|| path_webview_helper(runner))
}

fn configured_webview_helper() -> Option<(&'static str, PathBuf)> {
    [AGENT_ISLAND_BIN_ENV, WEBVIEW_BIN_ENV]
        .into_iter()
        .find_map(|name| {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(|value| (name, PathBuf::from(value)))
        })
}

fn ensure_webview_helper_with(
    runner: &impl CommandRunner,
    current_exe: &Path,
) -> Result<PathBuf, String> {
    if let Some((name, path)) = configured_webview_helper() {
        if webview_supports_agent_island(runner, &path) {
            return Ok(path);
        }
        return Err(format!(
            "{name} points to {}, which does not expose the required Agent Island contract; update or unset the override",
            path.display()
        ));
    }
    if let Some(path) = webview_helper_path(runner, current_exe) {
        return Ok(path);
    }
    if !cfg!(any(target_os = "macos", target_os = "linux")) {
        return Err(format!(
            "{WEBVIEW_BINARY} is missing or obsolete and no automatic helper repair is available on this platform"
        ));
    }

    let _ = runner.status(OsStr::new("brew"), &args(&["tap", BREW_TAP, BREW_TAP_URL]));
    let installed = runner.status(OsStr::new("brew"), &args(&["install", WEBVIEW_FORMULA]));
    if let Some(path) = webview_helper_path(runner, current_exe) {
        return Ok(path);
    }
    if installed {
        Err(
            "Homebrew installed a3s-webview, but no helper with Agent Island support is available"
                .to_string(),
        )
    } else {
        Err("a3s-webview is missing or obsolete and Homebrew could not install it".to_string())
    }
}

/// Repair install-time companion tools. The helper must expose the Agent Island
/// contract; an older RemoteUI-only binary is not considered ready.
pub(crate) fn repair_installation() -> Result<Vec<String>, String> {
    let runner = RealCommandRunner;
    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate current binary: {e}"))?;
    let mut repaired = Vec::new();
    if let Some(formula) = managed_brew_formula(&runner) {
        let current = current_version();
        if let Some(bin) = verify_brew_binary(&runner, formula, &exe, &current) {
            repaired.push(format!("Homebrew command ready: {}", bin.display()));
        }
    }
    let path = ensure_webview_helper_with(&runner, &exe)?;
    repaired.push(format!("Native window helper ready: {}", path.display()));
    Ok(repaired)
}

/// Upgrade to `latest` in place. Returns the binary to exec on success —
/// Homebrew repoints `a3s` on PATH (exec by name); a direct download swaps
/// `current_exe` (exec that path) — or an error explaining why every path failed.
///
/// Run after the TUI has exited (terminal restored) so child stdio shows real
/// download/upgrade progress.
pub(crate) fn perform_upgrade(latest: &str) -> Result<PathBuf, String> {
    let runner = RealCommandRunner;
    let exe =
        std::env::current_exe().map_err(|e| format!("could not locate current binary: {e}"))?;
    perform_upgrade_with(latest, &runner, exe)
}

fn perform_upgrade_with(
    latest: &str,
    runner: &impl CommandRunner,
    current_exe: PathBuf,
) -> Result<PathBuf, String> {
    if latest.trim().is_empty() {
        return Err("latest version is empty".to_string());
    }

    let mut failures = Vec::new();
    if let Some(formula) = managed_brew_formula(runner) {
        // `brew upgrade` reads a *cached* formula — refresh the tap first, else
        // it no-ops with "already installed". Prefer a fast targeted git pull,
        // fall back to a full `brew update`.
        let _ = runner.status(OsStr::new("brew"), &args(&["tap", BREW_TAP, BREW_TAP_URL]));
        let tap = runner
            .output(OsStr::new("brew"), &args(&["--repo", BREW_TAP]))
            .filter(|o| o.success)
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty());
        let pulled = tap
            .as_ref()
            .map(|r| {
                runner.status(
                    OsStr::new("git"),
                    &[
                        OsString::from("-C"),
                        OsString::from(r),
                        OsString::from("pull"),
                        OsString::from("--quiet"),
                        OsString::from("--ff-only"),
                    ],
                )
            })
            .unwrap_or(false);
        if !pulled {
            let _ = runner.status(OsStr::new("brew"), &args(&["update"]));
        }
        println!("\n⬇  upgrading a3s {latest} via Homebrew…\n");
        let upgrade_ok = runner.status(OsStr::new("brew"), &args(&["upgrade", formula]));
        let mut brew_binary = verify_brew_binary(runner, formula, &current_exe, latest);
        if brew_binary.is_none() {
            // Homebrew metadata can claim the new version while PATH still runs
            // an older binary (stale link, failed pour, or partial tap refresh).
            // Reinstall once before falling back to the standalone updater.
            let metadata_has_latest = brew_has_version(runner, formula, latest);
            let reason = if metadata_has_latest {
                format!("Homebrew metadata says {latest}, but `a3s --version` did not")
            } else if upgrade_ok {
                "Homebrew upgrade finished, but the installed binary is still old".to_string()
            } else {
                "Homebrew upgrade failed".to_string()
            };
            eprintln!("\n⚠  {reason} — reinstalling…");
            let _ = runner.status(OsStr::new("brew"), &args(&["reinstall", formula]));
            brew_binary = verify_brew_binary(runner, formula, &current_exe, latest);
        }

        if let Some(bin) = brew_binary {
            match ensure_webview_helper_with(runner, &current_exe) {
                Ok(_) => return Ok(bin),
                Err(error) => failures.push(format!(
                    "Homebrew installed a3s {latest}, but its required native helper is not ready: {error}"
                )),
            }
        } else {
            failures.push(format!(
                "Homebrew formula {formula} did not install a3s {latest}"
            ));
        }
        eprintln!(
            "\n⚠  Homebrew did not produce a complete a3s {latest} installation — falling back to a direct download…"
        );
    }
    standalone_upgrade_with(latest, runner, current_exe).map_err(|e| {
        failures.push(e);
        failures.join("; ")
    })
}

fn standalone_upgrade_with(
    latest: &str,
    runner: &impl CommandRunner,
    exe: PathBuf,
) -> Result<PathBuf, String> {
    let target = release_target().ok_or_else(|| {
        format!(
            "automatic self-update is not supported on {}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let url = format!(
        "https://github.com/A3S-Lab/a3s/releases/download/v{latest}/a3s-v{latest}-{target}.tar.gz"
    );
    let asset_name = format!("a3s-v{latest}-{target}.tar.gz");
    let digest = release_asset_digest(runner, latest, &asset_name)?;
    let tmp = unique_update_dir();
    if std::fs::create_dir_all(&tmp).is_err() {
        return Err(format!(
            "could not create temporary directory {}",
            tmp.display()
        ));
    }
    println!("\n⬇  downloading a3s {latest}…\n");
    let download = runner.output(
        OsStr::new("curl"),
        &[
            OsString::from("-fL"),
            OsString::from("--silent"),
            OsString::from("--show-error"),
            OsString::from("--retry"),
            OsString::from("3"),
            OsString::from("--connect-timeout"),
            OsString::from("10"),
            OsString::from("--max-time"),
            OsString::from("180"),
            OsString::from("--max-filesize"),
            OsString::from(MAX_SELF_UPDATE_ARCHIVE_BYTES.to_string()),
            OsString::from("--proto"),
            OsString::from("=https"),
            OsString::from("--proto-redir"),
            OsString::from("=https"),
            OsString::from(&url),
        ],
    );
    let Some(download) = download else {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!("download failed: {url}"));
    };
    if !download.success {
        let _ = std::fs::remove_dir_all(&tmp);
        let detail = String::from_utf8_lossy(&download.stderr);
        return Err(format!("download failed: {url}: {}", detail.trim()));
    }
    if download.stdout.len() > MAX_SELF_UPDATE_ARCHIVE_BYTES {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!(
            "downloaded archive exceeds the {} byte limit",
            MAX_SELF_UPDATE_ARCHIVE_BYTES
        ));
    }
    verify_sha256(&download.stdout, &digest).map_err(|error| {
        let _ = std::fs::remove_dir_all(&tmp);
        format!("self-update checksum verification failed: {error:#}")
    })?;
    extract_tar_gz_archive(&download.stdout, &tmp).map_err(|error| {
        let _ = std::fs::remove_dir_all(&tmp);
        format!("failed to safely extract release archive: {error:#}")
    })?;
    let new_bin = find_downloaded_binary(&tmp);
    if new_bin.is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err("release archive did not contain an a3s binary".to_string());
    }
    let new_bin = new_bin.unwrap();
    let Some(new_webview) = find_downloaded_webview_helper(&tmp) else {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!(
            "release archive did not contain the required {WEBVIEW_BINARY} companion"
        ));
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&new_bin, std::fs::Permissions::from_mode(0o755));
        let _ = std::fs::set_permissions(&new_webview, std::fs::Permissions::from_mode(0o755));
    }
    if verify_binary_version(runner, new_bin.as_os_str(), latest).is_none() {
        eprintln!("\n✗ downloaded a3s did not report version {latest}");
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!(
            "downloaded binary {} did not report version {latest}",
            new_bin.display()
        ));
    }
    if !webview_supports_agent_island(runner, &new_webview) {
        let _ = std::fs::remove_dir_all(&tmp);
        return Err(format!(
            "downloaded helper {} does not expose the required Agent Island contract",
            new_webview.display()
        ));
    }

    let installed_helper = exe
        .parent()
        .ok_or_else(|| format!("cannot locate the install directory for {}", exe.display()))?
        .join(WEBVIEW_BINARY);
    let helper_existed = installed_helper.is_file();
    let helper_backup = tmp.join(format!(
        ".previous-{WEBVIEW_BINARY}-{:016x}",
        rand::random::<u64>()
    ));
    if helper_existed {
        copy_executable(&installed_helper, &helper_backup).map_err(|error| {
            let _ = std::fs::remove_dir_all(&tmp);
            format!(
                "preserve existing helper {} before update: {error}",
                installed_helper.display()
            )
        })?;
    }
    let restore_helper = || {
        if helper_existed {
            install_sibling_companion(&helper_backup, &exe, WEBVIEW_BINARY).map(|_| ())
        } else if installed_helper.exists() {
            std::fs::remove_file(&installed_helper).map_err(|error| {
                format!(
                    "remove newly installed helper {}: {error}",
                    installed_helper.display()
                )
            })
        } else {
            Ok(())
        }
    };
    if let Err(error) = install_sibling_companion(&new_webview, &exe, WEBVIEW_BINARY) {
        let helper_rollback = restore_helper();
        let _ = std::fs::remove_dir_all(&tmp);
        let mut detail = format!(
            "native window helper validation passed but installation failed before updating a3s: {error}"
        );
        if let Err(rollback_error) = helper_rollback {
            detail.push_str(&format!(
                "; additionally failed to restore the previous native helper: {rollback_error}"
            ));
        }
        return Err(detail);
    }

    let result = match swap_binary_and_verify(runner, &new_bin, &exe, latest) {
        Ok(()) => Ok(exe),
        Err(err) => {
            eprintln!("\n✗ failed to install downloaded a3s: {err}");
            let helper_rollback = restore_helper();
            match helper_rollback {
                Ok(()) => Err(err),
                Err(rollback_error) => {
                    if helper_existed && helper_backup.is_file() {
                        return Err(format!(
                            "{err}; additionally failed to restore the previous native helper: {rollback_error}; its recovery copy remains at {}",
                            helper_backup.display()
                        ));
                    }
                    Err(format!(
                        "{err}; additionally failed to restore the previous native helper: {rollback_error}"
                    ))
                }
            }
        }
    };
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

fn release_asset_digest(
    runner: &impl CommandRunner,
    latest: &str,
    asset_name: &str,
) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/A3S-Lab/a3s/releases/tags/v{latest}");
    let output = runner
        .output(
            OsStr::new("curl"),
            &[
                OsString::from("-fsSL"),
                OsString::from("--retry"),
                OsString::from("3"),
                OsString::from("--connect-timeout"),
                OsString::from("10"),
                OsString::from("--max-time"),
                OsString::from("30"),
                OsString::from("--max-filesize"),
                OsString::from((4 * 1024 * 1024).to_string()),
                OsString::from("--proto"),
                OsString::from("=https"),
                OsString::from("--proto-redir"),
                OsString::from("=https"),
                OsString::from("-H"),
                OsString::from("Accept: application/vnd.github+json"),
                OsString::from("-H"),
                OsString::from("User-Agent: a3s-self-update/1.0"),
                OsString::from(&url),
            ],
        )
        .ok_or_else(|| format!("failed to query release metadata from {url}"))?;
    if !output.success {
        return Err(format!(
            "release metadata request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("release metadata is invalid JSON: {error}"))?;
    let digest = value
        .get("assets")
        .and_then(serde_json::Value::as_array)
        .and_then(|assets| {
            assets.iter().find(|asset| {
                asset.get("name").and_then(serde_json::Value::as_str) == Some(asset_name)
            })
        })
        .and_then(|asset| asset.get("digest"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!("release asset '{asset_name}' has no GitHub SHA-256 digest; refusing an unverified self-update")
        })?;
    let digest = digest.strip_prefix("sha256:").unwrap_or(digest);
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "release asset '{asset_name}' has an invalid SHA-256 digest"
        ));
    }
    Ok(digest.to_ascii_lowercase())
}

fn find_downloaded_binary(root: &Path) -> Option<PathBuf> {
    find_downloaded_named_binary(root, A3S_BINARY)
}

fn find_downloaded_webview_helper(root: &Path) -> Option<PathBuf> {
    find_downloaded_named_binary(root, WEBVIEW_BINARY)
}

fn find_downloaded_named_binary(root: &Path, binary_name: &str) -> Option<PathBuf> {
    let direct = root.join(binary_name);
    if direct.is_file() {
        return Some(direct);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == OsStr::new(binary_name))
            {
                return Some(path);
            }
        }
    }
    None
}

fn unique_update_dir() -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("a3s-update-{}-{nanos}", std::process::id()))
}

fn sibling_temp_path(target: &Path, suffix: &str) -> Option<PathBuf> {
    let dir = target.parent()?;
    let name = target.file_name()?.to_string_lossy();
    Some(dir.join(format!(
        ".{name}.a3s-update-{}.{suffix}",
        std::process::id()
    )))
}

fn copy_executable(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::copy(src, dst).map(|_| ())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dst, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_sibling_companion(
    staging: &Path,
    target: &Path,
    _target_existed: bool,
) -> std::io::Result<()> {
    // Both paths are in the install directory, so rename replaces the visible
    // helper in one filesystem operation without a missing-target window.
    std::fs::rename(staging, target)
}

#[cfg(windows)]
fn replace_sibling_companion(
    staging: &Path,
    target: &Path,
    target_existed: bool,
) -> std::io::Result<()> {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_WRITE_THROUGH, REPLACEFILE_WRITE_THROUGH,
    };

    let staging = staging
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    let target = target
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    // SAFETY: both path buffers are NUL-terminated and remain alive for the
    // call. ReplaceFileW preserves a continuously addressable target; the
    // no-target case uses a same-directory, write-through move.
    let replaced = unsafe {
        if target_existed {
            ReplaceFileW(
                target.as_ptr(),
                staging.as_ptr(),
                std::ptr::null(),
                REPLACEFILE_WRITE_THROUGH,
                std::ptr::null(),
                std::ptr::null(),
            )
        } else {
            MoveFileExW(staging.as_ptr(), target.as_ptr(), MOVEFILE_WRITE_THROUGH)
        }
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn preserve_sibling_companion(target: &Path, backup: &Path) -> std::io::Result<()> {
    std::fs::hard_link(target, backup).or_else(|_| std::fs::copy(target, backup).map(|_| ()))
}

fn install_sibling_companion(
    source: &Path,
    current_exe: &Path,
    binary_name: &str,
) -> Result<PathBuf, String> {
    let directory = current_exe.parent().ok_or_else(|| {
        format!(
            "cannot locate the install directory for {}",
            current_exe.display()
        )
    })?;
    let target = directory.join(binary_name);
    if target.exists() && !target.is_file() {
        return Err(format!("{} is not a regular file", target.display()));
    }
    let staging = sibling_temp_path(&target, "new")
        .ok_or_else(|| format!("cannot derive a staging path for {}", target.display()))?;
    let backup = sibling_temp_path(&target, "bak")
        .ok_or_else(|| format!("cannot derive a backup path for {}", target.display()))?;

    let _ = std::fs::remove_file(&staging);
    let _ = std::fs::remove_file(&backup);
    copy_executable(source, &staging).map_err(|error| {
        format!(
            "copy {} to {}: {error}",
            source.display(),
            staging.display()
        )
    })?;

    let had_target = target.is_file();
    #[cfg(windows)]
    if had_target {
        if let Err(error) = preserve_sibling_companion(&target, &backup) {
            return Err(format!(
                "preserve existing helper {} at {} before replacement: {error}; the installed helper remains intact and the new helper remains staged at {}",
                target.display(),
                backup.display(),
                staging.display()
            ));
        }
    }

    if let Err(error) = replace_sibling_companion(&staging, &target, had_target) {
        #[cfg(windows)]
        {
            let preserved = if !had_target {
                "no previous helper existed".to_string()
            } else if backup.is_file() {
                format!("the previous helper is preserved at {}", backup.display())
            } else if target.is_file() {
                format!("the installed helper remains at {}", target.display())
            } else {
                "the installed helper location must be checked manually".to_string()
            };
            let staged = if staging.is_file() {
                format!("the new helper remains staged at {}", staging.display())
            } else {
                "the staged helper was consumed by the operating system".to_string()
            };
            let retry = if had_target {
                format!("Close any running {binary_name} windows and retry the update")
            } else {
                "Retry the update".to_string()
            };
            return Err(format!(
                "atomically replace {}: {error}; {preserved}; {staged}. {retry}",
                target.display()
            ));
        }
        #[cfg(not(windows))]
        {
            let _ = std::fs::remove_file(&staging);
            let _ = std::fs::remove_file(&backup);
            return Err(format!(
                "atomically replace staged helper {} at {}: {error}",
                staging.display(),
                target.display()
            ));
        }
    }
    if !target.is_file() {
        let _ = std::fs::remove_file(&staging);
        return Err(format!(
            "replacement completed without a regular helper at {}",
            target.display()
        ));
    }
    let _ = std::fs::remove_file(&backup);
    Ok(target)
}

fn swap_binary_and_verify(
    runner: &impl CommandRunner,
    new_bin: &Path,
    target: &Path,
    latest: &str,
) -> Result<(), String> {
    let staging = sibling_temp_path(target, "new")
        .ok_or_else(|| format!("cannot derive staging path for {}", target.display()))?;
    let backup = sibling_temp_path(target, "bak")
        .ok_or_else(|| format!("cannot derive backup path for {}", target.display()))?;

    let _ = std::fs::remove_file(&staging);
    let _ = std::fs::remove_file(&backup);

    copy_executable(new_bin, &staging)
        .map_err(|e| format!("copy {} to {}: {e}", new_bin.display(), staging.display()))?;

    std::fs::hard_link(target, &backup)
        .or_else(|_| std::fs::copy(target, &backup).map(|_| ()))
        .map_err(|e| format!("backup {} to {}: {e}", target.display(), backup.display()))?;

    if let Err(err) = std::fs::rename(&staging, target) {
        let _ = std::fs::remove_file(&staging);
        let _ = std::fs::remove_file(&backup);
        return Err(format!(
            "rename {} over {}: {err}",
            staging.display(),
            target.display()
        ));
    }

    if verify_binary_version(runner, target.as_os_str(), latest).is_some() {
        let _ = std::fs::remove_file(&backup);
        return Ok(());
    }

    std::fs::rename(&backup, target).map_err(|e| {
        format!(
            "restore {} from {}: {e}",
            target.display(),
            backup.display()
        )
    })?;
    Err(format!(
        "{} did not report version {latest} after replacement",
        target.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};

    static REAL_PROCESS_TEST_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

    async fn lock_real_process_tests() -> tokio::sync::MutexGuard<'static, ()> {
        REAL_PROCESS_TEST_LOCK
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await
    }

    #[test]
    fn version_ordering() {
        assert!(version_ge("0.5.6", "0.5.5"));
        assert!(version_ge("0.5.5", "0.5.5"));
        assert!(!version_ge("0.5.4", "0.5.5"));
        assert!(version_ge("1.0.0", "0.9.9"));
        assert!(version_ge("v0.5.11", "0.5.9"));
        assert!(version_ge("1.0", "1.0.0"));
        assert!(!version_ge("1.0.0", "1.0.1"));
    }

    #[test]
    fn parse_latest_stable_cli_version_from_api_json() {
        let json = br#"[
            {"tag_name":"a3s-code-v9.0.0","draft":false,"prerelease":false},
            {"tag_name":"v2.3.3","draft":false,"prerelease":false},
            {"tag_name":"v9.0.0","draft":true,"prerelease":false},
            {"tag_name":"v2.3.4","draft":false,"prerelease":false},
            {"tag_name":"v3.0.0-rc.1","draft":false,"prerelease":true}
        ]"#;
        assert_eq!(version_from_api_response(json).as_deref(), Some("2.3.4"));
        assert_eq!(version_from_api_response(br#"[{"name":"v2.3.4"}]"#), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancelling_async_output_terminates_the_child() {
        use std::time::Duration;

        let _process_guard = lock_real_process_tests().await;
        let temp = tempfile::tempdir().unwrap();
        let started = temp.path().join("started");
        let finished = temp.path().join("finished");
        let mut command = tokio::process::Command::new("sh");
        command
            .args([
                "-c",
                r#"printf started > "$1"; sleep 1; printf finished > "$2""#,
                "a3s-update-cancellation-test",
            ])
            .arg(&started)
            .arg(&finished);

        let task = tokio::spawn(cancellable_command_output(command));
        tokio::time::timeout(Duration::from_secs(2), async {
            while !started.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("test child did not start");

        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::sleep(Duration::from_millis(1_500)).await;

        assert!(
            !finished.exists(),
            "cancelled child continued running after its future was dropped"
        );
    }

    #[test]
    fn parse_version_from_binary_output() {
        assert_eq!(
            version_from_output("a3s 0.5.11\n").as_deref(),
            Some("0.5.11")
        );
        assert_eq!(
            version_from_output("a3s-code v0.6.0 (release)\n").as_deref(),
            Some("0.6.0")
        );
        assert_eq!(version_from_output("not a version\n"), None);
    }

    #[test]
    fn target_is_known_on_this_host() {
        // CI runs on macOS + Linux, both supported.
        if cfg!(any(target_os = "macos", target_os = "linux")) {
            assert!(release_target().is_some());
            assert!(can_self_update());
        }
    }

    fn compatible_helper_probe() -> CommandOutput {
        CommandOutput {
            // The real helper intentionally exits non-zero after printing its
            // mode-specific usage for this capability probe.
            success: false,
            stdout: Vec::new(),
            stderr: b"usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>\n"
                .to_vec(),
        }
    }

    #[derive(Default)]
    struct FakeRunner {
        commands: Mutex<Vec<String>>,
        version_checks: AtomicUsize,
    }

    impl FakeRunner {
        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }

        fn record(&self, program: &OsStr, args: &[OsString]) -> String {
            let mut line = program.to_string_lossy().to_string();
            for arg in args {
                line.push(' ');
                line.push_str(&arg.to_string_lossy());
            }
            self.commands.lock().unwrap().push(line.clone());
            line
        }
    }

    impl CommandRunner for FakeRunner {
        fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput> {
            let line = self.record(program, args);
            if line == format!("{WEBVIEW_BINARY} --agent-island --help") {
                return Some(compatible_helper_probe());
            }
            let stdout = match line.as_str() {
                "brew list --versions a3s" => b"a3s 9.9.9\n".to_vec(),
                "brew --repo a3s-lab/tap" => b"/tmp/a3s-tap\n".to_vec(),
                "a3s --version" => {
                    if self.version_checks.fetch_add(1, Ordering::SeqCst) == 0 {
                        b"a3s 0.1.0\n".to_vec()
                    } else {
                        b"a3s 9.9.9\n".to_vec()
                    }
                }
                _ => return None,
            };
            Some(CommandOutput {
                success: true,
                stdout,
                stderr: Vec::new(),
            })
        }

        fn status(&self, program: &OsStr, args: &[OsString]) -> bool {
            self.record(program, args);
            true
        }
    }

    #[test]
    fn brew_upgrade_reinstalls_when_metadata_is_new_but_binary_is_old() {
        let runner = FakeRunner::default();
        let result = perform_upgrade_with("9.9.9", &runner, PathBuf::from("/unused/a3s"));

        assert_eq!(result.as_deref(), Ok(Path::new("a3s")));
        let commands = runner.commands();
        assert!(commands
            .iter()
            .any(|c| c == "brew tap a3s-lab/tap https://github.com/A3S-Lab/homebrew-tap"));
        assert!(commands.iter().any(|c| c == "brew upgrade a3s-lab/tap/a3s"));
        assert!(commands
            .iter()
            .any(|c| c == "brew reinstall a3s-lab/tap/a3s"));
        assert_eq!(runner.version_checks.load(Ordering::SeqCst), 2);
    }

    struct ShadowedBrewRunner {
        commands: Mutex<Vec<String>>,
        linked: AtomicBool,
        prefix: PathBuf,
    }

    impl ShadowedBrewRunner {
        fn new(prefix: PathBuf) -> Self {
            Self {
                commands: Mutex::new(Vec::new()),
                linked: AtomicBool::new(false),
                prefix,
            }
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }

        fn record(&self, program: &OsStr, args: &[OsString]) -> String {
            let mut line = program.to_string_lossy().to_string();
            for arg in args {
                line.push(' ');
                line.push_str(&arg.to_string_lossy());
            }
            self.commands.lock().unwrap().push(line.clone());
            line
        }

        fn prefix_bin(&self) -> PathBuf {
            self.prefix.join("bin").join("a3s")
        }
    }

    impl CommandRunner for ShadowedBrewRunner {
        fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput> {
            let line = self.record(program, args);
            if line == format!("{WEBVIEW_BINARY} --agent-island --help") {
                return Some(compatible_helper_probe());
            }
            let prefix_line = format!("brew --prefix {BREW_FORMULA}");
            let prefix_bin = self.prefix_bin();
            let stdout = if line == "brew list --versions a3s" {
                b"a3s 9.9.9\n".to_vec()
            } else if line == "brew --repo a3s-lab/tap" {
                b"/tmp/a3s-tap\n".to_vec()
            } else if line == prefix_line {
                format!("{}\n", self.prefix.display()).into_bytes()
            } else if line == "a3s --version" {
                if self.linked.load(Ordering::SeqCst) {
                    b"a3s 9.9.9\n".to_vec()
                } else {
                    b"a3s 0.1.0\n".to_vec()
                }
            } else if program == prefix_bin.as_os_str() && args == [OsString::from("--version")] {
                b"a3s 9.9.9\n".to_vec()
            } else {
                return None;
            };
            Some(CommandOutput {
                success: true,
                stdout,
                stderr: Vec::new(),
            })
        }

        fn status(&self, program: &OsStr, args: &[OsString]) -> bool {
            let line = self.record(program, args);
            if line == format!("brew link --overwrite {BREW_FORMULA}") {
                self.linked.store(true, Ordering::SeqCst);
                return true;
            }
            line == format!("brew tap {BREW_TAP} {BREW_TAP_URL}")
                || line == "git -C /tmp/a3s-tap pull --quiet --ff-only"
                || line == format!("brew upgrade {BREW_FORMULA}")
        }
    }

    #[test]
    fn brew_upgrade_relinks_when_keg_is_latest_but_path_is_old() {
        let runner = ShadowedBrewRunner::new(PathBuf::from("/tmp/a3s-shadowed-prefix"));
        let result = perform_upgrade_with("9.9.9", &runner, PathBuf::from("/unused/a3s"));

        assert_eq!(result.as_deref(), Ok(Path::new("a3s")));
        assert!(runner
            .commands()
            .iter()
            .any(|c| c == &format!("brew link --overwrite {BREW_FORMULA}")));
    }

    #[cfg(unix)]
    struct TempDir {
        root: PathBuf,
    }

    #[cfg(unix)]
    impl TempDir {
        fn new(name: &str) -> Self {
            static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "a3s-update-test-{name}-{}-{id}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.root.join(name)
        }
    }

    #[cfg(unix)]
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn write_executable(path: &Path, version: &str) {
        use std::os::unix::fs::PermissionsExt;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, format!("#!/bin/sh\nprintf 'a3s {version}\\n'\n")).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    fn write_webview_executable(path: &Path, version: &str) {
        use std::os::unix::fs::PermissionsExt;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(
            path,
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"--agent-island\" ]; then\n  printf '%s\\n' 'usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>' >&2\n  exit 2\nfi\nprintf 'a3s {version}\\n'\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    struct LinkFailingBrewRunner {
        commands: Mutex<Vec<String>>,
        prefix: PathBuf,
    }

    #[cfg(unix)]
    impl LinkFailingBrewRunner {
        fn new(prefix: PathBuf) -> Self {
            Self {
                commands: Mutex::new(Vec::new()),
                prefix,
            }
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }

        fn record(&self, program: &OsStr, args: &[OsString]) -> String {
            let mut line = program.to_string_lossy().to_string();
            for arg in args {
                line.push(' ');
                line.push_str(&arg.to_string_lossy());
            }
            self.commands.lock().unwrap().push(line.clone());
            line
        }
    }

    #[cfg(unix)]
    impl CommandRunner for LinkFailingBrewRunner {
        fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput> {
            let line = self.record(program, args);
            if line == "brew list --versions a3s" {
                return Some(CommandOutput {
                    success: true,
                    stdout: b"a3s 9.9.9\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            if line == "brew --repo a3s-lab/tap" {
                return Some(CommandOutput {
                    success: true,
                    stdout: b"/tmp/a3s-tap\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            if line == format!("brew --prefix {BREW_FORMULA}") {
                return Some(CommandOutput {
                    success: true,
                    stdout: format!("{}\n", self.prefix.display()).into_bytes(),
                    stderr: Vec::new(),
                });
            }
            if line == "a3s --version" {
                return Some(CommandOutput {
                    success: true,
                    stdout: b"a3s 0.1.0\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            let path = Path::new(program);
            if path.is_absolute() && args == [OsString::from("--version")] {
                let out = Command::new(path).arg("--version").output().ok()?;
                return Some(CommandOutput {
                    success: out.status.success(),
                    stdout: out.stdout,
                    stderr: out.stderr,
                });
            }
            None
        }

        fn status(&self, program: &OsStr, args: &[OsString]) -> bool {
            let line = self.record(program, args);
            line == format!("brew tap {BREW_TAP} {BREW_TAP_URL}")
                || line == "git -C /tmp/a3s-tap pull --quiet --ff-only"
                || line == format!("brew upgrade {BREW_FORMULA}")
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn brew_upgrade_does_not_report_success_without_the_required_helper() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("brew-shadowed-current");
        let current_exe = tmp.path("shadowed-a3s");
        let prefix = tmp.path("prefix");
        let prefix_bin = prefix.join("bin").join("a3s");
        write_executable(&current_exe, "0.1.0");
        write_executable(&prefix_bin, "9.9.9");
        let runner = LinkFailingBrewRunner::new(prefix);

        let result = perform_upgrade_with("9.9.9", &runner, current_exe.clone());

        let error = result.unwrap_err();
        assert!(error.contains("required native helper"), "{error}");
        let out = Command::new(&current_exe)
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 9.9.9\n");
        assert!(runner
            .commands()
            .iter()
            .any(|c| c == &format!("brew link --overwrite {BREW_FORMULA}")));
    }

    #[derive(Default)]
    struct HelperRunner {
        commands: Mutex<Vec<String>>,
        helper_available: AtomicBool,
    }

    impl HelperRunner {
        fn with_helper_available() -> Self {
            Self {
                helper_available: AtomicBool::new(true),
                ..Self::default()
            }
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }

        fn record(&self, program: &OsStr, args: &[OsString]) -> String {
            let mut line = program.to_string_lossy().to_string();
            for arg in args {
                line.push(' ');
                line.push_str(&arg.to_string_lossy());
            }
            self.commands.lock().unwrap().push(line.clone());
            line
        }
    }

    impl CommandRunner for HelperRunner {
        fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput> {
            let line = self.record(program, args);
            if line == format!("{WEBVIEW_BINARY} --agent-island --help") {
                let available = self.helper_available.load(Ordering::SeqCst);
                return Some(CommandOutput {
                    // The real helper intentionally exits non-zero after printing
                    // its mode-specific usage for this capability probe.
                    success: false,
                    stdout: Vec::new(),
                    stderr: if available {
                        b"usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>\n"
                                .to_vec()
                    } else {
                        b"usage: a3s-webview --url <http(s)://...>\n".to_vec()
                    },
                });
            }
            None
        }

        fn status(&self, program: &OsStr, args: &[OsString]) -> bool {
            let line = self.record(program, args);
            if line == format!("brew install {WEBVIEW_FORMULA}") {
                self.helper_available.store(true, Ordering::SeqCst);
                return true;
            }
            line == format!("brew tap {BREW_TAP} {BREW_TAP_URL}")
        }
    }

    #[test]
    #[cfg(unix)]
    fn remoteui_helper_uses_existing_path_helper_without_brew() {
        let tmp = TempDir::new("helper-path");
        let runner = HelperRunner::with_helper_available();

        let result = ensure_webview_helper_with(&runner, &tmp.path("a3s")).unwrap();

        assert_eq!(result, PathBuf::from(WEBVIEW_BINARY));
        assert_eq!(
            runner.commands(),
            vec![format!("{WEBVIEW_BINARY} --agent-island --help")]
        );
    }

    #[test]
    #[cfg(unix)]
    fn remoteui_helper_installs_missing_homebrew_helper() {
        let tmp = TempDir::new("helper-install");
        let runner = HelperRunner::default();

        let result = ensure_webview_helper_with(&runner, &tmp.path("a3s")).unwrap();

        assert_eq!(result, PathBuf::from(WEBVIEW_BINARY));
        let commands = runner.commands();
        assert!(commands
            .iter()
            .any(|c| c == &format!("brew tap {BREW_TAP} {BREW_TAP_URL}")));
        assert!(commands
            .iter()
            .any(|c| c == &format!("brew install {WEBVIEW_FORMULA}")));
        assert_eq!(
            commands
                .iter()
                .filter(|c| { c.as_str() == format!("{WEBVIEW_BINARY} --agent-island --help") })
                .count(),
            2
        );
    }

    #[test]
    fn agent_island_capability_rejects_remoteui_only_helpers() {
        assert!(!webview_supports_agent_island_output(
            &[],
            b"a3s-webview: unknown argument: --agent-island\nusage: a3s-webview --url <http(s)://...>\n",
        ));
        assert!(webview_supports_agent_island_output(
            &[],
            b"usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>\n",
        ));
    }

    fn synthetic_target_pe() -> Vec<u8> {
        const PE_OFFSET: usize = 0x80;

        let mut binary = vec![0_u8; PE_OFFSET + 24];
        binary[..2].copy_from_slice(b"MZ");
        binary[0x3c..0x40].copy_from_slice(&(PE_OFFSET as u32).to_le_bytes());
        binary[PE_OFFSET..PE_OFFSET + 4].copy_from_slice(b"PE\0\0");
        binary[PE_OFFSET + 4..PE_OFFSET + 6].copy_from_slice(
            &target_windows_pe_machine()
                .expect("tests require an x86_64 or aarch64 target")
                .to_le_bytes(),
        );
        binary
    }

    fn synthetic_target_pe_with_agent_island_contract() -> Vec<u8> {
        let mut binary = synthetic_target_pe();
        binary.extend_from_slice(AGENT_ISLAND_HELPER_USAGE);
        binary.extend_from_slice(b"\0other embedded data\0");
        binary.extend_from_slice(SYSTEM_AGENT_SNAPSHOT_MARKER);
        binary
    }

    #[test]
    fn static_windows_contract_accepts_target_pe_with_full_markers() {
        assert!(webview_binary_contains_agent_island_contract(
            &synthetic_target_pe_with_agent_island_contract()
        ));
    }

    #[test]
    fn static_windows_contract_rejects_non_pe_and_truncated_headers() {
        let mut marker_blob = AGENT_ISLAND_HELPER_USAGE.to_vec();
        marker_blob.extend_from_slice(SYSTEM_AGENT_SNAPSHOT_MARKER);
        assert!(!webview_binary_contains_agent_island_contract(&marker_blob));
        assert!(!webview_binary_has_target_pe_header(b"MZ"));

        let mut truncated = vec![0_u8; 0x84];
        truncated[..2].copy_from_slice(b"MZ");
        truncated[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        truncated[0x80..0x84].copy_from_slice(b"PE\0\0");
        assert!(!webview_binary_has_target_pe_header(&truncated));
    }

    #[test]
    fn static_windows_contract_rejects_invalid_or_unbounded_pe_offsets() {
        let mut overlapping = synthetic_target_pe_with_agent_island_contract();
        overlapping[0x3c..0x40].copy_from_slice(&0x20_u32.to_le_bytes());
        assert!(!webview_binary_contains_agent_island_contract(&overlapping));

        let pe_offset = MAX_WINDOWS_PE_HEADER_OFFSET + 1;
        let mut unbounded = vec![0_u8; pe_offset + 6];
        unbounded[..2].copy_from_slice(b"MZ");
        unbounded[0x3c..0x40].copy_from_slice(&(pe_offset as u32).to_le_bytes());
        unbounded[pe_offset..pe_offset + 4].copy_from_slice(b"PE\0\0");
        unbounded[pe_offset + 4..pe_offset + 6].copy_from_slice(
            &target_windows_pe_machine()
                .expect("tests require an x86_64 or aarch64 target")
                .to_le_bytes(),
        );
        assert!(!webview_binary_has_target_pe_header(&unbounded));
    }

    #[test]
    fn static_windows_contract_rejects_bad_signature_and_wrong_machine() {
        let mut bad_signature = synthetic_target_pe_with_agent_island_contract();
        bad_signature[0x80..0x84].copy_from_slice(b"PX\0\0");
        assert!(!webview_binary_contains_agent_island_contract(
            &bad_signature
        ));

        let mut wrong_machine = synthetic_target_pe_with_agent_island_contract();
        let machine = match target_windows_pe_machine() {
            Some(WINDOWS_PE_MACHINE_AMD64) => WINDOWS_PE_MACHINE_ARM64,
            Some(WINDOWS_PE_MACHINE_ARM64) => WINDOWS_PE_MACHINE_AMD64,
            _ => panic!("tests require an x86_64 or aarch64 target"),
        };
        wrong_machine[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
        assert!(!webview_binary_contains_agent_island_contract(
            &wrong_machine
        ));
    }

    #[test]
    fn static_windows_contract_still_requires_usage_and_snapshot_schema() {
        let mut usage_only = synthetic_target_pe();
        usage_only.extend_from_slice(AGENT_ISLAND_HELPER_USAGE);
        assert!(!webview_binary_contains_agent_island_contract(&usage_only));

        let mut schema_only = synthetic_target_pe();
        schema_only.extend_from_slice(SYSTEM_AGENT_SNAPSHOT_MARKER);
        assert!(!webview_binary_contains_agent_island_contract(&schema_only));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bounded_helper_probe_rejects_oversized_output_without_waiting_for_exit() {
        use std::os::unix::fs::PermissionsExt;

        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("bounded-helper-output");
        let helper = tmp.path("a3s-webview-noisy");
        std::fs::write(&helper, "#!/bin/sh\nhead -c 65536 /dev/zero\nsleep 5\n").unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let started = Instant::now();

        let output = bounded_command_output(
            helper.as_os_str(),
            &[OsString::from("--agent-island"), OsString::from("--help")],
            Duration::from_secs(2),
        );

        assert!(output.is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn bounded_helper_probe_closes_descendant_held_pipes() {
        use std::os::unix::fs::PermissionsExt;

        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("bounded-helper-descendant");
        let helper = tmp.path("a3s-webview-descendant");
        std::fs::write(
            &helper,
            "#!/bin/sh\nprintf '%s\\n' 'usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>' >&2\n(sleep 5) &\nexit 2\n",
        )
        .unwrap();
        std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
        let started = Instant::now();

        let output = bounded_command_output(
            helper.as_os_str(),
            &[OsString::from("--agent-island"), OsString::from("--help")],
            Duration::from_secs(2),
        )
        .expect("descendant-held output pipes should be closed with the probe process tree");

        assert!(webview_supports_agent_island_output(
            &output.stdout,
            &output.stderr
        ));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_swap_replaces_target_and_verifies_new_version() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("swap-success");
        let target = tmp.path("a3s");
        let new_bin = tmp.path("downloaded-a3s");
        write_executable(&target, "0.1.0");
        write_executable(&new_bin, "9.9.9");

        let runner = RealCommandRunner;
        swap_binary_and_verify(&runner, &new_bin, &target, "9.9.9").unwrap();

        let out = Command::new(&target).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 9.9.9\n");
        assert!(!sibling_temp_path(&target, "new").unwrap().exists());
        assert!(!sibling_temp_path(&target, "bak").unwrap().exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_swap_restores_target_when_new_binary_reports_wrong_version() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("swap-restore");
        let target = tmp.path("a3s");
        let new_bin = tmp.path("downloaded-a3s");
        write_executable(&target, "0.1.0");
        write_executable(&new_bin, "0.2.0");

        let runner = RealCommandRunner;
        let err = swap_binary_and_verify(&runner, &new_bin, &target, "9.9.9").unwrap_err();

        assert!(err.contains("did not report version 9.9.9"));
        let out = Command::new(&target).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 0.1.0\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sibling_companion_install_replaces_an_existing_helper_atomically() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("companion-replace");
        let current = tmp.path("a3s");
        let downloaded = tmp.path("downloaded-webview");
        let installed = tmp.path(WEBVIEW_BINARY);
        write_executable(&current, "9.9.9");
        write_executable(&downloaded, "0.1.2");
        write_executable(&installed, "0.1.1");

        let result = install_sibling_companion(&downloaded, &current, WEBVIEW_BINARY).unwrap();

        assert_eq!(result, installed);
        let out = Command::new(&installed).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 0.1.2\n");
        assert!(!sibling_temp_path(&installed, "new").unwrap().exists());
        assert!(!sibling_temp_path(&installed, "bak").unwrap().exists());
    }

    #[cfg(unix)]
    struct FakeStandaloneRunner {
        commands: Mutex<Vec<String>>,
        archive: Vec<u8>,
        digest: Option<String>,
        rejected_version_path: Option<PathBuf>,
    }

    #[cfg(unix)]
    impl FakeStandaloneRunner {
        fn new(binary_path: &str) -> Self {
            let archive = standalone_archive(binary_path, "9.9.9");
            let digest = a3s_updater::sha256_hex(&archive);
            Self {
                commands: Mutex::new(Vec::new()),
                archive,
                digest: Some(digest),
                rejected_version_path: None,
            }
        }

        fn commands(&self) -> Vec<String> {
            self.commands.lock().unwrap().clone()
        }

        fn record(&self, program: &OsStr, args: &[OsString]) -> String {
            let mut line = program.to_string_lossy().to_string();
            for arg in args {
                line.push(' ');
                line.push_str(&arg.to_string_lossy());
            }
            self.commands.lock().unwrap().push(line.clone());
            line
        }
    }

    #[cfg(unix)]
    impl CommandRunner for FakeStandaloneRunner {
        fn output(&self, program: &OsStr, args: &[OsString]) -> Option<CommandOutput> {
            let line = self.record(program, args);
            if line == "brew list --versions a3s" || line == "brew list --versions a3s-lab/tap/a3s"
            {
                return Some(CommandOutput {
                    success: false,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
            }
            if program == OsStr::new("curl") {
                let url = args.last()?.to_string_lossy();
                if url.contains("api.github.com/repos/A3S-Lab/a3s/releases/tags/v9.9.9") {
                    let target = release_target()?;
                    let metadata = serde_json::to_vec(&serde_json::json!({
                        "tag_name": "v9.9.9",
                        "assets": [{
                            "name": format!("a3s-v9.9.9-{target}.tar.gz"),
                            "digest": self.digest,
                        }]
                    }))
                    .ok()?;
                    return Some(CommandOutput {
                        success: true,
                        stdout: metadata,
                        stderr: Vec::new(),
                    });
                }
                if url.contains("github.com/A3S-Lab/a3s/releases/download/v9.9.9/") {
                    return Some(CommandOutput {
                        success: true,
                        stdout: self.archive.clone(),
                        stderr: Vec::new(),
                    });
                }
            }
            if self.rejected_version_path.as_deref() == Some(Path::new(program))
                && args == [OsString::from("--version")]
            {
                return Some(CommandOutput {
                    success: true,
                    stdout: b"a3s 0.1.0\n".to_vec(),
                    stderr: Vec::new(),
                });
            }
            RealCommandRunner.output(program, args)
        }

        fn status(&self, program: &OsStr, args: &[OsString]) -> bool {
            self.record(program, args);
            false
        }
    }

    #[cfg(unix)]
    fn standalone_archive(binary_path: &str, version: &str) -> Vec<u8> {
        standalone_archive_with_helper(binary_path, version, Some(true))
    }

    #[cfg(unix)]
    fn standalone_archive_with_helper(
        binary_path: &str,
        version: &str,
        helper_compatible: Option<bool>,
    ) -> Vec<u8> {
        let tmp = TempDir::new("standalone-archive");
        let root = tmp.path("root");
        let archive = tmp.path("release.tar.gz");
        write_executable(&root.join(binary_path), version);
        match helper_compatible {
            Some(true) => write_webview_executable(&root.join(WEBVIEW_BINARY), "0.1.2"),
            Some(false) => write_executable(&root.join(WEBVIEW_BINARY), "0.1.2"),
            None => {}
        }
        let top_level = Path::new(binary_path)
            .components()
            .next()
            .unwrap()
            .as_os_str();
        let mut command = Command::new("tar");
        command
            .arg("czf")
            .arg(&archive)
            .arg("-C")
            .arg(&root)
            .arg(top_level);
        if helper_compatible.is_some() {
            command.arg(WEBVIEW_BINARY);
        }
        let status = command.status().unwrap();
        assert!(status.success());
        std::fs::read(archive).unwrap()
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_fallback_downloads_installs_and_verifies() {
        let _process_guard = lock_real_process_tests().await;
        let Some(target) = release_target() else {
            return;
        };

        let tmp = TempDir::new("standalone-upgrade");
        let current = tmp.path("a3s");
        write_executable(&current, "0.1.0");

        let runner = FakeStandaloneRunner::new("a3s");
        let result = standalone_upgrade_with("9.9.9", &runner, current.clone());

        assert_eq!(result.as_deref(), Ok(current.as_path()));
        let out = Command::new(&current).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 9.9.9\n");
        assert!(tmp.path(WEBVIEW_BINARY).is_file());

        let commands = runner.commands();
        assert!(commands
            .iter()
            .any(|c| c.contains(&format!("a3s-v9.9.9-{target}.tar.gz"))));
        assert!(commands
            .iter()
            .any(|c| c.contains("api.github.com/repos/A3S-Lab/a3s/releases/tags/v9.9.9")));
        assert!(!commands.iter().any(|c| c.starts_with("tar ")));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_accepts_nested_archive_binary() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("standalone-nested");
        let current = tmp.path("a3s");
        write_executable(&current, "0.1.0");

        let runner = FakeStandaloneRunner::new("pkg/bin/a3s");
        let result = standalone_upgrade_with("9.9.9", &runner, current.clone());

        assert_eq!(result.as_deref(), Ok(current.as_path()));
        let out = Command::new(&current).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 9.9.9\n");
        assert!(tmp.path(WEBVIEW_BINARY).is_file());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_rejects_a_checksum_mismatch_without_replacing_the_binary() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("standalone-checksum-mismatch");
        let current = tmp.path("a3s");
        write_executable(&current, "0.1.0");
        let mut runner = FakeStandaloneRunner::new("a3s");
        runner.digest = Some("0".repeat(64));

        let error = standalone_upgrade_with("9.9.9", &runner, current.clone()).unwrap_err();

        assert!(error.contains("checksum verification failed"), "{error}");
        let out = Command::new(&current).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 0.1.0\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_refuses_release_metadata_without_a_digest() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("standalone-missing-digest");
        let current = tmp.path("a3s");
        write_executable(&current, "0.1.0");
        let mut runner = FakeStandaloneRunner::new("a3s");
        runner.digest = None;

        let error = standalone_upgrade_with("9.9.9", &runner, current.clone()).unwrap_err();

        assert!(error.contains("no GitHub SHA-256 digest"), "{error}");
        let commands = runner.commands();
        assert_eq!(
            commands
                .iter()
                .filter(|command| command.starts_with("curl "))
                .count(),
            1,
            "the archive must not download before metadata is trusted: {commands:?}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_rejects_a_missing_helper_before_replacing_a3s() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("standalone-missing-helper");
        let current = tmp.path("a3s");
        write_executable(&current, "0.1.0");
        let mut runner = FakeStandaloneRunner::new("a3s");
        runner.archive = standalone_archive_with_helper("a3s", "9.9.9", None);
        runner.digest = Some(a3s_updater::sha256_hex(&runner.archive));

        let error = standalone_upgrade_with("9.9.9", &runner, current.clone()).unwrap_err();

        assert!(error.contains("required"), "{error}");
        let out = Command::new(&current).arg("--version").output().unwrap();
        assert_eq!(String::from_utf8_lossy(&out.stdout), "a3s 0.1.0\n");
        assert!(!tmp.path(WEBVIEW_BINARY).exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_rejects_an_incompatible_helper_before_mutation() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("standalone-incompatible-helper");
        let current = tmp.path("a3s");
        let installed_helper = tmp.path(WEBVIEW_BINARY);
        write_executable(&current, "0.1.0");
        write_webview_executable(&installed_helper, "0.1.1");
        let mut runner = FakeStandaloneRunner::new("a3s");
        runner.archive = standalone_archive_with_helper("a3s", "9.9.9", Some(false));
        runner.digest = Some(a3s_updater::sha256_hex(&runner.archive));

        let error = standalone_upgrade_with("9.9.9", &runner, current.clone()).unwrap_err();

        assert!(error.contains("Agent Island contract"), "{error}");
        let cli = Command::new(&current).arg("--version").output().unwrap();
        let helper = Command::new(&installed_helper)
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&cli.stdout), "a3s 0.1.0\n");
        assert_eq!(String::from_utf8_lossy(&helper.stdout), "a3s 0.1.1\n");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn standalone_upgrade_restores_the_previous_helper_when_the_cli_swap_fails() {
        let _process_guard = lock_real_process_tests().await;
        let tmp = TempDir::new("standalone-helper-rollback");
        let current = tmp.path("a3s");
        let installed_helper = tmp.path(WEBVIEW_BINARY);
        write_executable(&current, "0.1.0");
        write_webview_executable(&installed_helper, "0.1.1");
        let mut runner = FakeStandaloneRunner::new("a3s");
        runner.rejected_version_path = Some(current.clone());

        let error = standalone_upgrade_with("9.9.9", &runner, current.clone()).unwrap_err();

        assert!(error.contains("did not report version 9.9.9"), "{error}");
        let cli = Command::new(&current).arg("--version").output().unwrap();
        let helper = Command::new(&installed_helper)
            .arg("--version")
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&cli.stdout), "a3s 0.1.0\n");
        assert_eq!(String::from_utf8_lossy(&helper.stdout), "a3s 0.1.1\n");
    }
}
