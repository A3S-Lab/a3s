use std::path::{Path, PathBuf};
use std::process::Command;

use a3s_code_core::CodeConfig;
use anyhow::{bail, Context};
use serde_json::json;

use crate::cli::args::{ConfigArgs, ConfigCommand, ConfigScope, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, usage_error};

#[derive(Clone, Debug)]
pub(crate) struct CodeAssetDirectories {
    pub agent: PathBuf,
    pub mcp: PathBuf,
    pub skill: PathBuf,
    pub flow: PathBuf,
    pub okf: PathBuf,
}

/// Effective configuration and paths for one A3S Code runtime invocation.
///
/// Interactive and non-interactive Code entry points resolve this value once
/// and pass its fields down explicitly. Runtime code must not rediscover these
/// paths from the process current directory.
#[derive(Debug)]
pub(crate) struct CodeRuntimeConfiguration {
    pub config: CodeConfig,
    pub config_path: PathBuf,
    pub asset_directories: CodeAssetDirectories,
    pub memory_dir: PathBuf,
}

pub(crate) async fn run(args: ConfigArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        ConfigCommand::Path => show_path(context),
        ConfigCommand::Paths => show_paths(context),
        ConfigCommand::Show => show(context),
        ConfigCommand::Init(args) => init(args.scope, args.force, context),
        ConfigCommand::Edit(args) => edit(args.scope, context),
        ConfigCommand::Validate(args) => validate(args.path.as_deref(), context),
    }
}

pub(crate) fn active_config_path(context: &InvocationContext) -> anyhow::Result<PathBuf> {
    if let Some(path) = context.explicit_config.clone() {
        return Ok(path);
    }
    if let Some(path) = super::config_resolver::workspace_config_path(&context.directory) {
        return Ok(path);
    }
    user_config_path(context)
}

pub(crate) fn target_config_path(
    scope: ConfigScope,
    context: &InvocationContext,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = context.explicit_config.clone() {
        return Ok(path);
    }
    match scope {
        ConfigScope::Workspace => Ok(context.directory.join(".a3s/config.acl")),
        ConfigScope::User => user_config_path(context),
    }
}

pub(crate) fn load_active_config(
    context: &InvocationContext,
) -> anyhow::Result<(PathBuf, CodeConfig)> {
    let effective = resolve_effective_config(context)?;
    Ok((effective.primary_path, effective.config))
}

pub(crate) fn resolve_effective_config(
    context: &InvocationContext,
) -> anyhow::Result<super::config_resolver::EffectiveConfig> {
    super::config_resolver::resolve(context)
}

pub(crate) fn resolve_code_runtime_configuration(
    context: &InvocationContext,
) -> anyhow::Result<CodeRuntimeConfiguration> {
    let effective = resolve_effective_config(context)?;
    let asset_directories = code_asset_directories_from_effective(context, Some(&effective))?;
    let memory_dir = context
        .environment
        .nonempty_var_os("A3S_MEMORY_DIR")
        .map(PathBuf::from)
        .or_else(|| effective.config.memory_dir.clone())
        .map(|path| context.resolve_path(path))
        .unwrap_or_else(|| context.directory.join(".a3s/memory"));

    Ok(CodeRuntimeConfiguration {
        config: effective.config,
        config_path: effective.primary_path,
        asset_directories,
        memory_dir,
    })
}

pub(crate) fn memory_directory(context: &InvocationContext) -> anyhow::Result<PathBuf> {
    if let Some(path) = context.environment.nonempty_var_os("A3S_MEMORY_DIR") {
        return Ok(context.resolve_path(PathBuf::from(path)));
    }
    let configured =
        optional_effective_config(context)?.and_then(|effective| effective.config.memory_dir);
    Ok(configured_asset_directory(
        context,
        "A3S_MEMORY_DIR",
        configured,
        ".a3s/memory",
    ))
}

pub(crate) fn code_asset_directories(
    context: &InvocationContext,
) -> anyhow::Result<CodeAssetDirectories> {
    let effective = optional_effective_config(context)?;
    code_asset_directories_from_effective(context, effective.as_ref())
}

fn code_asset_directories_from_effective(
    context: &InvocationContext,
    effective: Option<&super::config_resolver::EffectiveConfig>,
) -> anyhow::Result<CodeAssetDirectories> {
    let agent = effective
        .map(|value| {
            layered_acl_path(
                &value.layers,
                &["agent_dir", "agentDir", "agent_dirs", "agentDirs"],
                context,
            )
        })
        .transpose()?
        .flatten()
        .or_else(|| effective.and_then(|value| value.config.agent_dirs.first().cloned()));
    let skill = effective
        .map(|value| {
            layered_acl_path(
                &value.layers,
                &["skill_dir", "skillDir", "skill_dirs", "skillDirs"],
                context,
            )
        })
        .transpose()?
        .flatten()
        .or_else(|| effective.and_then(|value| value.config.skill_dirs.first().cloned()));
    let mcp = effective
        .map(|value| layered_acl_path(&value.layers, &["mcp_dir", "mcpDir"], context))
        .transpose()?
        .flatten();
    let flow = effective
        .map(|value| layered_acl_path(&value.layers, &["flow_dir", "flowDir"], context))
        .transpose()?
        .flatten();
    Ok(CodeAssetDirectories {
        agent: configured_asset_directory(context, "A3S_AGENT_DIR", agent, ".a3s/agents"),
        mcp: configured_asset_directory(context, "A3S_MCP_DIR", mcp, ".a3s/mcps"),
        skill: configured_asset_directory(context, "A3S_SKILL_DIR", skill, ".a3s/skills"),
        flow: configured_asset_directory(context, "A3S_FLOW_DIR", flow, ".a3s/flows"),
        okf: context.directory.join("okf"),
    })
}

fn optional_effective_config(
    context: &InvocationContext,
) -> anyhow::Result<Option<super::config_resolver::EffectiveConfig>> {
    let configured = context.explicit_config.is_some()
        || super::config_resolver::workspace_config_path(&context.directory).is_some()
        || context
            .user_config_path()
            .is_some_and(|path| path.is_file());
    configured
        .then(|| resolve_effective_config(context))
        .transpose()
}

fn layered_acl_path(
    layers: &[super::config_resolver::ConfigLayer],
    keys: &[&str],
    context: &InvocationContext,
) -> anyhow::Result<Option<PathBuf>> {
    let mut selected = None;
    for layer in layers {
        let source = std::fs::read_to_string(&layer.path)
            .with_context(|| format!("could not read A3S ACL {}", layer.path.display()))?;
        let document = a3s_acl::parse_acl(&source)
            .with_context(|| format!("invalid A3S ACL {}", layer.path.display()))?;
        for block in document
            .blocks
            .iter()
            .filter(|block| keys.contains(&block.name.as_str()))
        {
            let value = keys
                .iter()
                .find_map(|key| block.attributes.get(*key))
                .with_context(|| {
                    format!(
                        "A3S ACL {} must assign a value to {}",
                        layer.path.display(),
                        block.name
                    )
                })?;
            selected = acl_path_value(value, &layer.path, context)?;
        }
    }
    Ok(selected)
}

fn acl_path_value(
    value: &a3s_acl::Value,
    source: &Path,
    context: &InvocationContext,
) -> anyhow::Result<Option<PathBuf>> {
    match value {
        a3s_acl::Value::String(path) => Ok(Some(PathBuf::from(path))),
        a3s_acl::Value::List(paths) => paths
            .first()
            .map(|path| acl_path_value(path, source, context))
            .transpose()
            .map(Option::flatten),
        a3s_acl::Value::Call(name, arguments) if name == "env" => {
            let variable = arguments
                .first()
                .and_then(a3s_acl::Value::as_str)
                .with_context(|| {
                    format!("env() in {} requires one variable name", source.display())
                })?;
            context
                .environment
                .nonempty_var_os(variable)
                .map(PathBuf::from)
                .with_context(|| {
                    format!(
                        "{} referenced by A3S ACL {} is not set",
                        variable,
                        source.display()
                    )
                })
                .map(Some)
        }
        _ => bail!(
            "asset directory in {} must be a string path or non-empty path list",
            source.display()
        ),
    }
}

fn show_path(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = active_config_path(context)?;
    let exists = path.is_file();
    let data = json!({
        "path": path,
        "exists": exists,
        "explicit": context.explicit_config.is_some(),
    });
    render_value(output, "config.path", data, || {
        println!("{}", path.display());
        if !exists {
            eprintln!("not created; run `a3s config init`");
        }
    })
}

fn show_paths(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let config = active_config_path(context)?;
    let user_config = user_config_path(context)?;
    let workspace_config = context.directory.join(".a3s/config.acl");
    let data_root = context.component_paths.data_root.clone();
    let state_root = context.component_paths.state_root.clone();
    let cache_root = context.component_paths.cache_root.clone();
    let CodeAssetDirectories {
        agent,
        mcp,
        skill,
        flow,
        okf,
    } = code_asset_directories(context)?;
    let memory = memory_directory(context)?;
    let kb = crate::tui::kbutil::kb_dir(&context.directory.to_string_lossy());

    let data = json!({
        "config": config,
        "userConfig": user_config,
        "workspaceConfig": workspace_config,
        "data": data_root,
        "state": state_root,
        "cache": cache_root,
        "assets": {
            "agent": agent,
            "mcp": mcp,
            "skill": skill,
            "flow": flow,
        },
        "memory": memory,
        "knowledgeBase": kb,
        "okf": okf,
    });
    render_value(output, "config.paths", data, || {
        for (name, path) in [
            ("config", config.as_path()),
            ("user config", user_config.as_path()),
            ("workspace config", workspace_config.as_path()),
            ("data", data_root.as_path()),
            ("state", state_root.as_path()),
            ("cache", cache_root.as_path()),
            ("agent", agent.as_path()),
            ("mcp", mcp.as_path()),
            ("skill", skill.as_path()),
            ("flow", flow.as_path()),
            ("memory", memory.as_path()),
            ("kb", kb.as_path()),
            ("okf", okf.as_path()),
        ] {
            println!("{name:<18} {}", path.display());
        }
    })
}

fn show(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let effective = resolve_effective_config(context)?;
    let path = effective.primary_path;
    let config = effective.config;
    let layers = effective.layers;
    let provenance = effective.provenance;
    let explicit = effective.explicit;
    let models = config
        .list_models()
        .into_iter()
        .map(|(provider, model)| {
            json!({
                "id": format!("{}/{}", provider.name, model.id),
                "name": model.name,
                "reasoning": model.reasoning,
                "toolCall": model.tool_call,
            })
        })
        .collect::<Vec<_>>();
    let providers = config
        .providers
        .iter()
        .map(|provider| provider.name.clone())
        .collect::<Vec<_>>();
    let default_model = config.default_model.clone();
    let os_address = config.os.as_ref().map(|os| os.address.clone());
    let data = json!({
        "path": path,
        "explicit": explicit,
        "layers": layers,
        "provenance": provenance,
        "defaultModel": default_model,
        "providers": providers,
        "models": models,
        "os": {
            "configured": os_address.is_some(),
            "address": os_address,
        },
    });
    render_value(output, "config.show", data, || {
        println!("config: {}", path.display());
        if explicit {
            println!("resolution: explicit single file");
        } else {
            println!("layers: {}", layers.len());
            for layer in &layers {
                println!("  {:?}: {}", layer.kind, layer.path.display());
            }
        }
        println!(
            "default model: {}",
            default_model.as_deref().unwrap_or("(not set)")
        );
        println!("providers: {}", providers.len());
        println!("models: {}", models.len());
        println!(
            "os: {}",
            os_address.as_deref().unwrap_or("(not configured)")
        );
    })
}

fn init(scope: ConfigScope, force: bool, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = target_config_path(scope, context)?;
    let existed = path.exists();
    if existed && !force {
        let data = json!({"path": path, "created": false, "replaced": false});
        return render_value(output, "config.init", data, || {
            println!("config already exists: {}", path.display());
        });
    }

    CodeConfig::from_acl(crate::config::config_template())
        .map_err(|error| anyhow::anyhow!("built-in A3S ACL template is invalid: {error}"))?;
    crate::api::code_web::config::persistence::write_atomic(
        &path,
        crate::config::config_template().as_bytes(),
    )
    .map_err(|error| anyhow::anyhow!("could not write {}: {error}", path.display()))?;

    let data = json!({"path": path, "created": !existed, "replaced": existed});
    render_value(output, "config.init", data, || {
        if existed {
            println!("replaced config: {}", path.display());
        } else {
            println!("created config: {}", path.display());
        }
    })
}

fn edit(scope: ConfigScope, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = target_config_path(scope, context)?;
    if !path.exists() {
        crate::config::write_template_config(&path)
            .with_context(|| format!("could not create {}", path.display()))?;
    }
    if output != OutputMode::Human {
        return Err(usage_error(
            "`a3s config edit` is interactive and requires human output mode",
        ));
    }
    open_editor(&path, context)?;
    render_value(output, "config.edit", json!({"path": path}), || {})
}

fn validate(path: Option<&Path>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let (path, config, layers) = match path {
        Some(path) => {
            let path = context.resolve_path(path.to_path_buf());
            let config = CodeConfig::from_file(&path)
                .map_err(|error| anyhow::anyhow!("invalid A3S ACL {}: {error}", path.display()))?;
            (path, config, None)
        }
        None => {
            let effective = resolve_effective_config(context)?;
            (
                effective.primary_path,
                effective.config,
                Some(effective.layers),
            )
        }
    };
    let issues = crate::api::code_web::config::validation::validate_config(&config);
    if !issues.is_empty() {
        bail!("invalid A3S ACL {}: {}", path.display(), issues.join("; "));
    }
    let data = json!({
        "path": path,
        "valid": true,
        "layers": layers,
        "providers": config.providers.len(),
        "models": config.list_models().len(),
    });
    render_value(output, "config.validate", data, || {
        if let Some(layers) = layers {
            println!("valid effective A3S ACL ({} layers)", layers.len());
            for layer in layers {
                println!("  {:?}: {}", layer.kind, layer.path.display());
            }
        } else {
            println!("valid A3S ACL: {}", path.display());
        }
    })
}

fn user_config_path(context: &InvocationContext) -> anyhow::Result<PathBuf> {
    context
        .user_config_path()
        .ok_or_else(|| anyhow::anyhow!("user home is unavailable; pass `--config <path>`"))
}

fn configured_asset_directory(
    context: &InvocationContext,
    environment_name: &str,
    configured: Option<PathBuf>,
    fallback: &str,
) -> PathBuf {
    context
        .environment
        .nonempty_var_os(environment_name)
        .map(PathBuf::from)
        .or(configured)
        .map(|path| context.resolve_path(path))
        .or_else(|| context.home.as_deref().map(|home| home.join(fallback)))
        .unwrap_or_else(|| context.directory.join(fallback))
}

fn open_editor(path: &Path, context: &InvocationContext) -> anyhow::Result<()> {
    let editor = context
        .environment
        .utf8("VISUAL")?
        .filter(|value| !value.trim().is_empty())
        .or(context
            .environment
            .utf8("EDITOR")?
            .filter(|value| !value.trim().is_empty()));
    let Some(editor) = editor else {
        println!("{}", path.display());
        println!("set VISUAL or EDITOR to edit from the CLI");
        return Ok(());
    };
    let mut parts = editor.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("editor command is empty"))?;
    let status = Command::new(program)
        .args(parts)
        .arg(path)
        .current_dir(&context.directory)
        .status()
        .with_context(|| format!("failed to launch editor `{editor}`"))?;
    if !status.success() {
        bail!("editor `{editor}` exited with {status}");
    }
    Ok(())
}
