use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use zeroize::Zeroizing;

use super::model::{IdempotencyState, JournalRecord, OutboundState, RuntimeEvent, RuntimeState};
use super::RuntimeStoreError;
use crate::api::code_web::weixin::credential_store::{
    ensure_private_directory, set_private_file, sync_directory, verify_private_directory,
    verify_private_file_metadata,
};

pub(super) const SNAPSHOT_FILE_NAME: &str = "runtime.snapshot.json";
pub(super) const JOURNAL_FILE_NAME: &str = "runtime.journal.jsonl";
pub(super) const LOCK_FILE_NAME: &str = "account.lock";
pub(super) const RECOVERY_MARKER_FILE_NAME: &str = "recovery.required.json";
const MAX_SNAPSHOT_BYTES: u64 = 4 * 1024 * 1024;
const MAX_JOURNAL_BYTES: u64 = 8 * 1024 * 1024;
const MAX_JOURNAL_RECORD_BYTES: usize = 256 * 1024;
const MAX_RECOVERY_MARKER_BYTES: u64 = 16 * 1024;

pub(super) struct OpenedRuntime {
    pub(super) account_lock: File,
    pub(super) state: RuntimeState,
}

pub(super) fn open_runtime(directory: &Path) -> Result<OpenedRuntime, RuntimeStoreError> {
    ensure_private_directory(directory)?;
    verify_private_directory(directory)?;

    let account_lock = open_private_file(&directory.join(LOCK_FILE_NAME), false)?;
    fs2::FileExt::try_lock_exclusive(&account_lock).map_err(|error| {
        if error.kind() == std::io::ErrorKind::WouldBlock {
            RuntimeStoreError::LockContended
        } else {
            RuntimeStoreError::Io(error)
        }
    })?;

    let recovery_marker = directory.join(RECOVERY_MARKER_FILE_NAME);
    if read_private_file(&recovery_marker, MAX_RECOVERY_MARKER_BYTES)?.is_some() {
        return Err(RuntimeStoreError::RecoveryRequired);
    }

    let snapshot_path = directory.join(SNAPSHOT_FILE_NAME);
    let snapshot_bytes = match read_private_file(&snapshot_path, MAX_SNAPSHOT_BYTES) {
        Ok(bytes) => bytes,
        Err(RuntimeStoreError::FileTooLarge) => {
            quarantine(directory, &snapshot_path)?;
            return Err(RuntimeStoreError::CorruptState);
        }
        Err(error) => return Err(error),
    };
    let mut state = match snapshot_bytes {
        Some(bytes) => match serde_json::from_slice::<RuntimeState>(&bytes)
            .map_err(RuntimeStoreError::Serialization)
            .and_then(|state| {
                state.validate()?;
                Ok(state)
            }) {
            Ok(state) => state,
            Err(_) => {
                quarantine(directory, &snapshot_path)?;
                return Err(RuntimeStoreError::CorruptState);
            }
        },
        None => RuntimeState::default(),
    };

    let journal_path = directory.join(JOURNAL_FILE_NAME);
    let journal_bytes = match read_private_file(&journal_path, MAX_JOURNAL_BYTES) {
        Ok(bytes) => bytes,
        Err(RuntimeStoreError::FileTooLarge) => {
            quarantine(directory, &journal_path)?;
            return Err(RuntimeStoreError::CorruptState);
        }
        Err(error) => return Err(error),
    };
    match journal_bytes {
        Some(bytes) => {
            if replay_journal(&mut state, &bytes).is_err() {
                quarantine(directory, &journal_path)?;
                return Err(RuntimeStoreError::CorruptState);
            }
        }
        None => {
            let journal = open_private_file(&journal_path, true)?;
            journal.sync_all().map_err(RuntimeStoreError::Io)?;
            sync_directory(directory)?;
        }
    }
    state.validate()?;
    recover_uncertain_operations(directory, &mut state)?;
    Ok(OpenedRuntime {
        account_lock,
        state,
    })
}

fn recover_uncertain_operations(
    directory: &Path,
    state: &mut RuntimeState,
) -> Result<(), RuntimeStoreError> {
    let mut events = state
        .outbox
        .iter()
        .filter(|(_, message)| message.state == OutboundState::Sending)
        .map(|(client_id, _)| RuntimeEvent::SetOutboundState {
            client_id: client_id.clone(),
            state: OutboundState::OutcomeUnknown,
        })
        .collect::<Vec<_>>();
    events.extend(
        state
            .idempotency
            .iter()
            .filter(|(_, status)| **status == IdempotencyState::Reserved)
            .map(|(key, _)| RuntimeEvent::SetIdempotencyState {
                key: key.clone(),
                state: IdempotencyState::OutcomeUnknown,
            }),
    );
    for event in events {
        let record = JournalRecord::new(state.next_sequence, event);
        let mut next = state.clone();
        next.apply(&record)?;
        append_record(directory, &record)?;
        *state = next;
    }
    Ok(())
}

pub(super) fn append_record(
    directory: &Path,
    record: &JournalRecord,
) -> Result<(), RuntimeStoreError> {
    verify_private_directory(directory)?;
    let mut bytes = Zeroizing::new(serde_json::to_vec(record)?);
    bytes.push(b'\n');
    if bytes.len() > MAX_JOURNAL_RECORD_BYTES {
        return Err(RuntimeStoreError::RecordTooLarge);
    }
    let path = directory.join(JOURNAL_FILE_NAME);
    let mut file = open_private_file(&path, true)?;
    let current_size = file.metadata().map_err(RuntimeStoreError::Io)?.len();
    if current_size.saturating_add(bytes.len() as u64) > MAX_JOURNAL_BYTES {
        return Err(RuntimeStoreError::JournalFull);
    }
    file.write_all(&bytes).map_err(RuntimeStoreError::Io)?;
    file.sync_all().map_err(RuntimeStoreError::Io)
}

pub(super) fn compact_runtime(
    directory: &Path,
    state: &RuntimeState,
) -> Result<(), RuntimeStoreError> {
    let snapshot = serialize_snapshot(state)?;
    write_atomic_file(&directory.join(SNAPSHOT_FILE_NAME), &snapshot)?;
    write_atomic_file(&directory.join(JOURNAL_FILE_NAME), &[])?;
    Ok(())
}

pub(super) fn validate_snapshot_size(state: &RuntimeState) -> Result<(), RuntimeStoreError> {
    serialize_snapshot(state).map(|_| ())
}

fn serialize_snapshot(state: &RuntimeState) -> Result<Zeroizing<Vec<u8>>, RuntimeStoreError> {
    state.validate()?;
    let snapshot = Zeroizing::new(serde_json::to_vec(state)?);
    if snapshot.len() as u64 > MAX_SNAPSHOT_BYTES {
        return Err(RuntimeStoreError::SnapshotTooLarge);
    }
    Ok(snapshot)
}

fn replay_journal(state: &mut RuntimeState, bytes: &[u8]) -> Result<(), RuntimeStoreError> {
    if bytes.is_empty() {
        return Ok(());
    }
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if !line.ends_with(b"\n") || line.len() > MAX_JOURNAL_RECORD_BYTES || line.len() == 1 {
            return Err(RuntimeStoreError::CorruptState);
        }
        let record: JournalRecord = serde_json::from_slice(&line[..line.len() - 1])?;
        if record.sequence < state.next_sequence {
            continue;
        }
        if record.sequence > state.next_sequence {
            return Err(RuntimeStoreError::CorruptState);
        }
        state.apply(&record)?;
    }
    Ok(())
}

fn read_private_file(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<Zeroizing<Vec<u8>>>, RuntimeStoreError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(RuntimeStoreError::Io(error)),
    };
    verify_private_file_metadata(&metadata)?;
    if metadata.len() > max_bytes {
        return Err(RuntimeStoreError::FileTooLarge);
    }

    let mut options = OpenOptions::new();
    options.read(true);
    configure_no_follow(&mut options);
    let mut file = options.open(path).map_err(RuntimeStoreError::Io)?;
    let opened_metadata = file.metadata().map_err(RuntimeStoreError::Io)?;
    verify_private_file_metadata(&opened_metadata)?;
    if opened_metadata.len() > max_bytes {
        return Err(RuntimeStoreError::FileTooLarge);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(opened_metadata.len() as usize));
    Read::by_ref(&mut file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(RuntimeStoreError::Io)?;
    if bytes.len() as u64 > max_bytes {
        return Err(RuntimeStoreError::FileTooLarge);
    }
    Ok(Some(bytes))
}

fn open_private_file(path: &Path, append: bool) -> Result<File, RuntimeStoreError> {
    let existed = match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            verify_private_file_metadata(&metadata)?;
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(RuntimeStoreError::Io(error)),
    };
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).append(append);
    configure_private_create(&mut options);
    let file = options.open(path).map_err(RuntimeStoreError::Io)?;
    if !existed {
        set_private_file(&file)?;
    }
    verify_private_file_metadata(&file.metadata().map_err(RuntimeStoreError::Io)?)?;
    Ok(file)
}

fn write_atomic_file(path: &Path, bytes: &[u8]) -> Result<(), RuntimeStoreError> {
    let parent = path.parent().ok_or(RuntimeStoreError::UnsafePath)?;
    ensure_private_directory(parent)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => verify_private_file_metadata(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(RuntimeStoreError::Io(error)),
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(RuntimeStoreError::Io)?;
    set_private_file(temporary.as_file())?;
    temporary.write_all(bytes).map_err(RuntimeStoreError::Io)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(RuntimeStoreError::Io)?;
    temporary
        .persist(path)
        .map_err(|error| RuntimeStoreError::Io(error.error))?;
    verify_private_file_metadata(&std::fs::symlink_metadata(path).map_err(RuntimeStoreError::Io)?)?;
    sync_directory(parent)?;
    Ok(())
}

fn quarantine(directory: &Path, source: &Path) -> Result<(), RuntimeStoreError> {
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or(RuntimeStoreError::UnsafePath)?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let quarantined_name = format!(
        "{file_name}.corrupt.{timestamp}.{:016x}",
        rand::random::<u64>()
    );
    let marker = serde_json::json!({
        "schemaVersion": 1,
        "reason": "corruptRuntimeState",
        "quarantinedFile": quarantined_name,
    });
    let marker_bytes = serde_json::to_vec(&marker)?;
    write_atomic_file(&directory.join(RECOVERY_MARKER_FILE_NAME), &marker_bytes)?;
    std::fs::rename(source, directory.join(&quarantined_name)).map_err(RuntimeStoreError::Io)?;
    sync_directory(directory)?;
    Ok(())
}

#[cfg(unix)]
fn configure_no_follow(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn configure_no_follow(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn configure_private_create(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;

    options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
}

#[cfg(not(unix))]
fn configure_private_create(_options: &mut OpenOptions) {}
