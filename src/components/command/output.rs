use std::io::{self, Write};

use serde::Serialize;

use super::super::catalog::{self, ComponentKind};
use super::super::lifecycle::OperationRecord;
use super::super::plan::OperationPlanSet;
use super::super::state::{ComponentReport, Health, Presence};
use super::{ComponentBatchFailure, ComponentFailure};

pub(super) fn print_human_report(report: &ComponentReport) {
    println!(
        "{:<18} {:<11} {:<17} {:<12} SOURCE",
        "COMPONENT", "TYPE", "STATUS", "VERSION"
    );
    for component in &report.components {
        let status = match (component.presence, component.health) {
            (Presence::Missing, _) => "missing".to_string(),
            (_, Health::Broken) => "broken".to_string(),
            (Presence::System, Health::Ready) => "ready (system)".to_string(),
            (Presence::External, Health::Ready) => "ready (external)".to_string(),
            (_, Health::Ready) => "ready".to_string(),
            _ => "unknown".to_string(),
        };
        let source = component
            .provenance
            .map(|value| format!("{value:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "-".to_string());
        println!(
            "{:<18} {:<11} {:<17} {:<12} {}",
            component.id,
            format!("{:?}", component.kind).to_ascii_lowercase(),
            status,
            component.version.as_deref().unwrap_or("-"),
            source
        );
        if let Some(message) = &component.message {
            println!("  note: {message}");
        }
    }
    if !report.external_tools.is_empty() {
        println!();
        println!("EXTERNAL TOOLS (discovered, never activated automatically)");
        for tool in &report.external_tools {
            println!(
                "  {:<16} {:<20} {}",
                tool.command,
                tool.binary,
                tool.path.display()
            );
        }
    }
}

pub(super) fn print_available(json: bool) -> anyhow::Result<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Available<'a> {
        schema_version: u32,
        components: Vec<AvailableItem<'a>>,
    }
    #[derive(Serialize)]
    struct AvailableItem<'a> {
        id: &'a str,
        kind: ComponentKind,
        description: &'a str,
    }
    let output = Available {
        schema_version: 1,
        components: catalog::all()
            .iter()
            .map(|component| AvailableItem {
                id: component.id,
                kind: component.kind,
                description: component.description,
            })
            .collect(),
    };
    if json {
        print_json("component.install", &output, true)?;
    } else {
        println!("Available A3S components:");
        for component in catalog::all() {
            println!("  {:<18} {}", component.id, component.description);
        }
    }
    Ok(())
}

fn print_operations(
    command: &'static str,
    plan_digest: &str,
    operations: &[OperationRecord],
    json: bool,
) -> anyhow::Result<()> {
    if json {
        print_json(
            command,
            &serde_json::json!({"planDigest": plan_digest, "operations": operations}),
            true,
        )?;
    } else {
        println!("plan digest: {plan_digest}");
        for operation in operations {
            let marker = if operation.changed { "✓" } else { "=" };
            println!("{marker} {}", operation.message);
        }
    }
    Ok(())
}

pub(super) fn finish_batch(
    command: &'static str,
    action: &'static str,
    plan_digest: String,
    operations: Vec<OperationRecord>,
    failures: Vec<ComponentFailure>,
    json: bool,
) -> anyhow::Result<()> {
    if failures.is_empty() {
        return print_operations(command, &plan_digest, &operations, json);
    }
    if !json && !operations.is_empty() {
        print_operations(command, &plan_digest, &operations, false)?;
    }
    Err(ComponentBatchFailure {
        action,
        plan_digest,
        operations,
        failures,
    }
    .into())
}

pub(super) fn print_plans(
    command: &'static str,
    plan_set: &OperationPlanSet,
    json: bool,
) -> anyhow::Result<()> {
    if json {
        print_json(
            command,
            &serde_json::json!({
                "dryRun": true,
                "planSchemaVersion": plan_set.plan_schema_version,
                "planCommand": plan_set.plan_command,
                "planDigest": plan_set.plan_digest,
                "plans": plan_set.plans,
            }),
            true,
        )?;
    } else {
        plan_set.print_human();
    }
    Ok(())
}

pub(super) fn print_json(
    command: &'static str,
    data: &impl Serialize,
    ok: bool,
) -> anyhow::Result<()> {
    let value = serde_json::json!({
        "schemaVersion": 1,
        "command": command,
        "ok": ok,
        "data": data,
        "warnings": [],
    });
    let bytes = serde_json::to_vec_pretty(&value)?;
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(&bytes)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}
