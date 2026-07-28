use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::AgentPresencePublisher;

pub(super) const AGENT_ISLAND_DISABLED_FILE: &str = "island.disabled";

pub(super) fn disabled_path(directory: Option<&Path>) -> Option<PathBuf> {
    directory.map(|directory| directory.join(AGENT_ISLAND_DISABLED_FILE))
}

/// A missing marker is the default-on state. Any existing filesystem entry,
/// or an unreadable marker path, fails closed so a saved opt-out is not lost.
pub(super) fn is_enabled(directory: Option<&Path>) -> bool {
    let Some(path) = disabled_path(directory) else {
        return true;
    };
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => true,
        Ok(_) | Err(_) => false,
    }
}

pub(super) fn set_enabled(directory: Option<&Path>, enabled: bool) -> io::Result<()> {
    let directory = directory.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "no per-user A3S state directory is available",
        )
    })?;
    ensure_private_directory(directory)?;
    let path = directory.join(AGENT_ISLAND_DISABLED_FILE);
    if enabled {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        };
    }

    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(b"disabled\n")?;
            file.sync_all()
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn ensure_private_directory(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    let mut builder = fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn ensure_private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)
}

impl AgentPresencePublisher {
    pub(crate) fn island_preference_enabled(&self) -> bool {
        is_enabled(self.directory.as_deref())
    }

    pub(crate) fn persist_island_enabled(&self, enabled: bool) -> io::Result<()> {
        set_enabled(self.directory.as_deref(), enabled)
    }
}
