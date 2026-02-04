# A3S Box CRI Runtime Implementation Plan

> **Decision**: Hybrid Architecture - OCI-compatible image format externally, libkrun microVM internally

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  Kubernetes Cluster                                             │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  kubelet                                                  │ │
│  └───────────────────────────────────────────────────────────┘ │
│                          │ CRI (gRPC)                           │
│                          ▼                                       │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  a3s-box-cri-runtime                                      │ │
│  │  ┌─────────────────────────────────────────────────────┐ │ │
│  │  │  CRI Service Layer                                  │ │ │
│  │  │  - RuntimeService (Pod/Container lifecycle)         │ │ │
│  │  │  - ImageService (OCI image management)              │ │ │
│  │  └─────────────────────────────────────────────────────┘ │ │
│  │                          │                                │ │
│  │                          ▼                                │ │
│  │  ┌─────────────────────────────────────────────────────┐ │ │
│  │  │  OCI Adapter Layer                                  │ │ │
│  │  │  - OCI image parsing                                │ │ │
│  │  │  - rootfs extraction                                │ │ │
│  │  │  - Configuration mapping                            │ │ │
│  │  └─────────────────────────────────────────────────────┘ │ │
│  │                          │                                │ │
│  │                          ▼                                │ │
│  │  ┌─────────────────────────────────────────────────────┐ │ │
│  │  │  a3s-box-runtime (Core)                             │ │ │
│  │  │  - libkrun (microVM)                                │ │ │
│  │  │  - Box lifecycle management                         │ │ │
│  │  │  - Session management                               │ │ │
│  │  └─────────────────────────────────────────────────────┘ │ │
│  └───────────────────────────────────────────────────────────┘ │
│                          │                                       │
│                          ▼                                       │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │  microVM Instances                                        │ │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐                  │ │
│  │  │ Box 1   │  │ Box 2   │  │ Box 3   │                  │ │
│  │  │ (VM)    │  │ (VM)    │  │ (VM)    │                  │ │
│  │  └─────────┘  └─────────┘  └─────────┘                  │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Core Design Principles

1. **External OCI Compatibility** - Use standard OCI image format, compatible with K8s ecosystem
2. **Internal microVM Isolation** - Maintain libkrun microVM hardware-level isolation
3. **Incremental Implementation** - Implement in phases, each phase independently deliverable
4. **Preserve Core Value** - Don't sacrifice A3S Box's security and isolation

## Implementation Phases

### Phase 1: OCI Image Support (2-3 weeks)

**Goal**: Enable A3S Box to start from OCI images

#### 1.1 OCI Image Format Definition

```dockerfile
# Dockerfile for a3s-box-code
FROM scratch

# Add minimal rootfs
ADD rootfs.tar.gz /

# Add a3s-box-code binary
COPY a3s-box-code /usr/local/bin/
COPY a3s-box-agent /usr/local/bin/

# A3S Box specific labels
LABEL a3s.agent.kind="a3s_code"
LABEL a3s.agent.version="0.1.0"
LABEL a3s.agent.entrypoint="/usr/local/bin/a3s-box-code"
LABEL a3s.agent.listen="vsock://3:4088"

# Standard OCI labels
LABEL org.opencontainers.image.title="A3S Code Agent"
LABEL org.opencontainers.image.description="A3S Box Coding Agent"
LABEL org.opencontainers.image.version="0.1.0"

# Entrypoint (executed in microVM)
ENTRYPOINT ["/usr/local/bin/a3s-box-code"]
CMD ["--listen", "vsock://3:4088"]
```

#### 1.2 OCI Image Parser

```rust
// src/runtime/oci/mod.rs
pub mod image;
pub mod manifest;
pub mod config;

// src/runtime/oci/image.rs
use oci_spec::image::{ImageManifest, ImageConfiguration};

pub struct OciImage {
    manifest: ImageManifest,
    config: ImageConfiguration,
    layers: Vec<PathBuf>,
}

impl OciImage {
    /// Pull OCI image from image reference
    pub async fn pull(image_ref: &str) -> Result<Self> {
        let manifest = Self::fetch_manifest(image_ref).await?;
        let config = Self::fetch_config(&manifest).await?;
        let layers = Self::fetch_layers(&manifest).await?;

        Ok(Self { manifest, config, layers })
    }

    /// Extract rootfs
    pub fn extract_rootfs(&self, target_dir: &Path) -> Result<()> {
        for layer in &self.layers {
            Self::extract_layer(layer, target_dir)?;
        }
        Ok(())
    }

    /// Get A3S Agent configuration
    pub fn get_agent_config(&self) -> Result<AgentConfig> {
        let labels = &self.config.config().labels();

        Ok(AgentConfig {
            kind: labels.get("a3s.agent.kind")
                .ok_or(BoxError::InvalidImage("missing a3s.agent.kind"))?,
            version: labels.get("a3s.agent.version").cloned(),
            entrypoint: labels.get("a3s.agent.entrypoint").cloned(),
            ..Default::default()
        })
    }
}
```

#### 1.3 Integration with Box Runtime

```rust
// src/runtime/box_manager.rs
impl BoxManager {
    /// Create Box from OCI image
    pub async fn create_box_from_oci_image(
        &self,
        image_ref: &str,
        config: BoxConfig,
    ) -> Result<Box> {
        // 1. Pull OCI image
        let oci_image = OciImage::pull(image_ref).await?;

        // 2. Extract rootfs
        let rootfs_dir = self.prepare_rootfs_dir(&config.box_id)?;
        oci_image.extract_rootfs(&rootfs_dir)?;

        // 3. Get Agent configuration
        let agent_config = oci_image.get_agent_config()?;

        // 4. Create Box (using existing libkrun logic)
        let box_config = BoxConfig {
            coding_agent: agent_config,
            ..config
        };

        self.create_box_from_rootfs(box_config, rootfs_dir).await
    }
}
```

### Phase 2: CRI RuntimeService Implementation (3-4 weeks)

**Goal**: Implement CRI RuntimeService interface

#### 2.1 CRI Service Structure

```rust
// src/cri/mod.rs
pub mod runtime_service;
pub mod image_service;
pub mod server;

// src/cri/runtime_service.rs
use k8s_cri::v1::runtime_service_server::{RuntimeService, RuntimeServiceServer};
use k8s_cri::v1::*;

pub struct A3sBoxRuntimeService {
    box_manager: Arc<BoxManager>,
    pod_sandbox_map: Arc<RwLock<HashMap<String, PodSandbox>>>,
    container_map: Arc<RwLock<HashMap<String, Container>>>,
}

#[tonic::async_trait]
impl RuntimeService for A3sBoxRuntimeService {
    async fn version(
        &self,
        _request: Request<VersionRequest>,
    ) -> Result<Response<VersionResponse>, Status> {
        Ok(Response::new(VersionResponse {
            version: "0.1.0".to_string(),
            runtime_name: "a3s-box".to_string(),
            runtime_version: "0.1.0".to_string(),
            runtime_api_version: "v1".to_string(),
        }))
    }

    async fn run_pod_sandbox(
        &self,
        request: Request<RunPodSandboxRequest>,
    ) -> Result<Response<RunPodSandboxResponse>, Status> {
        let req = request.into_inner();
        let config = req.config.ok_or_else(|| {
            Status::invalid_argument("missing pod sandbox config")
        })?;

        // Create BoxConfig from PodSandboxConfig
        let box_config = self.pod_config_to_box_config(&config)?;

        // Create Box instance (as Pod Sandbox)
        let box_instance = self.box_manager
            .create_box(box_config)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let pod_id = box_instance.id().to_string();

        // Save Pod Sandbox info
        let pod_sandbox = PodSandbox {
            id: pod_id.clone(),
            metadata: config.metadata,
            state: PodSandboxState::Ready,
            created_at: SystemTime::now(),
            box_instance,
        };

        self.pod_sandbox_map.write().await.insert(pod_id.clone(), pod_sandbox);

        Ok(Response::new(RunPodSandboxResponse {
            pod_sandbox_id: pod_id,
        }))
    }

    async fn create_container(
        &self,
        request: Request<CreateContainerRequest>,
    ) -> Result<Response<CreateContainerResponse>, Status> {
        let req = request.into_inner();
        let pod_id = req.pod_sandbox_id;
        let config = req.config.ok_or_else(|| {
            Status::invalid_argument("missing container config")
        })?;

        // Get Pod Sandbox (Box Instance)
        let pod_sandbox = self.pod_sandbox_map.read().await
            .get(&pod_id)
            .ok_or_else(|| Status::not_found("pod sandbox not found"))?
            .clone();

        // Create Session in Box (as Container)
        let session_id = pod_sandbox.box_instance
            .create_session()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        // Save Container info
        let container = Container {
            id: session_id.clone(),
            pod_sandbox_id: pod_id,
            metadata: config.metadata,
            image: config.image,
            state: ContainerState::Created,
            created_at: SystemTime::now(),
        };

        self.container_map.write().await.insert(session_id.clone(), container);

        Ok(Response::new(CreateContainerResponse {
            container_id: session_id,
        }))
    }

    // Implement other CRI methods...
}
```

#### 2.2 Configuration Mapping

```rust
// src/cri/config_mapper.rs
impl A3sBoxRuntimeService {
    fn pod_config_to_box_config(
        &self,
        pod_config: &PodSandboxConfig,
    ) -> Result<BoxConfig> {
        let metadata = pod_config.metadata.as_ref()
            .ok_or_else(|| BoxError::InvalidConfig("missing metadata"))?;

        // Read A3S Box configuration from Pod annotations
        let annotations = &pod_config.annotations;
        let agent_kind = annotations.get("a3s.box/agent-kind")
            .unwrap_or(&"a3s_code".to_string())
            .clone();
        let agent_image = annotations.get("a3s.box/agent-image");

        // Read resource limits from Linux config
        let resources = if let Some(linux) = &pod_config.linux {
            ResourceConfig {
                memory: linux.resources.as_ref()
                    .and_then(|r| r.memory_limit_in_bytes)
                    .unwrap_or(2 * 1024 * 1024 * 1024),
                cpus: linux.resources.as_ref()
                    .and_then(|r| r.cpu_quota)
                    .map(|q| (q / 100000) as u32)
                    .unwrap_or(2),
                ..Default::default()
            }
        } else {
            ResourceConfig::default()
        };

        Ok(BoxConfig {
            box_id: Some(metadata.uid.clone()),
            coding_agent: AgentConfig {
                kind: agent_kind,
                image: agent_image.cloned(),
                ..Default::default()
            },
            resources,
            ..Default::default()
        })
    }
}
```

### Phase 3: CRI ImageService Implementation (2-3 weeks)

**Goal**: Implement CRI ImageService interface

```rust
// src/cri/image_service.rs
use k8s_cri::v1::image_service_server::{ImageService, ImageServiceServer};

pub struct A3sBoxImageService {
    image_store: Arc<RwLock<HashMap<String, OciImage>>>,
    cache_dir: PathBuf,
}

#[tonic::async_trait]
impl ImageService for A3sBoxImageService {
    async fn list_images(
        &self,
        request: Request<ListImagesRequest>,
    ) -> Result<Response<ListImagesResponse>, Status> {
        let images = self.image_store.read().await;
        let image_list = images.values()
            .map(|img| Image {
                id: img.id().to_string(),
                repo_tags: img.repo_tags().to_vec(),
                size: img.size(),
                ..Default::default()
            })
            .collect();

        Ok(Response::new(ListImagesResponse {
            images: image_list,
        }))
    }

    async fn pull_image(
        &self,
        request: Request<PullImageRequest>,
    ) -> Result<Response<PullImageResponse>, Status> {
        let req = request.into_inner();
        let image_ref = req.image.ok_or_else(|| {
            Status::invalid_argument("missing image spec")
        })?.image;

        // Pull OCI image
        let oci_image = OciImage::pull(&image_ref)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let image_id = oci_image.id().to_string();

        // Save to image store
        self.image_store.write().await.insert(image_id.clone(), oci_image);

        Ok(Response::new(PullImageResponse {
            image_ref: image_id,
        }))
    }

    async fn remove_image(
        &self,
        request: Request<RemoveImageRequest>,
    ) -> Result<Response<RemoveImageResponse>, Status> {
        let image_ref = request.into_inner().image.ok_or_else(|| {
            Status::invalid_argument("missing image spec")
        })?.image;

        self.image_store.write().await.remove(&image_ref);

        Ok(Response::new(RemoveImageResponse {}))
    }
}
```

### Phase 4: Deployment and Testing (2-3 weeks)

#### 4.1 RuntimeClass Configuration

```yaml
# runtime-class.yaml
apiVersion: node.k8s.io/v1
kind: RuntimeClass
metadata:
  name: a3s-box
handler: a3s-box
scheduling:
  nodeSelector:
    a3s.box/enabled: "true"
  tolerations:
  - key: a3s.box/runtime
    operator: Exists
    effect: NoSchedule
```

#### 4.2 DaemonSet Deployment

```yaml
# a3s-box-cri-daemonset.yaml
apiVersion: apps/v1
kind: DaemonSet
metadata:
  name: a3s-box-cri-runtime
  namespace: kube-system
spec:
  selector:
    matchLabels:
      app: a3s-box-cri-runtime
  template:
    metadata:
      labels:
        app: a3s-box-cri-runtime
    spec:
      hostNetwork: true
      hostPID: true
      nodeSelector:
        a3s.box/enabled: "true"
      containers:
      - name: a3s-box-cri-runtime
        image: ghcr.io/a3s-box/cri-runtime:v0.1.0
        securityContext:
          privileged: true
        volumeMounts:
        - name: cri-socket
          mountPath: /var/run/a3s-box
        - name: dev-kvm
          mountPath: /dev/kvm
        - name: image-cache
          mountPath: /var/lib/a3s-box/images
        env:
        - name: CRI_SOCKET_PATH
          value: /var/run/a3s-box/a3s-box.sock
        resources:
          limits:
            memory: 4Gi
            cpu: 2
      volumes:
      - name: cri-socket
        hostPath:
          path: /var/run/a3s-box
          type: DirectoryOrCreate
      - name: dev-kvm
        hostPath:
          path: /dev/kvm
      - name: image-cache
        hostPath:
          path: /var/lib/a3s-box/images
          type: DirectoryOrCreate
```

#### 4.3 kubelet Configuration

```yaml
# /var/lib/kubelet/config.yaml
apiVersion: kubelet.config.k8s.io/v1beta1
kind: KubeletConfiguration
containerRuntimeEndpoint: unix:///var/run/a3s-box/a3s-box.sock
imageServiceEndpoint: unix:///var/run/a3s-box/a3s-box.sock
```

#### 4.4 Test Pod

```yaml
# test-pod.yaml
apiVersion: v1
kind: Pod
metadata:
  name: test-a3s-box
spec:
  runtimeClassName: a3s-box
  containers:
  - name: app
    image: ghcr.io/a3s-box/a3s-code:v0.1.0
    command: ["/usr/local/bin/a3s-box-code"]
    args: ["--listen", "vsock://3:4088"]
```

## Data Flow

```
1. kubectl apply -f pod.yaml
   ↓
2. API Server → Scheduler → kubelet
   ↓
3. kubelet → CRI (RunPodSandbox)
   ↓
4. a3s-box-cri-runtime → ImageService.PullImage
   ↓
5. OCI Image → extract rootfs
   ↓
6. a3s-box-runtime → libkrun.create_vm(rootfs)
   ↓
7. microVM started with a3s-box-code
   ↓
8. kubelet → CRI (CreateContainer)
   ↓
9. a3s-box-runtime → box.create_session()
   ↓
10. Session created in microVM
```

## Dependencies

### Rust Crates

```toml
[dependencies]
# CRI
tonic = "0.10"
prost = "0.12"
k8s-cri = "0.7"

# OCI
oci-spec = "0.6"
oci-distribution = "0.10"
containerd-client = "0.4"

# Existing dependencies
a3s-box-core = { path = "../core" }
a3s-box-runtime = { path = "../runtime" }
```

### External Tools

- **containerd**: Used for OCI image pulling and management
- **skopeo**: Alternative image tool
- **crictl**: CRI testing tool

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oci_image_pull() {
        let image = OciImage::pull("ghcr.io/a3s-box/a3s-code:v0.1.0")
            .await
            .unwrap();
        assert_eq!(image.get_agent_config().unwrap().kind, "a3s_code");
    }

    #[tokio::test]
    async fn test_cri_run_pod_sandbox() {
        let service = A3sBoxRuntimeService::new();
        let request = RunPodSandboxRequest {
            config: Some(PodSandboxConfig {
                metadata: Some(PodSandboxMetadata {
                    name: "test-pod".to_string(),
                    uid: "test-uid".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        };

        let response = service.run_pod_sandbox(Request::new(request))
            .await
            .unwrap();
        assert!(!response.into_inner().pod_sandbox_id.is_empty());
    }
}
```

### Integration Tests

```bash
# Test with crictl
crictl --runtime-endpoint unix:///var/run/a3s-box/a3s-box.sock version
crictl --runtime-endpoint unix:///var/run/a3s-box/a3s-box.sock pull ghcr.io/a3s-box/a3s-code:v0.1.0
crictl --runtime-endpoint unix:///var/run/a3s-box/a3s-box.sock runp pod-config.json
crictl --runtime-endpoint unix:///var/run/a3s-box/a3s-box.sock create <pod-id> container-config.json pod-config.json
```

## Performance Optimization

### Image Cache

```rust
// src/cri/image_cache.rs
pub struct ImageCache {
    cache_dir: PathBuf,
    lru: LruCache<String, OciImage>,
}

impl ImageCache {
    pub async fn get_or_pull(&mut self, image_ref: &str) -> Result<OciImage> {
        // 1. Check memory cache
        if let Some(image) = self.lru.get(image_ref) {
            return Ok(image.clone());
        }

        // 2. Check disk cache
        let cache_path = self.cache_dir.join(Self::image_hash(image_ref));
        if cache_path.exists() {
            let image = OciImage::load_from_cache(&cache_path)?;
            self.lru.put(image_ref.to_string(), image.clone());
            return Ok(image);
        }

        // 3. Pull image
        let image = OciImage::pull(image_ref).await?;

        // 4. Save to cache
        image.save_to_cache(&cache_path)?;
        self.lru.put(image_ref.to_string(), image.clone());

        Ok(image)
    }
}
```

### Box Instance Pool

```rust
// src/runtime/box_pool.rs
pub struct BoxPool {
    pool: Vec<Box>,
    max_size: usize,
}

impl BoxPool {
    pub async fn get_or_create(&mut self, config: BoxConfig) -> Result<Box> {
        // Try to get from pool
        if let Some(box_instance) = self.pool.pop() {
            box_instance.reconfigure(config).await?;
            return Ok(box_instance);
        }

        // Create new instance
        BoxManager::create_box(config).await
    }

    pub async fn return_box(&mut self, box_instance: Box) {
        if self.pool.len() < self.max_size {
            box_instance.reset().await.ok();
            self.pool.push(box_instance);
        }
    }
}
```

## Timeline

| Phase | Duration | Deliverables |
|-------|----------|--------------|
| Phase 1 | 2-3 weeks | OCI Image Support |
| Phase 2 | 3-4 weeks | CRI RuntimeService |
| Phase 3 | 2-3 weeks | CRI ImageService |
| Phase 4 | 2-3 weeks | Deployment and Testing |
| **Total** | **9-13 weeks** | **Complete CRI Runtime** |

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| CRI interface complexity | High | Reference containerd/CRI-O implementations |
| OCI image compatibility | Medium | Use standard libraries, thorough testing |
| Performance issues | Medium | Implement caching and pooling |
| Nested virtualization limitations | High | Document requirements, provide cloud environment configs |

## Next Steps

1. [ ] Create `src/cri/` directory structure
2. [ ] Implement OCI image parser
3. [ ] Write unit tests
4. [ ] Build first OCI image
5. [ ] Test starting Box from OCI image

---

**Status**: Implementation Plan
**Decision**: Hybrid Architecture (Option B)
**Last Updated**: 2026-02-04
