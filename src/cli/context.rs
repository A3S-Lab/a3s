use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context};
use tokio_util::sync::CancellationToken;

use super::args::{Cli, ColorMode, OutputMode};

/// Immutable process inputs captured once at the CLI boundary.
///
/// Keeping the snapshot private prevents command handlers from observing a
/// process environment that another task changed after parsing began. Values
/// are intentionally not `Debug`: the environment can contain credentials.
#[derive(Clone)]
pub(crate) struct EnvironmentSnapshot {
    values: Arc<HashMap<OsString, OsString>>,
}

impl EnvironmentSnapshot {
    fn capture() -> Self {
        Self {
            values: Arc::new(std::env::vars_os().collect()),
        }
    }

    pub(crate) fn var_os(&self, name: &str) -> Option<OsString> {
        self.values.get(OsStr::new(name)).cloned()
    }

    pub(crate) fn nonempty_var_os(&self, name: &str) -> Option<OsString> {
        self.var_os(name).filter(|value| !value.is_empty())
    }

    pub(crate) fn utf8(&self, name: &str) -> anyhow::Result<Option<String>> {
        self.nonempty_var_os(name)
            .map(|value| {
                value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("{name} must be valid UTF-8"))
            })
            .transpose()
    }

    fn boolean(&self, name: &str) -> anyhow::Result<Option<bool>> {
        let Some(value) = self.var_os(name) else {
            return Ok(None);
        };
        if value.is_empty() {
            return Ok(Some(true));
        }
        let value = value
            .into_string()
            .map_err(|_| anyhow::anyhow!("{name} must be valid UTF-8"))?;
        match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(Some(true)),
            "0" | "false" | "no" | "off" => Ok(Some(false)),
            _ => bail!("{name} must be a boolean value"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OutputPolicy {
    pub mode: OutputMode,
    pub quiet: bool,
    pub verbosity: u8,
    pub color: ColorMode,
    pub progress: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct InteractionPolicy {
    pub non_interactive: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NetworkPolicy {
    pub offline: bool,
    pub allow_first_use_install: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalCapabilities {
    pub stdin: bool,
    pub stdout: bool,
    pub stderr: bool,
}

/// One resolved view of directory, configuration, policy, terminal facts, and
/// platform paths for a single umbrella CLI invocation.
pub(crate) struct InvocationContext {
    pub directory: PathBuf,
    pub explicit_config: Option<PathBuf>,
    pub home: Option<PathBuf>,
    pub component_paths: a3s::components::ComponentPaths,
    pub output: OutputPolicy,
    pub interaction: InteractionPolicy,
    pub network: NetworkPolicy,
    pub terminal: TerminalCapabilities,
    pub environment: EnvironmentSnapshot,
    pub cancellation: CancellationToken,
}

impl InvocationContext {
    pub(crate) fn build(cli: &Cli) -> anyhow::Result<Self> {
        let environment = EnvironmentSnapshot::capture();
        let initial_directory =
            std::env::current_dir().context("failed to determine the invocation directory")?;
        let directory = resolve_directory(cli.directory.as_deref(), &initial_directory)?;
        let home = crate::user_paths::user_home_dir_from(|name| environment.var_os(name));
        let explicit_config = cli
            .config
            .clone()
            .or_else(|| {
                environment
                    .nonempty_var_os("A3S_CONFIG_FILE")
                    .map(PathBuf::from)
            })
            .map(|path| resolve_input_path(path, &directory, home.as_deref()));

        let mode = cli.output_mode();
        let machine = mode != OutputMode::Human;
        let terminal = TerminalCapabilities {
            stdin: std::io::stdin().is_terminal(),
            stdout: std::io::stdout().is_terminal(),
            stderr: std::io::stderr().is_terminal(),
        };
        let environment_quiet = environment.boolean("A3S_QUIET")?.unwrap_or(false);
        let quiet = cli.quiet || environment_quiet;
        let environment_no_progress = environment.boolean("A3S_NO_PROGRESS")?.unwrap_or(false);
        let progress =
            !machine && !quiet && !cli.no_progress && !environment_no_progress && terminal.stderr;
        let color = resolve_color(cli.color, machine, &environment)?;
        let verbosity = if quiet {
            0
        } else if cli.verbose > 0 {
            cli.verbose
        } else {
            environment
                .utf8("A3S_VERBOSE")?
                .map(|value| {
                    value
                        .parse::<u8>()
                        .context("A3S_VERBOSE must be an integer from 0 to 255")
                })
                .transpose()?
                .unwrap_or(0)
        };
        let offline = cli.offline || environment.boolean("A3S_OFFLINE")?.unwrap_or(false);
        let no_auto_install = environment.boolean("A3S_NO_AUTO_INSTALL")?.unwrap_or(false);
        let non_interactive = machine
            || cli.non_interactive
            || environment.boolean("A3S_NON_INTERACTIVE")?.unwrap_or(false)
            || !terminal.stdin
            || !terminal.stderr;
        let component_paths = a3s::components::ComponentPaths::from_env_at(&directory)?;

        Ok(Self {
            directory,
            explicit_config,
            home,
            component_paths,
            output: OutputPolicy {
                mode,
                quiet,
                verbosity,
                color,
                progress,
            },
            interaction: InteractionPolicy { non_interactive },
            network: NetworkPolicy {
                offline,
                allow_first_use_install: !offline && !no_auto_install,
            },
            terminal,
            environment,
            cancellation: CancellationToken::new(),
        })
    }

    pub(crate) fn output_mode(&self) -> OutputMode {
        self.output.mode
    }

    pub(crate) fn user_config_path(&self) -> Option<PathBuf> {
        self.home
            .as_deref()
            .map(|home| home.join(".a3s/config.acl"))
    }

    pub(crate) fn resolve_path(&self, path: impl Into<PathBuf>) -> PathBuf {
        resolve_input_path(path.into(), &self.directory, self.home.as_deref())
    }

    pub(crate) fn configure_child(&self, command: &mut tokio::process::Command) {
        command.current_dir(&self.directory);
        set_optional_env(command, "A3S_CONFIG_FILE", self.explicit_config.as_deref());
        set_boolean_env(command, "A3S_OFFLINE", self.network.offline);
        set_boolean_env(
            command,
            "A3S_NO_AUTO_INSTALL",
            !self.network.allow_first_use_install,
        );
        set_boolean_env(
            command,
            "A3S_NON_INTERACTIVE",
            self.interaction.non_interactive,
        );
        set_boolean_env(command, "A3S_NO_PROGRESS", !self.output.progress);
        set_boolean_env(command, "A3S_QUIET", self.output.quiet);
        if self.output.verbosity > 0 {
            command.env("A3S_VERBOSE", self.output.verbosity.to_string());
        } else {
            command.env_remove("A3S_VERBOSE");
        }
        match self.output.color {
            ColorMode::Auto => {
                command.env_remove("A3S_COLOR");
                command.env_remove("NO_COLOR");
            }
            ColorMode::Always => {
                command.env("A3S_COLOR", "always");
                command.env_remove("NO_COLOR");
            }
            ColorMode::Never => {
                command.env("A3S_COLOR", "never");
                command.env("NO_COLOR", "1");
            }
        }

        command.env("A3S_CLI_CONTEXT_VERSION", "1");
        command.env("A3S_CLI_DIRECTORY", &self.directory);
        command.env("A3S_CLI_OUTPUT", output_mode_name(self.output.mode));
        command.env("A3S_CLI_OFFLINE", bool_name(self.network.offline));
        command.env(
            "A3S_CLI_NON_INTERACTIVE",
            bool_name(self.interaction.non_interactive),
        );
        command.env("A3S_CLI_NO_PROGRESS", bool_name(!self.output.progress));
    }
}

fn resolve_directory(requested: Option<&Path>, initial: &Path) -> anyhow::Result<PathBuf> {
    let directory = requested
        .map(|path| resolve_input_path(path.to_path_buf(), initial, None))
        .unwrap_or_else(|| initial.to_path_buf());
    let metadata = std::fs::metadata(&directory)
        .with_context(|| format!("could not use directory {}", directory.display()))?;
    if !metadata.is_dir() {
        bail!(
            "could not use directory {}: not a directory",
            directory.display()
        );
    }
    directory
        .canonicalize()
        .with_context(|| format!("could not resolve directory {}", directory.display()))
}

fn resolve_input_path(path: PathBuf, directory: &Path, home: Option<&Path>) -> PathBuf {
    let path = expand_home(path, home);
    if path.is_absolute() {
        path
    } else {
        directory.join(path)
    }
}

fn expand_home(path: PathBuf, home: Option<&Path>) -> PathBuf {
    let Ok(rest) = path.strip_prefix("~") else {
        return path;
    };
    home.map(|home| home.join(rest)).unwrap_or(path)
}

fn resolve_color(
    requested: ColorMode,
    machine: bool,
    environment: &EnvironmentSnapshot,
) -> anyhow::Result<ColorMode> {
    if machine {
        return Ok(ColorMode::Never);
    }
    if requested != ColorMode::Auto {
        return Ok(requested);
    }
    if environment.var_os("NO_COLOR").is_some() {
        return Ok(ColorMode::Never);
    }
    match environment.utf8("A3S_COLOR")?.as_deref() {
        None | Some("auto") => Ok(ColorMode::Auto),
        Some("always") => Ok(ColorMode::Always),
        Some("never") => Ok(ColorMode::Never),
        Some(_) => bail!("A3S_COLOR must be one of: auto, always, never"),
    }
}

fn set_optional_env(command: &mut tokio::process::Command, name: &str, value: Option<&Path>) {
    if let Some(value) = value {
        command.env(name, value);
    } else {
        command.env_remove(name);
    }
}

fn set_boolean_env(command: &mut tokio::process::Command, name: &str, enabled: bool) {
    if enabled {
        command.env(name, "1");
    } else {
        command.env_remove(name);
    }
}

fn output_mode_name(mode: OutputMode) -> &'static str {
    match mode {
        OutputMode::Human => "human",
        OutputMode::Json => "json",
        OutputMode::Jsonl => "jsonl",
    }
}

fn bool_name(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli_for(directory: &Path, config: &str) -> Cli {
        Cli {
            directory: Some(directory.to_path_buf()),
            config: Some(PathBuf::from(config)),
            output: OutputMode::Human,
            json: false,
            quiet: false,
            verbose: 0,
            color: ColorMode::Auto,
            no_progress: false,
            offline: false,
            non_interactive: false,
            command: None,
        }
    }

    #[test]
    fn relative_paths_resolve_from_the_effective_directory() {
        let root = tempfile::tempdir().unwrap();
        let path = resolve_input_path(PathBuf::from("config.acl"), root.path(), None);
        assert_eq!(path, root.path().join("config.acl"));
    }

    #[test]
    fn home_expansion_is_explicit_and_does_not_read_the_process() {
        let home = Path::new("/tmp/a3s-home");
        assert_eq!(
            expand_home(PathBuf::from("~/.a3s/config.acl"), Some(home)),
            home.join(".a3s/config.acl")
        );
    }

    #[test]
    fn sequential_contexts_do_not_change_or_reuse_invocation_directories() {
        let root = tempfile::tempdir().unwrap();
        let first_directory = root.path().join("first");
        let second_directory = root.path().join("second");
        std::fs::create_dir_all(&first_directory).unwrap();
        std::fs::create_dir_all(&second_directory).unwrap();
        let process_directory = std::env::current_dir().unwrap();

        let first = InvocationContext::build(&cli_for(&first_directory, "first.acl")).unwrap();
        let second = InvocationContext::build(&cli_for(&second_directory, "second.acl")).unwrap();

        assert_eq!(first.directory, first_directory.canonicalize().unwrap());
        assert_eq!(second.directory, second_directory.canonicalize().unwrap());
        assert_eq!(
            first.explicit_config,
            Some(first.directory.join("first.acl"))
        );
        assert_eq!(
            second.explicit_config,
            Some(second.directory.join("second.acl"))
        );
        assert_eq!(std::env::current_dir().unwrap(), process_directory);
    }
}
