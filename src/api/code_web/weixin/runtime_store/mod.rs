mod model;
mod storage;

use std::collections::BTreeMap;
use std::fmt;
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use tokio::sync::Mutex;

use super::credential_store::CredentialStoreError;
use a3s_boot::ilink::SecretValue;
pub(super) use model::{
    IdempotencyState, InboundMessage, InboxRecord, InboxState, OutboundDraft, OutboundRecord,
    OutboundState, RemoteListContext, RemoteListKind, RemoteSelection,
};
use model::{JournalRecord, RuntimeEvent, RuntimeModelError, RuntimeState};

#[derive(Clone)]
pub(super) struct WeixinRuntimeStore {
    inner: Arc<RuntimeStoreInner>,
}

struct RuntimeStoreInner {
    directory: PathBuf,
    state: Mutex<RuntimeState>,
    poisoned: AtomicBool,
    _account_lock: std::fs::File,
}

impl WeixinRuntimeStore {
    pub(super) async fn open(directory: impl Into<PathBuf>) -> Result<Self, RuntimeStoreError> {
        let directory = directory.into();
        let open_directory = directory.clone();
        let opened = tokio::task::spawn_blocking(move || storage::open_runtime(&open_directory))
            .await
            .map_err(|_| RuntimeStoreError::TaskJoin)??;
        Ok(Self {
            inner: Arc::new(RuntimeStoreInner {
                directory,
                state: Mutex::new(opened.state),
                poisoned: AtomicBool::new(false),
                _account_lock: opened.account_lock,
            }),
        })
    }

    pub(super) async fn checkpoint(&self) -> RuntimeCheckpoint {
        let state = self.inner.state.lock().await;
        RuntimeCheckpoint::from_state(&state)
    }

    pub(super) async fn stage_inbound(
        &self,
        cursor: SecretValue,
        messages: Vec<InboundMessage>,
    ) -> Result<Vec<InboxRecord>, RuntimeStoreError> {
        self.append(RuntimeEvent::StageInboundBatch { cursor, messages })
            .await?;
        let mut staged = self
            .checkpoint()
            .await
            .inbox
            .into_values()
            .filter(|record| record.state == InboxState::Staged)
            .collect::<Vec<_>>();
        staged.sort_by(|left, right| {
            left.staged_sequence
                .cmp(&right.staged_sequence)
                .then_with(|| left.staged_index.cmp(&right.staged_index))
                .then_with(|| left.message.key.cmp(&right.message.key))
        });
        Ok(staged)
    }

    pub(super) async fn set_inbox_state(
        &self,
        key: impl Into<String>,
        state: InboxState,
    ) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::SetInboxState {
            key: key.into(),
            state,
        })
        .await
    }

    #[cfg(test)]
    pub(super) async fn queue_outbound(
        &self,
        draft: OutboundDraft,
    ) -> Result<OutboundRecord, RuntimeStoreError> {
        let message = outbound_record(draft, None);
        self.append(RuntimeEvent::QueueOutbound {
            message: message.clone(),
        })
        .await?;
        Ok(message)
    }

    pub(super) async fn queue_outbound_for_inbox(
        &self,
        key: impl Into<String>,
        draft: OutboundDraft,
    ) -> Result<OutboundRecord, RuntimeStoreError> {
        let key = key.into();
        let source_order = self
            .checkpoint()
            .await
            .inbox
            .get(&key)
            .map(|record| (record.staged_sequence, record.staged_index));
        let message = outbound_record(draft, source_order);
        self.append(RuntimeEvent::QueueOutboundForInbox {
            key,
            message: message.clone(),
        })
        .await?;
        Ok(message)
    }

    pub(super) async fn set_outbound_state(
        &self,
        client_id: impl Into<String>,
        state: OutboundState,
    ) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::SetOutboundState {
            client_id: client_id.into(),
            state,
        })
        .await
    }

    #[cfg(test)]
    pub(super) async fn reserve_idempotency(
        &self,
        key: impl Into<String>,
    ) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::ReserveIdempotency { key: key.into() })
            .await
    }

    #[cfg(test)]
    pub(super) async fn set_idempotency_state(
        &self,
        key: impl Into<String>,
        state: IdempotencyState,
    ) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::SetIdempotencyState {
            key: key.into(),
            state,
        })
        .await
    }

    pub(super) async fn set_selection(
        &self,
        target_id: impl Into<String>,
    ) -> Result<RemoteSelection, RuntimeStoreError> {
        let selection = RemoteSelection {
            target_id: target_id.into(),
            selected_at_ms: unix_time_ms(),
        };
        self.append(RuntimeEvent::SetSelection {
            selection: Some(selection.clone()),
        })
        .await?;
        Ok(selection)
    }

    pub(super) async fn clear_selection(&self) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::SetSelection { selection: None })
            .await
    }

    pub(super) async fn set_list_context(
        &self,
        kind: RemoteListKind,
        page: u16,
        target_ids: Vec<String>,
    ) -> Result<RemoteListContext, RuntimeStoreError> {
        let context = RemoteListContext {
            kind,
            page,
            target_ids,
            listed_at_ms: unix_time_ms(),
        };
        self.append(RuntimeEvent::SetListContext {
            context: Some(context.clone()),
        })
        .await?;
        Ok(context)
    }

    pub(super) async fn clear_list_context(&self) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::SetListContext { context: None })
            .await
    }

    pub(super) async fn compact(&self) -> Result<(), RuntimeStoreError> {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move { inner.compact().await })
            .await
            .map_err(|_| RuntimeStoreError::TaskJoin)?
    }

    pub(super) async fn clear(&self) -> Result<(), RuntimeStoreError> {
        self.append(RuntimeEvent::Reset).await?;
        self.compact().await
    }

    async fn append(&self, event: RuntimeEvent) -> Result<(), RuntimeStoreError> {
        let inner = Arc::clone(&self.inner);
        tokio::spawn(async move { inner.append(event).await })
            .await
            .map_err(|_| RuntimeStoreError::TaskJoin)?
    }

    #[cfg(test)]
    pub(super) fn directory_for_test(&self) -> &Path {
        &self.inner.directory
    }
}

impl RuntimeStoreInner {
    async fn append(&self, event: RuntimeEvent) -> Result<(), RuntimeStoreError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(RuntimeStoreError::RecoveryRequired);
        }
        let mut current = self.state.lock().await;
        if self.poisoned.load(Ordering::Acquire) {
            return Err(RuntimeStoreError::RecoveryRequired);
        }
        let record = JournalRecord::new(current.next_sequence, event);
        let mut next = current.clone();
        next.apply(&record)?;
        storage::validate_snapshot_size(&next)?;
        let directory = self.directory.clone();
        let persisted_record = record.clone();
        let baseline = current.clone();
        let persisted = tokio::task::spawn_blocking(move || {
            match storage::append_record(&directory, &persisted_record) {
                Err(RuntimeStoreError::JournalFull) => {
                    storage::compact_runtime(&directory, &baseline)?;
                    storage::append_record(&directory, &persisted_record)
                }
                result => result,
            }
        })
        .await;
        let persisted = match persisted {
            Ok(result) => result,
            Err(_) => {
                self.poisoned.store(true, Ordering::Release);
                return Err(RuntimeStoreError::TaskJoin);
            }
        };
        if let Err(error) = persisted {
            if error.requires_reopen() {
                self.poisoned.store(true, Ordering::Release);
            }
            return Err(error);
        }
        *current = next;
        Ok(())
    }

    async fn compact(&self) -> Result<(), RuntimeStoreError> {
        if self.poisoned.load(Ordering::Acquire) {
            return Err(RuntimeStoreError::RecoveryRequired);
        }
        let current = self.state.lock().await;
        if self.poisoned.load(Ordering::Acquire) {
            return Err(RuntimeStoreError::RecoveryRequired);
        }
        let snapshot = current.clone();
        let directory = self.directory.clone();
        let compacted =
            tokio::task::spawn_blocking(move || storage::compact_runtime(&directory, &snapshot))
                .await;
        match compacted {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                if error.requires_reopen() {
                    self.poisoned.store(true, Ordering::Release);
                }
                Err(error)
            }
            Err(_) => {
                self.poisoned.store(true, Ordering::Release);
                Err(RuntimeStoreError::TaskJoin)
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct RuntimeCheckpoint {
    pub(super) cursor: Option<SecretValue>,
    pub(super) inbox: BTreeMap<String, InboxRecord>,
    pub(super) outbox: BTreeMap<String, OutboundRecord>,
    pub(super) idempotency: BTreeMap<String, IdempotencyState>,
    pub(super) selection: Option<RemoteSelection>,
    pub(super) list_context: Option<RemoteListContext>,
}

impl RuntimeCheckpoint {
    fn from_state(state: &RuntimeState) -> Self {
        Self {
            cursor: state.cursor.clone(),
            inbox: state.inbox.clone(),
            outbox: state.outbox.clone(),
            idempotency: state.idempotency.clone(),
            selection: state.selection.clone(),
            list_context: state.list_context.clone(),
        }
    }
}

impl fmt::Debug for RuntimeCheckpoint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeCheckpoint")
            .field("has_cursor", &self.cursor.is_some())
            .field("inbox_count", &self.inbox.len())
            .field("outbox_count", &self.outbox.len())
            .field("idempotency_count", &self.idempotency.len())
            .field("has_selection", &self.selection.is_some())
            .field("has_list_context", &self.list_context.is_some())
            .finish()
    }
}

fn random_client_id() -> String {
    format!("a3s-{:032x}", rand::random::<u128>())
}

fn outbound_record(draft: OutboundDraft, source_order: Option<(u64, u16)>) -> OutboundRecord {
    let (source_inbox_sequence, source_inbox_index) = source_order
        .map(|(sequence, index)| (Some(sequence), Some(index)))
        .unwrap_or((None, None));
    OutboundRecord {
        client_id: random_client_id(),
        recipient_id: draft.recipient_id,
        context_token: draft.context_token,
        text: draft.text,
        run_id: draft.run_id,
        source_inbox_sequence,
        source_inbox_index,
        created_at_ms: unix_time_ms(),
        state: OutboundState::Pending,
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Error)]
pub(super) enum RuntimeStoreError {
    #[error("Weixin runtime storage path is unsafe")]
    UnsafePath,
    #[error("another A3S runtime already owns the Weixin account lock")]
    LockContended,
    #[error("Weixin runtime recovery is required")]
    RecoveryRequired,
    #[error("Weixin runtime state is corrupt")]
    CorruptState,
    #[error("Weixin runtime file exceeds its size limit")]
    FileTooLarge,
    #[error("Weixin runtime journal record exceeds its size limit")]
    RecordTooLarge,
    #[error("Weixin runtime journal requires compaction")]
    JournalFull,
    #[error("Weixin runtime snapshot exceeds its size limit")]
    SnapshotTooLarge,
    #[error("Weixin private storage validation failed")]
    PrivateStorage(#[from] CredentialStoreError),
    #[error("Weixin runtime model validation failed")]
    Model(#[from] RuntimeModelError),
    #[error("Weixin runtime serialization failed")]
    Serialization(#[from] serde_json::Error),
    #[error("Weixin runtime storage I/O failed")]
    Io(#[source] std::io::Error),
    #[error("Weixin runtime storage task failed")]
    TaskJoin,
}

impl RuntimeStoreError {
    fn requires_reopen(&self) -> bool {
        matches!(
            self,
            Self::RecoveryRequired
                | Self::CorruptState
                | Self::PrivateStorage(_)
                | Self::Io(_)
                | Self::TaskJoin
        )
    }
}
