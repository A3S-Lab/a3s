use crate::user_paths::user_home_dir;
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER,
    HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ,
};

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";
const WOW64_UNINSTALL_KEY: &str =
    r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
const MAX_SUBKEY_CHARS: usize = 512;
const MAX_REGISTRY_STRING_BYTES: u32 = 64 * 1024;

pub(super) fn workbuddy_executable_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(local_app_data) = env_path("LOCALAPPDATA") {
        push_executable_under(&mut candidates, local_app_data.join("Programs/WorkBuddy"));
        push_executable_under(&mut candidates, local_app_data.join("WorkBuddy"));
    }
    if let Some(home) = user_home_dir() {
        push_executable_under(
            &mut candidates,
            home.join("AppData/Local/Programs/WorkBuddy"),
        );
    }
    for name in ["ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(program_files) = env_path(name) {
            push_executable_under(&mut candidates, program_files.join("WorkBuddy"));
        }
    }
    for executable in registered_workbuddy_executables() {
        push_unique(&mut candidates, executable);
    }

    candidates
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn push_executable_under(candidates: &mut Vec<PathBuf>, directory: PathBuf) {
    push_unique(candidates, directory.join("WorkBuddy.exe"));
}

fn push_unique(candidates: &mut Vec<PathBuf>, candidate: PathBuf) {
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn registered_workbuddy_executables() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for (root, path) in [
        (HKEY_CURRENT_USER, UNINSTALL_KEY),
        (HKEY_LOCAL_MACHINE, UNINSTALL_KEY),
        (HKEY_LOCAL_MACHINE, WOW64_UNINSTALL_KEY),
    ] {
        let Some(uninstall) = RegistryKey::open(root, path) else {
            continue;
        };
        for product_key in uninstall.subkey_names() {
            let Some(product) = RegistryKey::open(uninstall.raw(), &product_key) else {
                continue;
            };
            let Some(display_name) = product.string_value("DisplayName") else {
                continue;
            };
            if !display_name
                .trim()
                .to_ascii_lowercase()
                .starts_with("workbuddy")
            {
                continue;
            }

            if let Some(display_icon) = product.string_value("DisplayIcon") {
                if let Some(executable) = display_icon_executable(&display_icon) {
                    push_unique(&mut candidates, executable);
                }
            }
            if let Some(install_location) = product.string_value("InstallLocation") {
                let install_location = install_location.trim().trim_matches('"');
                if !install_location.is_empty() {
                    push_executable_under(&mut candidates, PathBuf::from(install_location));
                }
            }
            if let Some(uninstall_command) = product.string_value("UninstallString") {
                if let Some(uninstaller) = command_executable(&uninstall_command) {
                    if let Some(directory) = uninstaller.parent() {
                        push_executable_under(&mut candidates, directory.to_path_buf());
                    }
                }
            }
        }
    }
    candidates
}

fn display_icon_executable(value: &str) -> Option<PathBuf> {
    let mut path = value.trim();
    if let Some((candidate, icon_index)) = path.rsplit_once(',') {
        if icon_index.trim().parse::<i32>().is_ok() {
            path = candidate.trim();
        }
    }
    let path = PathBuf::from(path.trim_matches('"'));
    is_workbuddy_executable(&path).then_some(path)
}

fn command_executable(value: &str) -> Option<PathBuf> {
    let value = value.trim();
    let executable = if let Some(rest) = value.strip_prefix('"') {
        rest.split_once('"').map(|(path, _)| path)?
    } else {
        value.split_whitespace().next()?
    };
    let executable = PathBuf::from(executable);
    (!executable.as_os_str().is_empty()).then_some(executable)
}

fn is_workbuddy_executable(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("WorkBuddy.exe"))
}

struct RegistryKey(HKEY);

impl RegistryKey {
    fn open(root: HKEY, path: &str) -> Option<Self> {
        let path = wide_null(path);
        let mut key = null_mut();
        // SAFETY: `path` is NUL-terminated for the duration of the call and
        // `key` points to writable storage. The returned handle is owned by
        // `RegistryKey` and closed in `Drop`.
        let result = unsafe { RegOpenKeyExW(root, path.as_ptr(), 0, KEY_READ, &mut key) };
        if result == ERROR_SUCCESS {
            Some(Self(key))
        } else {
            None
        }
    }

    fn raw(&self) -> HKEY {
        self.0
    }

    fn subkey_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut index = 0;
        loop {
            let mut buffer = vec![0_u16; MAX_SUBKEY_CHARS];
            let mut length = buffer.len() as u32;
            // SAFETY: `buffer` and `length` remain valid writable storage for
            // the call. Optional output pointers are null because unused.
            let result = unsafe {
                RegEnumKeyExW(
                    self.0,
                    index,
                    buffer.as_mut_ptr(),
                    &mut length,
                    null(),
                    null_mut(),
                    null_mut(),
                    null_mut(),
                )
            };
            if result == ERROR_NO_MORE_ITEMS {
                break;
            }
            if result != ERROR_SUCCESS {
                break;
            }
            names.push(String::from_utf16_lossy(&buffer[..length as usize]));
            index += 1;
        }
        names
    }

    fn string_value(&self, name: &str) -> Option<String> {
        let name = wide_null(name);
        let mut value_type = 0;
        let mut byte_len = 0;
        // SAFETY: The value name is NUL-terminated and the size query passes
        // null data while providing valid type and length outputs.
        let result = unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                null(),
                &mut value_type,
                null_mut(),
                &mut byte_len,
            )
        };
        if result != ERROR_SUCCESS
            || !matches!(value_type, REG_SZ | REG_EXPAND_SZ)
            || byte_len == 0
            || byte_len > MAX_REGISTRY_STRING_BYTES
        {
            return None;
        }

        let mut buffer = vec![0_u16; (byte_len as usize).div_ceil(2)];
        let mut actual_len = byte_len;
        // SAFETY: `buffer` has at least `byte_len` bytes of writable storage;
        // the other pointers remain valid for the duration of the call.
        let result = unsafe {
            RegQueryValueExW(
                self.0,
                name.as_ptr(),
                null(),
                &mut value_type,
                buffer.as_mut_ptr().cast(),
                &mut actual_len,
            )
        };
        if result != ERROR_SUCCESS {
            return None;
        }
        let units = (actual_len as usize / 2).min(buffer.len());
        let end = buffer[..units]
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(units);
        Some(String::from_utf16_lossy(&buffer[..end]))
    }
}

impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: `self.0` is an owned handle returned by `RegOpenKeyExW` and
        // this destructor runs exactly once.
        let _ = unsafe { RegCloseKey(self.0) };
    }
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_workbuddy_executable_from_display_icon() {
        assert_eq!(
            display_icon_executable(r#""D:\Apps\WorkBuddy\WorkBuddy.exe",0"#),
            Some(PathBuf::from(r"D:\Apps\WorkBuddy\WorkBuddy.exe"))
        );
        assert_eq!(display_icon_executable(r"D:\Apps\Other.exe,0"), None);
    }

    #[test]
    fn extracts_quoted_uninstall_command_executable() {
        assert_eq!(
            command_executable(r#""D:\Apps\WorkBuddy\Uninstall WorkBuddy.exe" /currentuser"#),
            Some(PathBuf::from(r"D:\Apps\WorkBuddy\Uninstall WorkBuddy.exe"))
        );
    }
}
