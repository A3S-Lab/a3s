//! Native HTTP host for the sole `a3s code harness` process.
//!
//! This module owns process transport, health, and bounded shutdown only. The
//! Code Core `AgentProtocolHarness` delegates every command, run, checkpoint,
//! and event-page operation to the existing Agent/Session/Run/Event authority.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::release::{
    AgentReleaseManifest, AgentReleasePersistentDataMode, AgentReleaseSecretTarget,
};
use a3s_code_core::{
    Agent, AgentProtocolCommandV1, AgentProtocolEventPageRequestV1, AgentProtocolHarness,
    AgentProtocolHarnessError, CodeConfig, SessionOptions, AGENT_PROTOCOL_COMMAND_HTTP_PATH_V1,
    AGENT_PROTOCOL_EVENT_PAGE_HTTP_PATH_V1, AGENT_PROTOCOL_MAX_PROMPT_BYTES,
};
use anyhow::{bail, Context};
use axum::extract::rejection::JsonRejection;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::cli::args::CodeHarnessArgs;
use crate::cli::context::InvocationContext;

// JSON may encode one valid 64-KiB prompt with six-byte `\uXXXX` escapes.
// Keep the wire budget finite while accepting every semantically valid v1
// command plus its bounded identity and envelope fields.
const MAXIMUM_REQUEST_BODY_BYTES: usize = AGENT_PROTOCOL_MAX_PROMPT_BYTES * 6 + 64 * 1024;

const LIFECYCLE_STARTING: u8 = 0;
const LIFECYCLE_READY: u8 = 1;
const LIFECYCLE_DRAINING: u8 = 2;
const LIFECYCLE_STOPPED: u8 = 3;

const INVALID_JSON_CODE: &str = "a3s.code.agent_protocol.invalid_json";
const UNSUPPORTED_MEDIA_TYPE_CODE: &str = "a3s.code.agent_protocol.unsupported_media_type";
const REQUEST_TOO_LARGE_CODE: &str = "a3s.code.agent_protocol.request_too_large";
const HARNESS_UNAVAILABLE_CODE: &str = "a3s.code.agent_protocol.harness_unavailable";

#[derive(Clone)]
struct HarnessHttpState {
    harness: Arc<AgentProtocolHarness>,
    lifecycle: Arc<AtomicU8>,
}

impl HarnessHttpState {
    fn new(harness: Arc<AgentProtocolHarness>) -> Self {
        Self {
            harness,
            lifecycle: Arc::new(AtomicU8::new(LIFECYCLE_STARTING)),
        }
    }

    fn mark_ready(&self) {
        self.lifecycle.store(LIFECYCLE_READY, Ordering::Release);
    }

    fn begin_draining(&self) {
        let mut current = self.lifecycle.load(Ordering::Acquire);
        while current < LIFECYCLE_DRAINING {
            match self.lifecycle.compare_exchange_weak(
                current,
                LIFECYCLE_DRAINING,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => return,
                Err(observed) => current = observed,
            }
        }
    }

    fn mark_stopped(&self) {
        self.lifecycle.store(LIFECYCLE_STOPPED, Ordering::Release);
    }

    fn is_ready(&self) -> bool {
        self.lifecycle.load(Ordering::Acquire) == LIFECYCLE_READY && !self.harness.is_closed()
    }

    fn is_live(&self) -> bool {
        self.lifecycle.load(Ordering::Acquire) != LIFECYCLE_STOPPED
    }
}

#[derive(Serialize)]
struct HealthBody {
    status: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

pub(crate) async fn run(args: CodeHarnessArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let manifest_path = context.resolve_path(args.manifest);
    let manifest = tokio::task::spawn_blocking({
        let manifest_path = manifest_path.clone();
        move || AgentReleaseManifest::from_file(manifest_path)
    })
    .await
    .context("Agent release manifest admission task failed")?
    .with_context(|| {
        format!(
            "failed to admit Agent release manifest {}",
            manifest_path.display()
        )
    })?;
    validate_health_routes(
        manifest.health().readiness_path(),
        manifest.health().liveness_path(),
    )?;
    validate_required_secrets(&manifest, context).await?;

    let (_, mut code_config) = crate::commands::config::load_active_config(context)?;
    let session_options = session_options(&manifest, &mut code_config).await?;
    let workspace = utf8_workspace(&context.directory)?;
    let port = manifest.health().port();
    let readiness_path = manifest.health().readiness_path().to_owned();
    let liveness_path = manifest.health().liveness_path().to_owned();
    let shutdown_grace = Duration::from_secs(u64::from(manifest.health().shutdown_grace_seconds()));
    let address = SocketAddr::from((Ipv4Addr::UNSPECIFIED, port));
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to bind A3S Code Harness to {address}"))?;

    let agent = Arc::new(
        Agent::from_config(code_config)
            .await
            .map_err(|error| anyhow::anyhow!("failed to initialize A3S Code: {}", error.code()))?,
    );
    let harness = match AgentProtocolHarness::new(manifest, Arc::clone(&agent), workspace) {
        Ok(harness) => Arc::new(harness.with_session_options(session_options)),
        Err(error) => {
            agent.close().await;
            return Err(anyhow::anyhow!(
                "failed to activate A3S Code Harness: {}",
                error.code()
            ));
        }
    };
    let state = HarnessHttpState::new(Arc::clone(&harness));
    let router = harness_router(state.clone(), &readiness_path, &liveness_path);

    if context.cancellation.is_cancelled() {
        harness.close().await;
        bail!("A3S Code Harness startup was cancelled");
    }

    #[cfg(unix)]
    let termination_task = match install_termination_signal(context.cancellation.clone()) {
        Ok(task) => task,
        Err(error) => {
            harness.close().await;
            return Err(error).context("failed to install the Harness termination signal");
        }
    };

    state.mark_ready();
    tracing::info!(
        address = %address,
        protocol = harness.manifest().protocol(),
        agent_release_identity = harness.agent_release_identity(),
        "A3S Code Harness is ready"
    );

    let result = serve_until_shutdown(
        listener,
        router,
        state,
        context.cancellation.clone(),
        shutdown_grace,
    )
    .await;

    #[cfg(unix)]
    termination_task.abort();

    result
}

fn utf8_workspace(workspace: &Path) -> anyhow::Result<String> {
    workspace
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("A3S Code Harness workspace path must be valid UTF-8"))
}

async fn session_options(
    manifest: &AgentReleaseManifest,
    code_config: &mut CodeConfig,
) -> anyhow::Result<SessionOptions> {
    let configured_sessions = code_config.sessions_dir.take();
    let configured_memory = code_config.memory_dir.take();
    let mut options = SessionOptions::new();

    match manifest.storage().persistent_data() {
        AgentReleasePersistentDataMode::None => {
            options = options.with_memory(Arc::new(a3s_memory::InMemoryStore::new()));
        }
        AgentReleasePersistentDataMode::External => {
            let sessions = configured_sessions.ok_or_else(|| {
                anyhow::anyhow!(
                    "storage.persistent_data is external but Code sessions_dir is not configured"
                )
            })?;
            options = options.with_file_session_store(
                existing_external_directory("sessions_dir", sessions).await?,
            );
            options = match configured_memory {
                Some(memory) => options
                    .with_file_memory(existing_external_directory("memory_dir", memory).await?),
                None => options.with_memory(Arc::new(a3s_memory::InMemoryStore::new())),
            };
        }
        _ => bail!("Agent release declares an unsupported persistent-data mode"),
    }

    // The Runtime owns workspace/cache isolation. Clearing the file paths here
    // prevents Code configuration from silently bypassing the admitted release
    // storage boundary; only the typed options above may supply durable stores.
    code_config.sessions_dir = None;
    code_config.memory_dir = None;
    code_config.storage_url = None;
    Ok(options)
}

async fn existing_external_directory(
    field: &'static str,
    configured: PathBuf,
) -> anyhow::Result<PathBuf> {
    if !configured.is_absolute() {
        bail!("Code {field} must be an absolute externally mounted directory");
    }
    let resolved = configured;
    let metadata = tokio::fs::symlink_metadata(&resolved)
        .await
        .with_context(|| format!("Code {field} external directory is unavailable"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!("Code {field} must be a non-symlink external directory");
    }
    tokio::fs::canonicalize(resolved)
        .await
        .with_context(|| format!("failed to resolve Code {field} external directory"))
}

async fn validate_required_secrets(
    manifest: &AgentReleaseManifest,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    for secret in manifest.required_secrets() {
        let injected = match secret.target() {
            AgentReleaseSecretTarget::Environment => context
                .environment
                .nonempty_var_os(secret.destination())
                .is_some(),
            AgentReleaseSecretTarget::File => required_secret_file(secret.destination()).await,
            _ => bail!(
                "Agent release secret slot '{}' has an unsupported target",
                secret.name()
            ),
        };
        if !injected {
            bail!(
                "required Agent release secret slot '{}' was not injected",
                secret.name()
            );
        }
    }
    Ok(())
}

async fn required_secret_file(destination: &str) -> bool {
    tokio::fs::symlink_metadata(destination)
        .await
        .is_ok_and(|metadata| {
            !metadata.file_type().is_symlink() && metadata.is_file() && metadata.len() > 0
        })
}

fn validate_health_routes(readiness_path: &str, liveness_path: &str) -> anyhow::Result<()> {
    for (name, path) in [("readiness", readiness_path), ("liveness", liveness_path)] {
        if matches!(
            path,
            AGENT_PROTOCOL_COMMAND_HTTP_PATH_V1 | AGENT_PROTOCOL_EVENT_PAGE_HTTP_PATH_V1
        ) {
            bail!("Agent release {name} path conflicts with a Code protocol endpoint");
        }
    }
    Ok(())
}

fn harness_router(state: HarnessHttpState, readiness_path: &str, liveness_path: &str) -> Router {
    Router::new()
        .route(AGENT_PROTOCOL_COMMAND_HTTP_PATH_V1, post(execute_command))
        .route(
            AGENT_PROTOCOL_EVENT_PAGE_HTTP_PATH_V1,
            post(read_event_page),
        )
        .route(readiness_path, get(readiness))
        .route(liveness_path, get(liveness))
        .layer(DefaultBodyLimit::max(MAXIMUM_REQUEST_BODY_BYTES))
        .with_state(state)
}

async fn execute_command(
    State(state): State<HarnessHttpState>,
    payload: Result<Json<AgentProtocolCommandV1>, JsonRejection>,
) -> Response {
    let Json(command) = match payload {
        Ok(command) => command,
        Err(rejection) => return json_rejection_response(rejection),
    };
    if !state.is_ready() {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            HARNESS_UNAVAILABLE_CODE,
            "A3S Code Harness is not accepting work",
        );
    }
    match state.harness.execute(&command).await {
        Ok(receipt) => no_store(Json(receipt).into_response()),
        Err(error) => harness_error_response(error),
    }
}

async fn read_event_page(
    State(state): State<HarnessHttpState>,
    payload: Result<Json<AgentProtocolEventPageRequestV1>, JsonRejection>,
) -> Response {
    let Json(request) = match payload {
        Ok(request) => request,
        Err(rejection) => return json_rejection_response(rejection),
    };
    if !state.is_ready() {
        return error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            HARNESS_UNAVAILABLE_CODE,
            "A3S Code Harness is not accepting work",
        );
    }
    match state.harness.event_page(&request).await {
        Ok(page) => no_store(Json(page).into_response()),
        Err(error) => harness_error_response(error),
    }
}

async fn readiness(State(state): State<HarnessHttpState>) -> Response {
    if state.is_ready() {
        health_response(StatusCode::OK, "ready")
    } else {
        health_response(StatusCode::SERVICE_UNAVAILABLE, "not_ready")
    }
}

async fn liveness(State(state): State<HarnessHttpState>) -> Response {
    if state.is_live() {
        health_response(StatusCode::OK, "live")
    } else {
        health_response(StatusCode::SERVICE_UNAVAILABLE, "stopped")
    }
}

fn health_response(status: StatusCode, state: &'static str) -> Response {
    no_store((status, Json(HealthBody { status: state })).into_response())
}

fn json_rejection_response(rejection: JsonRejection) -> Response {
    match rejection.status() {
        StatusCode::PAYLOAD_TOO_LARGE => error_response(
            StatusCode::PAYLOAD_TOO_LARGE,
            REQUEST_TOO_LARGE_CODE,
            "request body exceeds the A3S Code Agent protocol bound",
        ),
        StatusCode::UNSUPPORTED_MEDIA_TYPE => error_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            UNSUPPORTED_MEDIA_TYPE_CODE,
            "request content type must be application/json",
        ),
        _ => error_response(
            StatusCode::BAD_REQUEST,
            INVALID_JSON_CODE,
            "request body is not a valid A3S Code Agent protocol document",
        ),
    }
}

fn harness_error_response(error: AgentProtocolHarnessError) -> Response {
    let code = error.code();
    let status = status_for_harness_error(code);
    if status.is_server_error() {
        tracing::warn!(
            code,
            http_status = status.as_u16(),
            "Harness request failed"
        );
    } else {
        tracing::debug!(
            code,
            http_status = status.as_u16(),
            "Harness request rejected"
        );
    }
    let message = match status {
        StatusCode::BAD_REQUEST => "request violates the A3S Code Agent protocol",
        StatusCode::NOT_FOUND => "requested Code session or run was not found",
        StatusCode::CONFLICT => "request conflicts with existing Code run state",
        StatusCode::TOO_MANY_REQUESTS => "Code execution budget is exhausted",
        StatusCode::SERVICE_UNAVAILABLE => "A3S Code Harness is unavailable",
        _ => "A3S Code Harness could not complete the request",
    };
    error_response(status, code, message)
}

fn status_for_harness_error(code: &str) -> StatusCode {
    match code {
        "a3s.code.agent_protocol.unsupported_schema"
        | "a3s.code.agent_protocol.invalid_field"
        | "a3s.code.agent_protocol.encoding" => StatusCode::BAD_REQUEST,
        "a3s.code.agent_protocol.session_not_found" | "a3s.code.agent_protocol.run_not_found" => {
            StatusCode::NOT_FOUND
        }
        "a3s.code.agent_protocol.identity_mismatch"
        | "a3s.code.agent_protocol.release_mismatch"
        | "a3s.code.agent_protocol.release_protocol_mismatch"
        | "a3s.code.agent_protocol.session_mismatch"
        | "a3s.code.agent_protocol.run_unavailable"
        | "RUN_IDENTITY_CONFLICT"
        | "SESSION_BUSY" => StatusCode::CONFLICT,
        "BUDGET_EXHAUSTED" => StatusCode::TOO_MANY_REQUESTS,
        "a3s.code.agent_protocol.session_capacity"
        | "a3s.code.agent_protocol.harness_closed"
        | "SESSION_CLOSED" => StatusCode::SERVICE_UNAVAILABLE,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    no_store((status, Json(ErrorBody { code, message })).into_response())
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

async fn serve_until_shutdown(
    listener: TcpListener,
    router: Router,
    state: HarnessHttpState,
    cancellation: CancellationToken,
    shutdown_grace: Duration,
) -> anyhow::Result<()> {
    let shutdown_state = state.clone();
    let shutdown_cancellation = cancellation.clone();
    let server = axum::serve(listener, router).with_graceful_shutdown(async move {
        shutdown_cancellation.cancelled().await;
        shutdown_state.begin_draining();
    });
    tokio::pin!(server);

    tokio::select! {
        result = &mut server => {
            state.begin_draining();
            let close = timeout(shutdown_grace, state.harness.close()).await;
            state.mark_stopped();
            close.map_err(|_| anyhow::anyhow!(
                "A3S Code Harness exceeded its declared shutdown grace"
            ))?;
            result.context("A3S Code Harness HTTP server failed")
        }
        _ = cancellation.cancelled() => {
            state.begin_draining();
            let shutdown = async {
                let (server_result, ()) = tokio::join!(&mut server, state.harness.close());
                server_result
            };
            let result = timeout(shutdown_grace, shutdown).await;
            state.mark_stopped();
            match result {
                Ok(server_result) => {
                    server_result.context("A3S Code Harness HTTP server failed")
                }
                Err(_) => bail!("A3S Code Harness exceeded its declared shutdown grace"),
            }
        }
    }
}

#[cfg(unix)]
fn install_termination_signal(
    cancellation: CancellationToken,
) -> std::io::Result<tokio::task::JoinHandle<()>> {
    let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    Ok(tokio::spawn(async move {
        if signal.recv().await.is_some() {
            cancellation.cancel();
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::release::AgentReleaseManifest;
    use a3s_code_core::{
        AgentProtocolEventPageRequestV1, AgentProtocolRunIdentityV1, AGENT_PROTOCOL_V1,
    };
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const MANIFEST: &str = r#"
agent_release {
  schema = "a3s.code.agent-release.v1"
  protocol = "a3s.code.agent.v1"
  artifact {
    digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111"
    media_type = "application/vnd.oci.image.manifest.v1+json"
  }
  entrypoint {
    command = "/usr/bin/a3s"
    args = ["code", "harness", "--manifest", "/app/.a3s/asset.acl"]
  }
  health {
    transport = "http"
    port = 8080
    readiness_path = "/health/ready"
    liveness_path = "/health/live"
    shutdown_grace_seconds = 30
  }
  storage {
    workspace = "ephemeral"
    cache = "ephemeral"
    persistent_data = "none"
  }
  capability "runtime.service" { level = 1 }
  capability "secrets.external" { level = 1 }
  capability "workspace.local" { level = 1 }
  provenance "source" {
    uri = "https://github.com/A3S-Lab/Code"
    digest = "sha256:2222222222222222222222222222222222222222222222222222222222222222"
  }
}
"#;

    async fn test_service() -> (
        Router,
        HarnessHttpState,
        Arc<AgentProtocolHarness>,
        tempfile::TempDir,
    ) {
        let workspace = tempfile::tempdir().unwrap();
        let manifest = AgentReleaseManifest::parse(MANIFEST).unwrap();
        let agent = Arc::new(Agent::from_config(CodeConfig::default()).await.unwrap());
        let harness = Arc::new(
            AgentProtocolHarness::new(
                manifest,
                agent,
                workspace.path().to_string_lossy().into_owned(),
            )
            .unwrap()
            .with_session_options(
                SessionOptions::new().with_memory(Arc::new(a3s_memory::InMemoryStore::new())),
            ),
        );
        let state = HarnessHttpState::new(Arc::clone(&harness));
        let router = harness_router(state.clone(), "/health/ready", "/health/live");
        (router, state, harness, workspace)
    }

    #[tokio::test]
    async fn health_and_protocol_routes_share_the_code_owned_harness() {
        let (router, state, harness, _workspace) = test_service().await;
        state.mark_ready();

        let ready = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(ready.status(), StatusCode::OK);
        assert_eq!(ready.headers()[header::CACHE_CONTROL], "no-store");
        let ready_body = ready.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(ready_body.as_ref(), br#"{"status":"ready"}"#);

        let request = AgentProtocolEventPageRequestV1 {
            schema: AgentProtocolEventPageRequestV1::SCHEMA.into(),
            identity: AgentProtocolRunIdentityV1 {
                schema: AgentProtocolRunIdentityV1::SCHEMA.into(),
                protocol: AGENT_PROTOCOL_V1.into(),
                agent_release_identity: harness.agent_release_identity().into(),
                session_id: "conversation-one".into(),
                run_id: "execution-one".into(),
            },
            after_event_sequence: None,
            limit: 1,
        };
        let missing = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(AGENT_PROTOCOL_EVENT_PAGE_HTTP_PATH_V1)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let missing_body = missing.into_body().collect().await.unwrap().to_bytes();
        assert!(std::str::from_utf8(&missing_body)
            .unwrap()
            .contains("a3s.code.agent_protocol.session_not_found"));

        state.begin_draining();
        let draining = router
            .oneshot(
                Request::builder()
                    .uri("/health/ready")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(draining.status(), StatusCode::SERVICE_UNAVAILABLE);
        harness.close().await;
    }

    #[tokio::test]
    async fn malformed_json_is_redacted_and_stably_rejected() {
        let (router, state, harness, _workspace) = test_service().await;
        state.mark_ready();
        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(AGENT_PROTOCOL_COMMAND_HTTP_PATH_V1)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{secret-prompt"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = std::str::from_utf8(&body).unwrap();
        assert!(body.contains(INVALID_JSON_CODE));
        assert!(!body.contains("secret-prompt"));
        harness.close().await;
    }

    #[tokio::test]
    async fn external_persistence_requires_an_absolute_existing_directory() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            existing_external_directory("sessions_dir", directory.path().to_path_buf())
                .await
                .is_ok()
        );
        assert!(
            existing_external_directory("sessions_dir", PathBuf::from("relative/sessions"))
                .await
                .is_err()
        );
    }

    #[test]
    fn health_paths_cannot_shadow_protocol_routes() {
        assert!(validate_health_routes("/health/ready", "/health/live").is_ok());
        assert!(
            validate_health_routes(AGENT_PROTOCOL_COMMAND_HTTP_PATH_V1, "/health/live").is_err()
        );
        assert!(
            validate_health_routes("/health/ready", AGENT_PROTOCOL_EVENT_PAGE_HTTP_PATH_V1)
                .is_err()
        );
    }

    #[test]
    fn stable_harness_errors_have_transport_statuses() {
        assert_eq!(
            status_for_harness_error("a3s.code.agent_protocol.session_not_found"),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            status_for_harness_error("RUN_IDENTITY_CONFLICT"),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_for_harness_error("a3s.code.agent_protocol.harness_closed"),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            status_for_harness_error("INTERNAL_ERROR"),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
