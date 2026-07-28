use std::io::Write;

use super::runtime_store::{
    IdempotencyState, InboundMessage, InboxState, OutboundDraft, OutboundState, RemoteListKind,
    RuntimeStoreError, WeixinRuntimeStore,
};
use a3s_boot::ilink::SecretValue;

fn secret(value: impl Into<String>) -> SecretValue {
    SecretValue::new(value).unwrap()
}

fn inbound(key: &str, text: impl Into<String>) -> InboundMessage {
    InboundMessage {
        key: key.to_string(),
        sender_id: secret("owner-id-runtime-canary"),
        recipient_id: Some(secret("bot-id-runtime-canary")),
        group_id: None,
        context_token: Some(secret("context-token-runtime-canary")),
        text: secret(text),
        run_id: Some("run-runtime-1".to_string()),
        created_at_ms: Some(1_784_710_000_000),
    }
}

fn runtime_directory() -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temporary.path()).unwrap();
    let directory = root.join("weixin-runtime");
    (temporary, directory)
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_runtime_store_recovers_snapshot_journal_and_uncertain_operations() {
    use std::os::unix::fs::PermissionsExt;

    let (_temporary, directory) = runtime_directory();
    let store = WeixinRuntimeStore::open(&directory).await.unwrap();
    assert!(matches!(
        WeixinRuntimeStore::open(&directory).await,
        Err(RuntimeStoreError::LockContended)
    ));

    let staged = store
        .stage_inbound(
            secret("cursor-runtime-canary"),
            vec![inbound("message-1", "进度")],
        )
        .await
        .unwrap();
    assert_eq!(staged.len(), 1);
    store
        .set_inbox_state("message-1", InboxState::Completed)
        .await
        .unwrap();
    let outbound = store
        .queue_outbound(OutboundDraft {
            recipient_id: secret("owner-id-runtime-canary"),
            context_token: Some(secret("context-token-runtime-canary")),
            text: secret("outbound-text-runtime-canary"),
            run_id: Some("run-runtime-1".to_string()),
        })
        .await
        .unwrap();
    assert!(outbound.client_id.starts_with("a3s-"));
    store
        .set_outbound_state(&outbound.client_id, OutboundState::Sending)
        .await
        .unwrap();
    store
        .reserve_idempotency("command-runtime-1")
        .await
        .unwrap();
    store
        .set_selection("rtm_0123456789abcdef01234567")
        .await
        .unwrap();
    store
        .set_list_context(
            RemoteListKind::Targets,
            2,
            vec![
                "rtm_0123456789abcdef01234567".to_string(),
                "rto_0123456789abcdef01234567".to_string(),
            ],
        )
        .await
        .unwrap();
    store.compact().await.unwrap();

    let checkpoint = store.checkpoint().await;
    let rendered = format!("{checkpoint:?} {outbound:?}");
    for canary in [
        "cursor-runtime-canary",
        "owner-id-runtime-canary",
        "context-token-runtime-canary",
        "outbound-text-runtime-canary",
        "message-1",
    ] {
        assert!(!rendered.contains(canary));
    }
    assert_eq!(
        std::fs::symlink_metadata(&directory)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    for file_name in [
        "account.lock",
        "runtime.snapshot.json",
        "runtime.journal.jsonl",
    ] {
        assert_eq!(
            std::fs::symlink_metadata(directory.join(file_name))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    let client_id = outbound.client_id.clone();
    drop(store);
    let reopened = WeixinRuntimeStore::open(&directory).await.unwrap();
    let checkpoint = reopened.checkpoint().await;
    assert_eq!(
        checkpoint.cursor.as_ref().map(SecretValue::expose),
        Some("cursor-runtime-canary")
    );
    assert_eq!(checkpoint.inbox["message-1"].state, InboxState::Completed);
    assert_eq!(
        checkpoint.outbox[&client_id].state,
        OutboundState::OutcomeUnknown
    );
    assert_eq!(
        checkpoint.idempotency["command-runtime-1"],
        IdempotencyState::OutcomeUnknown
    );
    assert_eq!(checkpoint.outbox[&client_id].client_id, client_id);
    assert_eq!(
        checkpoint
            .selection
            .as_ref()
            .map(|selection| selection.target_id.as_str()),
        Some("rtm_0123456789abcdef01234567")
    );
    let list_context = checkpoint.list_context.as_ref().expect("list context");
    assert_eq!(list_context.kind, RemoteListKind::Targets);
    assert_eq!(list_context.page, 2);
    assert_eq!(list_context.target_ids.len(), 2);

    reopened.clear().await.unwrap();
    let cleared = reopened.checkpoint().await;
    assert!(cleared.cursor.is_none());
    assert!(cleared.inbox.is_empty());
    assert!(cleared.outbox.is_empty());
    assert!(cleared.idempotency.is_empty());
    assert!(cleared.selection.is_none());
    assert!(cleared.list_context.is_none());
    drop(reopened);

    let reopened = WeixinRuntimeStore::open(&directory).await.unwrap();
    let cleared = reopened.checkpoint().await;
    assert!(cleared.cursor.is_none());
    assert!(cleared.inbox.is_empty());
    assert!(cleared.outbox.is_empty());
    assert!(cleared.idempotency.is_empty());
    assert!(cleared.selection.is_none());
    assert!(cleared.list_context.is_none());
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_runtime_schema_v1_without_list_context_remains_compatible() {
    let (_temporary, directory) = runtime_directory();
    let store = WeixinRuntimeStore::open(&directory).await.unwrap();
    store
        .set_selection("rtm_0123456789abcdef01234567")
        .await
        .unwrap();
    store.compact().await.unwrap();

    let snapshot = std::fs::read_to_string(directory.join("runtime.snapshot.json")).unwrap();
    assert!(snapshot.contains("\"schemaVersion\":1"));
    assert!(!snapshot.contains("\"listContext\""));
    drop(store);

    let reopened = WeixinRuntimeStore::open(&directory).await.unwrap();
    let checkpoint = reopened.checkpoint().await;
    assert!(checkpoint.selection.is_some());
    assert!(checkpoint.list_context.is_none());
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_runtime_store_deduplicates_inbox_and_rejects_duplicate_idempotency() {
    let (_temporary, directory) = runtime_directory();
    let store = WeixinRuntimeStore::open(&directory).await.unwrap();
    store
        .stage_inbound(secret("cursor-1"), vec![inbound("message-1", "first")])
        .await
        .unwrap();
    store
        .set_inbox_state("message-1", InboxState::Completed)
        .await
        .unwrap();
    let staged = store
        .stage_inbound(
            secret("cursor-2"),
            vec![
                inbound("message-1", "duplicate"),
                inbound("message-2", "second"),
            ],
        )
        .await
        .unwrap();
    assert_eq!(staged.len(), 1);
    assert_eq!(staged[0].message.key, "message-2");

    store.reserve_idempotency("command-1").await.unwrap();
    assert!(matches!(
        store.reserve_idempotency("command-1").await,
        Err(RuntimeStoreError::Model(_))
    ));
    store
        .set_idempotency_state("command-1", IdempotencyState::Succeeded)
        .await
        .unwrap();

    drop(store);
    let reopened = WeixinRuntimeStore::open(&directory).await.unwrap();
    let checkpoint = reopened.checkpoint().await;
    assert_eq!(
        checkpoint.cursor.as_ref().map(SecretValue::expose),
        Some("cursor-2")
    );
    assert_eq!(checkpoint.inbox.len(), 2);
    assert_eq!(checkpoint.inbox["message-1"].message.text.expose(), "first");
    assert_eq!(checkpoint.inbox["message-1"].state, InboxState::Completed);
    assert_eq!(checkpoint.inbox["message-2"].state, InboxState::Staged);
    assert_eq!(
        checkpoint.idempotency["command-1"],
        IdempotencyState::Succeeded
    );
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_runtime_store_preserves_upstream_order_within_one_batch() {
    let (_temporary, directory) = runtime_directory();
    let store = WeixinRuntimeStore::open(&directory).await.unwrap();

    let staged = store
        .stage_inbound(
            secret("cursor-ordered"),
            vec![
                inbound("z-first-by-upstream", "智能体"),
                inbound("a-second-by-upstream", "选择 1"),
            ],
        )
        .await
        .unwrap();

    assert_eq!(staged[0].message.key, "z-first-by-upstream");
    assert_eq!(staged[0].staged_index, 0);
    assert_eq!(staged[1].message.key, "a-second-by-upstream");
    assert_eq!(staged[1].staged_index, 1);
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_runtime_store_quarantines_corrupt_tail_and_requires_recovery() {
    let (_temporary, directory) = runtime_directory();
    let store = WeixinRuntimeStore::open(&directory).await.unwrap();
    store
        .stage_inbound(secret("cursor-1"), vec![inbound("message-1", "first")])
        .await
        .unwrap();
    let journal = store.directory_for_test().join("runtime.journal.jsonl");
    drop(store);

    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap();
    file.write_all(b"{\"truncated\":").unwrap();
    file.sync_all().unwrap();

    assert!(matches!(
        WeixinRuntimeStore::open(&directory).await,
        Err(RuntimeStoreError::CorruptState)
    ));
    assert!(directory.join("recovery.required.json").is_file());
    assert!(!journal.exists());
    assert!(std::fs::read_dir(&directory).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("runtime.journal.jsonl.corrupt.")
    }));
    assert!(matches!(
        WeixinRuntimeStore::open(&directory).await,
        Err(RuntimeStoreError::RecoveryRequired)
    ));
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_runtime_store_does_not_commit_an_oversized_record() {
    let (_temporary, directory) = runtime_directory();
    let store = WeixinRuntimeStore::open(&directory).await.unwrap();
    let messages = (0..5)
        .map(|index| inbound(&format!("message-{index}"), "x".repeat(64 * 1024)))
        .collect::<Vec<_>>();

    assert!(matches!(
        store
            .stage_inbound(secret("cursor-too-large"), messages)
            .await,
        Err(RuntimeStoreError::RecordTooLarge)
    ));
    let checkpoint = store.checkpoint().await;
    assert!(checkpoint.cursor.is_none());
    assert!(checkpoint.inbox.is_empty());
}
