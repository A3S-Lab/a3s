use std::ffi::OsString;
use std::path::PathBuf;

/// Resolve the current user's home directory from native environment
/// conventions. `HOME` remains authoritative when explicitly configured;
/// native Windows shells normally expose `USERPROFILE` instead.
pub(crate) fn user_home_dir() -> Option<PathBuf> {
    user_home_dir_from(|name| std::env::var_os(name))
}

pub(crate) fn user_home_dir_from(
    mut read_env: impl FnMut(&str) -> Option<OsString>,
) -> Option<PathBuf> {
    if let Some(home) = read_non_empty(&mut read_env, "HOME") {
        return Some(PathBuf::from(home));
    }

    #[cfg(windows)]
    {
        if let Some(profile) = read_non_empty(&mut read_env, "USERPROFILE") {
            return Some(PathBuf::from(profile));
        }

        let drive = read_non_empty(&mut read_env, "HOMEDRIVE");
        let path = read_non_empty(&mut read_env, "HOMEPATH");
        if let (Some(mut drive), Some(path)) = (drive, path) {
            drive.push(path);
            return Some(PathBuf::from(drive));
        }
    }

    None
}

fn read_non_empty(
    read_env: &mut impl FnMut(&str) -> Option<OsString>,
    name: &str,
) -> Option<OsString> {
    read_env(name).filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn resolve(values: &[(&str, &str)]) -> Option<PathBuf> {
        let values = values
            .iter()
            .map(|(name, value)| ((*name).to_string(), OsString::from(value)))
            .collect::<HashMap<_, _>>();
        user_home_dir_from(|name| values.get(name).cloned())
    }

    #[test]
    fn explicit_home_is_authoritative() {
        assert_eq!(
            resolve(&[("HOME", "/explicit"), ("USERPROFILE", r"C:\profile")]),
            Some(PathBuf::from("/explicit"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_profile_is_used_when_home_is_absent_or_empty() {
        assert_eq!(
            resolve(&[("HOME", ""), ("USERPROFILE", r"C:\Users\account")]),
            Some(PathBuf::from(r"C:\Users\account"))
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_home_drive_and_path_are_the_final_fallback() {
        assert_eq!(
            resolve(&[("HOMEDRIVE", "D:"), ("HOMEPATH", r"\Users\account")]),
            Some(PathBuf::from(r"D:\Users\account"))
        );
    }
}
