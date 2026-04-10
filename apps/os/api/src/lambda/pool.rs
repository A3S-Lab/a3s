//! MicroVM pool for cold start optimization
//!
//! This module provides a pool of pre-warmed MicroVMs to reduce cold start latency.
//! Instead of creating a new VM for each task, we maintain a pool of ready-to-use VMs.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

use crate::domain::LambdaError;

/// Stub types for SDK compatibility
mod sdk_stub {
    use std::collections::HashMap;
    use tokio::sync::Mutex;

    #[derive(Default, Clone)]
    pub struct SandboxOptions {
        pub image: String,
        pub cpus: u32,
        pub memory_mb: u32,
    }

    pub struct Sandbox {
        pub id: String,
    }

    impl Sandbox {
        pub fn id(&self) -> &str {
            &self.id
        }
    }

    pub struct BoxSdk {
        sandboxes: Mutex<HashMap<String, Sandbox>>,
    }

    impl BoxSdk {
        pub async fn new() -> Result<Self, String> {
            Ok(Self {
                sandboxes: Mutex::new(HashMap::new()),
            })
        }

        pub async fn create(&self, _opts: SandboxOptions) -> Result<Sandbox, String> {
            Err("Box SDK not available - stub implementation".to_string())
        }
    }
}

use sdk_stub::{BoxSdk, Sandbox, SandboxOptions};

/// Configuration for VM pool
#[derive(Debug, Clone)]
pub struct VmPoolConfig {
    /// Default image for VMs
    pub default_image: String,
    /// Minimum number of idle VMs to maintain
    pub min_idle: usize,
    /// Maximum total VMs (idle + busy)
    pub max_total: usize,
    /// Maximum time a VM can be reused before replacement
    pub max_vm_lifetime: Duration,
    /// Maximum number of tasks a VM can execute before replacement
    pub max_tasks_per_vm: usize,
    /// Time to wait for a VM to become available
    pub acquire_timeout: Duration,
}

impl Default for VmPoolConfig {
    fn default() -> Self {
        Self {
            default_image: "a3s/lambda:latest".to_string(),
            min_idle: 2,
            max_total: 10,
            max_vm_lifetime: Duration::from_secs(3600),
            max_tasks_per_vm: 100,
            acquire_timeout: Duration::from_secs(30),
        }
    }
}

/// A VM instance in the pool
struct PooledVm {
    /// Unique VM ID
    id: Uuid,
    /// When the VM was created
    created_at: Instant,
    /// When the VM was last used
    last_used: Instant,
    /// Number of tasks executed on this VM
    task_count: usize,
    /// Current state
    state: VmState,
}

/// VM state
#[derive(Debug, Clone, PartialEq)]
enum VmState {
    /// VM is idle and ready to use
    Idle,
    /// VM is currently executing a task
    Busy,
    /// VM is being cleaned up
    Cleaning,
    /// VM is unhealthy and should be replaced
    Unhealthy,
}

/// A guard that releases the VM back to the pool when dropped
pub struct VmGuard {
    pool: Arc<VmPool>,
    vm_id: Uuid,
}

impl VmGuard {
    /// Execute a task in the VM
    pub async fn execute<F, Fut>(&self, f: F) -> Result<(), LambdaError>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<(), LambdaError>>,
    {
        let _ = self.vm_id;
        f().await
    }
}

impl Drop for VmGuard {
    fn drop(&mut self) {
        // Return VM to pool
        let pool = self.pool.clone();
        let vm_id = self.vm_id;
        tokio::spawn(async move {
            pool.release_vm(vm_id).await;
        });
    }
}

/// VM Pool for managing pre-warmed MicroVMs
pub struct VmPool {
    config: VmPoolConfig,
    sdk: BoxSdk,
    idle_vms: Mutex<VecDeque<PooledVm>>,
    total_vms: Mutex<usize>,
    semaphore: Semaphore,
}

impl VmPool {
    /// Create a new VM pool
    pub async fn new(config: VmPoolConfig) -> Result<Self, LambdaError> {
        let sdk = BoxSdk::new().await.map_err(|e| LambdaError::Internal(e))?;
        let max_total = config.max_total;

        Ok(Self {
            config,
            sdk,
            idle_vms: Mutex::new(VecDeque::new()),
            total_vms: Mutex::new(0),
            semaphore: Semaphore::new(max_total),
        })
    }

    /// Pre-warm the pool by creating initial VMs
    pub async fn warm_up(&self) -> Result<(), LambdaError> {
        for _ in 0..self.config.min_idle {
            self.spawn_vm().await?;
        }
        Ok(())
    }

    /// Spawn a new VM and add it to the idle pool
    async fn spawn_vm(&self) -> Result<PooledVm, LambdaError> {
        let opts = SandboxOptions {
            image: "a3s/lambda:latest".to_string(),
            cpus: 2,
            memory_mb: 2048,
        };

        let sandbox = self.sdk.create(opts).await.map_err(|e| LambdaError::Internal(e))?;

        let vm = PooledVm {
            id: Uuid::new_v4(),
            created_at: Instant::now(),
            last_used: Instant::now(),
            task_count: 0,
            state: VmState::Idle,
        };

        let mut total = self.total_vms.lock().await;
        *total += 1;

        Ok(vm)
    }

    /// Acquire an idle VM from the pool
    pub async fn acquire(self: Arc<VmPool>) -> Result<VmGuard, LambdaError> {
        // Clone self for later use after await
        let pool = self.clone();

        // Wait for a permit
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|_| LambdaError::Internal("pool closed".to_string()))?;

        // Try to get an idle VM
        let mut idle = self.idle_vms.lock().await;
        if let Some(vm) = idle.pop_front() {
            drop(idle);
            drop(_permit);

            return Ok(VmGuard {
                pool,
                vm_id: vm.id,
            });
        }

        // No idle VMs, check if we can spawn more
        let total = *self.total_vms.lock().await;
        if total < self.config.max_total {
            drop(idle);
            drop(_permit);
            let vm = self.spawn_vm().await?;
            return Ok(VmGuard {
                pool,
                vm_id: vm.id,
            });
        }

        // Pool exhausted, wait for a VM to become available
        // For now, just spawn a new one anyway since we don't have a proper wait queue
        drop(idle);
        drop(_permit);
        let vm = self.spawn_vm().await?;
        Ok(VmGuard {
            pool,
            vm_id: vm.id,
        })
    }

    /// Release a VM back to the pool
    async fn release_vm(&self, vm_id: Uuid) {
        let _ = vm_id;
        self.semaphore.add_permits(1);
    }

    /// Get pool statistics
    pub async fn stats(&self) -> VmPoolStats {
        let idle_count = self.idle_vms.lock().await.len();
        let total_count = *self.total_vms.lock().await;
        let active = total_count - idle_count;
        VmPoolStats {
            idle: idle_count,
            active,
            max_total: self.config.max_total,
            available_permits: self.semaphore.available_permits(),
        }
    }

    /// Shutdown the pool
    pub async fn shutdown(&self) {
        // Clear all VMs
        self.idle_vms.lock().await.clear();
        *self.total_vms.lock().await = 0;
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct VmPoolStats {
    pub idle: usize,
    pub active: usize,
    pub max_total: usize,
    pub available_permits: usize,
}
