//! TEE (Trusted Execution Environment) integration
//!
//! Provides runtime self-detection and sealed storage for SafeClaw
//! running inside an A3S Box MicroVM with AMD SEV-SNP.
//!
//! ## Architecture (Phase 11)
//!
//! SafeClaw runs as a **guest** inside the TEE, not as a host that boots VMs.
//! At startup, `TeeRuntime::detect()` probes the environment:
//!
//! - `/dev/sev-guest` + CPUID → `TeeHardware` (memory encrypted by CPU)
//! - VM heuristics (DMI, hypervisor CPUID) → `VmIsolation`
//! - Neither → `ProcessOnly`
//!
//! ## Modules
//!
//! - `runtime` — Self-detection, attestation, lifecycle
//! - `sealed` — Encrypted storage bound to TEE identity
//! - `client` — Frame-based TEE client (uses `Transport` trait)
//! - `protocol` — Shared protocol types re-exported from `a3s-transport`

mod client;
mod protocol;
pub mod runtime;
pub mod sealed;

pub use client::TeeClient;
pub use protocol::{TeeMessage, TeeRequest, TeeResponse};
pub use runtime::{DetectionResult, RuntimeState, TeeRuntime};
pub use sealed::SealedStorage;

/// Security level of the current runtime environment.
///
/// Exposed in API responses so clients know their actual protection level.
/// This prevents silent degradation when TEE hardware is unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityLevel {
    /// AMD SEV-SNP or Intel TDX active — memory encrypted by CPU.
    TeeHardware,
    /// Running inside a MicroVM but no hardware TEE.
    VmIsolation,
    /// No VM, no TEE — application-level security only.
    ProcessOnly,
}

impl SecurityLevel {
    /// Human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::TeeHardware => "Hardware TEE active (memory encrypted)",
            Self::VmIsolation => "VM isolation (no hardware TEE)",
            Self::ProcessOnly => "Process-level security only (no VM, no TEE)",
        }
    }
}

impl std::fmt::Display for SecurityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.description())
    }
}

#[cfg(test)]
mod security_level_tests {
    use super::*;

    #[test]
    fn test_security_level_serialization() {
        assert_eq!(
            serde_json::to_string(&SecurityLevel::TeeHardware).unwrap(),
            "\"tee_hardware\""
        );
        assert_eq!(
            serde_json::to_string(&SecurityLevel::VmIsolation).unwrap(),
            "\"vm_isolation\""
        );
        assert_eq!(
            serde_json::to_string(&SecurityLevel::ProcessOnly).unwrap(),
            "\"process_only\""
        );
    }

    #[test]
    fn test_security_level_display() {
        assert!(SecurityLevel::TeeHardware
            .description()
            .contains("Hardware TEE"));
        assert!(SecurityLevel::ProcessOnly.description().contains("no VM"));
    }

    #[test]
    fn test_security_level_roundtrip() {
        for level in [
            SecurityLevel::TeeHardware,
            SecurityLevel::VmIsolation,
            SecurityLevel::ProcessOnly,
        ] {
            let json = serde_json::to_string(&level).unwrap();
            let parsed: SecurityLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, level);
        }
    }
}
