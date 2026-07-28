use std::path::{Path, PathBuf};

use a3s_boot::{BootError, BootResponse, Result as BootResult, SseEvent};
use futures::StreamExt;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{json, Value};

pub(super) async fn watch_workspace(root_path: String) -> BootResult<BootResponse> {
    let requested = root_path.trim();
    if requested.is_empty() {
        return Err(BootError::BadRequest("rootPath is required".to_string()));
    }
    let root = tokio::fs::canonicalize(requested)
        .await
        .map_err(|error| BootError::BadRequest(format!("workspace cannot be watched: {error}")))?;
    let metadata = tokio::fs::metadata(&root)
        .await
        .map_err(|error| BootError::BadRequest(format!("workspace cannot be watched: {error}")))?;
    if !metadata.is_dir() {
        return Err(BootError::BadRequest(
            "rootPath must point to a directory".to_string(),
        ));
    }

    let (sender, receiver) = tokio::sync::mpsc::channel::<BootResult<SseEvent>>(256);
    let watcher_root = root.clone();
    let watcher = tokio::task::spawn_blocking(move || create_watcher(watcher_root, sender))
        .await
        .map_err(|error| BootError::Internal(format!("workspace watcher task failed: {error}")))?
        .map_err(|error| {
            BootError::Internal(format!("workspace watcher could not start: {error}"))
        })?;

    let ready = SseEvent::json(&json!({
        "type": "workspace_watch_ready",
        "rootPath": root.display().to_string(),
    }));
    let changes =
        futures::stream::unfold((receiver, watcher), |(mut receiver, watcher)| async move {
            receiver
                .recv()
                .await
                .map(|event| (event, (receiver, watcher)))
        });
    Ok(BootResponse::sse(
        futures::stream::once(async move { ready }).chain(changes),
    ))
}

fn create_watcher(
    root: PathBuf,
    sender: tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
) -> notify::Result<RecommendedWatcher> {
    let event_root = root.clone();
    let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
        let event = match result {
            Ok(event) => workspace_change_event(&event_root, event)
                .and_then(|payload| SseEvent::json(&payload).ok()),
            Err(error) => SseEvent::json(&json!({
                "type": "workspace_watch_error",
                "message": error.to_string(),
            }))
            .ok(),
        };
        if let Some(event) = event {
            let _ = sender.blocking_send(Ok(event));
        }
    })?;
    watcher.watch(&root, RecursiveMode::Recursive)?;
    Ok(watcher)
}

fn workspace_change_event(root: &Path, event: Event) -> Option<Value> {
    let paths = event
        .paths
        .into_iter()
        .filter(|path| path.starts_with(root) && !is_ignored_path(root, path))
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return None;
    }
    Some(json!({
        "type": "workspace_change",
        "kind": event_kind(&event.kind),
        "paths": paths,
    }))
}

fn event_kind(kind: &EventKind) -> &'static str {
    match kind {
        EventKind::Create(_) => "create",
        EventKind::Modify(notify::event::ModifyKind::Name(_)) => "rename",
        EventKind::Modify(_) => "modify",
        EventKind::Remove(_) => "remove",
        _ => "other",
    }
}

fn is_ignored_path(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative.components().any(|component| {
            matches!(
                component.as_os_str().to_str(),
                Some(".git" | ".a3s" | "node_modules" | "target")
            )
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_native_events_without_leaking_ignored_build_paths() {
        let root = Path::new("/repo");
        let visible = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Content,
            )),
            paths: vec![root.join("src/app.ts")],
            attrs: Default::default(),
        };
        let payload = workspace_change_event(root, visible).expect("visible change");
        assert_eq!(payload["kind"], "modify");
        assert_eq!(payload["paths"][0], "/repo/src/app.ts");

        let ignored = Event {
            kind: EventKind::Create(notify::event::CreateKind::File),
            paths: vec![root.join("target/debug/a3s")],
            attrs: Default::default(),
        };
        assert!(workspace_change_event(root, ignored).is_none());
    }
}
