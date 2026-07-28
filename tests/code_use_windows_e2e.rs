#![cfg(all(windows, target_arch = "x86_64"))]

mod support;

#[path = "code_use_windows_e2e/fake_openai.rs"]
mod fake_openai;
#[path = "code_use_windows_e2e/fixture.rs"]
mod fixture;
#[path = "code_use_windows_e2e/plans.rs"]
mod plans;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use fake_openai::FakeOpenAi;
use fixture::{
    create_ocr_fixture, prepare_use_install, prepare_webview_probe, required_path, FixtureSite,
};
use plans::{browser_plan, ocr_plan, office_compat_plan, office_plan, PlannedToolCall};
use support::{a3s_bin, TempWorkspace};

struct WindowsE2e {
    workspace: TempWorkspace,
    project: PathBuf,
    use_install: PathBuf,
    webview_install: PathBuf,
    edge: PathBuf,
}

impl WindowsE2e {
    fn prepare() -> Self {
        let workspace = TempWorkspace::new("code-use-windows-all-tools");
        let project = workspace.path("project");
        let use_install = workspace.path("use-install");
        let webview_install = workspace.path("webview-install");
        std::fs::create_dir_all(project.join(".a3s")).expect("create Windows E2E project");

        let use_binary = required_path("A3S_USE_E2E_BIN");
        let source_root = required_path("A3S_USE_E2E_SOURCE_ROOT");
        prepare_use_install(&use_binary, &source_root, &use_install);
        prepare_webview_probe(&webview_install);

        let edge = PathBuf::from(r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe");
        assert!(edge.is_file(), "Windows E2E requires Microsoft Edge");
        Self {
            workspace,
            project,
            use_install,
            webview_install,
            edge,
        }
    }

    fn run_phase(&self, label: &str, plan: Vec<PlannedToolCall>) {
        let model = FakeOpenAi::start(label, plan.clone());
        self.write_config(&model.base_url);
        let system_root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let isolated_path = std::env::join_paths([system_root.join("System32"), system_root])
            .expect("construct isolated Windows PATH");
        let prompt = format!(
            "Delegate the complete {label} Windows matrix to the dedicated use worker. Required tools: {}",
            plan.iter()
                .map(|call| call.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );

        let mut command = Command::new(a3s_bin());
        command
            .args(["code", "-C"])
            .arg(&self.project)
            .arg("--config")
            .arg(self.project.join(".a3s/config.acl"))
            .env("HOME", self.workspace.path("home"))
            .env("USERPROFILE", self.workspace.path("home"))
            .env("LOCALAPPDATA", self.workspace.path("local-app-data"))
            .env("APPDATA", self.workspace.path("app-data"))
            .env("A3S_DATA_HOME", self.workspace.path("data"))
            .env("A3S_STATE_HOME", self.workspace.path("state"))
            .env("A3S_CACHE_HOME", self.workspace.path("cache"))
            .env("A3S_RUNTIME_HOME", self.workspace.path("runtime"))
            .env("A3S_USE_INSTALL_DIR", &self.use_install)
            .env("A3S_WEBVIEW_INSTALL_DIR", &self.webview_install)
            .env("A3S_BROWSER_EXECUTABLE", &self.edge)
            .env("A3S_USE_BROWSER_EXECUTABLE_PATH", &self.edge)
            .env("A3S_USE_BROWSER_HEADED", "false")
            .env("A3S_CODE_TUI_SMOKE", "1")
            .env("A3S_CODE_TUI_PROMPT", prompt)
            .env("PATH", isolated_path)
            .env("NO_PROXY", "127.0.0.1,localhost")
            .env("no_proxy", "127.0.0.1,localhost")
            .env_remove("A3S_OFFLINE")
            .env_remove("A3S_NO_AUTO_INSTALL")
            .env_remove("A3S_OFFICECLI_EXECUTABLE");
        let output = run_with_live_output(command, label);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            output.status.success(),
            "{label}: stdout={stdout}\nstderr={stderr}"
        );
        assert!(
            stderr.contains("[tool start] task") && stderr.contains("[tool end] task (exit 0)"),
            "{label}: the primary TUI did not complete its delegated Use task\nstderr={stderr}"
        );
        model.assert_complete(&plan);
    }

    fn write_config(&self, base_url: &str) {
        std::fs::write(
            self.project.join(".a3s/config.acl"),
            format!(
                r#"default_model = "openai/fake"
providers "openai" {{
  apiKey = "test"
  baseUrl = "{base_url}"
  models "fake" {{
    name = "Windows Use E2E"
    toolCall = true
  }}
}}
memory {{ llmExtraction = false }}
"#
            ),
        )
        .expect("write Windows E2E model config");
    }

    fn path(&self, name: &str) -> PathBuf {
        self.project.join(name)
    }
}

fn run_with_live_output(mut command: Command, label: &str) -> Output {
    let phase_timeout = std::env::var("A3S_USE_WINDOWS_E2E_PHASE_TIMEOUT_SECS")
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| Duration::from_secs(30 * 60));
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| panic!("run {label} Code TUI smoke: {error}"));
    let stdout = capture_live_output(
        child.stdout.take().expect("capture Code TUI stdout"),
        "stdout",
    );
    let stderr = capture_live_output(
        child.stderr.take().expect("capture Code TUI stderr"),
        "stderr",
    );
    let deadline = Instant::now() + phase_timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .unwrap_or_else(|error| panic!("poll {label} Code TUI smoke: {error}"))
        {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = stdout.join().expect("join Code TUI stdout capture");
            let stderr = stderr.join().expect("join Code TUI stderr capture");
            panic!(
                "{label} exceeded its {:?} phase timeout\nstdout={}\nstderr={}",
                phase_timeout,
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            );
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    Output {
        status,
        stdout: stdout.join().expect("join Code TUI stdout capture"),
        stderr: stderr.join().expect("join Code TUI stderr capture"),
    }
}

fn capture_live_output(
    mut stream: impl Read + Send + 'static,
    channel: &'static str,
) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut buffer = [0_u8; 8 * 1024];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => {
                    output.extend_from_slice(&buffer[..read]);
                    eprint!(
                        "[windows-use-e2e:{channel}] {}",
                        String::from_utf8_lossy(&buffer[..read])
                    );
                }
                Err(error) => panic!("read Code TUI {channel}: {error}"),
            }
        }
        output
    })
}

#[test]
#[ignore = "requires real Use artifacts, Edge, and network access for bounded providers"]
fn code_tui_exercises_every_projected_use_tool_and_optional_capability_on_windows() {
    let e2e = WindowsE2e::prepare();
    let site = FixtureSite::start();

    let browser_screenshot = e2e.path("browser.png");
    let browser_calls = browser_plan(&site.url, &browser_screenshot);
    e2e.run_phase("Browser core tools", browser_calls);
    assert_nonempty_file(&browser_screenshot);

    let document = e2e.path("native.docx");
    let merged = e2e.path("merged.docx");
    let office_screenshot = e2e.path("office.png");
    let office_calls = office_plan(&document, &merged, &office_screenshot);
    e2e.run_phase("native Office tools and views", office_calls);
    for artifact in [&document, &merged, &office_screenshot] {
        assert_nonempty_file(artifact);
    }

    let ocr_image = e2e.path("ocr.png");
    create_ocr_fixture(&ocr_image);
    let ocr_calls = ocr_plan(&ocr_image);
    e2e.run_phase("native OCR tools", ocr_calls);

    let compat_calls = office_compat_plan();
    e2e.run_phase("Office compatibility MCP", compat_calls);
}

fn assert_nonempty_file(path: &Path) {
    let metadata = std::fs::metadata(path)
        .unwrap_or_else(|error| panic!("missing artifact {}: {error}", path.display()));
    assert!(
        metadata.is_file() && metadata.len() > 0,
        "artifact is empty: {}",
        path.display()
    );
}
