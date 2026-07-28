use std::future::IntoFuture;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use a3s_boot::{BootApplication, BootError};
use a3s_code_core::{Agent, CodeConfig};
use anyhow::Context;
use axum::body::{Body, HttpBody};
use axum::extract::{Request, State};
use axum::http::{header::CONTENT_LENGTH, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Json;
use futures::StreamExt;
use http_body_util::{BodyStream, StreamBody};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::config;

use self::api_gateway::ApiGateway;
use self::options::ServeOptions;
use super::code_web::{
    preview_content_router, CodeWebModule, CodeWebSessionRepository, CodeWebState, KernelService,
};
use super::web::{api_only_fallback, prepare_default_web_dir, serve_static};

mod api_gateway;
mod background;
mod foreground_interrupt;
mod options;

pub(crate) use background::{
    open as open_instance, read_log_tail, status as instance_status, stop as stop_instance,
    WebEndpoint, WebInstanceRecord, WebInstanceStatus,
};

const API_PREFIX: &str = "/api";
// Work accepts Office source files up to 50 MiB. Keep enough headroom for
// request metadata while retaining one explicit limit for every API route.
const BODY_LIMIT_BYTES: usize = 52 * 1024 * 1024;
const SERVER_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
const APPLICATION_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) enum ServeOutcome {
    Help,
    ForegroundStopped,
    Detached {
        instance: WebInstanceRecord,
        reused: bool,
    },
    Existing(WebEndpoint),
}

pub(crate) fn usage_text() -> String {
    [
        "a3s web".to_string(),
        String::new(),
        "usage:".to_string(),
        "  a3s web [-d] [--replace] [--host 127.0.0.1] [--port 29653]".to_string(),
        "          [--workspace <path>] [--config <path>] [--web-dir <path>] [--api-only]"
            .to_string(),
        String::new(),
        "Starts the local Boot-backed A3S Code API and serves the Shu Xiao'an web UI.".to_string(),
        "Use -d to start in the background; the command prints its PID, URL, and log path."
            .to_string(),
    ]
    .join("\n")
        + "\n"
}

pub(crate) async fn run(
    args: &[String],
    cancellation: &CancellationToken,
    offline: bool,
    allow_asset_download: bool,
) -> anyhow::Result<ServeOutcome> {
    let mut options = ServeOptions::parse(args)?;
    options.offline = offline;
    options.allow_asset_download = allow_asset_download;
    if options.help {
        print!("{}", usage_text());
        return Ok(ServeOutcome::Help);
    }
    if options.background {
        return Ok(match background::start(args, &options).await? {
            background::BackgroundStart::Started(instance) => ServeOutcome::Detached {
                instance,
                reused: false,
            },
            background::BackgroundStart::Reused(instance) => ServeOutcome::Detached {
                instance,
                reused: true,
            },
            background::BackgroundStart::Existing(instance) => ServeOutcome::Existing(instance),
        });
    }

    match run_foreground(options, cancellation).await? {
        Some(instance) => Ok(ServeOutcome::Existing(instance)),
        None => Ok(ServeOutcome::ForegroundStopped),
    }
}

async fn run_foreground(
    options: ServeOptions,
    cancellation: &CancellationToken,
) -> anyhow::Result<Option<WebEndpoint>> {
    let _interrupt_mode = foreground_interrupt::InterruptSignalGuard::enable(cancellation.clone())
        .context("failed to enable Ctrl+C for foreground A3S Web")?;
    if options.replace {
        background::replace_managed(&options.workspace).await?;
    }
    let listener = match tokio::net::TcpListener::bind(options.addr).await {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {
            if let Some(existing) = background::discover_requested_instance(&options).await? {
                if options.replace {
                    let executable = std::env::current_exe().map_err(|error| {
                        anyhow::anyhow!(
                            "could not locate the current a3s executable before replacement: {error}"
                        )
                    })?;
                    background::replace_observed_instance(&existing, &options, &executable).await?;
                    tokio::net::TcpListener::bind(options.addr)
                        .await
                        .with_context(|| {
                            format!(
                                "failed to bind {} after replacing the previous A3S Web process",
                                options.addr
                            )
                        })?
                } else {
                    return Ok(Some(existing));
                }
            } else {
                anyhow::bail!(
                    "{} is already in use by another application; no process was stopped. Stop that \
                     application or select an available port with --port 0",
                    options.addr
                );
            }
        }
        Err(error) => {
            return Err(anyhow::anyhow!(
                "failed to bind {} before A3S Web startup: {error}",
                options.addr
            ))
        }
    };
    let actual_addr = listener.local_addr()?;
    let web_root = resolve_web_root(&options).await?;
    let config_path = ensure_config_path(&options)?;
    let code_config = CodeConfig::from_file(Path::new(&config_path))
        .map_err(|e| anyhow::anyhow!("failed to parse {config_path}: {e}"))?;
    let agent = Arc::new(
        Agent::new(config_path.clone())
            .await
            .map_err(|e| anyhow::anyhow!("failed to load A3S Code from {config_path}: {e}"))?,
    );
    let session_repository = Arc::new(
        CodeWebSessionRepository::open_default()
            .await
            .context("failed to open A3S Code Web session store")?,
    );
    let component_paths = match a3s::components::ComponentPaths::from_env_at(&options.workspace) {
        Ok(paths) => Some(paths),
        Err(error) => {
            eprintln!("warning: A3S managed components are unavailable for Code Web: {error}");
            None
        }
    };
    let state = Arc::new(CodeWebState::new(
        agent,
        PathBuf::from(&config_path),
        options.workspace.clone(),
        code_config,
        session_repository,
    ));
    if let Some(paths) = component_paths.as_ref() {
        match a3s::components::find_ready_executable_with("use", paths) {
            Ok(Some(executable)) => {
                let (registry, warning) = crate::use_registry::start_detached(
                    executable,
                    options.workspace.clone(),
                    CancellationToken::new(),
                )
                .await;
                if let Some(warning) = warning {
                    eprintln!("warning: A3S Use capabilities will continue loading: {warning}");
                }
                state.install_use_registry(registry);
            }
            Ok(None) => {}
            Err(error) => {
                eprintln!("warning: A3S Use hot-plug is unavailable for Code Web: {error}");
            }
        }
    }
    let restore_report = KernelService::new(Arc::clone(&state))
        .restore_persisted_sessions()
        .await
        .map_err(boot_to_anyhow)?;

    let app = BootApplication::builder()
        .global_prefix(API_PREFIX)
        .import(CodeWebModule::new(Arc::clone(&state)))
        .build()
        .map_err(boot_to_anyhow)?;

    app.bootstrap().await.map_err(boot_to_anyhow)?;
    let api_router = ApiGateway::new(app.clone(), BODY_LIMIT_BYTES)
        .router()
        .merge(preview_content_router(state.preview_registry()));

    let router = if options.api_only {
        api_router.fallback(api_only_fallback)
    } else {
        let web_root = web_root.ok_or_else(|| {
            anyhow::anyhow!("internal error: Web root validation did not produce a directory")
        })?;
        api_router.fallback({
            let web_root = Arc::clone(&web_root);
            move |uri| serve_static(uri, Arc::clone(&web_root))
        })
    };

    let shutdown = Arc::new(Notify::new());
    let instance_nonce = std::env::var(background::INSTANCE_NONCE_ENV).ok();
    let router = if let Some(nonce) = instance_nonce.as_deref() {
        let status_path = format!("/.a3s/web/{nonce}/status");
        let stop_path = format!("/.a3s/web/{nonce}/stop");
        let status_nonce = nonce.to_string();
        let stop_signal = Arc::clone(&shutdown);
        router
            .route(
                &status_path,
                get(move || {
                    let nonce = status_nonce.clone();
                    async move {
                        Json(serde_json::json!({
                            "schemaVersion": 1,
                            "pid": std::process::id(),
                            "nonce": nonce,
                        }))
                    }
                }),
            )
            .route(
                &stop_path,
                post(move || {
                    let signal = Arc::clone(&stop_signal);
                    async move {
                        signal.notify_one();
                        Json(serde_json::json!({"ok": true}))
                    }
                }),
            )
    } else {
        router
    };

    // Axum's graceful shutdown waits for every response body to finish. That
    // never happens naturally for SSE and other long-lived responses, so all
    // requests and response bodies share one server-lifetime cancellation
    // boundary. This keeps shutdown behavior independent of individual routes.
    let connection_shutdown = CancellationToken::new();
    let router = router.layer(middleware::from_fn_with_state(
        connection_shutdown.clone(),
        cancel_request_on_shutdown,
    ));

    background::notify_ready(actual_addr)?;
    println!("A3S Code API:  http://{actual_addr}/api/health");
    if options.api_only {
        println!("A3S Web:       disabled (--api-only)");
    } else {
        println!("A3S Web:       http://{actual_addr}/");
    }
    println!("Workspace:     {}", options.workspace.display());
    println!("Config:        {config_path}");
    println!("Sessions restored: {}", restore_report.restored);
    if restore_report.unavailable > 0 || restore_report.failed > 0 {
        println!(
            "Sessions unavailable: {}",
            restore_report.unavailable + restore_report.failed
        );
    }
    println!("Press Ctrl+C to stop.");

    let shutdown_signal = Arc::clone(&shutdown);
    let cancellation = cancellation.clone();
    let drain_started = CancellationToken::new();
    let drain_started_signal = drain_started.clone();
    let connection_shutdown_signal = connection_shutdown.clone();
    let mut server = Box::pin(
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                tokio::select! {
                    _ = cancellation.cancelled() => {}
                    _ = shutdown_signal.notified() => {}
                }
                connection_shutdown_signal.cancel();
                drain_started_signal.cancel();
            })
            .into_future(),
    );
    let serve_result = tokio::select! {
        result = &mut server => result.map_err(|error| anyhow::anyhow!("server failed: {error}")),
        _ = drain_started.cancelled() => {
            match tokio::time::timeout(SERVER_DRAIN_TIMEOUT, &mut server).await {
                Ok(result) => result.map_err(|error| anyhow::anyhow!("server failed: {error}")),
                Err(_) => {
                    eprintln!(
                        "warning: A3S Web forced the remaining connections closed after {} seconds",
                        SERVER_DRAIN_TIMEOUT.as_secs()
                    );
                    Ok(())
                }
            }
        }
    };
    drop(server);
    connection_shutdown.cancel();
    let shutdown_result =
        match tokio::time::timeout(APPLICATION_SHUTDOWN_TIMEOUT, app.shutdown()).await {
            Ok(result) => result.map_err(boot_to_anyhow),
            Err(_) => {
                eprintln!(
                    "warning: A3S Web stopped waiting for application cleanup after {} seconds",
                    APPLICATION_SHUTDOWN_TIMEOUT.as_secs()
                );
                Ok(())
            }
        };
    if let (Some(path), Some(nonce)) = (
        std::env::var_os(background::INSTANCE_FILE_ENV),
        instance_nonce.as_deref(),
    ) {
        background::remove_instance_if_owned(Path::new(&path), nonce);
    }

    match (serve_result, shutdown_result) {
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(None),
    }
}

async fn cancel_request_on_shutdown(
    State(shutdown): State<CancellationToken>,
    request: Request,
    next: Next,
) -> Response {
    let response = tokio::select! {
        biased;
        _ = shutdown.cancelled() => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        response = next.run(request) => response,
    };
    let (mut parts, body) = response.into_parts();
    if let Some(length) = body.size_hint().exact().filter(|length| *length > 0) {
        parts
            .headers
            .entry(CONTENT_LENGTH)
            .or_insert_with(|| HeaderValue::from(length));
    }
    let frames = BodyStream::new(body).take_until(shutdown.cancelled_owned());
    Response::from_parts(parts, Body::new(StreamBody::new(frames)))
}

async fn resolve_web_root(options: &ServeOptions) -> anyhow::Result<Option<Arc<PathBuf>>> {
    if options.api_only {
        return Ok(None);
    }
    let web_dir = match options.web_dir.clone() {
        Some(web_dir) => web_dir,
        None => {
            prepare_default_web_dir(
                &options.workspace,
                options.offline,
                options.allow_asset_download,
            )
            .await?
        }
    };
    if !web_dir.join("index.html").is_file() {
        anyhow::bail!(
            "web assets at {} do not contain index.html; pass a built workspace with --web-dir or \
             use --api-only",
            web_dir.display()
        );
    }
    Ok(Some(Arc::new(web_dir)))
}

fn ensure_config_path(options: &ServeOptions) -> anyhow::Result<String> {
    if let Some(path) = options.config_path.as_ref() {
        return Ok(path.to_string_lossy().into_owned());
    }
    for directory in options.workspace.ancestors() {
        let candidate = directory.join(".a3s/config.acl");
        if candidate.is_file() {
            return Ok(candidate.to_string_lossy().into_owned());
        }
    }

    let path = config::default_config_path()
        .ok_or_else(|| anyhow::anyhow!("user home directory is unavailable"))?;
    config::write_template_config(&path)?;
    anyhow::bail!(
        "created starter config at {}; fill in a provider/model, then rerun `a3s web`",
        path.display()
    );
}

fn boot_to_anyhow(error: BootError) -> anyhow::Error {
    anyhow::anyhow!("{error}")
}
