use std::path::{Path, PathBuf};

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};
use tokio::fs;

/// Select a workspace directory through either a client-owned picker or the
/// native picker owned by the local A3S Web daemon.
pub(super) async fn pick_directory(
    default_root: &Path,
    request: Value,
) -> BootResult<serde_json::Value> {
    if let Some(path) = optional_path(&request, "path")? {
        return selected_directory(path).await;
    }

    let requested_initial_path = optional_path(&request, "initialPath")?;
    let initial_path = match requested_initial_path {
        Some(path) if is_directory(&path).await => path,
        _ if is_directory(default_root).await => default_root.to_path_buf(),
        _ => std::env::current_dir().map_err(BootError::Io)?,
    };
    match native_directory_picker(&initial_path).await? {
        Some(path) => selected_directory(path).await,
        None => Ok(cancelled_selection()),
    }
}

async fn is_directory(path: &Path) -> bool {
    fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_dir())
}

fn optional_path(request: &Value, field: &str) -> BootResult<Option<PathBuf>> {
    match request.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(expand_home(value)))
            }
        }
        Some(_) => Err(BootError::BadRequest(format!("{field} must be a string"))),
    }
}

async fn selected_directory(path: PathBuf) -> BootResult<serde_json::Value> {
    let metadata = fs::metadata(&path).await.map_err(picker_fs_error)?;
    if !metadata.is_dir() {
        return Err(BootError::BadRequest(format!(
            "selected path is not a directory: {}",
            path.display()
        )));
    }
    let path = fs::canonicalize(path).await.map_err(picker_fs_error)?;
    Ok(json!({
        "cancelled": false,
        "path": display_path(&path),
    }))
}

fn cancelled_selection() -> serde_json::Value {
    json!({
        "cancelled": true,
        "path": Value::Null,
    })
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = crate::user_paths::user_home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    if let Some(value) = value.strip_prefix(r"\\?\") {
        return value.to_string();
    }
    value.to_string()
}

fn picker_fs_error(error: std::io::Error) -> BootError {
    match error.kind() {
        std::io::ErrorKind::NotFound => BootError::BadRequest(error.to_string()),
        std::io::ErrorKind::PermissionDenied => BootError::Forbidden(error.to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            BootError::BadRequest(error.to_string())
        }
        _ => BootError::Io(error),
    }
}

#[cfg(target_os = "macos")]
async fn native_directory_picker(initial_path: &Path) -> BootResult<Option<PathBuf>> {
    use tokio::process::Command;

    const SCRIPT: &str = r#"
set initialPath to system attribute "A3S_PICK_DIRECTORY_INITIAL_PATH"
set selectedFolder to choose folder with prompt "Choose an A3S workspace" default location POSIX file initialPath
POSIX path of selectedFolder
"#;
    let output = Command::new("osascript")
        .args(["-e", SCRIPT])
        .env("A3S_PICK_DIRECTORY_INITIAL_PATH", initial_path)
        .output()
        .await
        .map_err(BootError::Io)?;
    if output.status.success() {
        return Ok(output_path(&output.stdout));
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("User canceled") || stderr.contains("-128") {
        return Ok(None);
    }
    Err(picker_process_error("osascript", &stderr))
}

#[cfg(target_os = "linux")]
async fn native_directory_picker(initial_path: &Path) -> BootResult<Option<PathBuf>> {
    use std::io::ErrorKind;
    use tokio::process::Command;

    let initial = directory_argument(initial_path);
    let candidates: [(&str, Vec<String>); 3] = [
        (
            "zenity",
            vec![
                "--file-selection".to_string(),
                "--directory".to_string(),
                "--title=Choose an A3S workspace".to_string(),
                format!("--filename={initial}"),
            ],
        ),
        (
            "yad",
            vec![
                "--file-selection".to_string(),
                "--directory".to_string(),
                "--title=Choose an A3S workspace".to_string(),
                format!("--filename={initial}"),
            ],
        ),
        (
            "kdialog",
            vec![
                "--getexistingdirectory".to_string(),
                initial_path.display().to_string(),
                "--title".to_string(),
                "Choose an A3S workspace".to_string(),
            ],
        ),
    ];

    for (program, arguments) in candidates {
        let output = match Command::new(program).args(arguments).output().await {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(BootError::Io(error)),
        };
        if output.status.success() {
            return Ok(output_path(&output.stdout));
        }
        if output.status.code() == Some(1) {
            return Ok(None);
        }
        return Err(picker_process_error(
            program,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }

    Err(BootError::Internal(
        "no supported directory picker found; install zenity, yad, or kdialog".to_string(),
    ))
}

#[cfg(windows)]
async fn native_directory_picker(initial_path: &Path) -> BootResult<Option<PathBuf>> {
    use std::io::ErrorKind;
    use tokio::process::Command;

    const SCRIPT: &str = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.Description = 'Choose an A3S workspace'
$dialog.ShowNewFolderButton = $true
$dialog.SelectedPath = $env:A3S_PICK_DIRECTORY_INITIAL_PATH
if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
  [Console]::Out.WriteLine($dialog.SelectedPath)
}
"#;
    for program in ["pwsh", "powershell"] {
        let output = match Command::new(program)
            .args(["-NoProfile", "-STA", "-Command", SCRIPT])
            .env("A3S_PICK_DIRECTORY_INITIAL_PATH", initial_path)
            .output()
            .await
        {
            Ok(output) => output,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(BootError::Io(error)),
        };
        if output.status.success() {
            return Ok(output_path(&output.stdout));
        }
        return Err(picker_process_error(
            program,
            &String::from_utf8_lossy(&output.stderr),
        ));
    }

    Err(BootError::Internal(
        "no supported PowerShell executable found for the directory picker".to_string(),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
async fn native_directory_picker(_initial_path: &Path) -> BootResult<Option<PathBuf>> {
    Err(BootError::Internal(
        "the native directory picker is not supported on this platform".to_string(),
    ))
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn output_path(output: &[u8]) -> Option<PathBuf> {
    let path = String::from_utf8_lossy(output);
    let path = path.trim();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

#[cfg(target_os = "linux")]
fn directory_argument(path: &Path) -> String {
    let mut value = path.display().to_string();
    if !value.ends_with(std::path::MAIN_SEPARATOR) {
        value.push(std::path::MAIN_SEPARATOR);
    }
    value
}

#[cfg(any(target_os = "macos", target_os = "linux", windows))]
fn picker_process_error(program: &str, stderr: &str) -> BootError {
    let detail = stderr.trim();
    if detail.is_empty() {
        BootError::Internal(format!("{program} directory picker failed"))
    } else {
        BootError::Internal(format!("{program} directory picker failed: {detail}"))
    }
}
