//! `a3s cp` command - Copy files to/from a pod (kubectl cp style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

/// Copy direction.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum CopyDirection {
    /// Copy from local to pod.
    ToPod,
    /// Copy from pod to local.
    FromPod,
}

/// Cp command - Copy files between local and pods.
#[derive(clap::Parser, Debug)]
pub struct CpCommand {
    /// Source path (format: pod:/path or /path).
    src: String,

    /// Destination path (format: pod:/path or /path).
    dest: String,

    /// Container name (for multi-container pods).
    #[arg(short, long)]
    container: Option<String>,

    /// Create parent directories if they don't exist.
    #[arg(short = 'p', long)]
    create_parent_dirs: bool,
}

impl CpCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    /// Parse a path that might be in pod:/path format.
    fn parse_pod_path(path: &str) -> Result<(Option<String>, &str)> {
        if let Some((pod, pod_path)) = path.split_once(':') {
            // Check if it looks like a pod reference (could be just "podname:/path")
            // or an absolute path on local (e.g., "C:/path" on Windows)
            if pod.contains('/') || pod.contains('\\') {
                // It's a Windows path or similar, not a pod
                Ok((None, path))
            } else if pod.is_empty() {
                // Starts with ":" which is odd, treat as local path
                Ok((None, path))
            } else {
                // It's a pod reference
                Ok((Some(pod.to_string()), pod_path))
            }
        } else {
            Ok((None, path))
        }
    }

    /// Find sandbox ID by name.
    fn find_sandbox_id(name: &str) -> Result<String> {
        let boxes_dir = Self::boxes_dir();

        if boxes_dir.join(name).exists() {
            return Ok(name.to_string());
        }

        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let info_path = path.join("info.json");
                    if info_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&info_path) {
                            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                                if info
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s == name)
                                    .unwrap_or(false)
                                {
                                    return path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .ok_or_else(|| {
                                            A3sError::Project(format!(
                                                "invalid sandbox id for '{}'",
                                                name
                                            ))
                                        });
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(A3sError::Project(format!("pod '{}' not found", name)))
    }

    /// Get the workspace path inside a sandbox.
    fn get_sandbox_workspace(sandbox_id: &str) -> Result<PathBuf> {
        let workspace_path = Self::boxes_dir().join(sandbox_id).join("workspace");
        if !workspace_path.exists() {
            std::fs::create_dir_all(&workspace_path)?;
        }
        Ok(workspace_path)
    }

    /// Copy file to a pod.
    fn copy_to_pod(&self, pod: &str, src: &Path, dest: &Path) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(pod)?;
        let workspace = Self::get_sandbox_workspace(&sandbox_id)?;

        // Determine destination in sandbox
        let pod_dest = if dest.is_absolute() {
            dest.to_path_buf()
        } else {
            workspace.join(dest)
        };

        // Ensure parent directory exists
        if self.create_parent_dirs {
            if let Some(parent) = pod_dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Copy the file
        if src.is_file() {
            std::fs::copy(src, &pod_dest)?;
            println!(
                "Copied {} bytes to {}:{}",
                std::fs::metadata(src)?.len(),
                pod,
                pod_dest.display()
            );
        } else if src.is_dir() {
            Self::copy_dir_to_pod(src, &pod_dest)?;
            println!(
                "Copied directory {} to {}:{}",
                src.display(),
                pod,
                pod_dest.display()
            );
        }

        Ok(())
    }

    /// Copy directory to pod.
    fn copy_dir_to_pod(src: &Path, dest: &Path) -> Result<()> {
        if !src.is_dir() {
            return Err(A3sError::Project(format!(
                "{} is not a directory",
                src.display()
            )));
        }

        std::fs::create_dir_all(dest)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if src_path.is_dir() {
                Self::copy_dir_to_pod(&src_path, &dest_path)?;
            } else {
                std::fs::copy(&src_path, &dest_path)?;
            }
        }

        Ok(())
    }

    /// Copy file from a pod.
    fn copy_from_pod(&self, pod: &str, src: &Path, dest: &Path) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(pod)?;
        let workspace = Self::get_sandbox_workspace(&sandbox_id)?;

        // Determine source in sandbox
        let pod_src = if src.is_absolute() {
            src.to_path_buf()
        } else {
            workspace.join(src)
        };

        if !pod_src.exists() {
            return Err(A3sError::Project(format!(
                "file '{}' not found in pod '{}'",
                pod_src.display(),
                pod
            )));
        }

        // Ensure parent directory exists at destination
        if self.create_parent_dirs {
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
        }

        // Copy the file
        if pod_src.is_file() {
            std::fs::copy(&pod_src, dest)?;
            println!(
                "Copied {} bytes from {}:{} to {}",
                std::fs::metadata(&pod_src)?.len(),
                pod,
                pod_src.display(),
                dest.display()
            );
        } else if pod_src.is_dir() {
            Self::copy_dir_from_pod(&pod_src, dest)?;
            println!(
                "Copied directory from {}:{} to {}",
                pod,
                pod_src.display(),
                dest.display()
            );
        }

        Ok(())
    }

    /// Copy directory from pod.
    fn copy_dir_from_pod(src: &Path, dest: &Path) -> Result<()> {
        if !src.is_dir() {
            return Err(A3sError::Project(format!(
                "{} is not a directory",
                src.display()
            )));
        }

        std::fs::create_dir_all(dest)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dest_path = dest.join(entry.file_name());

            if src_path.is_dir() {
                Self::copy_dir_from_pod(&src_path, &dest_path)?;
            } else {
                std::fs::copy(&src_path, &dest_path)?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Command for CpCommand {
    async fn run(&self) -> Result<()> {
        let (src_pod, src_path) = Self::parse_pod_path(&self.src)?;
        let (dest_pod, dest_path) = Self::parse_pod_path(&self.dest)?;

        // Determine direction
        match (src_pod, dest_pod) {
            (Some(pod), None) => {
                // From pod to local
                let src = Path::new(src_path);
                let dest = Path::new(dest_path);
                self.copy_from_pod(&pod, src, dest)?;
            }
            (None, Some(pod)) => {
                // From local to pod
                let src = Path::new(src_path);
                let dest = Path::new(dest_path);
                self.copy_to_pod(&pod, src, dest)?;
            }
            (Some(_src_pod), Some(_dest_pod)) => {
                // Pod to pod - not supported
                return Err(A3sError::Project(
                    "copying directly between pods is not supported".to_string(),
                ));
            }
            (None, None) => {
                // Local to local - show usage
                return Err(A3sError::Project(
                    "at least one of source or destination must be a pod (use pod:/path format)"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}
