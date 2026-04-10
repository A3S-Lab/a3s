//! Persistent Volume Management with pluggable CSI-compatible storage interface.
//!
//! Provides volume lifecycle management for MicroVMs with pluggable storage drivers,
//! similar to Kubernetes PV/PVC with CSI support.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

// Re-export for convenience
pub use crate::state::network_policy::LabelSelector;

// ============================================================================
// CSI-Compatible Storage Driver Interface
// ============================================================================

/// Storage driver capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverCapability {
    /// Plugin supports block storage.
    Block,
    /// Plugin supports filesystem storage.
    Filesystem,
    /// Plugin supports both.
    Both,
}

/// Volume capability for CSI.
#[derive(Debug, Clone)]
pub struct VolumeCapability {
    /// Access mode.
    pub access_mode: AccessMode,
    /// Mount flags (for filesystem).
    pub mount_flags: Vec<String>,
}

/// Provisioner parameters for dynamic volume creation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvisionerParams {
    /// Provisioner name (e.g., "hostpath", "emptydir", "nfs").
    pub provisioner: String,
    /// Storage type specific parameters.
    #[serde(default)]
    pub parameters: HashMap<String, String>,
}

/// Storage driver trait - CSI-compatible interface for storage providers.
pub trait StorageDriver: Send + Sync {
    /// Get driver name.
    fn name(&self) -> &str;

    /// Get driver capabilities.
    fn capabilities(&self) -> DriverCapability;

    /// Create a new volume.
    fn create_volume(
        &self,
        name: &str,
        capacity: i64,
        params: &ProvisionerParams,
    ) -> Result<VolumeSource>;

    /// Delete a volume.
    fn delete_volume(&self, name: &str, source: &VolumeSource) -> Result<()>;

    /// Validate volume capabilities.
    fn validate_capabilities(
        &self,
        source: &VolumeSource,
        capabilities: &[VolumeCapability],
    ) -> bool;

    /// Get mount path for a volume.
    fn get_mount_path(&self, name: &str, source: &VolumeSource) -> PathBuf;

    /// Check if volume supports the requested access mode.
    fn supports_access_mode(&self, source: &VolumeSource, mode: AccessMode) -> bool;
}

/// Storage plugin registry - manages registered storage drivers.
pub struct StoragePluginRegistry {
    /// Registered drivers.
    drivers: RwLock<HashMap<String, Arc<dyn StorageDriver>>>,
}

impl StoragePluginRegistry {
    /// Create a new registry.
    pub fn new() -> Self {
        let registry = Self {
            drivers: RwLock::new(HashMap::new()),
        };
        registry.register_builtin_drivers();
        registry
    }

    /// Register a storage driver.
    pub async fn register(&self, driver: Arc<dyn StorageDriver>) {
        let mut drivers = self.drivers.write().await;
        drivers.insert(driver.name().to_string(), driver);
    }

    /// Unregister a storage driver.
    pub async fn unregister(&self, name: &str) {
        let mut drivers = self.drivers.write().await;
        drivers.remove(name);
    }

    /// Get a driver by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn StorageDriver>> {
        let drivers = self.drivers.read().await;
        drivers.get(name).cloned()
    }

    /// List all registered driver names.
    pub async fn list_drivers(&self) -> Vec<String> {
        let drivers = self.drivers.read().await;
        drivers.keys().cloned().collect()
    }

    /// Register built-in drivers.
    fn register_builtin_drivers(&self) {
        // Register built-in drivers using a simple approach
        // In a real implementation, this would use Arc<Self> properly
    }
}

impl Default for StoragePluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// HostPath storage driver.
pub struct HostPathDriver {
    /// Base directory for volumes.
    base_dir: PathBuf,
}

impl HostPathDriver {
    /// Create a new HostPath driver.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Get the host path for a volume.
    pub fn get_host_path(&self, name: &str) -> PathBuf {
        self.base_dir.join("hostpath").join(name)
    }
}

impl StorageDriver for HostPathDriver {
    fn name(&self) -> &str {
        "hostpath"
    }

    fn capabilities(&self) -> DriverCapability {
        DriverCapability::Both
    }

    fn create_volume(
        &self,
        name: &str,
        _capacity: i64,
        params: &ProvisionerParams,
    ) -> Result<VolumeSource> {
        let path = params
            .parameters
            .get("path")
            .cloned()
            .unwrap_or_else(|| self.get_host_path(name).to_string_lossy().to_string());

        let path_type = params
            .parameters
            .get("type")
            .and_then(|t| match t.as_str() {
                "DirectoryOrCreate" => Some(HostPathType::DirectoryOrCreate),
                "Directory" => Some(HostPathType::Directory),
                "FileOrCreate" => Some(HostPathType::FileOrCreate),
                "File" => Some(HostPathType::File),
                "Socket" => Some(HostPathType::Socket),
                "CharDevice" => Some(HostPathType::CharDevice),
                "BlockDevice" => Some(HostPathType::BlockDevice),
                _ => Some(HostPathType::DirectoryOrCreate),
            })
            .unwrap_or(HostPathType::DirectoryOrCreate);

        let host_path = PathBuf::from(&path);
        if !host_path.exists() {
            std::fs::create_dir_all(&host_path).map_err(|e| {
                A3sError::Project(format!("failed to create host path {}: {}", path, e))
            })?;
        }

        Ok(VolumeSource::HostPath {
            path,
            type_: path_type,
        })
    }

    fn delete_volume(&self, _name: &str, source: &VolumeSource) -> Result<()> {
        if let VolumeSource::HostPath { path, .. } = source {
            // Don't delete user-specified paths outside our base
            let base_path = self.base_dir.join("hostpath");
            if !path.starts_with(&base_path.to_string_lossy().to_string()) {
                return Ok(());
            }
            let host_path = PathBuf::from(path);
            if host_path.exists() && host_path.is_dir() {
                std::fs::remove_dir_all(&host_path)
                    .map_err(|e| A3sError::Project(format!("failed to delete host path: {}", e)))?;
            }
        }
        Ok(())
    }

    fn validate_capabilities(
        &self,
        source: &VolumeSource,
        capabilities: &[VolumeCapability],
    ) -> bool {
        capabilities
            .iter()
            .all(|cap| self.supports_access_mode(source, cap.access_mode))
    }

    fn get_mount_path(&self, name: &str, source: &VolumeSource) -> PathBuf {
        if let VolumeSource::HostPath { path, .. } = source {
            PathBuf::from(path)
        } else {
            self.get_host_path(name)
        }
    }

    fn supports_access_mode(&self, _source: &VolumeSource, mode: AccessMode) -> bool {
        // HostPath only supports ReadWriteOnce
        matches!(mode, AccessMode::ReadWriteOnce)
    }
}

/// EmptyDir storage driver.
pub struct EmptyDirDriver {
    /// Base directory for volumes.
    base_dir: PathBuf,
}

impl EmptyDirDriver {
    /// Create a new EmptyDir driver.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Get the volume directory.
    pub fn get_volume_dir(&self, name: &str) -> PathBuf {
        self.base_dir.join("emptydir").join(name)
    }
}

impl StorageDriver for EmptyDirDriver {
    fn name(&self) -> &str {
        "emptydir"
    }

    fn capabilities(&self) -> DriverCapability {
        DriverCapability::Both
    }

    fn create_volume(
        &self,
        name: &str,
        _capacity: i64,
        _params: &ProvisionerParams,
    ) -> Result<VolumeSource> {
        let volume_dir = self.get_volume_dir(name);
        std::fs::create_dir_all(&volume_dir)
            .map_err(|e| A3sError::Project(format!("failed to create empty dir volume: {}", e)))?;
        Ok(VolumeSource::EmptyDir {})
    }

    fn delete_volume(&self, name: &str, source: &VolumeSource) -> Result<()> {
        if matches!(source, VolumeSource::EmptyDir {}) {
            let volume_dir = self.get_volume_dir(name);
            if volume_dir.exists() {
                std::fs::remove_dir_all(&volume_dir)
                    .map_err(|e| A3sError::Project(format!("failed to delete empty dir: {}", e)))?;
            }
        }
        Ok(())
    }

    fn validate_capabilities(
        &self,
        source: &VolumeSource,
        capabilities: &[VolumeCapability],
    ) -> bool {
        capabilities
            .iter()
            .all(|cap| self.supports_access_mode(source, cap.access_mode))
    }

    fn get_mount_path(&self, name: &str, _source: &VolumeSource) -> PathBuf {
        self.get_volume_dir(name)
    }

    fn supports_access_mode(&self, _source: &VolumeSource, mode: AccessMode) -> bool {
        // EmptyDir supports ReadWriteOnce only (local storage)
        matches!(mode, AccessMode::ReadWriteOnce)
    }
}

/// NFS storage driver configuration.
#[derive(Debug, Clone)]
pub struct NfsDriverConfig {
    /// NFS server address.
    pub server: String,
    /// NFS export path.
    pub export: String,
}

/// NFS storage driver.
pub struct NfsDriver {
    /// Configuration.
    config: NfsDriverConfig,
}

impl NfsDriver {
    /// Create a new NFS driver.
    pub fn new(config: NfsDriverConfig) -> Self {
        Self { config }
    }
}

impl StorageDriver for NfsDriver {
    fn name(&self) -> &str {
        "nfs"
    }

    fn capabilities(&self) -> DriverCapability {
        DriverCapability::Filesystem
    }

    fn create_volume(
        &self,
        _name: &str,
        _capacity: i64,
        params: &ProvisionerParams,
    ) -> Result<VolumeSource> {
        let server = params
            .parameters
            .get("server")
            .cloned()
            .unwrap_or_else(|| self.config.server.clone());
        let export = params
            .parameters
            .get("export")
            .cloned()
            .unwrap_or_else(|| self.config.export.clone());

        Ok(VolumeSource::Nfs {
            server,
            path: export,
            read_only: false,
        })
    }

    fn delete_volume(&self, _name: &str, _source: &VolumeSource) -> Result<()> {
        // NFS volumes don't need cleanup on the driver side
        Ok(())
    }

    fn validate_capabilities(
        &self,
        source: &VolumeSource,
        capabilities: &[VolumeCapability],
    ) -> bool {
        if let VolumeSource::Nfs { read_only, .. } = source {
            capabilities.iter().all(|cap| {
                if *read_only {
                    matches!(cap.access_mode, AccessMode::ReadOnlyMany)
                } else {
                    matches!(
                        cap.access_mode,
                        AccessMode::ReadWriteOnce
                            | AccessMode::ReadWriteMany
                            | AccessMode::ReadOnlyMany
                    )
                }
            })
        } else {
            false
        }
    }

    fn get_mount_path(&self, _name: &str, source: &VolumeSource) -> PathBuf {
        if let VolumeSource::Nfs { server, path, .. } = source {
            PathBuf::from(format!("{}:{}", server, path))
        } else {
            PathBuf::new()
        }
    }

    fn supports_access_mode(&self, source: &VolumeSource, mode: AccessMode) -> bool {
        if let VolumeSource::Nfs { read_only, .. } = source {
            if *read_only {
                matches!(mode, AccessMode::ReadOnlyMany)
            } else {
                matches!(
                    mode,
                    AccessMode::ReadWriteOnce
                        | AccessMode::ReadWriteMany
                        | AccessMode::ReadOnlyMany
                )
            }
        } else {
            false
        }
    }
}

// Add NFS variant to VolumeSource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum VolumeSource {
    /// HostPath volume source.
    HostPath {
        path: String,
        #[serde(default)]
        type_: HostPathType,
    },
    /// EmptyDir volume source (temporary storage).
    EmptyDir {},
    /// NFS volume source.
    Nfs {
        /// NFS server address.
        server: String,
        /// NFS export path.
        path: String,
        /// Read-only mount.
        #[serde(default)]
        read_only: bool,
    },
    /// Persistent volume claim reference.
    PersistentVolumeClaim {
        claim_name: String,
        #[serde(default)]
        read_only: bool,
    },
}

/// Volume access mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AccessMode {
    /// Single node read-write.
    ReadWriteOnce,
    /// Multiple nodes read-only.
    ReadOnlyMany,
    /// Multiple nodes read-write.
    ReadWriteMany,
}

impl Default for AccessMode {
    fn default() -> Self {
        AccessMode::ReadWriteOnce
    }
}

/// Volume status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeStatus {
    /// Volume is pending provisioning.
    Pending,
    /// Volume is available.
    Available,
    /// Volume is bound to a claim.
    Bound,
    /// Volume is released.
    Released,
    /// Volume has failed.
    Failed,
}

/// Persistent Volume specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeSpec {
    /// Storage capacity in bytes.
    pub capacity: i64,
    /// Access mode.
    pub access_modes: Vec<AccessMode>,
    /// Reclaim policy.
    pub reclaim_policy: ReclaimPolicy,
    /// Storage class name.
    pub storage_class: Option<String>,
    /// Volume mode (Filesystem or Block).
    pub volume_mode: VolumeMode,
    /// Mount options.
    #[serde(default)]
    pub mount_options: Vec<String>,
    /// Source type.
    pub source: VolumeSource,
}

/// Reclaim policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReclaimPolicy {
    Retain,
    Recycle,
    Delete,
}

impl Default for ReclaimPolicy {
    fn default() -> Self {
        ReclaimPolicy::Retain
    }
}

/// Volume mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeMode {
    Filesystem,
    Block,
}

impl Default for VolumeMode {
    fn default() -> Self {
        VolumeMode::Filesystem
    }
}

/// HostPath type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HostPathType {
    /// If nothing exists at the given path, an empty directory is created.
    DirectoryOrCreate,
    /// A directory must exist at the given path.
    Directory,
    /// If nothing exists at the given path, an empty file is created.
    FileOrCreate,
    /// A file must exist at the given path.
    File,
    /// A Unix socket must exist at the given path.
    Socket,
    /// A character device must exist at the given path.
    CharDevice,
    /// A block device must exist at the given path.
    BlockDevice,
}

impl Default for HostPathType {
    fn default() -> Self {
        HostPathType::DirectoryOrCreate
    }
}

/// Persistent Volume desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeDesired {
    /// Volume name.
    pub name: String,
    /// Volume specification.
    pub spec: PersistentVolumeSpec,
    /// Status.
    pub status: VolumeStatus,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Associated claim name if bound.
    pub claim_name: Option<String>,
}

/// Persistent Volume Claim specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeClaimSpec {
    /// Access modes requested.
    pub access_modes: Vec<AccessMode>,
    /// Resource requirements.
    pub resources: ClaimResources,
    /// Storage class name.
    pub storage_class_name: Option<String>,
    /// Volume name to bind to.
    pub volume_name: Option<String>,
    /// Volume mode.
    pub volume_mode: VolumeMode,
    /// Selector for dynamic provisioning.
    #[serde(default)]
    pub selector: Option<LabelSelector>,
}

/// Claim resources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaimResources {
    /// Storage request in bytes.
    pub requests: HashMap<String, i64>,
    /// Storage limit in bytes.
    #[serde(default)]
    pub limits: HashMap<String, i64>,
}

/// Persistent Volume Claim desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistentVolumeClaimDesired {
    /// Claim name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Claim specification.
    pub spec: PersistentVolumeClaimSpec,
    /// Status.
    pub status: VolumeStatus,
    /// Bound volume name.
    pub volume_name: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Volume mount in pod spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeMount {
    /// Name of the volume.
    pub name: String,
    /// Mount path in container.
    pub mount_path: String,
    /// Sub-path within the volume.
    #[serde(default)]
    pub sub_path: Option<String>,
    /// Read-only mount.
    #[serde(default)]
    pub read_only: bool,
}

/// Volume reference in pod spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeRef {
    /// Volume name.
    pub name: String,
    /// Volume source.
    pub source: VolumeSource,
    /// Mount options.
    #[serde(default)]
    pub options: HashMap<String, String>,
}

/// Volume manager - handles volume lifecycle.
pub struct VolumeManager {
    /// Persistent volumes.
    volumes: RwLock<HashMap<String, PersistentVolumeDesired>>,
    /// Persistent volume claims.
    claims: RwLock<HashMap<String, HashMap<String, PersistentVolumeClaimDesired>>>,
    /// Base directory for volumes.
    base_dir: PathBuf,
}

impl VolumeManager {
    /// Create a new volume manager.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            volumes: RwLock::new(HashMap::new()),
            claims: RwLock::new(HashMap::new()),
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Ensure volume directory exists.
    fn ensure_volume_dir(&self, name: &str) -> Result<PathBuf> {
        let path = self.base_dir.join("volumes").join(name);
        std::fs::create_dir_all(&path).map_err(|e| {
            A3sError::Project(format!(
                "failed to create volume directory {}: {}",
                path.display(),
                e
            ))
        })?;
        Ok(path)
    }

    // ==================== Persistent Volume Operations ====================

    /// Create a persistent volume.
    pub async fn create_pv(&self, pv: PersistentVolumeDesired) -> Result<()> {
        // Validate source
        match &pv.spec.source {
            VolumeSource::HostPath { path, .. } => {
                // For DirectoryOrCreate, ensure the host path exists
                let host_path = PathBuf::from(path);
                if !host_path.exists() {
                    std::fs::create_dir_all(&host_path).map_err(|e| {
                        A3sError::Project(format!("failed to create host path: {}", e))
                    })?;
                }
            }
            VolumeSource::EmptyDir {} => {
                // Create the volume directory
                self.ensure_volume_dir(&pv.name)?;
            }
            VolumeSource::Nfs { .. } => {
                // NFS volumes don't need local setup
            }
            VolumeSource::PersistentVolumeClaim { .. } => {
                // No-op for PVC references
            }
        }

        let mut volumes = self.volumes.write().await;
        volumes.insert(pv.name.clone(), pv);
        Ok(())
    }

    /// Get a persistent volume.
    pub async fn get_pv(&self, name: &str) -> Option<PersistentVolumeDesired> {
        let volumes = self.volumes.read().await;
        volumes.get(name).cloned()
    }

    /// List all persistent volumes.
    pub async fn list_pvs(&self) -> Vec<PersistentVolumeDesired> {
        let volumes = self.volumes.read().await;
        volumes.values().cloned().collect()
    }

    /// Delete a persistent volume.
    pub async fn delete_pv(&self, name: &str) -> Result<()> {
        let mut volumes = self.volumes.write().await;

        if let Some(pv) = volumes.get(name) {
            // Check if volume is bound
            if pv.status == VolumeStatus::Bound {
                return Err(A3sError::Project(format!(
                    "cannot delete bound volume {}: must unbind first",
                    name
                )));
            }

            // Handle reclaim policy
            match pv.spec.reclaim_policy {
                ReclaimPolicy::Delete => {
                    // Delete the underlying storage
                    let volume_dir = self.base_dir.join("volumes").join(name);
                    if volume_dir.exists() {
                        std::fs::remove_dir_all(&volume_dir).map_err(|e| {
                            A3sError::Project(format!("failed to delete volume directory: {}", e))
                        })?;
                    }
                }
                ReclaimPolicy::Retain => {
                    // Just remove the PV, keep the data
                }
                ReclaimPolicy::Recycle => {
                    // Recycle: delete data and make available again
                    let volume_dir = self.base_dir.join("volumes").join(name);
                    if volume_dir.exists() {
                        Self::recycle_volume_dir(&volume_dir)?;
                    }
                    let pv = volumes.get_mut(name).unwrap();
                    pv.status = VolumeStatus::Available;
                    pv.claim_name = None;
                    return Ok(());
                }
            }
        }

        volumes.remove(name);
        Ok(())
    }

    /// Recycle a volume directory (rm -rf contents).
    fn recycle_volume_dir(path: &Path) -> Result<()> {
        for entry in std::fs::read_dir(path)
            .map_err(|e| A3sError::Project(format!("failed to read volume directory: {}", e)))?
        {
            let entry =
                entry.map_err(|e| A3sError::Project(format!("failed to read entry: {}", e)))?;
            let ty = entry
                .file_type()
                .map_err(|e| A3sError::Project(format!("failed to get file type: {}", e)))?;
            if ty.is_dir() {
                std::fs::remove_dir_all(entry.path())
                    .map_err(|e| A3sError::Project(format!("failed to remove directory: {}", e)))?;
            } else {
                std::fs::remove_file(entry.path())
                    .map_err(|e| A3sError::Project(format!("failed to remove file: {}", e)))?;
            }
        }
        Ok(())
    }

    /// Bind a volume to a claim.
    pub async fn bind_volume(
        &self,
        volume_name: &str,
        claim_name: &str,
        namespace: &str,
    ) -> Result<()> {
        let mut volumes = self.volumes.write().await;

        if let Some(volume) = volumes.get_mut(volume_name) {
            if volume.status != VolumeStatus::Available {
                return Err(A3sError::Project(format!(
                    "volume {} is not available (status: {:?})",
                    volume_name, volume.status
                )));
            }

            volume.status = VolumeStatus::Bound;
            volume.claim_name = Some(claim_name.to_string());

            // Update claim status
            drop(volumes);
            let mut claims = self.claims.write().await;
            if let Some(ns_claims) = claims.get_mut(namespace) {
                if let Some(claim) = ns_claims.get_mut(claim_name) {
                    claim.status = VolumeStatus::Bound;
                    claim.volume_name = Some(volume_name.to_string());
                }
            }

            Ok(())
        } else {
            Err(A3sError::Project(format!(
                "volume {} not found",
                volume_name
            )))
        }
    }

    /// Unbind a volume from a claim.
    pub async fn unbind_volume(&self, volume_name: &str) -> Result<()> {
        let mut volumes = self.volumes.write().await;

        if let Some(volume) = volumes.get_mut(volume_name) {
            if volume.status != VolumeStatus::Bound {
                return Err(A3sError::Project(format!(
                    "volume {} is not bound",
                    volume_name
                )));
            }

            let claim_name = volume.claim_name.take();
            volume.status = VolumeStatus::Available;

            // Also update claim if we can find it
            if let Some(name) = claim_name {
                // Try default namespace
                let mut claims = self.claims.write().await;
                if let Some(ns_claims) = claims.get_mut("default") {
                    if let Some(claim) = ns_claims.get_mut(&name) {
                        claim.status = VolumeStatus::Released;
                        claim.volume_name = None;
                    }
                }
            }

            Ok(())
        } else {
            Err(A3sError::Project(format!(
                "volume {} not found",
                volume_name
            )))
        }
    }

    // ==================== Persistent Volume Claim Operations ====================

    /// Create a persistent volume claim.
    pub async fn create_pvc(&self, pvc: PersistentVolumeClaimDesired) -> Result<()> {
        let mut claims = self.claims.write().await;
        let ns_claims = claims
            .entry(pvc.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_claims.insert(pvc.name.clone(), pvc);
        Ok(())
    }

    /// Get a persistent volume claim.
    pub async fn get_pvc(
        &self,
        namespace: &str,
        name: &str,
    ) -> Option<PersistentVolumeClaimDesired> {
        let claims = self.claims.read().await;
        claims
            .get(namespace)
            .and_then(|ns_claims| ns_claims.get(name).cloned())
    }

    /// List all persistent volume claims in a namespace.
    pub async fn list_pvcs_in_namespace(
        &self,
        namespace: &str,
    ) -> Vec<PersistentVolumeClaimDesired> {
        let claims = self.claims.read().await;
        claims
            .get(namespace)
            .map(|ns_claims| ns_claims.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all persistent volume claims.
    pub async fn list_all_pvcs(&self) -> Vec<(String, PersistentVolumeClaimDesired)> {
        let claims = self.claims.read().await;
        claims
            .iter()
            .flat_map(|(ns, ns_claims)| {
                ns_claims
                    .values()
                    .map(|pvc| (ns.clone(), pvc.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Delete a persistent volume claim.
    pub async fn delete_pvc(&self, namespace: &str, name: &str) -> Result<()> {
        let mut claims = self.claims.write().await;
        if let Some(ns_claims) = claims.get_mut(namespace) {
            ns_claims.remove(name);
        }
        Ok(())
    }

    /// Find a suitable volume for a claim.
    pub async fn find_matching_volume(&self, pvc: &PersistentVolumeClaimDesired) -> Option<String> {
        let volumes = self.volumes.read().await;

        for (name, pv) in volumes.iter() {
            // Skip non-available volumes
            if pv.status != VolumeStatus::Available {
                continue;
            }

            // Check storage capacity
            let requested = pvc
                .spec
                .resources
                .requests
                .get("storage")
                .copied()
                .unwrap_or(0);

            if pv.spec.capacity < requested {
                continue;
            }

            // Check access modes
            let has_matching_mode = pvc
                .spec
                .access_modes
                .iter()
                .any(|req_mode| pv.spec.access_modes.contains(req_mode));

            if !has_matching_mode {
                continue;
            }

            // Check storage class
            if let Some(ref class) = pvc.spec.storage_class_name {
                if pv.spec.storage_class.as_ref() != Some(class) {
                    continue;
                }
            }

            // Check selector if present
            if let Some(ref selector) = pvc.spec.selector {
                if !selector.match_labels.is_empty() {
                    let mut matches = true;
                    for (key, val) in &selector.match_labels {
                        if pv.name.contains(key) && pv.name.contains(val) {
                            // Name-based matching for now
                        } else {
                            matches = false;
                            break;
                        }
                    }
                    if !matches {
                        continue;
                    }
                }
            }

            return Some(name.clone());
        }

        None
    }

    /// Provision a volume dynamically for a claim.
    pub async fn provision_volume(
        &self,
        pvc: &PersistentVolumeClaimDesired,
    ) -> Result<PersistentVolumeDesired> {
        let requested_storage = pvc
            .spec
            .resources
            .requests
            .get("storage")
            .copied()
            .unwrap_or(64 * 1024 * 1024 * 1024); // Default 64Gi

        let volume_name = format!("pvc-{}", uuid::Uuid::new_v4());

        let pv = PersistentVolumeDesired {
            name: volume_name.clone(),
            spec: PersistentVolumeSpec {
                capacity: requested_storage,
                access_modes: pvc.spec.access_modes.clone(),
                reclaim_policy: ReclaimPolicy::Delete,
                storage_class: pvc.spec.storage_class_name.clone(),
                volume_mode: pvc.spec.volume_mode.clone(),
                mount_options: vec![],
                source: VolumeSource::EmptyDir {},
            },
            status: VolumeStatus::Bound,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claim_name: Some(pvc.name.clone()),
        };

        self.create_pv(pv.clone()).await?;

        Ok(pv)
    }

    /// Get volume mount path.
    pub fn get_mount_path(&self, volume_name: &str) -> PathBuf {
        self.base_dir.join("volumes").join(volume_name)
    }
}

/// Volume attachment - tracks which pods have volumes mounted.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAttachment {
    /// Pod ID.
    pub pod_id: String,
    /// Volume name.
    pub volume_name: String,
    /// Mount path in pod.
    pub mount_path: String,
    /// Read-only.
    pub read_only: bool,
    /// Attachment timestamp.
    pub attached_at: DateTime<Utc>,
}

/// Volume attachment manager.
pub struct VolumeAttachmentManager {
    /// Current attachments.
    attachments: RwLock<HashMap<String, Vec<VolumeAttachment>>>,
}

impl VolumeAttachmentManager {
    pub fn new() -> Self {
        Self {
            attachments: RwLock::new(HashMap::new()),
        }
    }

    /// Attach a volume to a pod.
    pub async fn attach(&self, attachment: VolumeAttachment) -> Result<()> {
        let mut attachments = self.attachments.write().await;
        let pod_attachments = attachments
            .entry(attachment.pod_id.clone())
            .or_insert_with(Vec::new);

        // Check if already attached
        if pod_attachments
            .iter()
            .any(|a| a.volume_name == attachment.volume_name)
        {
            return Err(A3sError::Project(format!(
                "volume {} is already attached to pod {}",
                attachment.volume_name, attachment.pod_id
            )));
        }

        pod_attachments.push(attachment);
        Ok(())
    }

    /// Detach a volume from a pod.
    pub async fn detach(&self, pod_id: &str, volume_name: &str) -> Result<()> {
        let mut attachments = self.attachments.write().await;
        if let Some(pod_attachments) = attachments.get_mut(pod_id) {
            pod_attachments.retain(|a| a.volume_name != volume_name);
        }
        Ok(())
    }

    /// Get attachments for a pod.
    pub async fn get_attachments(&self, pod_id: &str) -> Vec<VolumeAttachment> {
        let attachments = self.attachments.read().await;
        attachments.get(pod_id).cloned().unwrap_or_default()
    }

    /// Check if a volume is attached to any pod.
    pub async fn is_attached(&self, volume_name: &str) -> bool {
        let attachments = self.attachments.read().await;
        attachments
            .values()
            .any(|pod_atts| pod_atts.iter().any(|a| a.volume_name == volume_name))
    }

    /// Get pods that have a volume attached.
    pub async fn get_pods_with_volume(&self, volume_name: &str) -> Vec<String> {
        let attachments = self.attachments.read().await;
        attachments
            .iter()
            .filter(|(_, pod_atts)| pod_atts.iter().any(|a| a.volume_name == volume_name))
            .map(|(pod_id, _)| pod_id.clone())
            .collect()
    }
}

impl Default for VolumeAttachmentManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// StorageClass - Dynamic Provisioning
// ============================================================================

/// Storage class parameters for volume allocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageClassParameters {
    /// Provisioner type.
    pub provisioner: String,
    /// Type of storage (e.g., "hostpath", "emptydir", "nfs").
    pub storage_type: String,
    /// NFS server (if NFS).
    pub nfs_server: Option<String>,
    /// NFS path (if NFS).
    pub nfs_path: Option<String>,
    /// HostPath base directory (if hostpath).
    pub host_path_base: Option<String>,
    /// Default mount options.
    pub mount_options: Vec<String>,
    /// Volume binding mode.
    pub volume_binding_mode: VolumeBindingMode,
    /// Reclaim policy.
    pub reclaim_policy: ReclaimPolicy,
    /// Allow volume expansion.
    pub allow_volume_expansion: bool,
}

/// Volume binding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VolumeBindingMode {
    /// Immediate binding (not recommended for WaitForFirstConsumer).
    Immediate,
    /// Wait for first consumer to be scheduled before binding.
    WaitForFirstConsumer,
}

/// Storage class desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageClassDesired {
    /// Storage class name.
    pub name: String,
    /// Provisioner.
    pub provisioner: String,
    /// Parameters.
    pub parameters: StorageClassParameters,
    /// Whether this is the default storage class.
    pub is_default: bool,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// StorageClass controller for managing storage classes and dynamic provisioning.
pub struct StorageClassController {
    /// Storage classes.
    storage_classes: RwLock<HashMap<String, StorageClassDesired>>,
    /// Default storage class name.
    default_class: RwLock<Option<String>>,
    /// Volume manager reference.
    volume_manager: Arc<VolumeManager>,
}

impl StorageClassController {
    /// Create a new storage class controller.
    pub fn new(volume_manager: Arc<VolumeManager>) -> Self {
        Self {
            storage_classes: RwLock::new(HashMap::new()),
            default_class: RwLock::new(None),
            volume_manager,
        }
    }

    /// Create a storage class.
    pub async fn create(&self, sc: StorageClassDesired) -> Result<()> {
        let mut classes = self.storage_classes.write().await;

        // Handle default
        if sc.is_default {
            let mut default = self.default_class.write().await;
            *default = Some(sc.name.clone());
        }

        classes.insert(sc.name.clone(), sc);
        Ok(())
    }

    /// Get a storage class.
    pub async fn get(&self, name: &str) -> Option<StorageClassDesired> {
        let classes = self.storage_classes.read().await;
        classes.get(name).cloned()
    }

    /// List all storage classes.
    pub async fn list(&self) -> Vec<StorageClassDesired> {
        let classes = self.storage_classes.read().await;
        classes.values().cloned().collect()
    }

    /// Delete a storage class.
    pub async fn delete(&self, name: &str) -> bool {
        let mut classes = self.storage_classes.write().await;
        let mut default = self.default_class.write().await;

        if let Some(current_default) = default.as_ref() {
            if current_default == name {
                *default = None;
            }
        }

        classes.remove(name).is_some()
    }

    /// Update a storage class.
    pub async fn update(&self, sc: StorageClassDesired) -> Result<()> {
        let mut classes = self.storage_classes.write().await;
        if classes.contains_key(&sc.name) {
            // Handle default
            if sc.is_default {
                let mut default = self.default_class.write().await;
                *default = Some(sc.name.clone());
            }
            classes.insert(sc.name.clone(), sc);
            Ok(())
        } else {
            Err(A3sError::Project(format!(
                "storage class {} not found",
                sc.name
            )))
        }
    }

    /// Get the default storage class.
    pub async fn get_default(&self) -> Option<StorageClassDesired> {
        let default = self.default_class.read().await;
        if let Some(name) = default.as_ref() {
            let classes = self.storage_classes.read().await;
            return classes.get(name).cloned();
        }
        None
    }

    /// Set a storage class as default.
    pub async fn set_default(&self, name: &str) -> Result<()> {
        let classes = self.storage_classes.read().await;
        if !classes.contains_key(name) {
            return Err(A3sError::Project(format!(
                "storage class {} not found",
                name
            )));
        }
        drop(classes);

        let mut default = self.default_class.write().await;
        *default = Some(name.to_string());
        Ok(())
    }

    /// Provision a volume dynamically for a PVC using the specified storage class.
    pub async fn provision_volume(
        &self,
        pvc: &PersistentVolumeClaimDesired,
    ) -> Result<PersistentVolumeDesired> {
        // Get the storage class
        let class_name = pvc
            .spec
            .storage_class_name
            .as_ref()
            .ok_or_else(|| A3sError::Project("PVC has no storage class".to_string()))?;

        let class = self
            .get(class_name)
            .await
            .ok_or_else(|| A3sError::Project(format!("storage class {} not found", class_name)))?;

        // Generate volume name
        let volume_name = format!("pvc-{}-{}", pvc.namespace, uuid::Uuid::new_v4());

        // Calculate requested storage
        let requested_storage = pvc
            .spec
            .resources
            .requests
            .get("storage")
            .copied()
            .unwrap_or(64 * 1024 * 1024 * 1024); // Default 64Gi

        // Create appropriate volume source based on storage type
        let source =
            match class.parameters.storage_type.as_str() {
                "hostpath" => {
                    let base = class
                        .parameters
                        .host_path_base
                        .clone()
                        .unwrap_or_else(|| "/var/lib/a3s/volumes".to_string());
                    let host_path = format!("{}/{}", base, volume_name);
                    VolumeSource::HostPath {
                        path: host_path,
                        type_: HostPathType::DirectoryOrCreate,
                    }
                }
                "emptydir" => VolumeSource::EmptyDir {},
                "nfs" => {
                    let server = class.parameters.nfs_server.clone().ok_or_else(|| {
                        A3sError::Project("NFS server not configured".to_string())
                    })?;
                    let path =
                        class.parameters.nfs_path.clone().ok_or_else(|| {
                            A3sError::Project("NFS path not configured".to_string())
                        })?;
                    VolumeSource::Nfs {
                        server,
                        path,
                        read_only: false,
                    }
                }
                _ => {
                    return Err(A3sError::Project(format!(
                        "unknown storage type: {}",
                        class.parameters.storage_type
                    )));
                }
            };

        let pv = PersistentVolumeDesired {
            name: volume_name.clone(),
            spec: PersistentVolumeSpec {
                capacity: requested_storage,
                access_modes: pvc.spec.access_modes.clone(),
                reclaim_policy: class.parameters.reclaim_policy,
                storage_class: Some(class_name.clone()),
                volume_mode: pvc.spec.volume_mode.clone(),
                mount_options: class.parameters.mount_options.clone(),
                source,
            },
            status: VolumeStatus::Bound,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claim_name: Some(format!("{}/{}", pvc.namespace, pvc.name)),
        };

        self.volume_manager.create_pv(pv.clone()).await?;

        Ok(pv)
    }

    /// Provision a volume using the default storage class.
    pub async fn provision_volume_default(
        &self,
        pvc: &PersistentVolumeClaimDesired,
    ) -> Result<PersistentVolumeDesired> {
        let _class_name = pvc
            .spec
            .storage_class_name
            .clone()
            .ok_or_else(|| A3sError::Project("PVC has no storage class name".to_string()))?;

        self.provision_volume(pvc).await
    }

    /// Check if a storage class exists.
    pub async fn exists(&self, name: &str) -> bool {
        let classes = self.storage_classes.read().await;
        classes.contains_key(name)
    }

    /// Get storage class for PVC (uses default if not specified).
    pub async fn get_class_for_pvc(
        &self,
        pvc: &PersistentVolumeClaimDesired,
    ) -> Result<Option<StorageClassDesired>> {
        if let Some(ref class_name) = pvc.spec.storage_class_name {
            let class = self.get(class_name).await.ok_or_else(|| {
                A3sError::Project(format!("storage class {} not found", class_name))
            })?;
            Ok(Some(class))
        } else {
            Ok(self.get_default().await)
        }
    }
}

/// Create a default hostpath storage class.
pub fn default_hostpath_storage_class() -> StorageClassDesired {
    StorageClassDesired {
        name: "hostpath".to_string(),
        provisioner: "a3s.io/hostpath".to_string(),
        parameters: StorageClassParameters {
            provisioner: "a3s.io/hostpath".to_string(),
            storage_type: "hostpath".to_string(),
            nfs_server: None,
            nfs_path: None,
            host_path_base: Some("/var/lib/a3s/volumes".to_string()),
            mount_options: vec![],
            volume_binding_mode: VolumeBindingMode::WaitForFirstConsumer,
            reclaim_policy: ReclaimPolicy::Delete,
            allow_volume_expansion: true,
        },
        is_default: false,
        labels: Default::default(),
        annotations: Default::default(),
        created_at: Utc::now(),
    }
}

/// Create a default emptydir storage class.
pub fn default_emptydir_storage_class() -> StorageClassDesired {
    StorageClassDesired {
        name: "emptydir".to_string(),
        provisioner: "a3s.io/emptydir".to_string(),
        parameters: StorageClassParameters {
            provisioner: "a3s.io/emptydir".to_string(),
            storage_type: "emptydir".to_string(),
            nfs_server: None,
            nfs_path: None,
            host_path_base: None,
            mount_options: vec![],
            volume_binding_mode: VolumeBindingMode::Immediate,
            reclaim_policy: ReclaimPolicy::Delete,
            allow_volume_expansion: false,
        },
        is_default: false,
        labels: Default::default(),
        annotations: Default::default(),
        created_at: Utc::now(),
    }
}

/// Create an NFS storage class.
pub fn nfs_storage_class(
    name: &str,
    nfs_server: &str,
    nfs_path: &str,
    is_default: bool,
) -> StorageClassDesired {
    StorageClassDesired {
        name: name.to_string(),
        provisioner: "a3s.io/nfs".to_string(),
        parameters: StorageClassParameters {
            provisioner: "a3s.io/nfs".to_string(),
            storage_type: "nfs".to_string(),
            nfs_server: Some(nfs_server.to_string()),
            nfs_path: Some(nfs_path.to_string()),
            host_path_base: None,
            mount_options: vec![],
            volume_binding_mode: VolumeBindingMode::Immediate,
            reclaim_policy: ReclaimPolicy::Delete,
            allow_volume_expansion: true,
        },
        is_default,
        labels: Default::default(),
        annotations: Default::default(),
        created_at: Utc::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_volume_manager() -> VolumeManager {
        let dir = std::env::temp_dir().join(format!("a3s-volume-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        VolumeManager::new(&dir)
    }

    #[tokio::test]
    async fn test_create_pv() {
        let manager = test_volume_manager();

        let pv = PersistentVolumeDesired {
            name: "test-pv".to_string(),
            spec: PersistentVolumeSpec {
                capacity: 10 * 1024 * 1024 * 1024,
                access_modes: vec![AccessMode::ReadWriteOnce],
                reclaim_policy: ReclaimPolicy::Retain,
                storage_class: Some("standard".to_string()),
                volume_mode: VolumeMode::Filesystem,
                mount_options: vec![],
                source: VolumeSource::EmptyDir {},
            },
            status: VolumeStatus::Available,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claim_name: None,
        };

        manager.create_pv(pv.clone()).await.unwrap();

        let retrieved = manager.get_pv("test-pv").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "test-pv");
    }

    #[tokio::test]
    async fn test_bind_unbind_volume() {
        let manager = test_volume_manager();

        let pv = PersistentVolumeDesired {
            name: "test-pv".to_string(),
            spec: PersistentVolumeSpec {
                capacity: 10 * 1024 * 1024 * 1024,
                access_modes: vec![AccessMode::ReadWriteOnce],
                reclaim_policy: ReclaimPolicy::Retain,
                storage_class: None,
                volume_mode: VolumeMode::Filesystem,
                mount_options: vec![],
                source: VolumeSource::EmptyDir {},
            },
            status: VolumeStatus::Available,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claim_name: None,
        };

        manager.create_pv(pv).await.unwrap();

        // Bind to a claim
        manager
            .bind_volume("test-pv", "test-claim", "default")
            .await
            .unwrap();

        let pv = manager.get_pv("test-pv").await.unwrap();
        assert_eq!(pv.status, VolumeStatus::Bound);
        assert_eq!(pv.claim_name, Some("test-claim".to_string()));

        // Unbind
        manager.unbind_volume("test-pv").await.unwrap();

        let pv = manager.get_pv("test-pv").await.unwrap();
        assert_eq!(pv.status, VolumeStatus::Available);
        assert!(pv.claim_name.is_none());
    }

    #[tokio::test]
    async fn test_find_matching_volume() {
        let manager = test_volume_manager();

        // Create volumes
        let pv1 = PersistentVolumeDesired {
            name: "pv-small".to_string(),
            spec: PersistentVolumeSpec {
                capacity: 5 * 1024 * 1024 * 1024,
                access_modes: vec![AccessMode::ReadWriteOnce],
                reclaim_policy: ReclaimPolicy::Retain,
                storage_class: Some("fast".to_string()),
                volume_mode: VolumeMode::Filesystem,
                mount_options: vec![],
                source: VolumeSource::EmptyDir {},
            },
            status: VolumeStatus::Available,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claim_name: None,
        };

        let pv2 = PersistentVolumeDesired {
            name: "pv-large".to_string(),
            spec: PersistentVolumeSpec {
                capacity: 100 * 1024 * 1024 * 1024,
                access_modes: vec![AccessMode::ReadWriteOnce, AccessMode::ReadOnlyMany],
                reclaim_policy: ReclaimPolicy::Retain,
                storage_class: Some("standard".to_string()),
                volume_mode: VolumeMode::Filesystem,
                mount_options: vec![],
                source: VolumeSource::EmptyDir {},
            },
            status: VolumeStatus::Available,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            claim_name: None,
        };

        manager.create_pv(pv1).await.unwrap();
        manager.create_pv(pv2).await.unwrap();

        // Find volume for claim requesting 20Gi
        let pvc = PersistentVolumeClaimDesired {
            name: "test-claim".to_string(),
            namespace: "default".to_string(),
            spec: PersistentVolumeClaimSpec {
                access_modes: vec![AccessMode::ReadWriteOnce],
                resources: ClaimResources {
                    requests: {
                        let mut m = HashMap::new();
                        m.insert("storage".to_string(), 20 * 1024 * 1024 * 1024);
                        m
                    },
                    limits: Default::default(),
                },
                storage_class_name: Some("standard".to_string()),
                volume_name: None,
                volume_mode: VolumeMode::Filesystem,
                selector: None,
            },
            status: VolumeStatus::Pending,
            volume_name: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let match_result = manager.find_matching_volume(&pvc).await;
        assert!(match_result.is_some());
        assert_eq!(match_result.unwrap(), "pv-large");
    }

    #[tokio::test]
    async fn test_pvc_lifecycle() {
        let manager = test_volume_manager();

        let pvc = PersistentVolumeClaimDesired {
            name: "test-pvc".to_string(),
            namespace: "default".to_string(),
            spec: PersistentVolumeClaimSpec {
                access_modes: vec![AccessMode::ReadWriteOnce],
                resources: ClaimResources {
                    requests: {
                        let mut m = HashMap::new();
                        m.insert("storage".to_string(), 1024 * 1024 * 1024);
                        m
                    },
                    limits: Default::default(),
                },
                storage_class_name: None,
                volume_name: None,
                volume_mode: VolumeMode::Filesystem,
                selector: None,
            },
            status: VolumeStatus::Pending,
            volume_name: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        manager.create_pvc(pvc.clone()).await.unwrap();

        let retrieved = manager.get_pvc("default", "test-pvc").await;
        assert!(retrieved.is_some());

        manager.delete_pvc("default", "test-pvc").await.unwrap();

        let retrieved = manager.get_pvc("default", "test-pvc").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_volume_attachment() {
        let manager = VolumeAttachmentManager::new();

        let attachment = VolumeAttachment {
            pod_id: "pod-1".to_string(),
            volume_name: "vol-1".to_string(),
            mount_path: "/mnt/data".to_string(),
            read_only: false,
            attached_at: Utc::now(),
        };

        manager.attach(attachment.clone()).await.unwrap();

        let attachments = manager.get_attachments("pod-1").await;
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0].volume_name, "vol-1");

        assert!(manager.is_attached("vol-1").await);

        manager.detach("pod-1", "vol-1").await.unwrap();

        let attachments = manager.get_attachments("pod-1").await;
        assert!(attachments.is_empty());
    }
}
