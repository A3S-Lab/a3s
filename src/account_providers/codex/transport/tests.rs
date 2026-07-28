use super::*;
use futures::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone)]
enum Outcome {
    Events(Vec<Value>),
    Error(TransportError),
}

#[derive(Default)]
struct FakeWireClient {
    websocket: Mutex<VecDeque<Outcome>>,
    http: Mutex<VecDeque<Outcome>>,
    websocket_calls: std::sync::atomic::AtomicUsize,
    http_calls: std::sync::atomic::AtomicUsize,
}

impl FakeWireClient {
    fn with_outcomes(websocket: Vec<Outcome>, http: Vec<Outcome>) -> Self {
        Self {
            websocket: Mutex::new(websocket.into()),
            http: Mutex::new(http.into()),
            ..Default::default()
        }
    }

    fn stream(kind: TransportKind, events: Vec<Value>) -> WireStream {
        let (tx, rx) = mpsc::channel(events.len().max(1));
        for event in events {
            tx.try_send(Ok(event)).unwrap();
        }
        drop(tx);
        WireStream { kind, events: rx }
    }

    fn pop(
        queue: &Mutex<VecDeque<Outcome>>,
        kind: TransportKind,
    ) -> Result<WireStream, TransportError> {
        match queue
            .lock()
            .unwrap()
            .pop_front()
            .expect("missing fake outcome")
        {
            Outcome::Events(events) => Ok(Self::stream(kind, events)),
            Outcome::Error(error) => Err(error),
        }
    }
}

#[async_trait]
impl WireClient for FakeWireClient {
    async fn open_websocket(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> Result<WireStream, TransportError> {
        self.websocket_calls.fetch_add(1, Ordering::Relaxed);
        Self::pop(&self.websocket, TransportKind::WebSocket)
    }

    async fn open_http_sse(
        &self,
        _request: &WireRequest,
        _cancel: CancellationToken,
    ) -> Result<WireStream, TransportError> {
        self.http_calls.fetch_add(1, Ordering::Relaxed);
        Self::pop(&self.http, TransportKind::HttpSse)
    }
}

fn request() -> WireRequest {
    WireRequest {
        endpoint: "https://example.test/responses".to_string(),
        headers: Vec::new(),
        body: serde_json::json!({"stream": true}),
    }
}

fn test_config() -> TransportConfig {
    TransportConfig {
        websocket_retries: 2,
        http_retries: 2,
        retry_base: Duration::ZERO,
    }
}

#[tokio::test]
async fn websocket_success_does_not_call_http() {
    let wire = Arc::new(FakeWireClient::with_outcomes(
        vec![Outcome::Events(vec![serde_json::json!({
            "type": "response.completed"
        })])],
        vec![],
    ));
    let controller = TransportController::with_config(wire.clone(), test_config());

    let stream = controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(stream.kind, TransportKind::WebSocket);
    assert_eq!(wire.websocket_calls.load(Ordering::Relaxed), 1);
    assert_eq!(wire.http_calls.load(Ordering::Relaxed), 0);
    assert_eq!(controller.active_kind(), TransportKind::WebSocket);
}

#[tokio::test]
async fn websocket_403_falls_back_to_http_and_stays_there() {
    let wire = Arc::new(FakeWireClient::with_outcomes(
        vec![Outcome::Error(TransportError::http(
            403,
            Some("Unable to load site".to_string()),
            None,
        ))],
        vec![Outcome::Events(vec![]), Outcome::Events(vec![])],
    ));
    let controller = TransportController::with_config(wire.clone(), test_config());

    let first = controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap();
    let second = controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(first.kind, TransportKind::HttpSse);
    assert_eq!(second.kind, TransportKind::HttpSse);
    assert_eq!(wire.websocket_calls.load(Ordering::Relaxed), 1);
    assert_eq!(wire.http_calls.load(Ordering::Relaxed), 2);
    assert_eq!(controller.active_kind(), TransportKind::HttpSse);
}

#[tokio::test]
async fn retryable_websocket_failures_exhaust_budget_before_http() {
    let failure = || TransportError::network("connection reset");
    let wire = Arc::new(FakeWireClient::with_outcomes(
        vec![
            Outcome::Error(failure()),
            Outcome::Error(failure()),
            Outcome::Error(failure()),
        ],
        vec![Outcome::Events(vec![])],
    ));
    let controller = TransportController::with_config(wire.clone(), test_config());

    let stream = controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(stream.kind, TransportKind::HttpSse);
    assert_eq!(wire.websocket_calls.load(Ordering::Relaxed), 3);
    assert_eq!(wire.http_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn http_honors_its_own_retry_budget() {
    let wire = Arc::new(FakeWireClient::with_outcomes(
        vec![Outcome::Error(TransportError::http(426, None, None))],
        vec![
            Outcome::Error(TransportError::http(503, None, None)),
            Outcome::Error(TransportError::http(429, None, Some(Duration::ZERO))),
            Outcome::Events(vec![]),
        ],
    ));
    let controller = TransportController::with_config(wire.clone(), test_config());

    let stream = controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(stream.kind, TransportKind::HttpSse);
    assert_eq!(wire.http_calls.load(Ordering::Relaxed), 3);
}

#[tokio::test]
async fn usage_limit_does_not_retry_or_cross_transports() {
    let usage_limit = || {
        TransportError::http(
            429,
            Some(r#"{"error":{"type":"usage_limit_reached"}}"#.to_string()),
            None,
        )
    };
    let websocket_wire = Arc::new(FakeWireClient::with_outcomes(
        vec![Outcome::Error(usage_limit())],
        vec![],
    ));
    let websocket_controller =
        TransportController::with_config(websocket_wire.clone(), test_config());

    let websocket_error = websocket_controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap_err();

    assert!(websocket_error.is_terminal_usage_limit());
    assert_eq!(websocket_wire.websocket_calls.load(Ordering::Relaxed), 1);
    assert_eq!(websocket_wire.http_calls.load(Ordering::Relaxed), 0);

    let http_wire = Arc::new(FakeWireClient::with_outcomes(
        vec![Outcome::Error(TransportError::http(426, None, None))],
        vec![Outcome::Error(usage_limit())],
    ));
    let http_controller = TransportController::with_config(http_wire.clone(), test_config());

    let http_error = http_controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap_err();

    assert!(http_error.is_terminal_usage_limit());
    assert_eq!(http_wire.websocket_calls.load(Ordering::Relaxed), 1);
    assert_eq!(http_wire.http_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn stream_failure_makes_http_fallback_sticky() {
    let wire = Arc::new(FakeWireClient::with_outcomes(
        vec![],
        vec![Outcome::Events(vec![])],
    ));
    let controller = TransportController::with_config(wire.clone(), test_config());

    controller.note_stream_failure(
        TransportKind::WebSocket,
        &TransportError::stream_closed("socket dropped"),
    );
    let stream = controller
        .open(&request(), CancellationToken::new())
        .await
        .unwrap();

    assert_eq!(stream.kind, TransportKind::HttpSse);
    assert_eq!(wire.websocket_calls.load(Ordering::Relaxed), 0);
    assert_eq!(wire.http_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn cancelled_request_never_opens_a_transport() {
    let wire = Arc::new(FakeWireClient::default());
    let controller = TransportController::with_config(wire.clone(), test_config());
    let cancel = CancellationToken::new();
    cancel.cancel();

    let error = controller.open(&request(), cancel).await.unwrap_err();

    assert_eq!(error.kind, TransportErrorKind::Cancelled);
    assert_eq!(wire.websocket_calls.load(Ordering::Relaxed), 0);
    assert_eq!(wire.http_calls.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn real_wire_replays_websocket_403_over_http_sse() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let mut methods = Vec::new();

        let (mut websocket, _) = listener.accept().await.unwrap();
        let request = read_http_headers(&mut websocket).await;
        methods.push(request.lines().next().unwrap_or_default().to_string());
        websocket
            .write_all(
                b"HTTP/1.1 403 Forbidden\r\nContent-Type: text/html\r\nContent-Length: 19\r\nConnection: close\r\n\r\nUnable to load site",
            )
            .await
            .unwrap();

        let (mut http, _) = listener.accept().await.unwrap();
        let request = read_http_headers(&mut http).await;
        methods.push(request.lines().next().unwrap_or_default().to_string());
        let body = b"data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":1,\"output_tokens\":1,\"total_tokens\":2}}}\n\n";
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        http.write_all(response.as_bytes()).await.unwrap();
        http.write_all(body).await.unwrap();
        methods
    });

    let wire = Arc::new(NetworkWireClient::new().unwrap());
    let controller = TransportController::with_config(wire, test_config());
    let mut request = request();
    request.endpoint = format!("http://{address}/responses");
    request.headers = vec![
        ("authorization".to_string(), "Bearer test".to_string()),
        ("chatgpt-account-id".to_string(), "account".to_string()),
    ];

    let mut stream = controller
        .open(&request, CancellationToken::new())
        .await
        .unwrap();
    let event = stream.events.recv().await.unwrap().unwrap();

    assert_eq!(stream.kind, TransportKind::HttpSse);
    assert_eq!(event["type"], "response.completed");
    assert_eq!(controller.active_kind(), TransportKind::HttpSse);
    assert_eq!(
        server.await.unwrap(),
        vec![
            "GET /responses HTTP/1.1".to_string(),
            "POST /responses HTTP/1.1".to_string(),
        ]
    );
}

#[tokio::test]
async fn real_wire_uses_websocket_response_create_envelope() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        let mut websocket = tokio_tungstenite::accept_async(socket).await.unwrap();
        let payload = websocket.next().await.unwrap().unwrap();
        let payload = payload.into_text().unwrap();
        let request: Value = serde_json::from_str(payload.as_str()).unwrap();
        websocket
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::json!({
                    "type": "response.completed",
                    "response": {"usage": {"input_tokens": 1, "output_tokens": 1, "total_tokens": 2}}
                })
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
        request
    });

    let wire = Arc::new(NetworkWireClient::new().unwrap());
    let controller = TransportController::with_config(wire, test_config());
    let mut request = request();
    request.endpoint = format!("http://{address}/responses");
    request.headers = vec![
        ("authorization".to_string(), "Bearer test".to_string()),
        ("chatgpt-account-id".to_string(), "account".to_string()),
    ];

    let mut stream = controller
        .open(&request, CancellationToken::new())
        .await
        .unwrap();
    let event = stream.events.recv().await.unwrap().unwrap();
    let sent = server.await.unwrap();

    assert_eq!(stream.kind, TransportKind::WebSocket);
    assert_eq!(event["type"], "response.completed");
    assert_eq!(sent["type"], "response.create");
    assert_eq!(sent["stream"], true);
    assert_eq!(controller.active_kind(), TransportKind::WebSocket);
}

async fn read_http_headers(stream: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 1024];
    loop {
        let read = stream.read(&mut chunk).await.unwrap();
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..read]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}
