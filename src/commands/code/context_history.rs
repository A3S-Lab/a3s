use std::process::Stdio;

use anyhow::{bail, Context};
use serde_json::json;

use crate::cli::args::{ContextArgs, ContextCommand, ContextShowCommand, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, usage_error};

pub(super) async fn run(args: ContextArgs, context: &InvocationContext) -> anyhow::Result<()> {
    if context.output_mode() == OutputMode::Jsonl {
        return Err(usage_error(
            "Code context-history commands do not support JSONL output",
        ));
    }
    match args.command {
        ContextCommand::Search(args) => search(&args.query, context).await,
        ContextCommand::Show(args) => match args.command {
            ContextShowCommand::Event(args) => {
                if args.window == 0 {
                    return Err(usage_error("--window must be greater than zero"));
                }
                show_event(&args.event_id, args.window, context).await
            }
            ContextShowCommand::Session(args) => show_session(&args.session_id, context).await,
        },
    }
}

async fn search(query: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let stdout = run_ctx(
        &[
            "search",
            "--refresh",
            "off",
            "--limit",
            "8",
            "--json",
            "--",
            query,
        ],
        context,
    )
    .await?;
    let hits = crate::tui::parse_ctx_search(&stdout).map_err(anyhow::Error::msg)?;
    let values = hits
        .iter()
        .map(|hit| {
            json!({
                "eventId": hit.event_id,
                "sessionId": hit.session_id,
                "provider": hit.provider,
                "time": hit.time,
                "title": hit.title,
                "snippet": hit.snippet,
            })
        })
        .collect::<Vec<_>>();
    render_value(
        context.output_mode(),
        "code.context.search",
        json!({"query": query, "hits": values}),
        || {
            println!("{} context hit(s) for `{query}`", hits.len());
            for (index, hit) in hits.iter().enumerate() {
                println!(
                    "{}. {} · {} · {}",
                    index + 1,
                    hit.provider,
                    hit.time,
                    hit.title
                );
                println!("   event: {}", hit.event_id);
                if !hit.session_id.is_empty() {
                    println!("   session: {}", hit.session_id);
                }
                if !hit.snippet.is_empty() {
                    println!("   {}", hit.snippet);
                }
            }
        },
    )
}

async fn show_event(
    event_id: &str,
    window: usize,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    let window_text = window.to_string();
    let content = run_ctx(
        &["show", "event", event_id, "--window", &window_text],
        context,
    )
    .await?;
    let content = crate::tui::strip_controls(&content);
    render_content(
        "code.context.show.event",
        json!({"eventId": event_id, "window": window, "content": content}),
        &content,
        context,
    )
}

async fn show_session(session_id: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let content = run_ctx(&["show", "session", session_id], context).await?;
    let content = crate::tui::strip_controls(&content);
    render_content(
        "code.context.show.session",
        json!({"sessionId": session_id, "content": content}),
        &content,
        context,
    )
}

fn render_content(
    command: &'static str,
    data: serde_json::Value,
    content: &str,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    render_value(context.output_mode(), command, data, || {
        print!("{content}");
        if !content.ends_with('\n') {
            println!();
        }
    })
}

async fn run_ctx(args: &[&str], context: &InvocationContext) -> anyhow::Result<String> {
    let output = tokio::process::Command::new("ctx")
        .args(args)
        .current_dir(&context.directory)
        .stdin(Stdio::null())
        .output()
        .await
        .context("failed to run the ctx history CLI")?;
    if !output.status.success() {
        let stderr = crate::tui::strip_controls(&String::from_utf8_lossy(&output.stderr));
        let stderr = stderr.trim();
        if stderr.is_empty() {
            bail!("ctx history CLI exited with {}", output.status);
        }
        bail!("ctx history CLI failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
