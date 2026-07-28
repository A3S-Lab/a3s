use crate::cli::args::{
    OutputMode, TopArgs, TopConnector, TopEventKind, TopRisk, TopSort, TopView,
};
use crate::cli::context::InvocationContext;
use crate::cli::output::{usage_error, CliError, ExitClass};

pub(crate) async fn run(args: TopArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let argv = normalize(args, output)?;
    let result = match output {
        OutputMode::Human => crate::top::run(argv).await,
        OutputMode::Json => {
            crate::top::run_machine(
                argv,
                crate::top::MachineOutput::Json,
                context.cancellation.clone(),
            )
            .await
        }
        OutputMode::Jsonl => {
            crate::top::run_machine(
                argv,
                crate::top::MachineOutput::Jsonl,
                context.cancellation.clone(),
            )
            .await
        }
    };
    match result {
        Err(error) => {
            if let Some(interrupted) = error.downcast_ref::<crate::top::TopInterrupted>() {
                return Err(CliError::new(
                    "operation.cancelled",
                    "top monitoring cancelled",
                    ExitClass::Cancelled,
                )
                .with_jsonl_sequence(interrupted.next_sequence())
                .into());
            }
            Err(error)
        }
        Ok(()) => Ok(()),
    }
}

fn normalize(mut args: TopArgs, output: OutputMode) -> anyhow::Result<Vec<String>> {
    let legacy_views = [
        (args.agents, TopView::Agents, "--agents"),
        (args.view_sessions, TopView::Sessions, "--sessions"),
        (args.view_containers, TopView::Containers, "--containers"),
        (args.view_processes, TopView::Processes, "--processes"),
        (args.view_events, TopView::Events, "--events"),
    ]
    .into_iter()
    .filter(|(selected, _, _)| *selected)
    .collect::<Vec<_>>();
    if legacy_views.len() > 1 || (args.view.is_some() && !legacy_views.is_empty()) {
        return Err(usage_error(
            "select exactly one monitor view with `--view <view>`",
        ));
    }
    let view = args.view.or_else(|| {
        legacy_views.first().map(|(_, view, spelling)| {
            if output == OutputMode::Human {
                eprintln!(
                    "warning: `{spelling}` is deprecated; use `--view {}`",
                    view_name(*view)
                );
            }
            *view
        })
    });

    let mut container = args.container.take();
    if let Some(value) = args.legacy_container.take() {
        if args.watch
            && args.interval.is_none()
            && looks_like_duration(&value)
            && container.is_none()
        {
            if output == OutputMode::Human {
                eprintln!(
                    "warning: `--watch <duration>` is deprecated; use `--watch --interval <duration>`"
                );
            }
            args.interval = Some(value);
        } else if container.is_none() {
            if output == OutputMode::Human {
                eprintln!(
                    "warning: positional container targets are deprecated; use `--container <container>`"
                );
            }
            container = Some(value);
        } else {
            return Err(usage_error("only one container target may be selected"));
        }
    }
    if output == OutputMode::Jsonl && !args.watch {
        return Err(usage_error("JSONL output requires `--watch`"));
    }
    if output == OutputMode::Json && (args.watch || args.interval.is_some()) {
        return Err(usage_error(
            "JSON output returns one snapshot; use JSONL with `--watch` for a stream",
        ));
    }
    if args.count == Some(0) {
        return Err(usage_error("--count must be greater than zero"));
    }
    if args.count.is_some() && output == OutputMode::Human {
        return Err(usage_error("--count is available only with JSONL output"));
    }

    let mut argv = Vec::new();
    if let Some(container) = container {
        argv.extend(["--container".to_string(), container]);
    }
    if let Some(view) = view {
        argv.push(format!("--{}", view_name(view)));
    }
    if let Some(connector) = args.connector {
        argv.extend([
            "--connector".to_string(),
            connector_name(connector).to_string(),
        ]);
    }
    if args.active {
        argv.push("--active".to_string());
    }
    if args.all {
        argv.push("--all".to_string());
    }
    if let Some(filter) = args.filter {
        argv.extend(["--filter".to_string(), filter]);
    }
    if let Some(sort) = args.sort {
        argv.extend(["--sort".to_string(), sort_name(sort).to_string()]);
    }
    if args.reverse {
        argv.push("--reverse".to_string());
    }
    if let Some(risk) = args.risk {
        argv.extend(["--risk".to_string(), risk_name(risk).to_string()]);
    }
    if let Some(kind) = args.kind {
        argv.extend(["--kind".to_string(), kind_name(kind).to_string()]);
    }
    if args.compact {
        argv.push("--compact".to_string());
    }
    if args.no_header {
        argv.push("--no-header".to_string());
    }
    if args.invert {
        argv.push("--invert".to_string());
    }
    if args.watch || args.interval.is_some() {
        argv.extend([
            "--watch".to_string(),
            args.interval.unwrap_or_else(|| "1500ms".to_string()),
        ]);
    }
    if let Some(count) = args.count {
        argv.extend(["--count".to_string(), count.to_string()]);
    }
    Ok(argv)
}

fn looks_like_duration(value: &str) -> bool {
    let value = value.trim();
    value.ends_with("ms")
        || value.ends_with('s')
        || (!value.is_empty() && value.chars().all(|character| character.is_ascii_digit()))
}

fn view_name(value: TopView) -> &'static str {
    match value {
        TopView::Agents => "agents",
        TopView::Sessions => "sessions",
        TopView::Containers => "containers",
        TopView::Processes => "processes",
        TopView::Events => "events",
    }
}

fn connector_name(value: TopConnector) -> &'static str {
    match value {
        TopConnector::A3sBox => "a3s-box",
        TopConnector::Docker => "docker",
        TopConnector::Runc => "runc",
    }
}

fn sort_name(value: TopSort) -> &'static str {
    match value {
        TopSort::Cpu => "cpu",
        TopSort::Mem => "mem",
        TopSort::Net => "net",
        TopSort::Block => "block",
        TopSort::Pids => "pids",
        TopSort::State => "state",
        TopSort::Id => "id",
        TopSort::Uptime => "uptime",
        TopSort::Name => "name",
        TopSort::Tokens => "tokens",
    }
}

fn risk_name(value: TopRisk) -> &'static str {
    match value {
        TopRisk::All => "all",
        TopRisk::Medium => "medium",
        TopRisk::High => "high",
    }
}

fn kind_name(value: TopEventKind) -> &'static str {
    match value {
        TopEventKind::All => "all",
        TopEventKind::Tool => "tool",
        TopEventKind::Security => "security",
        TopEventKind::File => "file",
        TopEventKind::Egress => "egress",
        TopEventKind::Llm => "llm",
        TopEventKind::Other => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_watch_is_adapted_to_the_existing_snapshot_engine() {
        let args = TopArgs {
            view: Some(TopView::Events),
            watch: true,
            interval: Some("2s".to_string()),
            count: Some(3),
            ..TopArgs::default()
        };
        assert_eq!(
            normalize(args, OutputMode::Jsonl).unwrap(),
            ["--events", "--watch", "2s", "--count", "3"]
        );
    }
}
