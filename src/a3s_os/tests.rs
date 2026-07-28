use super::*;
use std::sync::{Arc, Mutex};

// A real ed25519 public key (test vector) + its ssh-keygen SHA256 fingerprint.
const TEST_PUBKEY: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGx2xh3F0Z8y0i8k1mV7pQ3rT9wLcN4bU6oJ2sK1dE0f tester@host";

#[test]
fn ssh_key_body_and_fingerprint() {
    assert_eq!(
        ssh_key_body(TEST_PUBKEY),
        Some("AAAAC3NzaC1lZDI1NTE5AAAAIGx2xh3F0Z8y0i8k1mV7pQ3rT9wLcN4bU6oJ2sK1dE0f")
    );
    // Fingerprint is the OpenSSH SHA256:… form (matches `ssh-keygen -lf`).
    let fp = openssh_sha256_fingerprint(TEST_PUBKEY).unwrap();
    assert!(fp.starts_with("SHA256:") && !fp.contains('='));
    // Deterministic + body-derived (comment is ignored).
    let no_comment =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGx2xh3F0Z8y0i8k1mV7pQ3rT9wLcN4bU6oJ2sK1dE0f";
    assert_eq!(openssh_sha256_fingerprint(no_comment), Some(fp));
    assert_eq!(openssh_sha256_fingerprint("garbage"), None);
}

/// Minimal HTTP/1.1 mock replaying the OS credential endpoints. `existing`
/// is the JSON array returned by GET developer-config; the POST body is
/// captured. Returns `http://127.0.0.1:port`.
async fn spawn_mock_os(existing: &'static str, captured: Arc<Mutex<Option<String>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let cap = captured.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 8192];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                let line = req.lines().next().unwrap_or("");
                let data = if line.starts_with("POST") {
                    *cap.lock().unwrap() =
                        Some(req.split("\r\n\r\n").nth(1).unwrap_or("").to_string());
                    r#"{"id":"cred-1","type":"ssh_key"}"#.to_string()
                } else {
                    existing.to_string()
                };
                let payload = format!(r#"{{"code":200,"status":"OK","data":{data}}}"#);
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(), payload
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.flush().await;
            });
        }
    });
    origin
}

/// One-shot current-user endpoint used by the login/profile tests. The
/// returned task yields the raw request so callers can assert bearer auth
/// without ever including the credential in a login response.
async fn spawn_profile_mock(
    status: &'static str,
    body: &'static str,
) -> (String, tokio::task::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buffer = vec![0_u8; 8192];
        let bytes_read = socket.read(&mut buffer).await.unwrap();
        let request = String::from_utf8_lossy(&buffer[..bytes_read]).into_owned();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket.write_all(response.as_bytes()).await.unwrap();
        socket.flush().await.unwrap();
        request
    });
    (origin, task)
}

#[tokio::test]
async fn register_ssh_key_uploads_when_absent() {
    let cap = Arc::new(Mutex::new(None));
    let origin = spawn_mock_os("[]", cap.clone()).await;
    let out = register_ssh_key(&origin, "tok", TEST_PUBKEY).await;
    assert!(matches!(out, SshKeyOutcome::Registered(_)));
    // Posted exactly {name, publicKey} — the OS DTO's required fields.
    let sent: serde_json::Value =
        serde_json::from_str(cap.lock().unwrap().as_ref().unwrap()).unwrap();
    assert_eq!(sent["publicKey"], TEST_PUBKEY);
    assert!(sent["name"].as_str().unwrap().starts_with("a3s-code · "));
}

#[tokio::test]
async fn register_ssh_key_dedups_by_body_and_fingerprint() {
    // Already present by key body → no POST, AlreadyRegistered.
    let body = ssh_key_body(TEST_PUBKEY).unwrap();
    let by_body: &'static str = Box::leak(
        format!(r#"[{{"type":"ssh_key","publicKey":"ssh-ed25519 {body} old"}}]"#).into_boxed_str(),
    );
    let cap = Arc::new(Mutex::new(None));
    let origin = spawn_mock_os(by_body, cap.clone()).await;
    assert!(matches!(
        register_ssh_key(&origin, "tok", TEST_PUBKEY).await,
        SshKeyOutcome::AlreadyRegistered
    ));
    assert!(cap.lock().unwrap().is_none(), "must not POST a duplicate");

    // Already present by fingerprint (list omits publicKey) → also dedups.
    let fp = openssh_sha256_fingerprint(TEST_PUBKEY).unwrap();
    let by_fp: &'static str =
        Box::leak(format!(r#"[{{"type":"ssh_key","fingerprint":"{fp}"}}]"#).into_boxed_str());
    let cap2 = Arc::new(Mutex::new(None));
    let origin2 = spawn_mock_os(by_fp, cap2.clone()).await;
    assert!(matches!(
        register_ssh_key(&origin2, "tok", TEST_PUBKEY).await,
        SshKeyOutcome::AlreadyRegistered
    ));
}

#[tokio::test]
async fn login_profile_uses_envelope_display_name_and_saves_it() {
    let (origin, request) = spawn_profile_mock(
        "200 OK",
        r#"{"code":200,"message":"Success","data":{"displayName":"Ada Lovelace","email":"ada@example.test","username":"ada"}}"#,
    )
    .await;
    let dir = tempfile_dir("a3s-os-profile-envelope");
    let path = dir.join(STORE_FILE);
    let mut session = StoredOsSession {
        address: origin,
        access_token: "profile-secret".to_string(),
        refresh_token: None,
        token_type: Some("Bearer".to_string()),
        expires_at_ms: None,
        account_label: None,
        login_at_ms: 1,
    };

    finalize_login_session_at(&path, &mut session)
        .await
        .unwrap();

    assert_eq!(session.account_label.as_deref(), Some("Ada Lovelace"));
    assert_eq!(
        current_session_at(&path, &session.address)
            .unwrap()
            .account_label
            .as_deref(),
        Some("Ada Lovelace")
    );
    let request = request.await.unwrap();
    assert!(request.starts_with("GET /api/v1/users/me HTTP/1.1"));
    assert!(request
        .to_ascii_lowercase()
        .contains("authorization: bearer profile-secret"));
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn login_profile_accepts_raw_user_and_falls_back_to_email_or_username() {
    let (email_origin, email_request) = spawn_profile_mock(
        "200 OK",
        r#"{"displayName":"  ","email":"ada@example.test","username":"ada"}"#,
    )
    .await;
    assert_eq!(
        fetch_account_label(&email_origin, "email-token").await,
        Some("ada@example.test".to_string())
    );
    email_request.await.unwrap();

    let (username_origin, username_request) =
        spawn_profile_mock("200 OK", r#"{"username":"ada"}"#).await;
    assert_eq!(
        fetch_account_label(&username_origin, "username-token").await,
        Some("ada".to_string())
    );
    username_request.await.unwrap();
}

#[tokio::test]
async fn login_profile_failure_does_not_fail_or_prevent_session_save() {
    let (origin, request) =
        spawn_profile_mock("503 Service Unavailable", r#"{"message":"try later"}"#).await;
    let dir = tempfile_dir("a3s-os-profile-failure");
    let path = dir.join(STORE_FILE);
    let mut session = StoredOsSession {
        address: origin,
        access_token: "still-valid-token".to_string(),
        refresh_token: None,
        token_type: Some("Bearer".to_string()),
        expires_at_ms: None,
        account_label: None,
        login_at_ms: 1,
    };

    finalize_login_session_at(&path, &mut session)
        .await
        .expect("profile failure must not fail login");

    assert_eq!(session.account_label, None);
    assert!(current_session_at(&path, &session.address).is_some());
    request.await.unwrap();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn parses_gateway_context_from_common_fields() {
    let j = |s: &str| serde_json::from_str::<serde_json::Value>(s).unwrap();
    // OpenAI-compatible variants seen across gateways.
    assert_eq!(
        parse_gateway_context(&j(r#"{"context_length":200000}"#)),
        Some(200000)
    );
    assert_eq!(
        parse_gateway_context(&j(r#"{"max_model_len":32768}"#)),
        Some(32768)
    );
    assert_eq!(
        parse_gateway_context(&j(r#"{"context_window":128000}"#)),
        Some(128000)
    );
    // LiteLLM nests it.
    assert_eq!(
        parse_gateway_context(&j(r#"{"model_info":{"max_input_tokens":1000000}}"#)),
        Some(1_000_000)
    );
    // Absent or zero → None so the caller keeps its default.
    assert_eq!(parse_gateway_context(&j(r#"{"id":"m"}"#)), None);
    assert_eq!(parse_gateway_context(&j(r#"{"context_length":0}"#)), None);
}

#[test]
fn os_origin_strips_any_path_for_the_gateway() {
    // The gateway endpoint is host-absolute (/v1/chat/completions), so the
    // OpenAI base must be the bare origin regardless of the platform path.
    assert_eq!(
        os_origin("https://os.example.com"),
        "https://os.example.com"
    );
    assert_eq!(
        os_origin("https://os.example.com/"),
        "https://os.example.com"
    );
    assert_eq!(
        os_origin("https://os.example.com/api/v1"),
        "https://os.example.com"
    );
    assert_eq!(os_origin("http://10.0.0.1:3888/x"), "http://10.0.0.1:3888");
}

#[test]
fn authorization_url_uses_oauth2_code_flow_with_pkce() {
    let url = build_authorization_url(
        "https://os.example.test/",
        "http://127.0.0.1:1234/callback",
        "state 1",
        "challenge-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOP",
    );

    assert!(url.starts_with("https://os.example.test/oauth/authorize?"));
    assert!(url.contains("response_type=code"));
    assert!(url.contains("client_id=a3s-code"));
    assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A1234%2Fcallback"));
    assert!(url.contains("state=state%201"));
    assert!(url.contains("code_challenge=challenge-abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOP"));
    assert!(url.contains("code_challenge_method=S256"));
    assert!(url.contains("scope=profile%20offline_access"));
}

#[test]
fn token_url_targets_standard_oauth2_token_endpoint() {
    assert_eq!(
        build_token_url("https://os.example.test/"),
        "https://os.example.test/api/v1/oauth/token"
    );
    assert_eq!(
        build_token_url("https://os.example.test/api/v1"),
        "https://os.example.test/api/v1/oauth/token"
    );
}

#[test]
fn builds_session_from_oauth2_token_response() {
    let session = session_from_token_response(
        "https://os.example.test",
        OAuthTokenResponse {
            access_token: "tok 1".to_string(),
            refresh_token: Some("ref".to_string()),
            token_type: Some("Bearer".to_string()),
            expires_in: Some(60),
        },
    );

    assert_eq!(session.address, "https://os.example.test");
    assert_eq!(session.access_token, "tok 1");
    assert_eq!(session.refresh_token.as_deref(), Some("ref"));
    assert!(session.expires_at_ms.is_some());
}

#[test]
fn needs_refresh_only_when_expiring_with_a_refresh_token() {
    let base = StoredOsSession {
        address: "https://os.example.test".to_string(),
        access_token: "a".to_string(),
        refresh_token: Some("r".to_string()),
        token_type: None,
        expires_at_ms: None,
        account_label: None,
        login_at_ms: 0,
    };
    // Unknown expiry → can't tell it's expiring → don't refresh.
    assert!(!needs_refresh(&base));
    // Far in the future → not yet.
    assert!(!needs_refresh(&StoredOsSession {
        expires_at_ms: Some(now_ms() + 3_600_000),
        ..base.clone()
    }));
    // Inside the skew window (or already past) → refresh.
    assert!(needs_refresh(&StoredOsSession {
        expires_at_ms: Some(now_ms() + 10_000),
        ..base.clone()
    }));
    // Expiring but no refresh token → nothing we can do.
    assert!(!needs_refresh(&StoredOsSession {
        refresh_token: None,
        expires_at_ms: Some(now_ms() + 10_000),
        ..base.clone()
    }));
}

#[test]
fn pkce_challenge_matches_rfc7636_example() {
    let challenge = pkce_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk");
    assert_eq!(challenge, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
}

#[test]
fn store_replaces_and_removes_sessions_by_address() {
    let dir = tempfile_dir("a3s-os-auth-test");
    let path = dir.join(STORE_FILE);
    let first = StoredOsSession {
        address: "https://os.example.test".to_string(),
        access_token: "one".to_string(),
        refresh_token: None,
        token_type: Some("Bearer".to_string()),
        expires_at_ms: None,
        account_label: None,
        login_at_ms: 1,
    };
    let second = StoredOsSession {
        access_token: "two".to_string(),
        login_at_ms: 2,
        ..first.clone()
    };

    save_session_at(&path, &first).unwrap();
    save_session_at(&path, &second).unwrap();
    let store = read_store(&path).unwrap();
    assert_eq!(store.sessions.len(), 1);
    assert_eq!(store.sessions[0].access_token, "two");

    assert!(remove_session_at(&path, "https://os.example.test").unwrap());
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn current_session_restores_persisted_login_and_clears_on_logout() {
    let dir = tempfile_dir("a3s-os-auth-restore");
    let path = dir.join(STORE_FILE);
    let addr = "https://os.example.test";

    // Nothing stored yet → signed out.
    assert!(current_session_at(&path, addr).is_none());

    let session = StoredOsSession {
        address: addr.to_string(),
        access_token: "tok".to_string(),
        refresh_token: None,
        token_type: Some("Bearer".to_string()),
        expires_at_ms: None,
        account_label: Some("alice".to_string()),
        login_at_ms: 1,
    };
    save_session_at(&path, &session).unwrap();

    // Persisted login is restored across "runs".
    let restored = current_session_at(&path, addr).expect("login should be remembered");
    assert_eq!(restored.access_token, "tok");
    assert_eq!(restored.display_label(), "alice");

    // A different address does not match.
    assert!(current_session_at(&path, "https://other.example").is_none());

    // /logout clears the remembered login.
    assert!(remove_session_at(&path, addr).unwrap());
    assert!(current_session_at(&path, addr).is_none());

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn capability_skill_materializes_templated_and_is_discoverable() {
    let dir = tempfile_dir("a3s-os-skill");
    let config = OsConfig {
        address: "https://os.example.test/".to_string(),
    };
    ensure_capability_skill_dir_at(&dir, &config).unwrap();

    // The cli skill loader discovers it by name (this is "in effect").
    let skills = crate::tui::skills::load_skills(std::slice::from_ref(&dir));
    assert!(
        skills.iter().any(|(n, _)| n == "a3s-os-capabilities"),
        "a3s-os-capabilities skill not discovered: {skills:?}"
    );

    // Base URL templated in; no placeholder left.
    let md = std::fs::read_to_string(dir.join("a3s-os-capabilities/SKILL.md")).unwrap();
    assert!(md.contains("https://os.example.test"));
    assert!(!md.contains("{{BASE_URL}}"));

    // Definitive "in effect": the *core* skill loader (stricter than the cli's
    // menu parser — validates kind + fail-secure allowed-tools + 10KiB body)
    // accepts it. If this parsed to None the skill would silently not load.
    let skill = a3s_code_core::skills::Skill::parse(&md)
        .expect("core skill loader must accept the materialized SKILL.md");
    assert_eq!(skill.name, "a3s-os-capabilities");
    assert!(
        skill.allowed_tools.is_some(),
        "allowed-tools must parse (fail-secure) so the skill is usable"
    );

    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn login_callback_page_is_english_and_branded() {
    for outcome in [
        LoginOutcome::Success,
        LoginOutcome::NotApproved,
        LoginOutcome::InvalidState,
    ] {
        let (_, body) = login_callback_page(outcome);
        assert!(body.contains("OS"), "missing OS branding: {outcome:?}");
        assert!(
            body.contains("sign-in") || body.contains("sign in"),
            "page should describe the sign-in outcome: {outcome:?}"
        );
        assert!(body.starts_with("<!doctype html>"), "not an HTML page");
        assert!(body.contains("charset=\"utf-8\""), "missing utf-8 charset");
    }
    let (status, body) = login_callback_page(LoginOutcome::Success);
    assert_eq!(status, "200 OK");
    assert!(body.contains("sign-in successful"));
    assert_eq!(
        login_callback_page(LoginOutcome::InvalidState).0,
        "400 Bad Request"
    );
}

// Regression: a browser preconnect (empty socket) and a favicon request
// arriving BEFORE the real ?code=...&state=... redirect must not kill the
// callback — the listener has to survive them. This is the "redirects back
// but can't be reached" bug.
#[tokio::test]
async fn wait_for_callback_survives_preconnect_and_favicon() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let task = tokio::spawn(async move { wait_for_callback(listener, "state-xyz").await });

    // 1) preconnect: open then immediately close, sending no bytes (EOF read).
    TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    // 2) favicon: a real request line but no OAuth params.
    let mut fav = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    fav.write_all(b"GET /favicon.ico HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();
    // 3) the real OAuth redirect.
    let mut cb = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    cb.write_all(b"GET /callback?code=abc&state=state-xyz HTTP/1.1\r\nhost: x\r\n\r\n")
        .await
        .unwrap();

    let params = task.await.unwrap().expect("callback should succeed");
    assert_eq!(params.get("code").map(String::as_str), Some("abc"));
    assert_eq!(params.get("state").map(String::as_str), Some("state-xyz"));
}

fn tempfile_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("{name}-{}-{}", std::process::id(), now_ms()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}
