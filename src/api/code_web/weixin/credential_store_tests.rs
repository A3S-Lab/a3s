use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::credential_store::{
    CredentialStoreError, PrivateFileCredentialStore, WeixinCredentialStore, WeixinCredentials,
};
use a3s_boot::ilink::SecretValue;

fn credentials() -> WeixinCredentials {
    credentials_with_token("bot-token-canary")
}

fn credentials_with_token(token: &str) -> WeixinCredentials {
    WeixinCredentials::new(
        SecretValue::new(token).unwrap(),
        SecretValue::new("bot-id-canary").unwrap(),
        SecretValue::new("owner-id-canary").unwrap(),
        "https://ilinkai.weixin.qq.com",
        Some(SecretValue::new("context-token-canary").unwrap()),
    )
    .unwrap()
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_credential_store_round_trips_private_file_and_deletes_it() {
    use std::os::unix::fs::PermissionsExt;

    let temporary = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temporary.path()).unwrap();
    let directory = root.join("weixin");
    let store = PrivateFileCredentialStore::new(&directory);
    let expected = credentials();

    assert!(store.load().await.unwrap().is_none());
    store.save(&expected).await.unwrap();

    let path = store.path_for_test();
    let file_metadata = std::fs::symlink_metadata(&path).unwrap();
    let directory_metadata = std::fs::symlink_metadata(&directory).unwrap();
    assert_eq!(file_metadata.permissions().mode() & 0o777, 0o600);
    assert_eq!(directory_metadata.permissions().mode() & 0o777, 0o700);
    assert_eq!(store.load().await.unwrap(), Some(expected.clone()));
    let rendered = format!("{:?}", store.load().await.unwrap().unwrap());
    for canary in [
        "bot-token-canary",
        "bot-id-canary",
        "owner-id-canary",
        "context-token-canary",
        "ilinkai.weixin.qq.com",
    ] {
        assert!(!rendered.contains(canary));
    }

    let replacement = credentials_with_token("replacement-token-canary");
    store.save(&replacement).await.unwrap();
    assert_eq!(store.load().await.unwrap(), Some(replacement));
    assert_eq!(
        std::fs::symlink_metadata(&path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );

    store.delete().await.unwrap();
    assert!(!path.exists());
    assert!(store.load().await.unwrap().is_none());
}

#[tokio::test]
#[cfg(unix)]
async fn weixin_credential_store_rejects_symlinks_and_permissive_files() {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let temporary = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temporary.path()).unwrap();
    let real_directory = root.join("real");
    std::fs::create_dir(&real_directory).unwrap();
    std::fs::set_permissions(&real_directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let linked_directory = root.join("linked");
    symlink(&real_directory, &linked_directory).unwrap();
    let linked_store = PrivateFileCredentialStore::new(&linked_directory);
    assert!(matches!(
        linked_store.save(&credentials()).await,
        Err(CredentialStoreError::UnsafePath)
    ));

    let nested_link_store = PrivateFileCredentialStore::new(linked_directory.join("nested"));
    assert!(matches!(
        nested_link_store.save(&credentials()).await,
        Err(CredentialStoreError::UnsafePath)
    ));

    let store = PrivateFileCredentialStore::new(&real_directory);
    store.save(&credentials()).await.unwrap();
    std::fs::set_permissions(
        store.path_for_test(),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(matches!(
        store.load().await,
        Err(CredentialStoreError::UnsafePermissions)
    ));

    let permissive_parent = root.join("permissive-parent");
    std::fs::create_dir(&permissive_parent).unwrap();
    std::fs::set_permissions(&permissive_parent, std::fs::Permissions::from_mode(0o777)).unwrap();
    let nested_store = PrivateFileCredentialStore::new(permissive_parent.join("weixin"));
    assert!(matches!(
        nested_store.save(&credentials()).await,
        Err(CredentialStoreError::UnsafePermissions)
    ));

    let relative_store = PrivateFileCredentialStore::new("relative-weixin-state");
    assert!(matches!(
        relative_store.save(&credentials()).await,
        Err(CredentialStoreError::UnsafePath)
    ));
}

struct MemoryCredentialStore {
    value: Mutex<Option<WeixinCredentials>>,
}

impl MemoryCredentialStore {
    fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }
}

#[async_trait]
impl WeixinCredentialStore for MemoryCredentialStore {
    async fn load(&self) -> Result<Option<WeixinCredentials>, CredentialStoreError> {
        Ok(self.value.lock().await.clone())
    }

    async fn save(&self, credentials: &WeixinCredentials) -> Result<(), CredentialStoreError> {
        *self.value.lock().await = Some(credentials.clone());
        Ok(())
    }

    async fn delete(&self) -> Result<(), CredentialStoreError> {
        *self.value.lock().await = None;
        Ok(())
    }
}

#[tokio::test]
async fn weixin_credential_store_trait_supports_in_memory_test_double() {
    let store: Arc<dyn WeixinCredentialStore> = Arc::new(MemoryCredentialStore::new());
    let expected = credentials();

    store.save(&expected).await.unwrap();
    assert_eq!(store.load().await.unwrap(), Some(expected));
    store.delete().await.unwrap();
    assert!(store.load().await.unwrap().is_none());
}
