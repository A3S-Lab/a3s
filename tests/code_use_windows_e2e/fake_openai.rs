use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

use super::plans::PlannedToolCall;

#[derive(Default)]
struct Observations {
    primary_rounds: usize,
    worker_rounds: usize,
    issued_tools: Vec<String>,
    settled_tools: Vec<(String, String)>,
    primary_tool_sets: Vec<Vec<String>>,
    worker_tool_sets: Vec<Vec<String>>,
    errors: Vec<String>,
}

pub(super) struct FakeOpenAi {
    pub(super) base_url: String,
    observations: Arc<Mutex<Observations>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeOpenAi {
    pub(super) fn start(label: &str, plan: Vec<PlannedToolCall>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake OpenAI server");
        listener
            .set_nonblocking(true)
            .expect("configure fake OpenAI listener");
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        let observations = Arc::new(Mutex::new(Observations::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_observations = Arc::clone(&observations);
        let thread_stop = Arc::clone(&stop);
        let label = label.to_string();
        let plan = Arc::new(plan);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let observations = Arc::clone(&thread_observations);
                        let plan = Arc::clone(&plan);
                        let label = label.clone();
                        std::thread::spawn(move || {
                            serve_request(stream, &label, plan.as_slice(), &observations)
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => {
                        thread_observations
                            .lock()
                            .unwrap()
                            .errors
                            .push(format!("fake OpenAI listener failed: {error}"));
                        break;
                    }
                }
            }
        });
        Self {
            base_url,
            observations,
            stop,
            thread: Some(thread),
        }
    }

    pub(super) fn assert_complete(&self, expected: &[PlannedToolCall]) {
        let observations = self.observations.lock().unwrap();
        let expected_names = expected
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>();
        let actual_names = observations
            .issued_tools
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();

        assert!(
            observations.errors.is_empty(),
            "fake model errors: {:?}",
            observations.errors
        );
        assert_eq!(actual_names, expected_names, "tool-call sequence drifted");
        assert_eq!(
            observations.settled_tools.len(),
            expected.len(),
            "not every issued Use tool returned a model-visible result: {:?}",
            observations.settled_tools
        );
        for (tool, result) in &observations.settled_tools {
            assert!(
                !result_looks_failed(result),
                "{tool} returned a failed result: {result}"
            );
        }
        assert!(
            observations.primary_rounds >= 2,
            "primary model did not delegate and summarize"
        );
        assert!(
            observations.worker_rounds >= expected.len() + 1,
            "Use worker did not reach its final model round"
        );
        assert!(
            observations
                .primary_tool_sets
                .iter()
                .flatten()
                .all(|name| !name.starts_with("mcp__use_")),
            "raw Use tools leaked to the primary model: {:?}",
            observations.primary_tool_sets
        );
        assert!(
            observations
                .worker_tool_sets
                .iter()
                .flatten()
                .all(|name| name.starts_with("mcp__use_")),
            "the Use worker received a non-Use tool: {:?}",
            observations.worker_tool_sets
        );
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

fn serve_request(
    mut stream: TcpStream,
    label: &str,
    plan: &[PlannedToolCall],
    observations: &Arc<Mutex<Observations>>,
) {
    if let Err(error) = stream.set_nonblocking(false) {
        observations
            .lock()
            .unwrap()
            .errors
            .push(format!("configure fake OpenAI connection: {error}"));
        return;
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let request = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(error) => {
            observations.lock().unwrap().errors.push(error);
            write_http_response(&mut stream, "500 Internal Server Error", "text/plain", b"");
            return;
        }
    };
    let body = match request_body(&request) {
        Ok(body) => body,
        Err(error) => {
            observations.lock().unwrap().errors.push(error);
            write_http_response(&mut stream, "400 Bad Request", "text/plain", b"");
            return;
        }
    };
    let streaming = body.get("stream").and_then(Value::as_bool) == Some(true);
    if !streaming {
        let response = non_streaming_preanalysis(label);
        write_http_response(
            &mut stream,
            "200 OK",
            "application/json",
            &serde_json::to_vec(&response).unwrap(),
        );
        return;
    }

    let tool_names = request_tool_names(&body);
    let is_worker = request_is_use_worker(&body)
        || (!tool_names.is_empty() && tool_names.iter().all(|name| name.starts_with("mcp__use_")));
    let response = if is_worker {
        worker_response(label, plan, tool_names, &body, observations)
    } else {
        primary_response(label, plan, tool_names, observations)
    };
    write_http_response(
        &mut stream,
        "200 OK",
        "text/event-stream",
        response.as_bytes(),
    );
}

fn primary_response(
    label: &str,
    plan: &[PlannedToolCall],
    tool_names: Vec<String>,
    observations: &Arc<Mutex<Observations>>,
) -> String {
    let mut state = observations.lock().unwrap();
    if tool_names.iter().any(|name| name.starts_with("mcp__use_")) {
        state.errors.push(format!(
            "primary request exposed raw Use tools: {tool_names:?}"
        ));
    }
    state.primary_tool_sets.push(tool_names.clone());
    let round = state.primary_rounds;
    state.primary_rounds += 1;
    drop(state);

    if round == 0 {
        if !tool_names.iter().any(|name| name == "task") {
            observations
                .lock()
                .unwrap()
                .errors
                .push(format!("primary request has no task tool: {tool_names:?}"));
            return streaming_text("Delegation tool missing.");
        }
        let names = plan
            .iter()
            .map(|call| call.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let args = json!({
            "agent":"use",
            "description":format!("Run the {label} Windows capability matrix"),
            "prompt":format!(
                "Invoke these A3S Use tools exactly once and in this exact order, using the supplied arguments: {names}. Preserve sessions between calls, report any typed failure, and finish only after every call settles."
            ),
            "max_steps":(plan.len() + 2).min(50)
        });
        streaming_tool_call("task", &args, "call-primary-use")
    } else {
        streaming_text(&format!("{label} Windows capability matrix completed."))
    }
}

fn worker_response(
    label: &str,
    plan: &[PlannedToolCall],
    tool_names: Vec<String>,
    request: &Value,
    observations: &Arc<Mutex<Observations>>,
) -> String {
    let mut observations = observations.lock().unwrap();
    observations.worker_tool_sets.push(tool_names.clone());
    let round = observations.worker_rounds;
    observations.worker_rounds += 1;
    if let Some(previous) = round.checked_sub(1).and_then(|index| plan.get(index)) {
        let summary = latest_message_summary(request);
        eprintln!(
            "[windows-use-e2e] settled {}: {}",
            previous.name,
            summary.chars().take(500).collect::<String>()
        );
        observations
            .settled_tools
            .push((previous.name.clone(), summary));
    }
    if let Some(call) = plan.get(round) {
        if !tool_names.iter().any(|name| name == &call.name) {
            observations.errors.push(format!(
                "worker round {round} did not expose planned tool '{}'; tools={tool_names:?}",
                call.name
            ));
            return streaming_text("A required Use tool was not exposed.");
        }
        eprintln!("[windows-use-e2e] issuing {}", call.name);
        observations.issued_tools.push(call.name.clone());
        return streaming_tool_call(&call.name, &call.arguments, &format!("call-use-{round}"));
    }
    drop(observations);
    streaming_text(&format!(
        "{label}: all {} planned Use calls settled with observable tool results.",
        plan.len()
    ))
}

fn latest_message_summary(request: &Value) -> String {
    const MAX_SUMMARY_CHARS: usize = 16 * 1024;

    let message = request
        .get("messages")
        .and_then(Value::as_array)
        .and_then(|messages| messages.last());
    let summary = message
        .map(Value::to_string)
        .unwrap_or_else(|| "<missing final message>".to_string());
    if summary.chars().count() <= MAX_SUMMARY_CHARS {
        summary
    } else {
        format!(
            "{}...[truncated]",
            summary.chars().take(MAX_SUMMARY_CHARS).collect::<String>()
        )
    }
}

fn result_looks_failed(result: &str) -> bool {
    let normalized = result.to_ascii_lowercase();
    [
        "tool execution error:",
        "invalid arguments for tool",
        "permission denied",
        "permission was denied",
        "\"iserror\":true",
        "\\\"iserror\\\":true",
        "\"success\":false",
        "\\\"success\\\":false",
        "\"status\":\"error\"",
        "\\\"status\\\":\\\"error\\\"",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn request_tool_names(body: &Value) -> Vec<String> {
    body.get("tools")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|tool| {
            tool.pointer("/function/name")
                .or_else(|| tool.get("name"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect()
}

fn request_is_use_worker(body: &Value) -> bool {
    body.get("messages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|message| message.get("content"))
        .any(|content| {
            content
                .as_str()
                .is_some_and(|text| text.contains("dedicated A3S Use subagent"))
        })
}

fn non_streaming_preanalysis(label: &str) -> Value {
    json!({
        "id":"chatcmpl-use-windows-e2e",
        "object":"chat.completion",
        "created":0,
        "model":"fake",
        "choices":[{
            "index":0,
            "message":{
                "role":"assistant",
                "content":json!({
                    "intent":"GeneralPurpose",
                    "requires_planning":false,
                    "optimized_input":format!("Delegate the {label} matrix to the dedicated use worker.")
                }).to_string()
            },
            "finish_reason":"stop"
        }],
        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
    })
}

fn streaming_tool_call(name: &str, arguments: &Value, id: &str) -> String {
    let arguments = serde_json::to_string(arguments).unwrap();
    let delta = json!({
        "id":"chatcmpl-use-windows-e2e",
        "object":"chat.completion.chunk",
        "created":0,
        "model":"fake",
        "choices":[{
            "index":0,
            "delta":{
                "role":"assistant",
                "tool_calls":[{
                    "index":0,
                    "id":id,
                    "type":"function",
                    "function":{"name":name,"arguments":arguments}
                }]
            },
            "finish_reason":Value::Null
        }],
        "usage":Value::Null
    });
    let done = json!({
        "id":"chatcmpl-use-windows-e2e",
        "object":"chat.completion.chunk",
        "created":0,
        "model":"fake",
        "choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],
        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
    });
    format!("data: {delta}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn streaming_text(text: &str) -> String {
    let delta = json!({
        "id":"chatcmpl-use-windows-e2e",
        "object":"chat.completion.chunk",
        "created":0,
        "model":"fake",
        "choices":[{
            "index":0,
            "delta":{"role":"assistant","content":text},
            "finish_reason":Value::Null
        }],
        "usage":Value::Null
    });
    let done = json!({
        "id":"chatcmpl-use-windows-e2e",
        "object":"chat.completion.chunk",
        "created":0,
        "model":"fake",
        "choices":[{"index":0,"delta":{},"finish_reason":"stop"}],
        "usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}
    });
    format!("data: {delta}\n\ndata: {done}\n\ndata: [DONE]\n\n")
}

fn read_http_request(stream: &mut TcpStream) -> Result<Vec<u8>, String> {
    const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

    let mut request = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = stream
            .read(&mut buffer)
            .map_err(|error| format!("read fake OpenAI request: {error}"))?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..read]);
        if request.len() > MAX_REQUEST_BYTES {
            return Err("fake OpenAI request exceeded 16 MiB".to_string());
        }
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
    Ok(request)
}

fn request_body(request: &[u8]) -> Result<Value, String> {
    let body_start = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "fake OpenAI request has no header terminator".to_string())?
        + 4;
    serde_json::from_slice(&request[body_start..])
        .map_err(|error| format!("decode fake OpenAI request: {error}"))
}

fn write_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
    let _ = stream.flush();
}
