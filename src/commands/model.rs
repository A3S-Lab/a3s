use std::path::Path;

use a3s_code_core::config::ConfigSection;
use a3s_code_core::CodeConfig;
use anyhow::bail;
use serde_json::{json, Value};

use crate::cli::args::{ModelArgs, ModelCommand, ModelScopeArgs, ModelUseArgs};
use crate::cli::context::InvocationContext;
use crate::cli::output::render_value;
use crate::model::catalog::{ModelCatalog, ModelEntry};
use crate::model::route::ModelRoute;

pub(crate) async fn run(args: ModelArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        ModelCommand::List => list(context).await,
        ModelCommand::Current => current(context),
        ModelCommand::Use(args) => select(args, context).await,
        ModelCommand::Reset(args) => reset(args, context),
    }
}

async fn list(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let (_, config) = super::config::load_active_config(context)?;
    let current = config.default_model.clone();
    let catalog = ModelCatalog::discover_with_config(&config, true).await;
    let models = discovered_models(&catalog.entries, current.as_deref());
    let warnings = catalog.warnings;
    render_value(
        output,
        "model.list",
        json!({"current": current, "models": models, "warnings": warnings}),
        || {
            println!(
                "current: {}",
                current.as_deref().unwrap_or("(not selected)")
            );
            if models.is_empty() {
                println!("no runtime-callable models were discovered");
                return;
            }
            println!("MODEL                                      SOURCE");
            for model in &models {
                let id = model.get("id").and_then(Value::as_str).unwrap_or_default();
                let source = model
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let marker = if model
                    .get("current")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    "*"
                } else {
                    " "
                };
                println!("{marker} {id:<40} {source}");
            }
            for warning in &warnings {
                eprintln!("warning: {warning}");
            }
        },
    )
}

fn current(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let effective = super::config::resolve_effective_config(context)?;
    let path = effective.primary_path;
    let model = effective.config.default_model.clone();
    let source = model
        .as_deref()
        .and_then(|model| model.parse::<ModelRoute>().ok())
        .map(|route| route.source.label());
    let config_source = effective.provenance.get("default_model").cloned();
    render_value(
        output,
        "model.current",
        json!({"model": model, "source": source, "configSource": config_source, "configPath": path}),
        || {
            println!("{}", model.as_deref().unwrap_or("(not selected)"));
            println!("config: {}", path.display());
        },
    )
}

async fn select(args: ModelUseArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let route: ModelRoute = args.model.parse()?;
    let selected_model = route.to_string();
    let (_, effective) = super::config::load_active_config(context)?;
    if !ModelCatalog::route_available_with_config(&route, &effective).await {
        bail!(
            "model `{}` is not available; run `a3s model list` to inspect runtime-callable models",
            selected_model
        );
    }

    let path = super::config::target_config_path(args.target.scope, context)?;
    let mut config = load_target_config(&path)?;
    config.default_model = Some(selected_model.clone());
    crate::api::code_web::config::persistence::persist_config_sections(
        &path,
        &config,
        &[ConfigSection::DefaultModel],
    )
    .map_err(|error| anyhow::anyhow!("could not update {}: {error}", path.display()))?;

    let data = json!({"model": selected_model, "configPath": path});
    render_value(output, "model.use", data, || {
        println!("default model: {selected_model}");
        println!("config: {}", path.display());
    })
}

fn reset(args: ModelScopeArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = super::config::target_config_path(args.scope, context)?;
    let mut config = load_target_config(&path)?;
    let previous = config.default_model.take();
    crate::api::code_web::config::persistence::persist_config_sections(
        &path,
        &config,
        &[ConfigSection::DefaultModel],
    )
    .map_err(|error| anyhow::anyhow!("could not update {}: {error}", path.display()))?;
    render_value(
        output,
        "model.reset",
        json!({"previous": previous, "configPath": path}),
        || {
            if let Some(previous) = previous {
                println!("removed default model `{previous}`");
            } else {
                println!("no default model was selected in {}", path.display());
            }
        },
    )
}

fn load_target_config(path: &Path) -> anyhow::Result<CodeConfig> {
    if !path.exists() {
        return Ok(CodeConfig::default());
    }
    CodeConfig::from_file(path)
        .map_err(|error| anyhow::anyhow!("failed to parse A3S ACL {}: {error}", path.display()))
}

fn discovered_models(entries: &[ModelEntry], current: Option<&str>) -> Vec<Value> {
    let mut models = entries
        .iter()
        .map(|entry| {
            let id = entry.route.to_string();
            json!({
                "id": id,
                "name": entry.display_name,
                "source": entry.route.source.label(),
                "current": current == Some(id.as_str()),
                "reasoning": entry.reasoning,
                "toolCall": entry.tool_call,
                "contextWindow": entry.context_window,
            })
        })
        .collect::<Vec<_>>();
    models.sort_by(|left, right| {
        left.get("id")
            .and_then(Value::as_str)
            .cmp(&right.get("id").and_then(Value::as_str))
    });
    models
}
