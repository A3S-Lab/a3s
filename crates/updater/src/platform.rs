//! Platform detection for OS and architecture.

/// Detect the current platform target as `(os, arch)`.
///
/// Returns normalized names matching the GitHub release asset naming convention:
/// - OS: `"darwin"` or `"linux"`
/// - Arch: `"arm64"` or `"x86_64"`
pub fn platform_target() -> anyhow::Result<(String, String)> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        other => return Err(anyhow::anyhow!("Unsupported operating system: {}", other)),
    };

    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => return Err(anyhow::anyhow!("Unsupported architecture: {}", other)),
    };

    Ok((os.to_string(), arch.to_string()))
}
