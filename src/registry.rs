//! Trusted extension-registry configuration and TUF package resolution.

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use a3s_acl::{Block, Document, Value};
use a3s_use_extension::{
    prepare_remote_package, refresh_remote_registry, ResolvedRemotePackage, TrustedRegistry,
    VerifiedRegistryMetadata,
};
use anyhow::{bail, Context};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const OFFICIAL_NAME: &str = "a3s";
pub const OFFICIAL_URL: &str = "https://components.a3s.dev/";
const OFFICIAL_TRUST_PLACEHOLDER: &str = "built-in TUF root";
const MAX_TRUSTED_ROOT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryRecord {
    pub name: String,
    pub url: String,
    pub trust_root: String,
    pub built_in: bool,
    pub configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trusted_root_path: Option<PathBuf>,
}

impl RegistryRecord {
    pub fn trusted_registry(&self, state_root: &Path) -> anyhow::Result<TrustedRegistry> {
        if !self.configured {
            bail!(
                "registry '{}' has no production TUF trust root configured",
                self.name
            );
        }
        TrustedRegistry::new(
            &self.name,
            &self.url,
            &self.trust_root,
            self.trusted_root_path.clone(),
            tuf_datastore(state_root, &self.name),
        )
        .map_err(anyhow::Error::new)
    }

    pub async fn refresh(&self, state_root: &Path) -> anyhow::Result<VerifiedRegistryMetadata> {
        let registry = self.trusted_registry(state_root)?;
        refresh_remote_registry(&registry)
            .await
            .map_err(|error| registry_error(self, error))
    }
}

#[derive(Debug)]
pub enum TrustRootSource<'a> {
    Digest(&'a str),
    File(&'a Path),
}

#[derive(Debug)]
pub struct RegistryEnrollment {
    pub record: RegistryRecord,
    root_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct ResolvedRegistryPackage {
    pub registry: RegistryRecord,
    pub package: ResolvedRemotePackage,
}

#[derive(Clone, Debug)]
pub struct RegistryStore {
    root: PathBuf,
}

impl RegistryStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn list(&self) -> anyhow::Result<Vec<RegistryRecord>> {
        let mut records = vec![official_registry()];
        if self.root.is_dir() {
            for entry in std::fs::read_dir(&self.root)
                .with_context(|| format!("could not read registry root {}", self.root.display()))?
            {
                let path = entry?.path();
                if path.extension().and_then(|value| value.to_str()) != Some("acl") {
                    continue;
                }
                if let Some(record) = self.read_path(&path)? {
                    records.push(record);
                }
            }
        }
        records.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(records)
    }

    pub fn get(&self, name: &str) -> anyhow::Result<Option<RegistryRecord>> {
        validate_name(name)?;
        if name == OFFICIAL_NAME {
            return Ok(Some(official_registry()));
        }
        self.read_path(&self.registry_path(name))
    }

    pub fn prepare_enrollment(
        &self,
        url: &str,
        source: TrustRootSource<'_>,
    ) -> anyhow::Result<RegistryEnrollment> {
        let url = normalize_url(url)?;
        let name = registry_name(&url)?;
        if name == OFFICIAL_NAME {
            bail!("registry name 'a3s' is reserved for the built-in official registry");
        }
        let trusted_root_path = self.trusted_root_path(&name);
        let (trust_root, root_bytes, trusted_root_path) = match source {
            TrustRootSource::Digest(value) => (normalize_digest(value)?, None, None),
            TrustRootSource::File(path) => {
                let metadata = std::fs::symlink_metadata(path).with_context(|| {
                    format!("could not inspect trust-root file {}", path.display())
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    bail!(
                        "trust-root path '{}' must be a regular file",
                        path.display()
                    );
                }
                if metadata.len() == 0 || metadata.len() > MAX_TRUSTED_ROOT_BYTES {
                    bail!(
                        "trust-root file must contain between 1 and {} bytes",
                        MAX_TRUSTED_ROOT_BYTES
                    );
                }
                let bytes = std::fs::read(path).with_context(|| {
                    format!("could not read trust-root file {}", path.display())
                })?;
                (
                    format!("sha256:{:x}", Sha256::digest(&bytes)),
                    Some(bytes),
                    Some(trusted_root_path),
                )
            }
        };
        Ok(RegistryEnrollment {
            record: RegistryRecord {
                name,
                url: url.to_string(),
                trust_root,
                built_in: false,
                configured: true,
                trusted_root_path,
            },
            root_bytes,
        })
    }

    pub fn add(&self, enrollment: &RegistryEnrollment) -> anyhow::Result<()> {
        let name = &enrollment.record.name;
        validate_name(name)?;
        if name == OFFICIAL_NAME {
            bail!("the built-in official registry cannot be replaced");
        }
        let path = self.registry_path(name);
        if path.exists() {
            bail!("registry '{name}' already exists; remove it before changing its trust root");
        }
        if let Some(bytes) = &enrollment.root_bytes {
            let root_path = enrollment
                .record
                .trusted_root_path
                .as_deref()
                .context("trusted root bytes have no destination path")?;
            ensure_real_directory(
                root_path
                    .parent()
                    .context("trusted root destination has no parent")?,
            )?;
            write_atomic(root_path, bytes)?;
        }
        write_registry(&path, &enrollment.record)
    }

    pub fn remove(&self, name: &str) -> anyhow::Result<RegistryRecord> {
        validate_name(name)?;
        if name == OFFICIAL_NAME {
            bail!("the built-in official registry cannot be removed");
        }
        let path = self.registry_path(name);
        let record = self
            .read_path(&path)?
            .with_context(|| format!("registry '{name}' is not configured"))?;
        std::fs::remove_file(&path)
            .with_context(|| format!("could not remove registry file {}", path.display()))?;
        let trusted_root_directory = self.root.join(name);
        match std::fs::symlink_metadata(&trusted_root_directory) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                std::fs::remove_file(&trusted_root_directory).with_context(|| {
                    format!(
                        "could not remove registry root link {}",
                        trusted_root_directory.display()
                    )
                })?;
            }
            Ok(metadata) if metadata.is_dir() => {
                std::fs::remove_dir_all(&trusted_root_directory).with_context(|| {
                    format!(
                        "could not remove registry root directory {}",
                        trusted_root_directory.display()
                    )
                })?;
            }
            Ok(_) => {
                std::fs::remove_file(&trusted_root_directory).with_context(|| {
                    format!(
                        "could not remove registry root file {}",
                        trusted_root_directory.display()
                    )
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(record)
    }

    pub async fn resolve_package(
        &self,
        state_root: &Path,
        package_id: &str,
        version: Option<&str>,
        channel: &str,
    ) -> anyhow::Result<ResolvedRegistryPackage> {
        let registries = self.configured_registries()?;
        let mut matches = Vec::new();
        for record in registries {
            let registry = record.trusted_registry(state_root)?;
            match prepare_remote_package(&registry, package_id, version, channel, None).await {
                Ok(prepared) => matches.push(ResolvedRegistryPackage {
                    registry: record,
                    package: prepared.resolved().clone(),
                }),
                Err(error) if error.code == "use.extension.registry_package_missing" => {}
                Err(error) => return Err(registry_error(&record, error)),
            }
        }
        match matches.len() {
            0 => bail!(
                "no trusted registry contains package '{}' for channel '{}'",
                package_id,
                channel
            ),
            1 => matches
                .pop()
                .context("one resolved registry package unexpectedly disappeared"),
            _ => {
                let names = matches
                    .iter()
                    .map(|resolved| resolved.registry.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                bail!(
                    "package '{}' is ambiguous across trusted registries: {}; remove the duplicate source",
                    package_id,
                    names
                )
            }
        }
    }

    pub fn require_configured_registry(&self) -> anyhow::Result<()> {
        self.configured_registries().map(|_| ())
    }

    fn configured_registries(&self) -> anyhow::Result<Vec<RegistryRecord>> {
        let registries = self
            .list()?
            .into_iter()
            .filter(|registry| registry.configured)
            .collect::<Vec<_>>();
        if registries.is_empty() {
            bail!(
                "no package registry has a production TUF trust root; add one with 'a3s registry add'"
            );
        }
        Ok(registries)
    }

    pub async fn resolve_upgrade(
        &self,
        state_root: &Path,
        installed: &ResolvedRemotePackage,
    ) -> anyhow::Result<ResolvedRegistryPackage> {
        let record = self.get(&installed.registry_name)?.with_context(|| {
            format!(
                "installed package source registry '{}' is no longer configured",
                installed.registry_name
            )
        })?;
        if !record.configured {
            bail!(
                "installed package source registry '{}' has no production TUF trust root configured",
                installed.registry_name
            );
        }
        let configured_root = record.trust_root.trim_start_matches("sha256:");
        if record.url != installed.registry_url || configured_root != installed.root_sha256 {
            bail!(
                "installed package source registry '{}' no longer matches its recorded URL and trust root; restore the original registry or reinstall with an explicit source migration",
                installed.registry_name
            );
        }

        let registry = record.trusted_registry(state_root)?;
        let prepared = prepare_remote_package(
            &registry,
            &installed.package_id,
            None,
            &installed.channel,
            None,
        )
        .await
        .map_err(|error| registry_error(&record, error))?;
        Ok(ResolvedRegistryPackage {
            registry: record,
            package: prepared.resolved().clone(),
        })
    }

    fn read_path(&self, path: &Path) -> anyhow::Result<Option<RegistryRecord>> {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let document = a3s_acl::parse_acl(&source)
            .with_context(|| format!("invalid registry ACL {}", path.display()))?;
        let block = document
            .blocks
            .into_iter()
            .find(|block| block.name == "registry")
            .with_context(|| format!("registry ACL {} has no registry block", path.display()))?;
        if block.labels.len() != 1 {
            bail!("registry ACL {} requires one name label", path.display());
        }
        let name = block.labels[0].clone();
        validate_name(&name)?;
        if name == OFFICIAL_NAME {
            bail!(
                "registry ACL {} uses the reserved name 'a3s'",
                path.display()
            );
        }
        if path.file_stem().and_then(|value| value.to_str()) != Some(name.as_str()) {
            bail!(
                "registry ACL filename '{}' does not match registry name '{}'",
                path.display(),
                name
            );
        }
        let url = block
            .attributes
            .get("url")
            .and_then(Value::as_str)
            .context("registry URL is missing")?;
        let url = normalize_url(url)?.to_string();
        let trust_root = normalize_digest(
            block
                .attributes
                .get("trust_root")
                .and_then(Value::as_str)
                .context("registry trust_root is missing")?,
        )?;
        let trusted_root_path = self.trusted_root_path(&name);
        let trusted_root_path = match std::fs::symlink_metadata(&trusted_root_path) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    bail!(
                        "trusted root '{}' must be a regular file",
                        trusted_root_path.display()
                    );
                }
                let bytes = std::fs::read(&trusted_root_path).with_context(|| {
                    format!(
                        "could not read trusted root {}",
                        trusted_root_path.display()
                    )
                })?;
                let actual = format!("sha256:{:x}", Sha256::digest(bytes));
                if actual != trust_root {
                    bail!(
                        "trusted root '{}' does not match the configured SHA-256 digest",
                        trusted_root_path.display()
                    );
                }
                Some(trusted_root_path)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        Ok(Some(RegistryRecord {
            name,
            url,
            trust_root,
            built_in: false,
            configured: true,
            trusted_root_path,
        }))
    }

    fn registry_path(&self, name: &str) -> PathBuf {
        self.root.join(format!("{name}.acl"))
    }

    fn trusted_root_path(&self, name: &str) -> PathBuf {
        self.root.join(name).join("root.json")
    }
}

fn official_registry() -> RegistryRecord {
    RegistryRecord {
        name: OFFICIAL_NAME.to_string(),
        url: OFFICIAL_URL.to_string(),
        trust_root: OFFICIAL_TRUST_PLACEHOLDER.to_string(),
        built_in: true,
        configured: false,
        trusted_root_path: None,
    }
}

fn write_registry(path: &Path, record: &RegistryRecord) -> anyhow::Result<()> {
    let attributes = HashMap::from([
        ("url".to_string(), Value::String(record.url.clone())),
        (
            "trust_root".to_string(),
            Value::String(record.trust_root.clone()),
        ),
    ]);
    let document = Document {
        blocks: vec![Block {
            name: "registry".to_string(),
            labels: vec![record.name.clone()],
            blocks: Vec::new(),
            attributes,
        }],
    };
    let rendered = a3s_acl::generate_acl(&document);
    a3s_acl::parse_acl(&rendered).context("generated registry ACL is invalid")?;
    write_atomic(path, rendered.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("path '{}' has no parent", path.display()))?;
    ensure_real_directory(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("could not create a temporary file in {}", parent.display()))?;
    temporary
        .write_all(bytes)
        .with_context(|| format!("could not write temporary file for {}", path.display()))?;
    temporary
        .as_file()
        .sync_all()
        .with_context(|| format!("could not sync temporary file for {}", path.display()))?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("could not atomically write {}", path.display()))?;
    Ok(())
}

fn ensure_real_directory(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("could not create directory {}", path.display()))?;
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect directory {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "registry path '{}' must be a real directory",
            path.display()
        );
    }
    Ok(())
}

fn normalize_url(value: &str) -> anyhow::Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(value).context("registry URL is invalid")?;
    let loopback_http = url.scheme() == "http"
        && url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case("localhost")
                || host
                    .parse::<std::net::IpAddr>()
                    .is_ok_and(|address| address.is_loopback())
        });
    if url.scheme() != "https" && !loopback_http {
        bail!("registry URLs must use HTTPS");
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        bail!("registry URLs must not contain credentials, query parameters, or fragments");
    }
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    Ok(url)
}

fn registry_name(url: &reqwest::Url) -> anyhow::Result<String> {
    let host = url.host_str().context("registry URL requires a host")?;
    let mut name = host
        .trim_start_matches("www.")
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    name.retain(|character| character.is_ascii_alphanumeric() || character == '-');
    validate_name(&name)?;
    Ok(name)
}

fn validate_name(name: &str) -> anyhow::Result<()> {
    let mut characters = name.chars();
    if !characters
        .next()
        .is_some_and(|value| value.is_ascii_lowercase())
        || !characters
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '-')
    {
        bail!(
            "registry names use lowercase letters, digits, and hyphens and must start with a letter"
        );
    }
    Ok(())
}

fn normalize_digest(value: &str) -> anyhow::Result<String> {
    let digest = value.strip_prefix("sha256:").unwrap_or(value);
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("SHA-256 trust roots require exactly 64 hexadecimal characters");
    }
    Ok(format!("sha256:{}", digest.to_ascii_lowercase()))
}

fn tuf_datastore(state_root: &Path, name: &str) -> PathBuf {
    state_root.join("use").join("remote-registries").join(name)
}

fn registry_error(
    registry: &RegistryRecord,
    error: impl std::error::Error + Send + Sync + 'static,
) -> anyhow::Error {
    anyhow::Error::new(error).context(format!(
        "registry '{}' failed TUF verification",
        registry.name
    ))
}

#[cfg(test)]
#[path = "../tests/support/tuf_test_support.rs"]
mod tuf_test_support;

#[cfg(test)]
mod tests {
    use super::tuf_test_support::{
        extension_archive, TestRepository, TestServer, FUTURE, PACKAGE_VERSION,
    };
    use super::*;

    #[tokio::test]
    async fn duplicate_package_sources_are_rejected_as_ambiguous() {
        let temp = tempfile::tempdir().unwrap();
        let repository = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
        let server = TestServer::start(repository.routes.clone());
        let store = RegistryStore::new(temp.path().join("registries"));
        for name in ["alpha", "beta"] {
            let record = RegistryRecord {
                name: name.to_string(),
                url: server.base_url().to_string(),
                trust_root: format!("sha256:{}", repository.root_sha256),
                built_in: false,
                configured: true,
                trusted_root_path: None,
            };
            write_registry(&store.registry_path(name), &record).unwrap();
        }

        let error = store
            .resolve_package(&temp.path().join("state"), "a3s/science", None, "stable")
            .await
            .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("ambiguous"), "{message}");
        assert!(message.contains("alpha, beta"), "{message}");
        assert!(server
            .requests()
            .iter()
            .all(|request| !request.starts_with("/targets/")));
    }

    #[tokio::test]
    async fn upgrade_uses_only_the_recorded_registry_and_rejects_identity_drift() {
        let temp = tempfile::tempdir().unwrap();
        let repository = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
        let server = TestServer::start(repository.routes.clone());
        let store = RegistryStore::new(temp.path().join("registries"));
        for name in ["alpha", "duplicate"] {
            let record = RegistryRecord {
                name: name.to_string(),
                url: server.base_url().to_string(),
                trust_root: format!("sha256:{}", repository.root_sha256),
                built_in: false,
                configured: true,
                trusted_root_path: None,
            };
            write_registry(&store.registry_path(name), &record).unwrap();
        }

        let alpha = store.get("alpha").unwrap().unwrap();
        let installed = prepare_remote_package(
            &alpha
                .trusted_registry(&temp.path().join("initial-state"))
                .unwrap(),
            "a3s/science",
            None,
            "stable",
            None,
        )
        .await
        .unwrap()
        .resolved()
        .clone();
        server.clear_requests();

        let resolved = store
            .resolve_upgrade(&temp.path().join("upgrade-state"), &installed)
            .await
            .unwrap();
        assert_eq!(resolved.registry.name, "alpha");
        assert_eq!(resolved.package.sha256, repository.target_sha256);
        assert!(server
            .requests()
            .iter()
            .all(|request| !request.starts_with("/targets/")));

        let changed = RegistryRecord {
            name: "alpha".to_string(),
            url: server.base_url().to_string(),
            trust_root: format!("sha256:{}", "f".repeat(64)),
            built_in: false,
            configured: true,
            trusted_root_path: None,
        };
        write_registry(&store.registry_path("alpha"), &changed).unwrap();
        server.clear_requests();
        let error = store
            .resolve_upgrade(&temp.path().join("changed-state"), &installed)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("no longer matches"), "{error:#}");
        assert!(server.requests().is_empty());
    }
}
