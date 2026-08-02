use std::io::Write;
use std::path::{Path, PathBuf};

use a3s::registry::{RegistryStore, TrustRootSource};
use anyhow::{bail, Context};
use serde_json::json;

use crate::cli::args::{OutputMode, RegistryArgs, RegistryCommand};
use crate::cli::context::InvocationContext;
use crate::cli::output::{coded_error, render_value, ExitClass};

pub(crate) async fn run(args: RegistryArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        RegistryCommand::List => list(context),
        RegistryCommand::Show(args) => show(&args.name, context),
        RegistryCommand::Add(args) => add(
            args.name.as_deref(),
            &args.url,
            &args.trust_root,
            args.yes,
            context,
        ),
        RegistryCommand::Replace(args) => {
            replace(&args.name, &args.url, &args.trust_root, args.yes, context)
        }
        RegistryCommand::Enable(args) => set_enabled(&args.name, true, context),
        RegistryCommand::Disable(args) => set_enabled(&args.name, false, context),
        RegistryCommand::Remove(args) => remove(&args.name, args.yes, context),
        RegistryCommand::Refresh(args) => refresh(args.name.as_deref(), context).await,
    }
}

fn list(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let registries = store(context)?.list()?;
    render_value(
        output,
        "registry.list",
        json!({"registries": registries}),
        || {
            println!("REGISTRY                 TRUST ROOT");
            for registry in &registries {
                println!("{:<24} {}", registry.name, registry.trust_root);
                println!("  {}", registry.url);
                if !registry.configured {
                    println!("  unavailable: production TUF root is not configured");
                } else if !registry.enabled {
                    println!("  disabled");
                }
            }
        },
    )
}

fn show(name: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let registry = store(context)?
        .get(name)?
        .with_context(|| format!("registry '{name}' is not configured"))?;
    render_value(
        output,
        "registry.show",
        json!({"registry": registry}),
        || {
            println!("name: {}", registry.name);
            println!("url: {}", registry.url);
            println!("trust root: {}", registry.trust_root);
            println!("built in: {}", registry.built_in);
            println!("configured: {}", registry.configured);
            println!("enabled: {}", registry.enabled);
            if let Some(path) = &registry.trusted_root_path {
                println!("trusted root file: {}", path.display());
            }
        },
    )
}

fn add(
    name: Option<&str>,
    url: &str,
    trust_root: &str,
    yes: bool,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    let output = context.output_mode();
    let store = store(context)?;
    let path;
    let source = if trust_root.starts_with("sha256:") {
        TrustRootSource::Digest(trust_root)
    } else {
        path = context.resolve_path(trust_root);
        TrustRootSource::File(&path)
    };
    let enrollment = store.prepare_named_enrollment(name, url, source)?;
    if !yes {
        confirm(
            &format!(
                "Trust registry '{}' at {} with root {}?",
                enrollment.record.name, enrollment.record.url, enrollment.record.trust_root
            ),
            context,
            "registry enrollment requires '--yes' in non-interactive mode",
        )?;
    }
    store.add(&enrollment)?;
    let record = enrollment.record;
    let name = record.name.clone();
    render_value(
        output,
        "registry.add",
        json!({"registry": record, "created": true}),
        || println!("added trusted registry '{name}'"),
    )
}

fn replace(
    name: &str,
    url: &str,
    trust_root: &str,
    yes: bool,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    let output = context.output_mode();
    let store = store(context)?;
    let path;
    let source = if trust_root.starts_with("sha256:") {
        TrustRootSource::Digest(trust_root)
    } else {
        path = context.resolve_path(trust_root);
        TrustRootSource::File(&path)
    };
    let enrollment = store.prepare_named_enrollment(Some(name), url, source)?;
    let previous = store
        .get(name)?
        .with_context(|| format!("registry '{name}' is not configured"))?;
    if !yes {
        confirm(
            &format!(
                "Replace registry '{}' at {} with {} and root {}?",
                name, previous.url, enrollment.record.url, enrollment.record.trust_root
            ),
            context,
            "registry replacement requires '--yes' in non-interactive mode",
        )?;
    }
    let record = store.replace(&enrollment)?;
    render_value(
        output,
        "registry.replace",
        json!({"registry": record, "previous": previous, "replaced": true}),
        || println!("replaced trusted registry '{name}'"),
    )
}

fn set_enabled(name: &str, enabled: bool, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let store = store(context)?;
    let previous = store
        .get(name)?
        .with_context(|| format!("registry '{name}' is not configured"))?;
    if previous.built_in {
        bail!(
            "the default registry is only a placeholder until it is explicitly configured or replaced"
        );
    }
    let changed = previous.enabled != enabled;
    let registry = store.set_enabled(name, enabled)?;
    let command = if enabled {
        "registry.enable"
    } else {
        "registry.disable"
    };
    let action = if enabled { "enabled" } else { "disabled" };
    render_value(
        output,
        command,
        json!({"registry": registry, "changed": changed}),
        || println!("{action} registry '{name}'"),
    )
}

fn remove(name: &str, yes: bool, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let store = store(context)?;
    let record = store
        .get(name)?
        .with_context(|| format!("registry '{name}' is not configured"))?;
    if record.built_in {
        bail!("the unconfigured default registry hint cannot be removed");
    }
    if !yes {
        confirm(
            &format!("Remove trusted registry '{name}'?"),
            context,
            "registry removal requires '--yes' in non-interactive mode",
        )?;
    }
    let record = store.remove(name)?;
    render_value(
        output,
        "registry.remove",
        json!({"registry": record, "removed": true}),
        || println!("removed registry '{name}'"),
    )
}

async fn refresh(name: Option<&str>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    if context.network.offline {
        bail!("registry refresh is unavailable in offline mode");
    }
    let store = store(context)?;
    let mut selected = if let Some(name) = name {
        vec![store
            .get(name)?
            .with_context(|| format!("registry '{name}' is not configured"))?]
    } else {
        store.list()?
    };
    selected.sort_by(|left, right| left.name.cmp(&right.name));
    let mut results = Vec::new();
    for registry in selected {
        if !registry.configured {
            if name.is_some() {
                bail!(
                    "registry '{}' has no production TUF trust root configured",
                    registry.name
                );
            }
            results.push(json!({
                "name": registry.name,
                "url": registry.url,
                "configured": false,
                "enabled": registry.enabled,
                "verified": false,
            }));
            continue;
        }
        if !registry.enabled && name.is_none() {
            results.push(json!({
                "name": registry.name,
                "url": registry.url,
                "configured": true,
                "enabled": false,
                "verified": false,
            }));
            continue;
        }
        let metadata = registry
            .refresh(&context.component_paths.state_root)
            .await?;
        results.push(json!({
            "name": registry.name,
            "url": registry.url,
            "configured": true,
            "enabled": registry.enabled,
            "verified": true,
            "metadata": metadata,
        }));
    }
    render_value(
        output,
        "registry.refresh",
        json!({"registries": results}),
        || {
            for result in &results {
                if result["verified"].as_bool() == Some(true) {
                    println!(
                        "verified {} (root {}, timestamp {}, snapshot {}, targets {})",
                        result["name"].as_str().unwrap_or_default(),
                        result["metadata"]["rootVersion"]
                            .as_u64()
                            .unwrap_or_default(),
                        result["metadata"]["timestampVersion"]
                            .as_u64()
                            .unwrap_or_default(),
                        result["metadata"]["snapshotVersion"]
                            .as_u64()
                            .unwrap_or_default(),
                        result["metadata"]["targetsVersion"]
                            .as_u64()
                            .unwrap_or_default(),
                    );
                } else {
                    let name = result["name"].as_str().unwrap_or_default();
                    if result["enabled"].as_bool() == Some(false)
                        && result["configured"].as_bool() == Some(true)
                    {
                        println!("disabled {name}");
                    } else {
                        println!("unavailable {name} (production TUF root is not configured)");
                    }
                }
            }
        },
    )
}

pub(crate) fn registry_root(context: &InvocationContext) -> anyhow::Result<PathBuf> {
    let config = crate::commands::config::active_config_path(context)?;
    let parent = config.parent().unwrap_or_else(|| Path::new("."));
    Ok(parent.join("registries"))
}

pub(crate) fn store(context: &InvocationContext) -> anyhow::Result<RegistryStore> {
    Ok(RegistryStore::new(registry_root(context)?))
}

fn confirm(prompt: &str, context: &InvocationContext, non_interactive: &str) -> anyhow::Result<()> {
    if context.output_mode() != OutputMode::Human
        || context.interaction.non_interactive
        || !context.terminal.stdin
        || !context.terminal.stderr
    {
        bail!("{non_interactive}");
    }
    eprint!("{prompt} [y/N] ");
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(coded_error(
            "operation.cancelled",
            "registry operation cancelled",
            ExitClass::Cancelled,
        ))
    }
}
