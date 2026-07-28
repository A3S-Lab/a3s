use anyhow::Context;
use serde_json::json;

use crate::cli::args::{KbArgs, KbCommand, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, usage_error};

pub(super) fn run(args: KbArgs, context: &InvocationContext) -> anyhow::Result<()> {
    if context.output_mode() == OutputMode::Jsonl {
        return Err(usage_error(
            "Code knowledge commands do not support JSONL output",
        ));
    }
    let workspace = context
        .directory
        .to_str()
        .context("the effective workspace path must be valid UTF-8 for the knowledge store")?;
    match args.command {
        KbCommand::Stats => stats(workspace, context),
        KbCommand::Add(args) => add(workspace, &args.text, context),
        KbCommand::Import(args) => {
            let path = context.resolve_path(args.path);
            let path = path
                .to_str()
                .context("the knowledge import path must be valid UTF-8")?;
            import(workspace, path, context)
        }
        KbCommand::Search(args) => search(workspace, &args.text, context),
        KbCommand::Path => path(workspace, context),
    }
}

fn stats(workspace: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let root = crate::tui::kbutil::kb_dir(workspace);
    let stats = crate::tui::kbutil::kb_stats(workspace);
    let recent = crate::tui::kbutil::recent_sources(workspace, 8);
    let data = json!({
        "path": root,
        "sources": stats.sources,
        "concepts": stats.concepts,
        "imports": stats.imports,
        "bytes": stats.bytes,
        "recent": recent,
    });
    render_value(output, "code.kb.stats", data, || {
        println!("kb: {}", root.display());
        println!(
            "sources: {} · concepts: {} · imports: {} · size: {}",
            stats.sources,
            stats.concepts,
            stats.imports,
            format_bytes(stats.bytes)
        );
        if recent.is_empty() {
            println!("recent: (none)");
        } else {
            println!("recent:");
            for item in recent {
                println!("  {item}");
            }
        }
    })
}

fn add(workspace: &str, text: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let destination =
        crate::tui::kbutil::capture_text(workspace, text, &chrono::Utc::now().to_rfc3339())
            .context("could not add text to the workspace knowledge base")?;
    let root = crate::tui::kbutil::kb_dir(workspace);
    render_value(
        output,
        "code.kb.add",
        json!({
            "created": true,
            "path": destination,
            "knowledgeBase": root,
        }),
        || println!("captured note to KB: {}", destination.display()),
    )
}

fn import(workspace: &str, source: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let outcome =
        crate::tui::kbutil::import_source(workspace, source, &chrono::Utc::now().to_rfc3339())
            .with_context(|| {
                format!("could not import {source} into the workspace knowledge base")
            })?;
    let kind = match outcome.kind {
        crate::tui::kbutil::ImportKind::File => "file",
        crate::tui::kbutil::ImportKind::Folder => "directory",
    };
    render_value(
        output,
        "code.kb.import",
        json!({
            "source": outcome.source,
            "destination": outcome.destination,
            "kind": kind,
            "added": outcome.added,
            "skipped": outcome.skipped,
            "capped": outcome.capped,
        }),
        || {
            println!(
                "imported {} file(s) from {} to {}",
                outcome.added,
                outcome.source.display(),
                outcome.destination.display()
            );
            if outcome.skipped > 0 {
                println!("skipped: {}", outcome.skipped);
            }
            if outcome.capped {
                println!("warning: directory import reached the safety file limit");
            }
        },
    )
}

fn search(workspace: &str, query: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let hits = crate::tui::kbutil::search_kb(workspace, query);
    let values = hits
        .iter()
        .map(|hit| {
            json!({
                "path": hit.path,
                "line": hit.line,
                "snippet": hit.snippet,
            })
        })
        .collect::<Vec<_>>();
    render_value(
        output,
        "code.kb.search",
        json!({"query": query, "hits": values}),
        || {
            println!("{} hit(s) for `{query}`", hits.len());
            for hit in hits {
                println!("{}:{}\t{}", hit.path, hit.line, hit.snippet);
            }
        },
    )
}

fn path(workspace: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let path = crate::tui::kbutil::kb_dir(workspace);
    render_value(
        output,
        "code.kb.path",
        json!({"path": path, "exists": path.is_dir()}),
        || println!("{}", path.display()),
    )
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}
