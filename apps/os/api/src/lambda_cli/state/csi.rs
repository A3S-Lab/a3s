//! A3sCSI - Container Storage Interface.
//!
//! A3sCSI provides persistent storage for containers, replacing Kubernetes CSI.
//! It is responsible for:
//! - Volume provisioning (create/delete)
//! - Volume attachment to nodes
//! - Volume mounting to containers
//! - Volume unmounting and detachment
//! - Snapshot management
//! - Capacity tracking
//!
//! A3sCSI supports multiple storage backends including local, network, and cloud storage.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::sync::RwLock;

/// CSI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiConfig {
    /// CSI driver name.
    pub driver_name: String,
    /// CSI version.
    pub version: String,
    /// Endpoint.
    pub endpoint: String,
    /// Volume directory.
    pub volume_dir: PathBuf,
    /// Default storage class.
    pub default_storage_class: String,
    /// Max volumes per node.
    pub max_volumes_per_node: u32,
    /// Enable snapshot support.
    pub enable_snapshot: bool,
    /// Enable capacity tracking.
    pub enable_capacity_tracking: bool,
}

impl Default for CsiConfig {
    fn default() -> Self {
        Self {
            driver_name: "a3s.csi.driver".to_string(),
            version: "1.0.0".to_string(),
            endpoint: "unix:///var/lib/a3s/csi.sock".to_string(),
            volume_dir: PathBuf::from("/var/lib/a3s/volumes"),
            default_storage_class: "local".to_string(),
            max_volumes_per_node: 16,
            enable_snapshot: true,
            enable_capacity_tracking: true,
        }
    }
}

/// Volume capability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeCapability {
    /// Access mode.
    pub access_mode: VolumeAccessMode,
    /// Mount options.
    pub mount_options: Vec<String>,
    /// Access type.
    pub access_type: AccessType,
}

/// Access mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum VolumeAccessMode {
    /// Single node read-write.
    SingleNodeRw,
    /// Single node read-only.
    SingleNodeRo,
    /// Multiple nodes read-write.
    MultiNodeRw,
    /// Multiple nodes read-only.
    MultiNodeRo,
    /// Single node read-write once.
    SingleNodeRwOnce,
}

/// Access type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AccessType {
    /// Mount access.
    Mount { fs_type: Option<String> },
    /// Block access.
    Block,
}

/// Volume content source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeContentSource {
    /// Source type.
    pub source_type: ContentSourceType,
    /// Volume ID (for clone).
    pub volume_id: Option<String>,
    /// Snapshot ID (for snapshot restore).
    pub snapshot_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentSourceType {
    /// Volume clone.
    Clone,
    /// Snapshot.
    Snapshot,
}

/// Create volume request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateVolumeRequest {
    /// Volume name.
    pub name: String,
    /// Capacity required.
    pub capacity: u64,
    /// Volume capabilities.
    pub capabilities: Vec<VolumeCapability>,
    /// Content source.
    pub content_source: Option<VolumeContentSource>,
    /// Parameters.
    pub parameters: HashMap<String, String>,
}

/// Volume.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    /// Volume ID.
    pub id: String,
    /// Volume name.
    pub name: String,
    /// Capacity in bytes.
    pub capacity: u64,
    /// Volume status.
    pub status: VolumeState,
    /// Accessible nodes.
    pub accessible_nodes: Vec<String>,
    /// Volume content source.
    pub content_source: Option<VolumeContentSource>,
    /// Parameters.
    pub parameters: HashMap<String, String>,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VolumeState {
    /// Volume is being created.
    Creating,
    /// Volume is created and ready.
    Created,
    /// Volume is being deleted.
    Deleting,
    /// Volume is deleted.
    Deleted,
    /// Volume has an error.
    Error,
}

/// CSI Volume attachment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsiVolumeAttachment {
    /// Attachment ID.
    pub id: String,
    /// Volume ID.
    pub volume_id: String,
    /// Node ID.
    pub node_id: String,
    /// Volume capability.
    pub capability: VolumeCapability,
    /// Attachment status.
    pub status: AttachmentStatus,
    /// Device path (block device path).
    pub device_path: Option<String>,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentStatus {
    /// Attaching.
    Attaching,
    /// Attached.
    Attached,
    /// Detaching.
    Detaching,
    /// Detached.
    Detached,
    /// Error.
    Error,
}

/// CSI Volume mount.
#[derive(Debug, Clone)]
pub struct CsiVolumeMount {
    /// Mount ID.
    pub id: String,
    /// Volume ID.
    pub volume_id: String,
    /// Container ID.
    pub container_id: String,
    /// Mount path.
    pub mount_path: String,
    /// Read only.
    pub read_only: bool,
    /// Mount options.
    pub mount_options: Vec<String>,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

/// Snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Snapshot ID.
    pub id: String,
    /// Source volume ID.
    pub source_volume_id: String,
    /// Snapshot size.
    pub size: u64,
    /// Snapshot status.
    pub status: SnapshotStatus,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotStatus {
    /// Creating.
    Creating,
    /// Ready.
    Ready,
    /// Deleting.
    Deleting,
    /// Error.
    Error,
}

/// Storage backend type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    /// Local storage.
    Local,
    /// NFS/Network storage.
    Network,
    /// iSCSI storage.
    Iscsi,
    /// Ceph/RBD storage.
    Ceph,
    /// Cloud block storage.
    Cloud,
}

/// Storage backend info.
#[derive(Debug, Clone)]
pub struct StorageBackendInfo {
    /// Backend type.
    pub backend_type: StorageBackend,
    /// Total capacity.
    pub total_capacity: u64,
    /// Used capacity.
    pub used_capacity: u64,
    /// Available capacity.
    pub available_capacity: u64,
    /// Volume count.
    pub volume_count: u32,
    /// Mount point.
    pub mount_point: Option<String>,
}

/// Node storage info.
#[derive(Debug, Clone)]
pub struct NodeStorageInfo {
    /// Node ID.
    pub node_id: String,
    /// Backends available on this node.
    pub backends: Vec<StorageBackendInfo>,
    /// Volumes attached to this node.
    pub attached_volumes: Vec<String>,
    /// Capacity tracking.
    pub total_capacity: u64,
    pub used_capacity: u64,
    pub available_capacity: u64,
}

/// A3sCSI - Container storage interface.
pub struct A3sCsi {
    /// Configuration.
    config: CsiConfig,
    /// Volumes.
    volumes: RwLock<HashMap<String, Volume>>,
    /// Attachments.
    attachments: RwLock<HashMap<String, CsiVolumeAttachment>>,
    /// Mounts.
    mounts: RwLock<HashMap<String, CsiVolumeMount>>,
    /// Snapshots.
    snapshots: RwLock<HashMap<String, Snapshot>>,
    /// Node storage info.
    node_storage: RwLock<HashMap<String, StorageBackendInfo>>,
    /// Running state.
    running: RwLock<bool>,
}

impl A3sCsi {
    /// Create a new CSI instance.
    pub async fn new(config: CsiConfig) -> Result<Self> {
        Ok(Self {
            config,
            volumes: RwLock::new(HashMap::new()),
            attachments: RwLock::new(HashMap::new()),
            mounts: RwLock::new(HashMap::new()),
            snapshots: RwLock::new(HashMap::new()),
            node_storage: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        })
    }

    /// Start the CSI.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Initialize storage backends
        let local_backend = StorageBackendInfo {
            backend_type: StorageBackend::Local,
            total_capacity: 100 * 1024 * 1024 * 1024, // 100GB
            used_capacity: 0,
            available_capacity: 100 * 1024 * 1024 * 1024,
            volume_count: 0,
            mount_point: Some(self.config.volume_dir.to_string_lossy().into_owned()),
        };

        let mut node_storage = self.node_storage.write().await;
        node_storage.insert("local".to_string(), local_backend);

        tracing::info!(
            driver = %self.config.driver_name,
            endpoint = %self.config.endpoint,
            volume_dir = %self.config.volume_dir.display(),
            "A3sCSI started"
        );

        Ok(())
    }

    /// Stop the CSI.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("A3sCSI stopped");
        Ok(())
    }

    /// Create a volume.
    pub async fn create_volume(&self, request: CreateVolumeRequest) -> Result<Volume> {
        if !*self.running.read().await {
            return Err(A3sError::Other("CSI not running".to_string()));
        }

        let volume_id = format!("vol-{}", uuid::Uuid::new_v4());

        let volume = Volume {
            id: volume_id.clone(),
            name: request.name.clone(),
            capacity: request.capacity,
            status: VolumeState::Created,
            accessible_nodes: vec![],
            content_source: request.content_source.clone(),
            parameters: request.parameters.clone(),
            created_at: Utc::now(),
        };

        let mut volumes = self.volumes.write().await;
        volumes.insert(volume_id.clone(), volume.clone());

        // Update backend capacity
        let mut node_storage = self.node_storage.write().await;
        if let Some(backend) = node_storage.get_mut("local") {
            backend.used_capacity += request.capacity;
            backend.available_capacity -= request.capacity;
            backend.volume_count += 1;
        }

        tracing::info!(
            volume_id = %volume_id,
            name = %request.name,
            capacity = %request.capacity,
            "Volume created"
        );

        Ok(volume)
    }

    /// Delete a volume.
    pub async fn delete_volume(&self, volume_id: &str) -> Result<()> {
        let mut volumes = self.volumes.write().await;

        if let Some(volume) = volumes.remove(volume_id) {
            // Update backend capacity
            let mut node_storage = self.node_storage.write().await;
            if let Some(backend) = node_storage.get_mut("local") {
                backend.used_capacity = backend.used_capacity.saturating_sub(volume.capacity);
                backend.available_capacity += volume.capacity;
                backend.volume_count = backend.volume_count.saturating_sub(1);
            }

            tracing::info!(volume_id = %volume_id, "Volume deleted");
        }

        Ok(())
    }

    /// Get volume.
    pub async fn get_volume(&self, volume_id: &str) -> Option<Volume> {
        let volumes = self.volumes.read().await;
        volumes.get(volume_id).cloned()
    }

    /// List volumes.
    pub async fn list_volumes(&self) -> Vec<Volume> {
        let volumes = self.volumes.read().await;
        volumes.values().cloned().collect()
    }

    /// Controller publish volume (attach to node).
    pub async fn controller_publish(
        &self,
        volume_id: &str,
        node_id: &str,
        capability: VolumeCapability,
    ) -> Result<CsiVolumeAttachment> {
        if !*self.running.read().await {
            return Err(A3sError::Other("CSI not running".to_string()));
        }

        let attachment_id = format!("att-{}", uuid::Uuid::new_v4());

        let attachment = CsiVolumeAttachment {
            id: attachment_id.clone(),
            volume_id: volume_id.to_string(),
            node_id: node_id.to_string(),
            capability,
            status: AttachmentStatus::Attached,
            device_path: Some(format!("/dev/a3s/{}", volume_id)),
            created_at: Utc::now(),
        };

        let mut attachments = self.attachments.write().await;
        attachments.insert(attachment_id, attachment.clone());

        tracing::info!(volume_id = %volume_id, node_id = %node_id, "Volume attached");

        Ok(attachment)
    }

    /// Controller unpublish volume (detach from node).
    pub async fn controller_unpublish(&self, volume_id: &str, node_id: &str) -> Result<()> {
        let mut attachments = self.attachments.write().await;

        // Find and remove attachment
        attachments.retain(|_, att| !(att.volume_id == volume_id && att.node_id == node_id));

        tracing::info!(volume_id = %volume_id, node_id = %node_id, "Volume detached");
        Ok(())
    }

    /// Node stage volume (prepare for mounting).
    pub async fn node_stage(&self, volume_id: &str, node_id: &str, mount_path: &str) -> Result<()> {
        // In real implementation, would mount the volume to a staging path
        tracing::info!(
            volume_id = %volume_id,
            node_id = %node_id,
            mount_path = %mount_path,
            "Volume staged"
        );
        Ok(())
    }

    /// Node unstage volume.
    pub async fn node_unstage(
        &self,
        volume_id: &str,
        node_id: &str,
        mount_path: &str,
    ) -> Result<()> {
        tracing::info!(
            volume_id = %volume_id,
            node_id = %node_id,
            mount_path = %mount_path,
            "Volume unstaged"
        );
        Ok(())
    }

    /// Node publish volume (mount to container).
    pub async fn node_publish(
        &self,
        volume_id: &str,
        container_id: &str,
        mount_path: &str,
        read_only: bool,
    ) -> Result<CsiVolumeMount> {
        let mount_id = format!("mnt-{}", uuid::Uuid::new_v4());

        let mount = CsiVolumeMount {
            id: mount_id.clone(),
            volume_id: volume_id.to_string(),
            container_id: container_id.to_string(),
            mount_path: mount_path.to_string(),
            read_only,
            mount_options: vec![],
            created_at: Utc::now(),
        };

        let mut mounts = self.mounts.write().await;
        mounts.insert(mount_id, mount.clone());

        tracing::info!(
            volume_id = %volume_id,
            container_id = %container_id,
            mount_path = %mount_path,
            "Volume mounted"
        );

        Ok(mount)
    }

    /// Node unpublish volume (unmount from container).
    pub async fn node_unpublish(
        &self,
        volume_id: &str,
        container_id: &str,
        mount_path: &str,
    ) -> Result<()> {
        let mut mounts = self.mounts.write().await;

        mounts.retain(|_, m| !(m.volume_id == volume_id && m.container_id == container_id));

        tracing::info!(
            volume_id = %volume_id,
            container_id = %container_id,
            mount_path = %mount_path,
            "Volume unmounted"
        );

        Ok(())
    }

    /// Create snapshot.
    pub async fn create_snapshot(
        &self,
        source_volume_id: &str,
        _name: Option<&str>,
    ) -> Result<Snapshot> {
        let snapshot_id = format!("snap-{}", uuid::Uuid::new_v4());

        // Get source volume to determine size
        let volume = self
            .get_volume(source_volume_id)
            .await
            .ok_or_else(|| A3sError::Other("Volume not found".to_string()))?;

        let snapshot = Snapshot {
            id: snapshot_id.clone(),
            source_volume_id: source_volume_id.to_string(),
            size: volume.capacity,
            status: SnapshotStatus::Ready,
            created_at: Utc::now(),
        };

        let mut snapshots = self.snapshots.write().await;
        snapshots.insert(snapshot_id.clone(), snapshot.clone());

        tracing::info!(
            snapshot_id = %snapshot_id,
            source_volume_id = %source_volume_id,
            "Snapshot created"
        );

        Ok(snapshot)
    }

    /// Delete snapshot.
    pub async fn delete_snapshot(&self, snapshot_id: &str) -> Result<()> {
        let mut snapshots = self.snapshots.write().await;
        snapshots.remove(snapshot_id);

        tracing::info!(snapshot_id = %snapshot_id, "Snapshot deleted");
        Ok(())
    }

    /// Get storage backend info.
    pub async fn get_backend_info(&self, backend_name: &str) -> Option<StorageBackendInfo> {
        let node_storage = self.node_storage.read().await;
        node_storage.get(backend_name).cloned()
    }

    /// List storage backends.
    pub async fn list_backends(&self) -> Vec<StorageBackendInfo> {
        let node_storage = self.node_storage.read().await;
        node_storage.values().cloned().collect()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get config.
    pub fn config(&self) -> &CsiConfig {
        &self.config
    }
}

impl Default for A3sCsi {
    fn default() -> Self {
        Self {
            config: CsiConfig::default(),
            volumes: RwLock::new(HashMap::new()),
            attachments: RwLock::new(HashMap::new()),
            mounts: RwLock::new(HashMap::new()),
            snapshots: RwLock::new(HashMap::new()),
            node_storage: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_csi_creation() {
        let csi = A3sCsi::new(CsiConfig::default()).await.unwrap();
        assert!(!csi.is_running().await);
    }

    #[tokio::test]
    async fn test_create_volume() {
        let csi = A3sCsi::new(CsiConfig::default()).await.unwrap();
        csi.start().await.unwrap();

        let request = CreateVolumeRequest {
            name: "test-volume".to_string(),
            capacity: 1024 * 1024 * 1024, // 1GB
            capabilities: vec![],
            content_source: None,
            parameters: HashMap::new(),
        };

        let volume = csi.create_volume(request).await.unwrap();
        assert_eq!(volume.name, "test-volume");
        assert_eq!(volume.capacity, 1024 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_delete_volume() {
        let csi = A3sCsi::new(CsiConfig::default()).await.unwrap();
        csi.start().await.unwrap();

        let request = CreateVolumeRequest {
            name: "test-volume".to_string(),
            capacity: 1024 * 1024 * 1024,
            capabilities: vec![],
            content_source: None,
            parameters: HashMap::new(),
        };

        let volume = csi.create_volume(request).await.unwrap();
        csi.delete_volume(&volume.id).await.unwrap();

        let found = csi.get_volume(&volume.id).await;
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_attach_detach() {
        let csi = A3sCsi::new(CsiConfig::default()).await.unwrap();
        csi.start().await.unwrap();

        let request = CreateVolumeRequest {
            name: "test-volume".to_string(),
            capacity: 1024 * 1024 * 1024,
            capabilities: vec![],
            content_source: None,
            parameters: HashMap::new(),
        };

        let volume = csi.create_volume(request).await.unwrap();

        let attachment = csi
            .controller_publish(
                &volume.id,
                "node-1",
                VolumeCapability {
                    access_mode: VolumeAccessMode::SingleNodeRw,
                    mount_options: vec![],
                    access_type: AccessType::Mount { fs_type: None },
                },
            )
            .await
            .unwrap();

        assert_eq!(attachment.node_id, "node-1");

        csi.controller_unpublish(&volume.id, "node-1")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_snapshot() {
        let csi = A3sCsi::new(CsiConfig::default()).await.unwrap();
        csi.start().await.unwrap();

        let request = CreateVolumeRequest {
            name: "test-volume".to_string(),
            capacity: 1024 * 1024 * 1024,
            capabilities: vec![],
            content_source: None,
            parameters: HashMap::new(),
        };

        let volume = csi.create_volume(request).await.unwrap();
        let snapshot = csi
            .create_snapshot(&volume.id, Some("test-snap"))
            .await
            .unwrap();

        assert_eq!(snapshot.source_volume_id, volume.id);

        csi.delete_snapshot(&snapshot.id).await.unwrap();
    }
}
