use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use a3s_code_core::store::{FileSessionStore, SessionData, SessionStore};
use anyhow::{bail, Context};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::json;

use crate::cli::args::{CodeSessionArgs, CodeSessionCommand, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::render_value;
use crate::tui::{resolve_tui_session_store_dir, tui_session_state_path};

pub(super) async fn run(args: CodeSessionArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        CodeSessionCommand::List => list(context).await,
        CodeSessionCommand::Show(args) => show(&args.session_id, context).await,
        CodeSessionCommand::Export(args) => {
            export(&args.session_id, args.output_file, context).await
        }
        CodeSessionCommand::Delete(args) => delete(&args.session_id, args.yes, context).await,
    }
}

async fn list(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let (root, store) = open_session_store(context).await?;
    let mut saved = Vec::new();
    for id in store
        .list()
        .await
        .context("could not list saved sessions")?
    {
        let session = match store.load(&id).await {
            Ok(Some(session)) => session,
            Ok(None) => continue,
            Err(error) => {
                tracing::warn!(%error, %id, "skipping unreadable saved session");
                continue;
            }
        };
        let path = stored_session_path(&root, &id);
        let metadata = std::fs::metadata(&path).with_context(|| {
            format!(
                "could not inspect session `{id}` stored at {}",
                path.display()
            )
        })?;
        let modified_at_ms = metadata
            .modified()
            .ok()
            .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_millis());
        let value = json!({
            "id": id,
            "bytes": metadata.len(),
            "modifiedAtMs": modified_at_ms,
        });
        saved.push((session.updated_at, id, value));
    }
    saved.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
    let sessions: Vec<_> = saved.into_iter().map(|(_, _, value)| value).collect();
    render_value(
        output,
        "code.session.list",
        json!({"workspace": context.directory, "sessions": sessions}),
        || {
            if sessions.is_empty() {
                println!("no saved sessions");
            } else {
                println!("SESSION ID                              SIZE");
                for session in &sessions {
                    println!(
                        "{:<39} {}",
                        session["id"].as_str().unwrap_or_default(),
                        session["bytes"].as_u64().unwrap_or_default()
                    );
                }
            }
        },
    )
}

async fn show(id: &str, context: &InvocationContext) -> anyhow::Result<()> {
    validate_id(id)?;
    let output = context.output_mode();
    let (root, store) = open_session_store(context).await?;
    let document = load_document(&store, id).await?;
    let path = stored_session_path(&root, id);
    render_value(
        output,
        "code.session.show",
        json!({"id": id, "path": path, "document": document}),
        || {
            println!(
                "{}",
                serde_json::to_string_pretty(&document).unwrap_or_default()
            )
        },
    )
}

async fn export(
    id: &str,
    output_file: Option<PathBuf>,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    validate_id(id)?;
    let output = context.output_mode();
    let (_, store) = open_session_store(context).await?;
    let document = load_document(&store, id).await?;
    if let Some(destination) = output_file.map(|path| context.resolve_path(path)) {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(&document)?;
        std::fs::write(&destination, &bytes)?;
        return render_value(
            output,
            "code.session.export",
            json!({"id": id, "outputFile": destination, "bytes": bytes.len()}),
            || println!("exported session `{id}` to {}", destination.display()),
        );
    }
    if output == OutputMode::Human {
        println!("{}", serde_json::to_string_pretty(&document)?);
        Ok(())
    } else {
        render_value(
            output,
            "code.session.export",
            json!({"id": id, "document": document}),
            || {},
        )
    }
}

async fn delete(id: &str, yes: bool, context: &InvocationContext) -> anyhow::Result<()> {
    validate_id(id)?;
    let output = context.output_mode();
    let (_, store) = open_session_store(context).await?;
    if !store
        .exists(id)
        .await
        .with_context(|| format!("could not inspect session `{id}`"))?
    {
        bail!("session `{id}` does not exist in this workspace");
    }
    if !yes {
        if output != OutputMode::Human
            || context.interaction.non_interactive
            || !context.terminal.stdin
            || !context.terminal.stderr
        {
            bail!("session deletion requires `--yes` in non-interactive mode");
        }
        eprint!("Delete session `{id}`? [y/N] ");
        std::io::stderr().flush()?;
        let mut answer = String::new();
        std::io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
            bail!("session deletion cancelled");
        }
    }
    store
        .delete(id)
        .await
        .with_context(|| format!("could not delete session `{id}`"))?;
    remove_tui_session_state(&context.directory, id)?;
    render_value(
        output,
        "code.session.delete",
        json!({"id": id, "deleted": true}),
        || println!("deleted session `{id}`"),
    )
}

async fn open_session_store(
    context: &InvocationContext,
) -> anyhow::Result<(PathBuf, FileSessionStore)> {
    let root = resolve_session_root(context);
    let store = FileSessionStore::new(&root)
        .await
        .with_context(|| format!("could not open session store {}", root.display()))?;
    Ok((root, store))
}

/// Keep command-side discovery identical to the TUI launch path: prefer the
/// canonical store, migrate the legacy directory when it is the only one, and
/// keep using the legacy directory if the same-filesystem rename fails.
fn resolve_session_root(context: &InvocationContext) -> PathBuf {
    resolve_tui_session_store_dir(&context.directory)
}

fn stored_session_path(root: &Path, id: &str) -> PathBuf {
    let key = URL_SAFE_NO_PAD.encode(id.as_bytes());
    let current = root
        .join("v1")
        .join("sessions")
        .join(format!("id_{key}.json"));
    if current.is_file() {
        current
    } else {
        root.join(format!("{}.json", legacy_safe_id(id)))
    }
}

fn legacy_safe_id(id: &str) -> String {
    id.replace(['/', '\\'], "_").replace("..", "_")
}

fn remove_tui_session_state(workspace: &Path, id: &str) -> anyhow::Result<()> {
    let path = tui_session_state_path(workspace, id);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("could not delete TUI session state {}", path.display())),
    }
}

fn validate_id(id: &str) -> anyhow::Result<()> {
    if id.is_empty()
        || !id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        bail!("session ID may contain only ASCII letters, digits, `-`, and `_`");
    }
    Ok(())
}

async fn load_document(store: &FileSessionStore, id: &str) -> anyhow::Result<serde_json::Value> {
    let session: SessionData = store
        .load(id)
        .await
        .with_context(|| format!("could not load session `{id}`"))?
        .ok_or_else(|| anyhow::anyhow!("session `{id}` does not exist in this workspace"))?;
    serde_json::to_value(session).with_context(|| format!("could not serialize session `{id}`"))
}
