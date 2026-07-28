mod support;

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use support::{a3s_bin, TempWorkspace};

struct FakeOpenAi {
    base_url: String,
    main_calls: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeOpenAi {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let main_calls = Arc::new(AtomicUsize::new(0));
        let thread_calls = Arc::clone(&main_calls);
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        stream
                            .set_read_timeout(Some(Duration::from_secs(2)))
                            .unwrap();
                        let request = read_request(&mut stream);
                        let body = request_body(&request);
                        if body.get("stream").and_then(serde_json::Value::as_bool) == Some(true) {
                            write_response(&mut stream, "400 Bad Request", b"");
                            continue;
                        }
                        let pre_analysis = body
                            .get("messages")
                            .and_then(serde_json::Value::as_array)
                            .is_some_and(|messages| {
                                messages.iter().any(|message| {
                                    message
                                        .get("content")
                                        .and_then(serde_json::Value::as_str)
                                        .is_some_and(|content| {
                                            content.contains("You are a pre-analysis assistant")
                                        })
                                })
                            });
                        let message = if pre_analysis {
                            pre_analysis_message()
                        } else if thread_calls.fetch_add(1, Ordering::SeqCst) == 0 {
                            serde_json::json!({
                                "role": "assistant",
                                "content": null,
                                "tool_calls": [{
                                    "id": "call-write-answer",
                                    "type": "function",
                                    "function": {
                                        "name": "write",
                                        "arguments": "{\"file_path\":\"answer.txt\",\"content\":\"42\\n\"}"
                                    }
                                }]
                            })
                        } else {
                            serde_json::json!({
                                "role": "assistant",
                                "content": "Completed and verified."
                            })
                        };
                        let response = serde_json::to_vec(&serde_json::json!({
                            "id": "chatcmpl-code-exec-test",
                            "object": "chat.completion",
                            "created": 0,
                            "model": "fake",
                            "choices": [{
                                "index": 0,
                                "message": message,
                                "finish_reason": if pre_analysis {
                                    "stop"
                                } else if thread_calls.load(Ordering::SeqCst) == 1 {
                                    "tool_calls"
                                } else {
                                    "stop"
                                }
                            }],
                            "usage": {
                                "prompt_tokens": 1,
                                "completion_tokens": 1,
                                "total_tokens": 2
                            }
                        }))
                        .unwrap();
                        write_response(&mut stream, "200 OK", &response);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("fake OpenAI listener failed: {error}"),
                }
            }
        });
        Self {
            base_url,
            main_calls,
            stop,
            thread: Some(thread),
        }
    }

    fn main_calls(&self) -> usize {
        self.main_calls.load(Ordering::SeqCst)
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

fn read_request(stream: &mut TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream.read(&mut buffer).unwrap();
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
        .unwrap()
        + 4;
    serde_json::from_slice(&request[body_start..]).unwrap()
}

fn write_response(stream: &mut TcpStream, status: &str, body: &[u8]) {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(body).unwrap();
}

fn pre_analysis_message() -> serde_json::Value {
    serde_json::json!({
        "role": "assistant",
        "content": serde_json::json!({
            "intent": "GeneralPurpose",
            "requires_planning": false,
            "goal": {
                "description": "Write 42 to answer.txt.",
                "success_criteria": ["answer.txt contains 42"]
            },
            "execution_plan": {
                "complexity": "Simple",
                "steps": [{
                    "id": "step-1",
                    "description": "Update answer.txt",
                    "tool": "write",
                    "dependencies": [],
                    "success_criteria": "answer.txt contains 42"
                }],
                "required_tools": ["write"]
            },
            "optimized_input": "Write 42 to answer.txt."
        })
        .to_string()
    })
}

fn fixture(name: &str) -> (TempWorkspace, std::path::PathBuf, FakeOpenAi) {
    let root = TempWorkspace::new(name);
    let project = root.path("project");
    std::fs::create_dir_all(project.join(".a3s")).unwrap();
    std::fs::write(project.join("answer.txt"), "0\n").unwrap();
    let server = FakeOpenAi::start();
    std::fs::write(
        project.join(".a3s/config.acl"),
        format!(
            "default_model = \"openai/fake\"\nproviders \"openai\" {{\n  apiKey = \"test\"\n  baseUrl = \"{}\"\n  models \"fake\" {{ name = \"Fake\" }}\n}}\n",
            server.base_url
        ),
    )
    .unwrap();
    (root, project, server)
}

fn run(project: &std::path::Path, mode: &str, root: &TempWorkspace) -> std::process::Output {
    Command::new(a3s_bin())
        .args(["--output", "json", "--non-interactive", "--directory"])
        .arg(project)
        .args([
            "code",
            "exec",
            "--mode",
            mode,
            "--model",
            "openai/fake",
            "Write 42 to answer.txt, then verify it.",
        ])
        .env("HOME", root.path("home"))
        .env("A3S_DATA_HOME", root.path("data"))
        .env("A3S_STATE_HOME", root.path("state"))
        .env("A3S_CACHE_HOME", root.path("cache"))
        .output()
        .unwrap()
}

#[test]
fn auto_mode_executes_bounded_workspace_edits() {
    let (root, project, server) = fixture("code-exec-auto");
    let output = run(&project, "auto", &root);

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["ok"], true);
    assert_eq!(
        std::fs::read_to_string(project.join("answer.txt")).unwrap(),
        "42\n"
    );
    assert_eq!(server.main_calls(), 2);
}

#[test]
fn unresolved_default_mode_approval_fails_without_model_retries() {
    let (root, project, server) = fixture("code-exec-approval");
    let output = run(&project, "default", &root);

    assert!(!output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["ok"], false);
    assert_eq!(
        result["error"]["code"],
        "approval.required",
        "result={result} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(project.join("answer.txt")).unwrap(),
        "0\n"
    );
    assert_eq!(
        server.main_calls(),
        1,
        "the model must not retry an approval that non-interactive exec cannot resolve"
    );
}
