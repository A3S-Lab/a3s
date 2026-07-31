#![cfg(unix)]

mod support;

#[path = "code_use_first_use/real_release.rs"]
mod real_release;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use sha2::{Digest, Sha256};
use support::{
    a3s_bin, make_executable, portable_release_target, FakeReleaseServer, TempWorkspace,
};

const USE_VERSION: &str = "0.1.1";

struct FakeOpenAi {
    base_url: String,
    saw_ready_ocr_route: Arc<AtomicBool>,
    tool_names: Arc<Mutex<Vec<String>>>,
    task_descriptions: Arc<Mutex<Vec<String>>>,
    requests: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeOpenAi {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake OpenAI server");
        listener
            .set_nonblocking(true)
            .expect("configure fake OpenAI listener");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let saw_ready_ocr_route = Arc::new(AtomicBool::new(false));
        let tool_names = Arc::new(Mutex::new(Vec::new()));
        let task_descriptions = Arc::new(Mutex::new(Vec::new()));
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_saw_ready_ocr_route = Arc::clone(&saw_ready_ocr_route);
        let thread_tool_names = Arc::clone(&tool_names);
        let thread_task_descriptions = Arc::clone(&task_descriptions);
        let thread_requests = Arc::clone(&requests);
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let saw_ready_ocr_route = Arc::clone(&thread_saw_ready_ocr_route);
                        let tool_names = Arc::clone(&thread_tool_names);
                        let task_descriptions = Arc::clone(&thread_task_descriptions);
                        let requests = Arc::clone(&thread_requests);
                        std::thread::spawn(move || {
                            serve_openai_request(
                                stream,
                                &saw_ready_ocr_route,
                                &tool_names,
                                &task_descriptions,
                                &requests,
                            )
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
            base_url,
            saw_ready_ocr_route,
            tool_names,
            task_descriptions,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    fn saw_ready_ocr_route(&self) -> bool {
        self.saw_ready_ocr_route.load(Ordering::SeqCst)
    }

    fn request_count(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }

    fn tool_names(&self) -> Vec<String> {
        self.tool_names.lock().unwrap().clone()
    }

    fn task_descriptions(&self) -> Vec<String> {
        self.task_descriptions.lock().unwrap().clone()
    }
}

impl Drop for FakeOpenAi {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_openai_request(
    mut stream: TcpStream,
    saw_ready_ocr_route: &AtomicBool,
    observed_tool_names: &Mutex<Vec<String>>,
    observed_task_descriptions: &Mutex<Vec<String>>,
    requests: &AtomicUsize,
) {
    stream
        .set_nonblocking(false)
        .expect("configure fake OpenAI connection");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("configure fake OpenAI timeout");
    let request = read_http_request(&mut stream);
    requests.fetch_add(1, Ordering::SeqCst);
    let body = request_body(&request);
    let tools = body
        .get("tools")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten();
    let tool_names = tools
        .clone()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .or_else(|| tool.get("name"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let task_descriptions = tools
        .filter_map(|tool| {
            let name = tool
                .pointer("/function/name")
                .or_else(|| tool.get("name"))
                .and_then(serde_json::Value::as_str)?;
            (name == "task")
                .then(|| {
                    tool.pointer("/function/description")
                        .or_else(|| tool.get("description"))
                        .and_then(serde_json::Value::as_str)
                })
                .flatten()
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    if task_descriptions
        .iter()
        .any(|description| description.contains("use/ocr"))
    {
        saw_ready_ocr_route.store(true, Ordering::SeqCst);
    }
    observed_tool_names.lock().unwrap().extend(tool_names);
    observed_task_descriptions
        .lock()
        .unwrap()
        .extend(task_descriptions);

    let streaming = body.get("stream").and_then(serde_json::Value::as_bool) == Some(true);
    if streaming {
        let response = concat!(
            "data: {\"id\":\"chatcmpl-use-first-use\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"fake\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Use ready.\"},\"finish_reason\":null}],\"usage\":null}\n\n",
            "data: {\"id\":\"chatcmpl-use-first-use\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"fake\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\n",
            "data: [DONE]\n\n"
        );
        write_http_response(&mut stream, "text/event-stream", response.as_bytes());
        return;
    }

    let response = serde_json::to_vec(&serde_json::json!({
        "id": "chatcmpl-use-first-use",
        "object": "chat.completion",
        "created": 0,
        "model": "fake",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": serde_json::json!({
                    "intent": "GeneralPurpose",
                    "requires_planning": false,
                    "optimized_input": "Report A3S Use readiness."
                }).to_string()
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 1,
            "completion_tokens": 1,
            "total_tokens": 2
        }
    }))
    .unwrap();
    write_http_response(&mut stream, "application/json", &response);
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 16_384];
    loop {
        let read = stream.read(&mut buffer).expect("read fake OpenAI request");
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .and_then(|value| value.trim().parse::<usize>().ok())
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
    request
}

fn request_body(request: &[u8]) -> serde_json::Value {
    let body_start = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("fake OpenAI request must contain headers")
        + 4;
    serde_json::from_slice(&request[body_start..]).expect("decode fake OpenAI request")
}

fn write_http_response(stream: &mut TcpStream, content_type: &str, body: &[u8]) {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .expect("write fake OpenAI response headers");
    stream
        .write_all(body)
        .expect("write fake OpenAI response body");
    stream.flush().expect("flush fake OpenAI response");
}

fn start_fake_use_release(workspace: &TempWorkspace) -> FakeReleaseServer {
    let target = portable_release_target().expect("test host must support a portable Use release");
    let package_name = format!("a3s-use-{USE_VERSION}-{target}");
    let release_root = workspace.path("release");
    let package_root = release_root.join(&package_name);
    let skill_content = r#"---
name: a3s-use-ocr
description: Diagnose OCR readiness through the built-in A3S Use route.
---

# A3S Use OCR

Use `mcp__use_ocr__ocr_doctor` to inspect OCR readiness.
"#;
    let skill_path = package_root.join("ocr-skills/a3s-use-ocr/SKILL.md");
    std::fs::create_dir_all(skill_path.parent().unwrap()).expect("create fake OCR Skill directory");
    std::fs::write(&skill_path, skill_content).expect("write fake OCR Skill");
    let digest = format!("{:x}", Sha256::digest(skill_content.as_bytes()));
    let script = fake_use_script(&digest);
    make_executable(&package_root.join("a3s-use"), &script);

    let archive_name = format!("a3s-use-{USE_VERSION}-{target}.tar.gz");
    let archive_path = workspace.path(&archive_name);
    let status = Command::new("tar")
        .arg("czf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&release_root)
        .arg(&package_name)
        .status()
        .expect("create fake Use release archive");
    assert!(
        status.success(),
        "failed to create fake Use release archive"
    );
    let archive = std::fs::read(archive_path).expect("read fake Use release archive");
    FakeReleaseServer::start("Use", USE_VERSION, &archive_name, archive)
}

fn fake_use_script(skill_digest: &str) -> String {
    let script = r#"#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  printf 'a3s-use __VERSION__\n'
  exit 0
fi

root=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
skill_root="$root/ocr-skills"
skill="$skill_root/a3s-use-ocr/SKILL.md"

if [ "${1:-}" = "mcp" ] && [ "${2:-}" = "serve" ]; then
  if [ -n "${A3S_USE_E2E_MCP_MARKER:-}" ]; then
    printf '%s\n' "$*" > "$A3S_USE_E2E_MCP_MARKER"
  fi
  while IFS= read -r line; do
    if [ -n "${A3S_USE_E2E_MCP_LOG:-}" ]; then
      printf '%s\n' "$line" >> "$A3S_USE_E2E_MCP_LOG"
    fi
    id=$(printf '%s\n' "$line" | sed -n 's/.*"id":\([^,}]*\).*/\1/p')
    case "$line" in
      *'"method":"initialize"'*)
        printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"protocolVersion\":\"2024-11-05\",\"capabilities\":{},\"serverInfo\":{\"name\":\"use-first-use-fixture\",\"version\":\"__VERSION__\"}}}"
        ;;
      *'"method":"notifications/initialized"'*) ;;
      *'"method":"tools/list"'*)
        printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"tools\":[{\"name\":\"ocr_doctor\",\"description\":\"Inspect OCR readiness\",\"inputSchema\":{\"type\":\"object\"},\"annotations\":{\"readOnlyHint\":true,\"destructiveHint\":false,\"idempotentHint\":true,\"openWorldHint\":false}}]}}"
        ;;
      *'"method":"tools/call"'*)
        printf '%s\n' "{\"jsonrpc\":\"2.0\",\"id\":$id,\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"{\\\"readiness\\\":\\\"missing\\\"}\"}],\"isError\":false}}"
        ;;
    esac
  done
  exit 0
fi

case "${1:-} ${2:-}" in
  "capability snapshot")
    printf '%s\n' "{\"schemaVersion\":1,\"ok\":true,\"data\":{\"registry\":{\"schemaVersion\":1,\"generation\":1,\"revision\":\"1111111111111111111111111111111111111111111111111111111111111111\",\"capabilities\":[{\"id\":\"use/ocr\",\"route\":\"ocr\",\"version\":\"__VERSION__\",\"origin\":\"built-in\",\"enabled\":true,\"readiness\":\"missing\",\"packageRoot\":\"$skill_root\",\"surfaces\":[\"mcp\",\"skill\"],\"mcp\":{\"target\":\"ocr-native\",\"transport\":\"stdio\"},\"skills\":[{\"path\":\"$skill\",\"sha256\":\"__DIGEST__\"}]}]}}}"
    ;;
  "capability watch")
    sleep 0.05
    printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"changed":false}}'
    ;;
  *)
    exit 2
    ;;
esac
"#;
    script
        .replace("__VERSION__", USE_VERSION)
        .replace("__DIGEST__", skill_digest)
}

fn write_config(project: &Path, base_url: &str) -> PathBuf {
    let config = project.join(".a3s/config.acl");
    std::fs::create_dir_all(config.parent().unwrap()).expect("create project config directory");
    std::fs::write(
        &config,
        format!(
            r#"default_model = "openai/fake"
providers "openai" {{
  apiKey = "test"
  baseUrl = "{base_url}"
  models "fake" {{
    name = "Fake"
    toolCall = true
  }}
}}
memory {{ llmExtraction = false }}
"#
        ),
    )
    .expect("write project config");
    config
}

enum FirstUsePolicy {
    Online,
    Offline,
    NoAutoInstall,
}

fn run_tui_smoke(
    workspace: &TempWorkspace,
    release: &FakeReleaseServer,
    policy: FirstUsePolicy,
) -> (std::process::Output, FakeOpenAi) {
    let project = workspace.path("project");
    std::fs::create_dir_all(&project).expect("create test project");
    let llm = FakeOpenAi::start();
    let config = write_config(&project, &llm.base_url);
    let mut command = Command::new(a3s_bin());
    command
        .args(["code", "-C"])
        .arg(&project)
        .arg("--config")
        .arg(&config)
        .env("HOME", workspace.path("home"))
        .env("A3S_DATA_HOME", workspace.path("data"))
        .env("A3S_STATE_HOME", workspace.path("state"))
        .env("A3S_CACHE_HOME", workspace.path("cache"))
        .env("A3S_RUNTIME_HOME", workspace.path("runtime"))
        .env("A3S_CODE_TUI_SMOKE", "1")
        .env("A3S_CODE_TUI_SMOKE_WAIT_USE", "1")
        .env(
            "A3S_CODE_TUI_PROMPT",
            "Report whether the projected A3S Use OCR capability is ready.",
        )
        .env("A3S_USE_E2E_MCP_MARKER", workspace.path("mcp-started"))
        .env("A3S_USE_E2E_MCP_LOG", workspace.path("mcp.log"))
        .env("A3S_UPDATER_GITHUB_API_BASE", release.api_base())
        .env("PATH", "/usr/bin:/bin")
        .env_remove("A3S_OFFLINE")
        .env_remove("A3S_NO_AUTO_INSTALL");
    match policy {
        FirstUsePolicy::Online => {}
        FirstUsePolicy::Offline => {
            command.arg("--offline");
        }
        FirstUsePolicy::NoAutoInstall => {
            command.env("A3S_NO_AUTO_INSTALL", "1");
        }
    }
    let output = command.output().expect("run Code TUI smoke");
    (output, llm)
}

#[test]
fn code_tui_first_use_installs_use_and_projects_ocr_before_the_first_turn() {
    let workspace = TempWorkspace::new("code-use-first-use");
    let release = start_fake_use_release(&workspace);
    let (output, llm) = run_tui_smoke(&workspace, &release, FirstUsePolicy::Online);

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        llm.request_count() > 0,
        "the TUI smoke did not call the model"
    );
    assert!(
        llm.saw_ready_ocr_route(),
        "the first model turn did not receive a task route for ready use/ocr; tools={:?}; task_descriptions={:?}; mcp_marker={:?}; mcp_log={:?}; receipt={}; release_requests={:?}; stderr={}",
        llm.tool_names(),
        llm.task_descriptions(),
        std::fs::read_to_string(workspace.path("mcp-started")).ok(),
        std::fs::read_to_string(workspace.path("mcp.log")).ok(),
        workspace.path("state/components/use.json").exists(),
        release.requests(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(workspace.path("mcp-started")).unwrap(),
        "mcp serve ocr-native\n"
    );

    let receipt_path = workspace.path("state/components/use.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    assert_eq!(receipt["componentId"], "use");
    assert_eq!(receipt["version"], USE_VERSION);
    let executable = receipt["executablePath"].as_str().unwrap();
    assert!(Path::new(executable).is_file(), "{executable}");

    let requests = release.requests();
    assert!(
        requests
            .iter()
            .any(|path| path == "/repos/A3S-Lab/Use/releases/latest"),
        "{requests:?}"
    );
    assert!(
        requests
            .iter()
            .any(|path| path.starts_with("/assets/a3s-use-")),
        "{requests:?}"
    );
}

#[test]
fn code_tui_offline_and_no_auto_install_never_download_or_write_a_receipt() {
    for (name, policy) in [
        ("offline", FirstUsePolicy::Offline),
        ("no-auto-install", FirstUsePolicy::NoAutoInstall),
    ] {
        let workspace = TempWorkspace::new(&format!("code-use-{name}"));
        let release = start_fake_use_release(&workspace);
        let (output, llm) = run_tui_smoke(&workspace, &release, policy);

        assert!(
            output.status.success(),
            "{name}: stdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            llm.request_count() > 0,
            "{name}: the TUI smoke did not call the model"
        );
        assert!(
            !llm.saw_ready_ocr_route(),
            "{name}: a missing Use component advertised a ready OCR route"
        );
        assert!(
            release.requests().is_empty(),
            "{name}: first-use policy accessed the release server: {:?}",
            release.requests()
        );
        assert!(
            !workspace.path("state/components/use.json").exists(),
            "{name}: first-use policy wrote a Use receipt"
        );
        assert!(
            !workspace.path("mcp-started").exists(),
            "{name}: first-use policy launched a Use MCP server"
        );
    }
}

#[test]
#[ignore = "requires real Use, Browser, and OCR source artifacts"]
fn code_tui_first_use_installs_a_real_use_release_before_the_first_turn() {
    let workspace = TempWorkspace::new("code-use-real-release");
    let release = real_release::start(&workspace);
    let (output, llm) = run_tui_smoke(&workspace, &release.server, FirstUsePolicy::Online);

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        llm.request_count() > 0,
        "the TUI smoke did not call the model"
    );
    let descriptions = llm.task_descriptions().join("\n");
    for capability in ["use/browser", "use/ocr"] {
        assert!(
            descriptions.contains(capability),
            "the first model turn did not advertise {capability}; descriptions={descriptions:?}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(
        llm.tool_names()
            .iter()
            .all(|name| !name.starts_with("mcp__use_")),
        "raw Use MCP tools leaked into the primary model: {:?}",
        llm.tool_names()
    );

    let receipt_path = workspace.path("state/components/use.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    assert_eq!(receipt["version"], release.version);
    let executable = PathBuf::from(receipt["executablePath"].as_str().unwrap());
    let install_root = executable.parent().unwrap();
    for path in [
        format!("a3s-use-browser-driver{}", std::env::consts::EXE_SUFFIX),
        "skills/a3s-use-browser/SKILL.md".to_string(),
        "ocr-skills/a3s-use-ocr/SKILL.md".to_string(),
        "ocr-models/PP-OCRv6_small/det/inference.onnx".to_string(),
        "ocr-models/PP-OCRv6_small/det/inference.yml".to_string(),
        "ocr-models/PP-OCRv6_small/rec/inference.onnx".to_string(),
        "ocr-models/PP-OCRv6_small/rec/inference.yml".to_string(),
    ] {
        assert!(
            install_root.join(&path).is_file(),
            "installed release is missing {path}"
        );
    }
}
