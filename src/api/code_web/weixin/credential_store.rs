use std::fmt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use a3s_boot::ilink::SecretValue;

const CREDENTIAL_SCHEMA_VERSION: u32 = 1;
const CREDENTIAL_FILE_NAME: &str = "credentials.json";
const MAX_CREDENTIAL_FILE_BYTES: u64 = 256 * 1024;
const MAX_BASE_URL_BYTES: usize = 2048;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub(super) struct WeixinCredentials {
    pub(super) bot_token: SecretValue,
    pub(super) bot_id: SecretValue,
    pub(super) owner_id: SecretValue,
    pub(super) base_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) context_token: Option<SecretValue>,
}

impl WeixinCredentials {
    pub(super) fn new(
        bot_token: SecretValue,
        bot_id: SecretValue,
        owner_id: SecretValue,
        base_url: impl Into<String>,
        context_token: Option<SecretValue>,
    ) -> Result<Self, CredentialStoreError> {
        let credentials = Self {
            bot_token,
            bot_id,
            owner_id,
            base_url: base_url.into(),
            context_token,
        };
        credentials.validate()?;
        Ok(credentials)
    }

    fn validate(&self) -> Result<(), CredentialStoreError> {
        if self.base_url.is_empty()
            || self.base_url.len() > MAX_BASE_URL_BYTES
            || self.base_url.contains('\0')
        {
            return Err(CredentialStoreError::InvalidCredential);
        }
        Ok(())
    }
}

impl fmt::Debug for WeixinCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WeixinCredentials")
            .field("bot_token", &"[REDACTED]")
            .field("bot_id", &"[REDACTED]")
            .field("owner_id", &"[REDACTED]")
            .field("base_url", &"[REDACTED]")
            .field("has_context_token", &self.context_token.is_some())
            .finish()
    }
}

#[async_trait]
pub(super) trait WeixinCredentialStore: Send + Sync {
    async fn load(&self) -> Result<Option<WeixinCredentials>, CredentialStoreError>;

    async fn save(&self, credentials: &WeixinCredentials) -> Result<(), CredentialStoreError>;

    async fn delete(&self) -> Result<(), CredentialStoreError>;
}

pub(super) struct PrivateFileCredentialStore {
    directory: PathBuf,
    operation_lock: Mutex<()>,
}

impl PrivateFileCredentialStore {
    pub(super) fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            operation_lock: Mutex::new(()),
        }
    }

    fn credential_path(&self) -> PathBuf {
        self.directory.join(CREDENTIAL_FILE_NAME)
    }

    #[cfg(test)]
    pub(super) fn path_for_test(&self) -> PathBuf {
        self.credential_path()
    }
}

#[async_trait]
impl WeixinCredentialStore for PrivateFileCredentialStore {
    async fn load(&self) -> Result<Option<WeixinCredentials>, CredentialStoreError> {
        let _guard = self.operation_lock.lock().await;
        let path = self.credential_path();
        tokio::task::spawn_blocking(move || load_credentials(&path))
            .await
            .map_err(|_| CredentialStoreError::TaskJoin)?
    }

    async fn save(&self, credentials: &WeixinCredentials) -> Result<(), CredentialStoreError> {
        credentials.validate()?;
        let _guard = self.operation_lock.lock().await;
        let path = self.credential_path();
        let credentials = credentials.clone();
        tokio::task::spawn_blocking(move || save_credentials(&path, &credentials))
            .await
            .map_err(|_| CredentialStoreError::TaskJoin)?
    }

    async fn delete(&self) -> Result<(), CredentialStoreError> {
        let _guard = self.operation_lock.lock().await;
        let path = self.credential_path();
        tokio::task::spawn_blocking(move || delete_credentials(&path))
            .await
            .map_err(|_| CredentialStoreError::TaskJoin)?
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialDocument {
    schema_version: u32,
    credentials: WeixinCredentials,
}

fn load_credentials(path: &Path) -> Result<Option<WeixinCredentials>, CredentialStoreError> {
    let Some(parent) = path.parent() else {
        return Err(CredentialStoreError::UnsafePath);
    };
    if !path_exists(parent)? {
        return Ok(None);
    }
    verify_private_directory(parent)?;
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(CredentialStoreError::Io(error)),
    };
    verify_private_file_metadata(&metadata)?;
    if metadata.len() == 0 || metadata.len() > MAX_CREDENTIAL_FILE_BYTES {
        return Err(CredentialStoreError::InvalidSize);
    }

    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options.open(path).map_err(CredentialStoreError::Io)?;
    let opened_metadata = file.metadata().map_err(CredentialStoreError::Io)?;
    verify_private_file_metadata(&opened_metadata)?;
    if opened_metadata.len() == 0 || opened_metadata.len() > MAX_CREDENTIAL_FILE_BYTES {
        return Err(CredentialStoreError::InvalidSize);
    }
    let mut bytes = Zeroizing::new(Vec::with_capacity(opened_metadata.len() as usize));
    Read::by_ref(&mut file)
        .take(MAX_CREDENTIAL_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(CredentialStoreError::Io)?;
    if bytes.len() as u64 > MAX_CREDENTIAL_FILE_BYTES {
        return Err(CredentialStoreError::InvalidSize);
    }
    let document: CredentialDocument =
        serde_json::from_slice(&bytes).map_err(CredentialStoreError::Serialization)?;
    if document.schema_version != CREDENTIAL_SCHEMA_VERSION {
        return Err(CredentialStoreError::UnsupportedSchema);
    }
    document.credentials.validate()?;
    Ok(Some(document.credentials))
}

fn save_credentials(
    path: &Path,
    credentials: &WeixinCredentials,
) -> Result<(), CredentialStoreError> {
    let parent = path.parent().ok_or(CredentialStoreError::UnsafePath)?;
    ensure_private_directory(parent)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => verify_private_file_metadata(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(CredentialStoreError::Io(error)),
    }
    let document = CredentialDocument {
        schema_version: CREDENTIAL_SCHEMA_VERSION,
        credentials: credentials.clone(),
    };
    let mut bytes =
        Zeroizing::new(serde_json::to_vec(&document).map_err(CredentialStoreError::Serialization)?);
    bytes.push(b'\n');
    if bytes.len() as u64 > MAX_CREDENTIAL_FILE_BYTES {
        return Err(CredentialStoreError::InvalidSize);
    }

    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(CredentialStoreError::Io)?;
    set_private_file(temporary.as_file())?;
    temporary
        .write_all(&bytes)
        .map_err(CredentialStoreError::Io)?;
    temporary
        .as_file()
        .sync_all()
        .map_err(CredentialStoreError::Io)?;
    temporary
        .persist(path)
        .map_err(|error| CredentialStoreError::Io(error.error))?;
    let metadata = std::fs::symlink_metadata(path).map_err(CredentialStoreError::Io)?;
    verify_private_file_metadata(&metadata)?;
    sync_directory(parent)
}

fn delete_credentials(path: &Path) -> Result<(), CredentialStoreError> {
    let Some(parent) = path.parent() else {
        return Err(CredentialStoreError::UnsafePath);
    };
    if !path_exists(parent)? {
        return Ok(());
    }
    verify_private_directory(parent)?;
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => verify_private_file_metadata(&metadata)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(CredentialStoreError::Io(error)),
    }
    std::fs::remove_file(path).map_err(CredentialStoreError::Io)?;
    sync_directory(parent)
}

fn path_exists(path: &Path) -> Result<bool, CredentialStoreError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(CredentialStoreError::Io(error)),
    }
}

#[cfg(unix)]
pub(super) fn ensure_private_directory(path: &Path) -> Result<(), CredentialStoreError> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    verify_ancestor_chain(path, true)?;
    match std::fs::symlink_metadata(path) {
        Ok(_) => verify_private_directory(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(path).map_err(CredentialStoreError::Io)?;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
                .map_err(CredentialStoreError::Io)?;
            verify_ancestor_chain(path, false)?;
            verify_private_directory(path)
        }
        Err(error) => Err(CredentialStoreError::Io(error)),
    }
}

#[cfg(not(unix))]
pub(super) fn ensure_private_directory(_path: &Path) -> Result<(), CredentialStoreError> {
    Err(CredentialStoreError::UnsupportedPlatform)
}

#[cfg(unix)]
pub(super) fn verify_private_directory(path: &Path) -> Result<(), CredentialStoreError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    verify_ancestor_chain(path, false)?;
    let metadata = std::fs::symlink_metadata(path).map_err(CredentialStoreError::Io)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(CredentialStoreError::UnsafePath);
    }
    if metadata.uid() != effective_uid() {
        return Err(CredentialStoreError::UnsafeOwner);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(CredentialStoreError::UnsafePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_private_directory(_path: &Path) -> Result<(), CredentialStoreError> {
    Err(CredentialStoreError::UnsupportedPlatform)
}

#[cfg(unix)]
pub(super) fn verify_private_file_metadata(
    metadata: &std::fs::Metadata,
) -> Result<(), CredentialStoreError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(CredentialStoreError::UnsafePath);
    }
    if metadata.uid() != effective_uid() {
        return Err(CredentialStoreError::UnsafeOwner);
    }
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(CredentialStoreError::UnsafePermissions);
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn verify_private_file_metadata(
    _metadata: &std::fs::Metadata,
) -> Result<(), CredentialStoreError> {
    Err(CredentialStoreError::UnsupportedPlatform)
}

#[cfg(unix)]
pub(super) fn set_private_file(file: &File) -> Result<(), CredentialStoreError> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(CredentialStoreError::Io)
}

#[cfg(not(unix))]
pub(super) fn set_private_file(_file: &File) -> Result<(), CredentialStoreError> {
    Err(CredentialStoreError::UnsupportedPlatform)
}

#[cfg(unix)]
pub(super) fn sync_directory(path: &Path) -> Result<(), CredentialStoreError> {
    use std::os::unix::fs::OpenOptionsExt;

    let directory = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(CredentialStoreError::Io)?;
    directory.sync_all().map_err(CredentialStoreError::Io)
}

#[cfg(unix)]
fn verify_ancestor_chain(
    path: &Path,
    allow_missing_suffix: bool,
) -> Result<(), CredentialStoreError> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::Component;

    if !path.is_absolute()
        || path
            .components()
            .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(CredentialStoreError::UnsafePath);
    }

    let effective_uid = effective_uid();
    let mut missing = false;
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if missing {
            continue;
        }
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if allow_missing_suffix && error.kind() == std::io::ErrorKind::NotFound => {
                missing = true;
                continue;
            }
            Err(error) => return Err(CredentialStoreError::Io(error)),
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(CredentialStoreError::UnsafePath);
        }
        let owner = metadata.uid();
        if owner != 0 && owner != effective_uid {
            return Err(CredentialStoreError::UnsafeOwner);
        }
        let mode = metadata.permissions().mode();
        let writable_by_others = mode & 0o022 != 0;
        let sticky = mode & 0o1000 != 0;
        if writable_by_others && !sticky {
            return Err(CredentialStoreError::UnsafePermissions);
        }
    }
    Ok(())
}

#[cfg(unix)]
fn effective_uid() -> u32 {
    // SAFETY: `geteuid` has no preconditions and does not dereference memory.
    unsafe { libc::geteuid() }
}

#[cfg(not(unix))]
pub(super) fn sync_directory(_path: &Path) -> Result<(), CredentialStoreError> {
    Err(CredentialStoreError::UnsupportedPlatform)
}

#[derive(Debug, Error)]
pub(super) enum CredentialStoreError {
    #[cfg(not(unix))]
    #[error("Weixin credential storage is not supported on this platform")]
    UnsupportedPlatform,
    #[error("Weixin credential path is unsafe")]
    UnsafePath,
    #[error("Weixin credential path ownership is unsafe")]
    UnsafeOwner,
    #[error("Weixin credential permissions are unsafe")]
    UnsafePermissions,
    #[error("Weixin credential file has an invalid size")]
    InvalidSize,
    #[error("Weixin credential schema is unsupported")]
    UnsupportedSchema,
    #[error("Weixin credential data is invalid")]
    InvalidCredential,
    #[error("Weixin credential storage I/O failed")]
    Io(#[source] std::io::Error),
    #[error("Weixin credential serialization failed")]
    Serialization(#[source] serde_json::Error),
    #[error("Weixin credential storage task failed")]
    TaskJoin,
}
