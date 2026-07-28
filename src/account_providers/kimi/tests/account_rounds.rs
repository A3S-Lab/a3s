use super::*;

#[tokio::test]
async fn expired_oauth_login_refreshes_and_rotates_credentials_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let credentials_dir = dir.path().join("credentials");
    std::fs::create_dir_all(&credentials_dir).unwrap();
    let credentials_path = credentials_dir.join("kimi-code.json");
    std::fs::write(
        &credentials_path,
        serde_json::to_vec(&KimiCredentials {
            access_token: "expired-access-secret".into(),
            refresh_token: "original-refresh-secret".into(),
            expires_at: 1.0,
            scope: "kimi-code".into(),
            token_type: "Bearer".into(),
            expires_in: None,
        })
        .unwrap(),
    )
    .unwrap();
    let (requests, mut received) = tokio::sync::mpsc::unbounded_channel();
    let app = Router::new()
        .route("/api/oauth/token", post(oauth_refresh_handler))
        .with_state(OAuthServerState { requests });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let auth = KimiAuth::new_oauth(
        dir.path().to_path_buf(),
        credentials_path.clone(),
        "http://kimi.test/coding/v1".to_string(),
        format!("http://{address}"),
    )
    .unwrap();

    let access_token = auth.access_token(false).await.unwrap();
    assert_eq!(access_token, "refreshed-access-secret");
    let (headers, form) = received.recv().await.unwrap();
    assert_eq!(
        form.get("client_id").map(String::as_str),
        Some(OAUTH_CLIENT_ID)
    );
    assert_eq!(
        form.get("refresh_token").map(String::as_str),
        Some("original-refresh-secret")
    );
    assert_eq!(
        headers
            .get("x-msh-platform")
            .and_then(|value| value.to_str().ok()),
        Some("kimi_code_cli")
    );
    let saved: KimiCredentials =
        serde_json::from_slice(&std::fs::read(&credentials_path).unwrap()).unwrap();
    assert_eq!(saved.access_token, "refreshed-access-secret");
    assert_eq!(saved.refresh_token, "rotated-refresh-secret");
    assert!(saved.expires_at > now_unix_seconds());
    server.abort();
}

/// Opt-in real-account coverage. CI remains credential-free; maintainers
/// run this with `A3S_TEST_KIMI_REAL=1` while signed in to Kimi desktop or
/// Kimi Code.
#[tokio::test]
async fn real_kimi_account_completes_an_a3s_tool_round() {
    if std::env::var("A3S_TEST_KIMI_REAL").as_deref() != Ok("1") {
        return;
    }
    let model = std::env::var("A3S_TEST_KIMI_MODEL").unwrap_or_else(|_| {
        fallback_models()
            .into_iter()
            .find(|model| model == "k3-agent")
            .unwrap_or_else(|| DEFAULT_MODEL.to_string())
    });
    let client = KimiClient::from_kimi_login(&model).unwrap();
    let tool = ToolDefinition {
        name: "echo".to_string(),
        description: "Echo the supplied text".to_string(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
            "additionalProperties": false
        }),
    };
    let user = Message::user(
        "Call the echo tool exactly once with text `kimi-tool-ok`. Do not answer in plain text.",
    );
    let mut stream = client
        .complete_streaming(
            std::slice::from_ref(&user),
            Some("Follow the user's tool instruction exactly."),
            std::slice::from_ref(&tool),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let mut first = None;
    while let Some(event) = stream.recv().await {
        if let StreamEvent::Done(response) = event {
            first = Some(response);
            break;
        }
    }
    let first = first.expect("Kimi stream should complete");
    let calls = first.tool_calls();
    assert_eq!(calls.len(), 1, "first response: {}", first.text());
    assert_eq!(calls[0].name, "echo");
    assert_eq!(calls[0].args["text"], "kimi-tool-ok");
    let result = Message::tool_result(&calls[0].id, "kimi-tool-ok", false);
    let second = client
        .complete(
            &[user, first.message, result],
            Some("After the tool result, reply with exactly KIMI_TOOL_OK."),
            &[tool],
        )
        .await
        .unwrap();
    assert_eq!(second.text().trim(), "KIMI_TOOL_OK");
}
