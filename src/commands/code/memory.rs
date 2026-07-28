use serde_json::json;

use crate::cli::args::{MemoryArgs, MemoryCommand, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, usage_error};

pub(super) fn run(args: MemoryArgs, context: &InvocationContext) -> anyhow::Result<()> {
    if context.output_mode() == OutputMode::Jsonl {
        return Err(usage_error(
            "Code memory commands do not support JSONL output",
        ));
    }
    let path = crate::commands::config::memory_directory(context)?;
    match args.command {
        MemoryCommand::List(args) => list(path, args.query.as_deref().unwrap_or_default(), context),
        MemoryCommand::Stats => stats(path, context),
        MemoryCommand::Path => show_path(path, context),
    }
}

fn list(path: std::path::PathBuf, query: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let data = crate::tui::memutil::load_panel_data(&path);
    let entries = data
        .entries
        .iter()
        .filter(|entry| {
            let content = memory_content(&data, entry);
            let haystack = format!("{} {} {}", entry.id, entry.tags.join(" "), content);
            matches_query(&haystack, query)
        })
        .collect::<Vec<_>>();
    let values = entries
        .iter()
        .map(|entry| {
            json!({
                "id": entry.id,
                "type": if entry.memory_type.is_empty() { "memory" } else { &entry.memory_type },
                "importance": entry.importance,
                "timestamp": entry.timestamp,
                "tags": entry.tags,
                "content": memory_content(&data, entry),
            })
        })
        .collect::<Vec<_>>();
    render_value(
        context.output_mode(),
        "code.memory.list",
        json!({"path": path, "query": query, "entries": values}),
        || {
            println!(
                "{} memory entr{} in {}",
                entries.len(),
                if entries.len() == 1 { "y" } else { "ies" },
                path.display()
            );
            for entry in entries {
                let content = memory_content(&data, entry);
                println!(
                    "{}\t{}\t{:.2}\t{}\t{}",
                    trim_column(&entry.id, 8),
                    if entry.memory_type.is_empty() {
                        "memory"
                    } else {
                        entry.memory_type.as_str()
                    },
                    entry.importance,
                    entry.timestamp.format("%Y-%m-%d"),
                    trim_column(&content.replace('\n', " "), 120)
                );
            }
        },
    )
}

fn stats(path: std::path::PathBuf, context: &InvocationContext) -> anyhow::Result<()> {
    let data = crate::tui::memutil::load_panel_data(&path);
    let stats = &data.graph.stats;
    let value = json!({
        "path": path,
        "entries": data.entries.len(),
        "graph": {
            "events": stats.events,
            "entities": stats.entities,
            "relations": stats.relations,
            "aliases": stats.aliases,
            "llmExtracted": stats.llm_extracted,
            "consolidated": stats.consolidated,
            "conflicts": stats.conflicts,
        },
        "tiers": {
            "short": stats.short,
            "mid": stats.mid,
            "long": stats.long,
            "forgetCandidates": stats.forget_candidates,
        },
    });
    render_value(context.output_mode(), "code.memory.stats", value, || {
        println!("memory: {}", path.display());
        println!("entries: {}", data.entries.len());
        println!(
            "graph: {} event(s) · {} entity(ies) · {} relation(s) · {} alias(es)",
            stats.events, stats.entities, stats.relations, stats.aliases
        );
        println!(
            "tiers: short {} · mid {} · long {} · forget candidates {}",
            stats.short, stats.mid, stats.long, stats.forget_candidates
        );
    })
}

fn show_path(path: std::path::PathBuf, context: &InvocationContext) -> anyhow::Result<()> {
    render_value(
        context.output_mode(),
        "code.memory.path",
        json!({"path": path, "exists": path.is_dir()}),
        || println!("{}", path.display()),
    )
}

fn memory_content(
    data: &crate::tui::memutil::MemPanelData,
    entry: &crate::tui::memutil::MemEntry,
) -> String {
    data.details
        .get(&entry.id)
        .map(|detail| detail.content.trim().to_string())
        .filter(|content| !content.is_empty())
        .unwrap_or_else(|| entry.content_lower.clone())
}

fn matches_query(text: &str, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    query.is_empty() || text.to_ascii_lowercase().contains(&query)
}

fn trim_column(value: &str, width: usize) -> String {
    let mut output = value.chars().take(width).collect::<String>();
    if value.chars().count() > width && width >= 1 {
        output.pop();
        output.push('~');
    }
    output
}
