use std::path::PathBuf;

use anyhow::{bail, Context};

use super::{parse_kind, required_value, InstallScope, ReleaseChannel};
use crate::components::catalog::ComponentKind;
use crate::components::id::ComponentId;
use crate::components::lifecycle::InstallSource;

#[derive(Debug, Default)]
pub(super) struct ListOptions {
    pub(super) installed: bool,
    pub(super) available: bool,
    pub(super) check_updates: bool,
    pub(super) json: bool,
    pub(super) kind: Option<ComponentKind>,
}

impl ListOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut options = Self::default();
        for argument in args {
            match argument.as_str() {
                "--installed" => options.installed = true,
                "--available" => options.available = true,
                "--updates" => options.check_updates = true,
                "--json" => options.json = true,
                "--kind" => bail!("--kind requires a value"),
                value if value.starts_with("--kind=") => {
                    options.kind = Some(parse_kind(value.trim_start_matches("--kind="))?);
                }
                other => bail!("unknown list option '{other}'"),
            }
        }
        if options.installed && options.available {
            bail!("--installed and --available are mutually exclusive");
        }
        Ok(options)
    }
}

#[derive(Debug, Default)]
pub(super) struct InstallOptions {
    pub(super) components: Vec<ComponentId>,
    pub(super) version: Option<String>,
    pub(super) source: InstallSource,
    pub(super) channel: ReleaseChannel,
    pub(super) scope: InstallScope,
    pub(super) package: Option<PathBuf>,
    pub(super) force: bool,
    pub(super) migrate: bool,
    pub(super) allow_unsigned: bool,
    pub(super) json: bool,
    pub(super) dry_run: bool,
    pub(super) plan_digest: Option<String>,
}

impl InstallOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut options = Self {
            source: InstallSource::Auto,
            channel: ReleaseChannel::Stable,
            scope: InstallScope::User,
            ..Self::default()
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--version" => {
                    index += 1;
                    options.version = Some(required_value(args, index, "--version")?.to_string());
                }
                "--source" => {
                    index += 1;
                    options.source = match required_value(args, index, "--source")? {
                        "auto" => InstallSource::Auto,
                        "homebrew" => InstallSource::Homebrew,
                        "release" => InstallSource::Release,
                        value => bail!("unsupported install source '{value}'"),
                    };
                }
                "--from" => {
                    index += 1;
                    options.package = Some(PathBuf::from(required_value(args, index, "--from")?));
                }
                "--channel" => {
                    index += 1;
                    options.channel = match required_value(args, index, "--channel")? {
                        "stable" => ReleaseChannel::Stable,
                        "beta" => ReleaseChannel::Beta,
                        "nightly" => ReleaseChannel::Nightly,
                        value => bail!("unsupported release channel '{value}'"),
                    };
                }
                "--scope" => {
                    index += 1;
                    options.scope = match required_value(args, index, "--scope")? {
                        "user" => InstallScope::User,
                        "system" => InstallScope::System,
                        value => bail!("unsupported install scope '{value}'"),
                    };
                }
                "--plan-digest" => {
                    index += 1;
                    options.plan_digest = Some(validate_plan_digest(required_value(
                        args,
                        index,
                        "--plan-digest",
                    )?)?);
                }
                "--force" => options.force = true,
                "--migrate" => options.migrate = true,
                "--allow-unsigned" => options.allow_unsigned = true,
                "--yes" => {}
                "--dry-run" => options.dry_run = true,
                "--json" => options.json = true,
                value if value.starts_with("--plan-digest=") => {
                    options.plan_digest = Some(validate_plan_digest(
                        value.trim_start_matches("--plan-digest="),
                    )?);
                }
                value if value.starts_with('-') => bail!("unknown install option '{value}'"),
                value => options.components.push(ComponentId::parse(value)?),
            }
            index += 1;
        }
        validate_plan_mode(options.dry_run, options.plan_digest.as_deref())?;
        Ok(options)
    }
}

#[derive(Debug, Default)]
pub(super) struct UninstallOptions {
    pub(super) components: Vec<ComponentId>,
    pub(super) cascade: bool,
    pub(super) purge: bool,
    pub(super) json: bool,
    pub(super) dry_run: bool,
    pub(super) plan_digest: Option<String>,
}

impl UninstallOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut options = Self::default();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--cascade" => options.cascade = true,
                "--purge" => options.purge = true,
                "--plan-digest" => {
                    index += 1;
                    options.plan_digest = Some(validate_plan_digest(required_value(
                        args,
                        index,
                        "--plan-digest",
                    )?)?);
                }
                "--yes" => {}
                "--json" => options.json = true,
                "--dry-run" => options.dry_run = true,
                value if value.starts_with("--plan-digest=") => {
                    options.plan_digest = Some(validate_plan_digest(
                        value.trim_start_matches("--plan-digest="),
                    )?);
                }
                value if value.starts_with('-') => {
                    bail!("unknown uninstall option '{value}'")
                }
                value => options.components.push(ComponentId::parse(value)?),
            }
            index += 1;
        }
        validate_plan_mode(options.dry_run, options.plan_digest.as_deref())?;
        Ok(options)
    }
}

#[derive(Debug, Default)]
pub(super) struct UpdateOptions {
    pub(super) components: Vec<ComponentId>,
    pub(super) all: bool,
    pub(super) json: bool,
    pub(super) dry_run: bool,
    pub(super) plan_digest: Option<String>,
}

impl UpdateOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut options = Self::default();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--all" => options.all = true,
                "--plan-digest" => {
                    index += 1;
                    options.plan_digest = Some(validate_plan_digest(required_value(
                        args,
                        index,
                        "--plan-digest",
                    )?)?);
                }
                "--yes" => {}
                "--json" => options.json = true,
                "--dry-run" => options.dry_run = true,
                value if value.starts_with("--plan-digest=") => {
                    options.plan_digest = Some(validate_plan_digest(
                        value.trim_start_matches("--plan-digest="),
                    )?);
                }
                value if value.starts_with('-') => bail!("unknown update option '{value}'"),
                value => options.components.push(ComponentId::parse(value)?),
            }
            index += 1;
        }
        if options.all && !options.components.is_empty() {
            bail!("--all cannot be combined with component IDs");
        }
        validate_plan_mode(options.dry_run, options.plan_digest.as_deref())?;
        Ok(options)
    }
}

fn validate_plan_digest(value: &str) -> anyhow::Result<String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        bail!("--plan-digest requires exactly 64 lowercase hexadecimal characters");
    }
    Ok(value.to_string())
}

fn validate_plan_mode(dry_run: bool, plan_digest: Option<&str>) -> anyhow::Result<()> {
    if dry_run && plan_digest.is_some() {
        bail!("--plan-digest cannot be combined with --dry-run");
    }
    Ok(())
}

#[derive(Debug)]
pub(super) struct InfoOptions {
    pub(super) component: String,
    pub(super) versions: bool,
    pub(super) sources: bool,
    pub(super) json: bool,
}

impl InfoOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut component = None;
        let mut versions = false;
        let mut sources = false;
        let mut json = false;
        for value in args {
            match value.as_str() {
                "--versions" => versions = true,
                "--sources" => sources = true,
                "--json" => json = true,
                value if value.starts_with('-') => bail!("unknown info option '{value}'"),
                value if component.is_none() => component = Some(value.to_string()),
                _ => bail!("a3s info accepts exactly one component"),
            }
        }
        Ok(Self {
            component: component.context("a3s info requires a component")?,
            versions,
            sources,
            json,
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct DoctorOptions {
    pub(super) component: Option<String>,
    pub(super) json: bool,
}

impl DoctorOptions {
    pub(super) fn parse(args: &[String]) -> anyhow::Result<Self> {
        let mut options = Self::default();
        for value in args {
            match value.as_str() {
                "--json" => options.json = true,
                value if value.starts_with('-') => bail!("unknown doctor option '{value}'"),
                value if options.component.is_none() => options.component = Some(value.to_string()),
                _ => bail!("a3s doctor accepts at most one component"),
            }
        }
        Ok(options)
    }
}
