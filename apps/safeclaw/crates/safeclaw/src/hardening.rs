//! Process hardening for sensitive data protection
//!
//! Provides OS-level hardening to prevent sensitive data from leaking
//! through core dumps, swap, or process inspection.
//!
//! Requires the `hardening` feature flag. Without it, all functions
//! are no-ops that return `Ok(())`.
//!
//! **Threat model**: Defends against physical/OS-level data extraction.
//! See `docs/threat-model.md` §5.

/// Disable core dumps on Linux via `prctl(PR_SET_DUMPABLE, 0)`.
///
/// This prevents the kernel from writing process memory to disk on crash,
/// which could expose secrets (API keys, taint registries, session data).
///
/// On non-Linux platforms or without the `hardening` feature, this is a
/// no-op and returns `Ok(())`.
///
/// # Errors
///
/// Returns an error string if the `prctl` syscall fails.
pub fn disable_core_dumps() -> std::result::Result<(), String> {
    #[cfg(all(feature = "hardening", target_os = "linux"))]
    {
        // PR_SET_DUMPABLE = 4, 0 = not dumpable
        let ret = unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0, 0, 0, 0) };
        if ret != 0 {
            return Err(format!(
                "prctl(PR_SET_DUMPABLE, 0) failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        tracing::info!("Core dumps disabled via prctl(PR_SET_DUMPABLE, 0)");
    }

    #[cfg(not(all(feature = "hardening", target_os = "linux")))]
    {
        tracing::debug!("Core dump protection not active (requires hardening feature + Linux)");
    }

    Ok(())
}

/// Apply all process hardening measures.
///
/// Currently includes:
/// - Core dump disabling (Linux only, `hardening` feature)
///
/// Call this early in `main()` before loading any secrets.
pub fn harden_process() -> std::result::Result<(), String> {
    disable_core_dumps()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disable_core_dumps_does_not_panic() {
        // On macOS/CI this is a no-op; on Linux+hardening it calls prctl.
        // Either way it should not panic.
        let result = disable_core_dumps();
        assert!(result.is_ok());
    }

    #[test]
    fn test_harden_process_does_not_panic() {
        let result = harden_process();
        assert!(result.is_ok());
    }
}
