#![cfg(windows)]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

struct FakeOpenAi {
    base_url: String,
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
        let requests = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_requests = Arc::clone(&requests);
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => serve_openai_request(stream, &thread_requests),
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("fake OpenAI listener failed: {error}"),
                }
            }
        });
        Self {
            base_url,
            requests,
            stop,
            thread: Some(thread),
        }
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

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

#[test]
fn code_smoke_reaches_the_session_on_windows() {
    let temp = tempfile::tempdir().expect("create Code startup test directory");
    let workspace = temp.path().join("workspace");
    let home = temp.path().join("home");
    let config = temp.path().join("config.acl");
    fs::create_dir_all(&workspace).expect("create Code startup workspace");
    fs::create_dir_all(&home).expect("create Code startup home");
    let llm = FakeOpenAi::start();
    fs::write(
        &config,
        r#"default_model = "openai/test"
providers "openai" {
  apiKey = "test"
  baseUrl = "__BASE_URL__"
  models "test" {
    name = "Test"
    toolCall = true
  }
}
memory { llmExtraction = false }
"#
        .replace("__BASE_URL__", &llm.base_url),
    )
    .expect("write Code startup config");

    let output = Command::new(a3s_binary())
        .arg("-C")
        .arg(&workspace)
        .arg("--config")
        .arg(&config)
        .arg("code")
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("A3S_DATA_HOME", temp.path().join("data"))
        .env("A3S_STATE_HOME", temp.path().join("state"))
        .env("A3S_CACHE_HOME", temp.path().join("cache"))
        .env("A3S_RUNTIME_HOME", temp.path().join("runtime"))
        .env("A3S_CODE_TUI_SMOKE", "1")
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_OFFLINE", "1")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .output()
        .expect("run Code startup smoke test");

    assert!(
        output.status.success(),
        "Code startup failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("[smoke] prompt:") || stderr.contains("[smoke] prompt:"),
        "Code did not reach the smoke session:\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
    assert!(
        llm.requests.load(Ordering::SeqCst) > 0,
        "Code startup did not reach the fake model"
    );
}

fn serve_openai_request(mut stream: TcpStream, requests: &AtomicUsize) {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("configure fake OpenAI connection");
    let request = read_http_request(&mut stream);
    requests.fetch_add(1, Ordering::SeqCst);
    let request_text = String::from_utf8_lossy(&request);
    if request_text.contains("\"stream\":true") || request_text.contains("\"stream\": true") {
        let response = concat!(
            "data: {\"id\":\"chatcmpl-windows-startup\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"test\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"Four.\"},\"finish_reason\":null}],\"usage\":null}\n\n",
            "data: {\"id\":\"chatcmpl-windows-startup\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"test\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":1,\"completion_tokens\":1,\"total_tokens\":2}}\n\n",
            "data: [DONE]\n\n"
        );
        write_http_response(&mut stream, "text/event-stream", response.as_bytes());
        return;
    }

    let response = serde_json::to_vec(&serde_json::json!({
        "id": "chatcmpl-windows-startup",
        "object": "chat.completion",
        "created": 0,
        "model": "test",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": serde_json::json!({
                    "intent": "GeneralPurpose",
                    "requires_planning": false,
                    "optimized_input": "Reply with exactly one short sentence."
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
