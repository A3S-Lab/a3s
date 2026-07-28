use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::{http_json, start_detached_web, temp_directory, wait_until_stopped};

#[test]
fn streams_in_priority_order_and_survives_cancel_and_restart() {
    let root = temp_directory("web-turn-queue");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_dir = root.join("state");
    let llm = MockLlmServer::start();
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S Web queue test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config_with_base_url(&llm.base_url()))
        .expect("write config fixture");

    let (mut first, first_address) = start_detached_web(&root, &config_path, &web_dir, &state_dir);
    let created = http_json(
        &first_address,
        "POST",
        "/api/v1/kernel/sessions",
        Some(r#"{"title":"Queue lifecycle"}"#),
        "200",
    );
    let session_id = created["session"]["sessionId"]
        .as_str()
        .expect("created session id")
        .to_string();
    let controls = http_json(
        &first_address,
        "PATCH",
        &format!("/api/v1/kernel/sessions/{session_id}/controls"),
        Some(r#"{"goal":"Ship the verified queue lifecycle"}"#),
        "200",
    );
    assert_eq!(controls["planningMode"], "enabled");
    assert_eq!(controls["goalTracking"], true);
    assert!(controls["context"].is_object());

    let first_queue = http_json(
        &first_address,
        "POST",
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        Some(r#"{"content":"start the queue lifecycle"}"#),
        "200",
    );
    let first_turn_id = first_queue["acceptedItemId"]
        .as_str()
        .expect("accepted first turn")
        .to_string();
    let first_stream = spawn_stream_request(&first_address, &session_id, &first_turn_id);

    let running = wait_for_json(
        &first_address,
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        |value| value["status"] == "running",
    );
    assert_eq!(running["active"]["turn"]["id"], first_turn_id);
    let follow_up = http_json(
        &first_address,
        "POST",
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        Some(r#"{"content":"urgent user follow-up"}"#),
        "200",
    );
    let follow_up_id = follow_up["acceptedItemId"]
        .as_str()
        .expect("accepted follow-up")
        .to_string();

    llm.release();
    let first_stream_response = first_stream.join().expect("join first stream request");
    assert!(
        first_stream_response.starts_with("HTTP/1.1 200"),
        "{first_stream_response}"
    );
    let ordered = wait_for_json(
        &first_address,
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        |value| {
            value["active"].is_null()
                && value["items"].as_array().is_some_and(|items| {
                    items.iter().any(|item| item["kind"] == "goalContinuation")
                })
        },
    );
    assert_eq!(ordered["items"][0]["id"], follow_up_id);
    assert_eq!(ordered["items"][0]["kind"], "user");
    assert_eq!(ordered["items"][1]["kind"], "goalContinuation");
    assert!(llm.request_count() > 0);

    llm.block();
    let second_stream = spawn_stream_request(&first_address, &session_id, &follow_up_id);
    wait_for_json(
        &first_address,
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        |value| value["active"]["turn"]["id"] == follow_up_id,
    );
    http_json(
        &first_address,
        "POST",
        &format!("/api/v1/kernel/sessions/{session_id}/actions/cancel"),
        Some("{}"),
        "200",
    );
    llm.release();
    let second_stream_response = second_stream.join().expect("join cancelled stream request");
    assert!(
        second_stream_response.starts_with("HTTP/1.1 200"),
        "{second_stream_response}"
    );
    let paused = wait_for_json(
        &first_address,
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        |value| value["active"].is_null() && value["paused"] == true,
    );
    assert!(paused["items"]
        .as_array()
        .is_some_and(|items| { items.iter().all(|item| item["kind"] != "goalContinuation") }));

    let queued_while_paused = http_json(
        &first_address,
        "POST",
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        Some(r#"{"content":"resume this after restart"}"#),
        "200",
    );
    assert_eq!(queued_while_paused["paused"], true);
    assert_eq!(queued_while_paused["status"], "paused");
    first.stop();
    wait_until_stopped(&first_address);

    let (mut second, second_address) =
        start_detached_web(&root, &config_path, &web_dir, &state_dir);
    let restored = http_json(
        &second_address,
        "GET",
        &format!("/api/v1/kernel/sessions/{session_id}/turn-queue"),
        None,
        "200",
    );
    assert_eq!(restored["paused"], true);
    assert_eq!(restored["status"], "paused");
    assert_eq!(restored["items"][0]["content"], "resume this after restart");

    second.stop();
    wait_until_stopped(&second_address);
    fs::remove_dir_all(root).expect("clean temporary queue directory");
}

fn spawn_stream_request(
    address: &str,
    session_id: &str,
    turn_id: &str,
) -> thread::JoinHandle<String> {
    let address = address.to_string();
    let session_id = session_id.to_string();
    let turn_id = turn_id.to_string();
    thread::spawn(move || {
        http_request_with_timeout(
            &address,
            "POST",
            &format!("/api/v1/kernel/sessions/{session_id}/messages/stream"),
            Some(&format!(r#"{{"queueId":"{turn_id}"}}"#)),
            Duration::from_secs(15),
        )
    })
}

fn http_request_with_timeout(
    address: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    timeout: Duration,
) -> String {
    let mut stream = TcpStream::connect(address).expect("connect to detached web process");
    stream
        .set_read_timeout(Some(timeout))
        .expect("set read timeout");
    let body = body.unwrap_or_default();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write HTTP request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read HTTP response");
    response
}

fn wait_for_json(
    address: &str,
    path: &str,
    predicate: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let mut latest = serde_json::Value::Null;
    for _ in 0..150 {
        latest = http_json(address, "GET", path, None, "200");
        if predicate(&latest) {
            return latest;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("timed out waiting for {path}; latest response: {latest:#}");
}

fn test_config_with_base_url(base_url: &str) -> String {
    format!(
        r#"default_model = "openai/test"
providers "openai" {{
  apiKey = "test"
  baseUrl = "{base_url}"
  models "test" {{
    name = "Test"
    toolCall = true
  }}
}}
memory {{ llmExtraction = false }}
"#
    )
}

struct MockLlmServer {
    address: String,
    blocked: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    request_count: Arc<AtomicUsize>,
    thread: Option<thread::JoinHandle<()>>,
}

impl MockLlmServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock LLM server");
        listener
            .set_nonblocking(true)
            .expect("configure mock LLM listener");
        let address = listener.local_addr().expect("mock LLM address").to_string();
        let blocked = Arc::new(AtomicBool::new(true));
        let stop = Arc::new(AtomicBool::new(false));
        let request_count = Arc::new(AtomicUsize::new(0));
        let thread_blocked = Arc::clone(&blocked);
        let thread_stop = Arc::clone(&stop);
        let thread_request_count = Arc::clone(&request_count);
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let blocked = Arc::clone(&thread_blocked);
                        let stop = Arc::clone(&thread_stop);
                        let request_count = Arc::clone(&thread_request_count);
                        thread::spawn(move || {
                            serve_mock_llm_request(stream, &blocked, &stop, &request_count)
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            address,
            blocked,
            stop,
            request_count,
            thread: Some(thread),
        }
    }

    fn base_url(&self) -> String {
        format!("http://{}", self.address)
    }

    fn block(&self) {
        self.blocked.store(true, Ordering::SeqCst);
    }

    fn release(&self) {
        self.blocked.store(false, Ordering::SeqCst);
    }

    fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }
}

impl Drop for MockLlmServer {
    fn drop(&mut self) {
        self.release();
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve_mock_llm_request(
    mut stream: TcpStream,
    blocked: &AtomicBool,
    stop: &AtomicBool,
    request_count: &AtomicUsize,
) {
    stream
        .set_nonblocking(false)
        .expect("configure mock LLM connection");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("configure mock LLM read timeout");
    let request = read_http_request(&mut stream);
    request_count.fetch_add(1, Ordering::Relaxed);
    let body = request
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or_default();
    let streaming = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value["stream"].as_bool())
        .unwrap_or(false);
    let response_body = if streaming {
        concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"mock reply\"},\"finish_reason\":null}],\"usage\":null}\n\n",
            "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":64,\"completion_tokens\":2,\"total_tokens\":66}}\n\n",
            "data: [DONE]\n\n"
        )
    } else {
        r#"{"choices":[{"message":{"role":"assistant","content":"mock reply"},"finish_reason":"stop"}],"usage":{"prompt_tokens":64,"completion_tokens":2,"total_tokens":66}}"#
    };
    let content_type = if streaming {
        "text/event-stream"
    } else {
        "application/json"
    };
    let headers = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response_body.len()
    );
    if stream.write_all(headers.as_bytes()).is_err() || stream.flush().is_err() {
        return;
    }
    while blocked.load(Ordering::SeqCst) && !stop.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(5));
    }
    let _ = stream.write_all(response_body.as_bytes());
    let _ = stream.flush();
}

fn read_http_request(stream: &mut TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 16_384];
    let mut expected_len = None;
    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(size) => bytes.extend_from_slice(&buffer[..size]),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break
            }
            Err(_) => break,
        }
        if expected_len.is_none() {
            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..header_end]);
                let content_length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().ok())
                                .flatten()
                        })
                    })
                    .unwrap_or_default();
                expected_len = Some(header_end + 4 + content_length);
            }
        }
        if expected_len.is_some_and(|length| bytes.len() >= length) {
            break;
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
