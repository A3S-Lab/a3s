//! TEE runtime self-detection and sealed storage.
//!
//! When SafeClaw runs inside an A3S Box MicroVM with AMD SEV-SNP enabled,
//! `TeeRuntime` detects the hardware TEE and provides:
//!
//! - **Self-detection**: Check `/dev/sev-guest` and CPUID for SEV-SNP
//! - **Sealed storage**: Encrypt/decrypt data bound to the TEE measurement
//! - **Attestation**: Generate attestation reports for remote verification
//! - **Security level**: Report the actual protection level to clients
//!
//! When no TEE hardware is detected, the runtime gracefully degrades to
//! `ProcessOnly` security level — no errors, no panics, just honest reporting.

use crate::error::{Error, Result};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::sealed::SealedStorage;
use super::SecurityLevel;

/// Runtime TEE environment detected at startup.
///
/// Unlike the old `TeeOrchestrator` which booted VMs from the host side,
/// `TeeRuntime` runs *inside* the guest and detects its own security level.
#[derive(Debug)]
pub struct TeeRuntime {
    /// Detected security level
    level: SecurityLevel,
    /// Sealed storage (available when TEE hardware is active)
    sealed: Option<Arc<SealedStorage>>,
    /// Whether attestation is available
    attestation_available: bool,
    /// Runtime state
    state: Arc<RwLock<RuntimeState>>,
}

/// Runtime state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    /// Not yet initialized
    Uninitialized,
    /// Running normally
    Active,
    /// Shutting down
    ShuttingDown,
}

/// Detection result from probing the environment.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Detected security level
    pub level: SecurityLevel,
    /// Whether `/dev/sev-guest` is accessible
    pub sev_guest_device: bool,
    /// Whether CPUID reports SEV-SNP support
    pub cpuid_sev_snp: bool,
    /// Whether we're inside a MicroVM (virtio/MMIO heuristic)
    pub inside_vm: bool,
    /// Human-readable summary
    pub summary: String,
}

impl TeeRuntime {
    /// Detect the TEE environment and create the runtime.
    ///
    /// This is the primary constructor. It probes the system for:
    /// 1. AMD SEV-SNP hardware (`/dev/sev-guest`)
    /// 2. VM environment (DMI/CPUID heuristics)
    /// 3. Process-only fallback
    ///
    /// Never fails — always returns a valid runtime with the detected level.
    pub async fn detect() -> Self {
        let result = Self::probe_environment().await;

        tracing::info!(
            level = %result.level,
            sev_guest = result.sev_guest_device,
            cpuid_snp = result.cpuid_sev_snp,
            inside_vm = result.inside_vm,
            "TEE runtime detection: {}",
            result.summary,
        );

        let sealed = if result.level == SecurityLevel::TeeHardware {
            match SealedStorage::new().await {
                Ok(storage) => {
                    tracing::info!("Sealed storage initialized");
                    Some(Arc::new(storage))
                }
                Err(e) => {
                    tracing::warn!("Sealed storage unavailable: {e}");
                    None
                }
            }
        } else {
            None
        };

        let attestation_available =
            result.level == SecurityLevel::TeeHardware && result.sev_guest_device;

        Self {
            level: result.level,
            sealed,
            attestation_available,
            state: Arc::new(RwLock::new(RuntimeState::Active)),
        }
    }

    /// Create a runtime with a known security level (for testing).
    #[cfg(test)]
    pub fn with_level(level: SecurityLevel) -> Self {
        Self {
            level,
            sealed: None,
            attestation_available: false,
            state: Arc::new(RwLock::new(RuntimeState::Active)),
        }
    }

    /// Create a process-only runtime (no TEE, no VM).
    pub fn process_only() -> Self {
        Self {
            level: SecurityLevel::ProcessOnly,
            sealed: None,
            attestation_available: false,
            state: Arc::new(RwLock::new(RuntimeState::Active)),
        }
    }

    // =========================================================================
    // Accessors
    // =========================================================================

    /// Current security level.
    pub fn security_level(&self) -> SecurityLevel {
        self.level
    }

    /// Whether TEE hardware is active.
    pub fn is_tee_active(&self) -> bool {
        self.level == SecurityLevel::TeeHardware
    }

    /// Whether sealed storage is available.
    pub fn has_sealed_storage(&self) -> bool {
        self.sealed.is_some()
    }

    /// Whether attestation reports can be generated.
    pub fn has_attestation(&self) -> bool {
        self.attestation_available
    }

    /// Get the sealed storage handle.
    pub fn sealed_storage(&self) -> Option<&Arc<SealedStorage>> {
        self.sealed.as_ref()
    }

    /// Current runtime state.
    pub async fn state(&self) -> RuntimeState {
        *self.state.read().await
    }

    // =========================================================================
    // Lifecycle
    // =========================================================================

    /// Shutdown the TEE runtime.
    pub async fn shutdown(&self) -> Result<()> {
        *self.state.write().await = RuntimeState::ShuttingDown;
        tracing::info!(level = %self.level, "TEE runtime shut down");
        Ok(())
    }

    // =========================================================================
    // Attestation
    // =========================================================================

    /// Generate an attestation report with the given user data.
    ///
    /// The report is signed by the AMD SP and can be verified by a remote party
    /// to prove this code is running inside a genuine SEV-SNP VM.
    ///
    /// Returns raw attestation report bytes.
    pub async fn generate_attestation_report(&self, user_data: &[u8; 64]) -> Result<Vec<u8>> {
        if !self.attestation_available {
            return Err(Error::Tee(
                "Attestation not available: no SEV-SNP hardware detected".to_string(),
            ));
        }

        // Read attestation report from /dev/sev-guest
        Self::read_sev_guest_report(user_data).await
    }

    // =========================================================================
    // Environment probing (private)
    // =========================================================================

    /// Probe the runtime environment for TEE capabilities.
    async fn probe_environment() -> DetectionResult {
        let sev_guest_device = Path::new("/dev/sev-guest").exists();

        let cpuid_sev_snp = Self::check_cpuid_sev_snp();

        let inside_vm = Self::check_inside_vm();

        let (level, summary) = if sev_guest_device && cpuid_sev_snp {
            (
                SecurityLevel::TeeHardware,
                "AMD SEV-SNP active — memory encrypted by CPU".to_string(),
            )
        } else if inside_vm {
            (
                SecurityLevel::VmIsolation,
                "Running inside VM — no hardware TEE detected".to_string(),
            )
        } else {
            (
                SecurityLevel::ProcessOnly,
                "Process-level security only — no VM, no TEE".to_string(),
            )
        };

        DetectionResult {
            level,
            sev_guest_device,
            cpuid_sev_snp,
            inside_vm,
            summary,
        }
    }

    /// Check CPUID for AMD SEV-SNP support.
    ///
    /// CPUID leaf 0x8000001F, bit 4 = SEV-SNP supported.
    fn check_cpuid_sev_snp() -> bool {
        #[cfg(target_arch = "x86_64")]
        {
            // CPUID leaf 0x8000001F: AMD Encrypted Memory Capabilities
            let result = unsafe { core::arch::x86_64::__cpuid(0x8000_001F) };
            // EAX bit 4 = SEV-SNP
            (result.eax & (1 << 4)) != 0
        }
        #[cfg(not(target_arch = "x86_64"))]
        {
            false
        }
    }

    /// Heuristic check for running inside a VM.
    ///
    /// Checks DMI product name and hypervisor CPUID leaf.
    fn check_inside_vm() -> bool {
        // Check DMI product name
        if let Ok(product) = std::fs::read_to_string("/sys/class/dmi/id/product_name") {
            let product = product.trim().to_lowercase();
            if product.contains("kvm")
                || product.contains("qemu")
                || product.contains("libkrun")
                || product.contains("a3s-box")
            {
                return true;
            }
        }

        // Check hypervisor CPUID leaf (leaf 0x40000000)
        #[cfg(target_arch = "x86_64")]
        {
            let result = unsafe { core::arch::x86_64::__cpuid(0x4000_0000) };
            // If max leaf >= 0x40000000, a hypervisor is present
            if result.eax >= 0x4000_0000 {
                return true;
            }
        }

        // Check /proc/cpuinfo for hypervisor flag
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            if cpuinfo.contains("hypervisor") {
                return true;
            }
        }

        false
    }

    /// Read attestation report from `/dev/sev-guest` ioctl.
    async fn read_sev_guest_report(user_data: &[u8; 64]) -> Result<Vec<u8>> {
        use std::os::unix::io::AsRawFd;

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/sev-guest")
            .map_err(|e| Error::Tee(format!("Failed to open /dev/sev-guest: {e}")))?;

        // SNP_GET_REPORT ioctl
        // struct snp_guest_request_ioctl { msg_version, req_data, resp_data, fw_err }
        // For simplicity, we use the raw ioctl number
        const SNP_GET_REPORT: u64 = 0xC018_0001; // ioctl number for SNP_GET_REPORT

        #[repr(C)]
        struct SnpReportReq {
            user_data: [u8; 64],
            vmpl: u32,
            rsvd: [u8; 28],
        }

        #[repr(C)]
        struct SnpReportResp {
            status: u32,
            report_size: u32,
            rsvd: [u8; 24],
            report: [u8; 4000],
        }

        #[repr(C)]
        struct SnpGuestRequest {
            msg_version: u8,
            req_data: u64,
            resp_data: u64,
            fw_err: u64,
        }

        let mut req = SnpReportReq {
            user_data: *user_data,
            vmpl: 0,
            rsvd: [0u8; 28],
        };

        let mut resp = SnpReportResp {
            status: 0,
            report_size: 0,
            rsvd: [0u8; 24],
            report: [0u8; 4000],
        };

        let mut guest_req = SnpGuestRequest {
            msg_version: 1,
            req_data: &mut req as *mut _ as u64,
            resp_data: &mut resp as *mut _ as u64,
            fw_err: 0,
        };

        let ret =
            unsafe { libc::ioctl(file.as_raw_fd(), SNP_GET_REPORT, &mut guest_req as *mut _) };

        if ret != 0 {
            return Err(Error::Tee(format!(
                "SNP_GET_REPORT ioctl failed: errno={}, fw_err={}",
                std::io::Error::last_os_error(),
                guest_req.fw_err,
            )));
        }

        let report_size = resp.report_size as usize;
        if report_size == 0 || report_size > 4000 {
            return Err(Error::Tee(format!(
                "Invalid attestation report size: {report_size}"
            )));
        }

        Ok(resp.report[..report_size].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_only_runtime() {
        let rt = TeeRuntime::process_only();
        assert_eq!(rt.security_level(), SecurityLevel::ProcessOnly);
        assert!(!rt.is_tee_active());
        assert!(!rt.has_sealed_storage());
        assert!(!rt.has_attestation());
    }

    #[test]
    fn test_with_level() {
        let rt = TeeRuntime::with_level(SecurityLevel::VmIsolation);
        assert_eq!(rt.security_level(), SecurityLevel::VmIsolation);
        assert!(!rt.is_tee_active());
    }

    #[test]
    fn test_tee_hardware_level() {
        let rt = TeeRuntime::with_level(SecurityLevel::TeeHardware);
        assert!(rt.is_tee_active());
        // No sealed storage in test mode (no real hardware)
        assert!(!rt.has_sealed_storage());
    }

    #[tokio::test]
    async fn test_detect_on_dev_machine() {
        // On a dev machine, this should detect ProcessOnly or VmIsolation
        let rt = TeeRuntime::detect().await;
        // Should not panic, should return a valid level
        let level = rt.security_level();
        assert!(
            level == SecurityLevel::ProcessOnly
                || level == SecurityLevel::VmIsolation
                || level == SecurityLevel::TeeHardware
        );
    }

    #[tokio::test]
    async fn test_shutdown() {
        let rt = TeeRuntime::process_only();
        assert_eq!(rt.state().await, RuntimeState::Active);
        rt.shutdown().await.unwrap();
        assert_eq!(rt.state().await, RuntimeState::ShuttingDown);
    }

    #[tokio::test]
    async fn test_attestation_unavailable_without_tee() {
        let rt = TeeRuntime::process_only();
        let result = rt.generate_attestation_report(&[0u8; 64]).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not available"));
    }

    #[test]
    fn test_detection_result_fields() {
        let result = DetectionResult {
            level: SecurityLevel::ProcessOnly,
            sev_guest_device: false,
            cpuid_sev_snp: false,
            inside_vm: false,
            summary: "test".to_string(),
        };
        assert!(!result.sev_guest_device);
        assert!(!result.cpuid_sev_snp);
        assert!(!result.inside_vm);
    }
}
