mod acquisition;
mod corpus;
mod evaluation;
mod planning;
mod report;
mod synthesis;

use super::*;
use clap::Parser;
use corpus::{LiveCase, LiveCorpus};
use planning::EvaluationStrategy;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const LIVE_EVALUATOR_PROTOCOL: &str = "core-acquisition-ab/v1";

#[tokio::test]
#[ignore = "live C01-C09 equal-budget DeepResearch product measurement"]
async fn live_corpus_equal_budget_comparison() {
    let corpus = corpus::load_live_corpus().expect("load versioned live corpus");
    let run_indices = selected_run_indices(&corpus).expect("select live run indices");
    let selected_case =
        std::env::var("A3S_DEEP_RESEARCH_EVAL_CASE").unwrap_or_else(|_| "C01".to_string());
    let selected_strategy =
        std::env::var("A3S_DEEP_RESEARCH_EVAL_STRATEGY").unwrap_or_else(|_| "both".to_string());
    let effective_config =
        load_live_evaluator_config().expect("resolve live evaluator configuration");
    let model = std::env::var("A3S_DEEP_RESEARCH_EVAL_MODEL")
        .ok()
        .or_else(|| effective_config.config.default_model.clone())
        .expect("live evaluator requires a configured model");
    let output_root = live_output_root();
    std::fs::create_dir_all(&output_root).expect("create live evaluator output root");
    let config_identity = evaluator_config_identity(&effective_config, &model)
        .expect("record live evaluator configuration identity");
    persist_json(output_root.join("evaluator-config.json"), &config_identity)
        .expect("persist live evaluator configuration identity");
    let config = effective_config.config;
    let mut measurements = Vec::new();
    let mut errors = Vec::new();

    for run_index in run_indices {
        let cases =
            selected_cases(&corpus, &selected_case, run_index).expect("select live corpus cases");
        let strategies = selected_strategies(&selected_strategy, run_index)
            .expect("select live evaluator strategies");
        for case in cases {
            for strategy in strategies.iter().copied() {
                let output_dir = output_root
                    .join(&case.id)
                    .join(strategy.label())
                    .join(format!("run-{run_index}"));
                std::fs::create_dir_all(&output_dir).expect("create live measurement directory");
                let measurement = tokio::time::timeout(
                    std::time::Duration::from_millis(corpus.budget.wall_clock_ms),
                    run_measurement(
                        case,
                        run_index,
                        strategy,
                        &corpus,
                        &config,
                        &model,
                        &output_dir,
                    ),
                )
                .await;
                match measurement {
                    Ok(Ok(summary)) => measurements.push(summary),
                    Ok(Err(error)) => {
                        persist_measurement_error(&output_dir, &error);
                        errors.push(format!(
                            "{}/{} run {}: {}",
                            case.id,
                            strategy.label(),
                            run_index,
                            error
                        ));
                    }
                    Err(_) => {
                        let error = format!(
                            "measurement exceeded the {} ms global wall-clock cap",
                            corpus.budget.wall_clock_ms
                        );
                        persist_measurement_error(&output_dir, &error);
                        errors.push(format!(
                            "{}/{} run {}: {}",
                            case.id,
                            strategy.label(),
                            run_index,
                            error
                        ));
                    }
                }
            }
        }
    }

    let matrix = serde_json::json!({
        "schema": "a3s/deep-research-live-matrix/v1",
        "evaluator_protocol": LIVE_EVALUATOR_PROTOCOL,
        "corpus_version": corpus.version,
        "model": model,
        "report_protocol": report::ACQUISITION_COMPARISON_REPORT_PROTOCOL,
        "evaluator_config": config_identity,
        "measurements": measurements,
        "measurement_errors": errors,
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::write(
        output_root.join("matrix-result.json"),
        serde_json::to_vec_pretty(&matrix).expect("encode live matrix result"),
    )
    .expect("write live matrix result");
    assert!(
        errors.is_empty(),
        "live evaluator instrumentation failed: {errors:#?}"
    );
}

fn load_live_evaluator_config() -> Result<crate::commands::config_resolver::EffectiveConfig, String>
{
    let explicit_config =
        std::env::var_os("A3S_DEEP_RESEARCH_EVAL_CONFIG").filter(|value| !value.is_empty());
    let cli = crate::cli::args::Cli::try_parse_from(live_evaluator_cli_args(explicit_config))
        .map_err(|error| format!("parse live evaluator invocation: {error}"))?;
    let context = crate::cli::context::InvocationContext::build(&cli)
        .map_err(|error| format!("build live evaluator invocation: {error:#}"))?;
    crate::commands::config::resolve_effective_config(&context)
        .map_err(|error| format!("resolve effective A3S configuration: {error:#}"))
}

fn live_evaluator_cli_args(explicit_config: Option<OsString>) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("a3s"),
        OsString::from("--directory"),
        OsString::from(env!("CARGO_MANIFEST_DIR")),
    ];
    if let Some(path) = explicit_config {
        args.push(OsString::from("--config"));
        args.push(path);
    }
    args
}

fn evaluator_config_identity(
    effective: &crate::commands::config_resolver::EffectiveConfig,
    model: &str,
) -> Result<JsonValue, String> {
    let layers = effective
        .layers
        .iter()
        .map(|layer| {
            let bytes = std::fs::read(&layer.path).map_err(|error| {
                format!(
                    "read evaluator config layer {} for fingerprinting: {error}",
                    layer.path.display()
                )
            })?;
            Ok(serde_json::json!({
                "kind": layer.kind,
                "path": layer.path,
                "sha256": format!("{:x}", Sha256::digest(bytes)),
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(serde_json::json!({
        "schema": "a3s/deep-research-evaluator-config/v1",
        "primary_path": effective.primary_path,
        "explicit": effective.explicit,
        "layers": layers,
        "configured_default_model": effective.config.default_model,
        "configured_default_model_source": effective.provenance.get("default_model"),
        "selected_model": model,
    }))
}

async fn run_measurement(
    case: &LiveCase,
    run_index: usize,
    strategy: EvaluationStrategy,
    corpus: &LiveCorpus,
    config: &CodeConfig,
    model: &str,
    output_dir: &Path,
) -> Result<JsonValue, String> {
    let run_started = std::time::Instant::now();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"));
    let session = build_deepresearch_session(
        workspace.to_string_lossy().as_ref(),
        config.clone(),
        output_dir.join("memory"),
    )
    .await
    .map_err(|error| format!("create evaluator session: {error:#}"))?;
    let options = SessionOptions::new()
        .with_model(model.to_string())
        .with_llm_api_timeout(
            corpus
                .budget
                .planner_timeout_ms
                .max(corpus.budget.report_timeout_ms),
        );
    let llm = crate::session_llm::resolve_session_llm_client(
        config,
        &options,
        &format!(
            "deep-research-live-{}-{}-{}-{}",
            case.id,
            strategy.label(),
            run_index,
            std::process::id()
        ),
    )
    .map_err(|error| format!("resolve evaluator model: {error:#}"))?;
    let now = chrono::Local::now();
    let current_date = now.date_naive().to_string();
    let display_utc_offset = now.offset().to_string();
    let planner_input = case.planner_input(&current_date, &display_utc_offset, &corpus.budget);
    let uses_persisted_bootstrap = matches!(
        strategy,
        EvaluationStrategy::Minimal | EvaluationStrategy::Brief
    );
    let (mut bootstrap, first_preliminary_artifact_ms) = if uses_persisted_bootstrap {
        let bootstrap = acquisition::acquire_bootstrap(
            &session,
            &planner_input,
            &corpus.budget,
            output_dir,
            run_started,
        )
        .await;
        persist_json(
            output_dir.join("bootstrap-discovery.json"),
            &bootstrap.discovery,
        )?;
        persist_json(output_dir.join("bootstrap-acquisition.json"), &bootstrap)?;
        let first_preliminary_artifact_ms = if bootstrap.sources.is_empty() {
            None
        } else {
            report::write_preliminary_sources(
                case,
                &bootstrap.sources,
                corpus.budget.public_excerpt_chars,
                output_dir,
            )?;
            let published_ms = run_started.elapsed().as_millis() as u64;
            persist_json(
                output_dir.join("bootstrap-preliminary-report.json"),
                &serde_json::json!({
                    "schema": "a3s/deep-research-preliminary-report/v1",
                    "published_ms": published_ms,
                    "source_count": bootstrap.sources.len(),
                }),
            )?;
            Some(published_ms)
        };
        (Some(bootstrap), first_preliminary_artifact_ms)
    } else {
        (None, None)
    };
    let bootstrap_observation = bootstrap
        .as_ref()
        .map(|bootstrap| acquisition::bootstrap_observation(&bootstrap.discovery));
    let planning_proposal = planning::generate_plan(
        llm.as_ref(),
        planner_input,
        &corpus.budget,
        strategy,
        usize::from(uses_persisted_bootstrap),
        bootstrap_observation.as_ref(),
    )
    .await?;
    persist_json(
        output_dir.join("planning-proposal.json"),
        &planning_proposal,
    )?;
    let mut planning = planning::validate_proposal(planning_proposal)?;
    if uses_persisted_bootstrap {
        let discovery = &mut bootstrap
            .as_mut()
            .ok_or_else(|| "persisted evaluator omitted its reserved bootstrap".to_string())?
            .discovery;
        acquisition::bind_persisted_bootstrap(&mut planning, discovery)?;
    }
    persist_json(output_dir.join("planning.json"), &planning)?;
    let acquisition = acquisition::acquire(
        &session,
        &planning,
        &corpus.budget,
        bootstrap,
        output_dir,
        run_started,
    )
    .await?;
    persist_json(output_dir.join("acquisition.json"), &acquisition)?;
    report::write_preliminary_source_report(
        case,
        &planning,
        &acquisition,
        corpus.budget.public_excerpt_chars,
        output_dir,
    )?;
    let preliminary_artifact_ms = run_started.elapsed().as_millis() as u64;
    let first_preliminary_artifact_ms =
        first_preliminary_artifact_ms.unwrap_or(preliminary_artifact_ms);
    persist_json(
        output_dir.join("preliminary-report.json"),
        &serde_json::json!({
            "schema": "a3s/deep-research-preliminary-report/v1",
            "first_published_ms": first_preliminary_artifact_ms,
            "published_ms": preliminary_artifact_ms,
            "source_count": acquisition.sources.len(),
        }),
    )?;
    let report = report::generate_report(
        llm.as_ref(),
        case,
        &planning,
        &acquisition,
        &corpus.budget,
        output_dir,
    )
    .await?;
    let terminal_elapsed_ms = run_started.elapsed().as_millis() as u64;
    persist_json(output_dir.join("report-result.json"), &report)?;
    let evaluation_packet =
        evaluation::write_evaluation_packet(evaluation::EvaluationPacketContext {
            case,
            run_index,
            planning: &planning,
            acquisition: &acquisition,
            report: &report,
            budget: &corpus.budget,
            output_dir,
            terminal_elapsed_ms,
        })?;
    let summary = serde_json::json!({
        "case_id": case.id,
        "run_index": run_index,
        "strategy": strategy,
        "status": report.status,
        "outcome": report.outcome,
        "source_count": acquisition.sources.len(),
        "planner_elapsed_ms": planning.elapsed_ms,
        "first_source_fetched_ms": acquisition.first_source_fetched_ms,
        "first_source_persisted_ms": acquisition.first_source_persisted_ms,
        "first_preliminary_artifact_ms": first_preliminary_artifact_ms,
        "preliminary_artifact_ms": preliminary_artifact_ms,
        "terminal_elapsed_ms": terminal_elapsed_ms,
        "evaluation_packet": evaluation_packet,
    });
    persist_json(output_dir.join("measurement-result.json"), &summary)?;
    Ok(summary)
}

fn selected_cases<'a>(
    corpus: &'a LiveCorpus,
    selected: &str,
    run_index: usize,
) -> Result<Vec<&'a LiveCase>, String> {
    let mut cases = if selected.eq_ignore_ascii_case("all") {
        corpus.cases.iter().collect::<Vec<_>>()
    } else {
        let mut seen = std::collections::BTreeSet::new();
        let mut cases = Vec::new();
        for case_id in selected.split(',').map(str::trim) {
            if case_id.is_empty() {
                return Err("live corpus case selection contains an empty ID".to_string());
            }
            if !seen.insert(case_id) {
                return Err(format!("duplicate selected live corpus case `{case_id}`"));
            }
            cases.push(
                corpus
                    .case(case_id)
                    .ok_or_else(|| format!("unknown live corpus case `{case_id}`"))?,
            );
        }
        cases
    };
    if cases.is_empty() {
        return Err("live corpus case selection is empty".to_string());
    }
    let case_count = cases.len();
    cases.rotate_left(run_index.saturating_sub(1) % case_count);
    Ok(cases)
}

fn selected_strategies(
    selected: &str,
    run_index: usize,
) -> Result<Vec<EvaluationStrategy>, String> {
    match selected {
        "minimal" => Ok(vec![EvaluationStrategy::Minimal]),
        "brief" => Ok(vec![EvaluationStrategy::Brief]),
        "compiler" => Err(
            "the rejected compiler strategy is historical evidence, not a product candidate"
                .to_string(),
        ),
        "candidates" if run_index.is_multiple_of(2) => {
            Ok(vec![EvaluationStrategy::Brief, EvaluationStrategy::Minimal])
        }
        "candidates" => Ok(vec![EvaluationStrategy::Minimal, EvaluationStrategy::Brief]),
        "all" if run_index.is_multiple_of(2) => {
            Ok(vec![EvaluationStrategy::Brief, EvaluationStrategy::Minimal])
        }
        "all" => Ok(vec![EvaluationStrategy::Minimal, EvaluationStrategy::Brief]),
        "both" if run_index.is_multiple_of(2) => {
            Ok(vec![EvaluationStrategy::Brief, EvaluationStrategy::Minimal])
        }
        "both" => Ok(vec![EvaluationStrategy::Minimal, EvaluationStrategy::Brief]),
        _ => Err(format!(
            "unsupported A3S_DEEP_RESEARCH_EVAL_STRATEGY `{selected}`"
        )),
    }
}

fn selected_run_indices(corpus: &LiveCorpus) -> Result<Vec<usize>, String> {
    let selected = std::env::var("A3S_DEEP_RESEARCH_EVAL_RUN").unwrap_or_else(|_| "1".to_string());
    if selected.eq_ignore_ascii_case("all") {
        return Ok((1..=corpus.runs_per_case).collect());
    }
    let run_index = selected
        .parse::<usize>()
        .map_err(|error| format!("invalid live run index `{selected}`: {error}"))?;
    if run_index == 0 || run_index > corpus.runs_per_case {
        return Err(format!(
            "live run index must be between 1 and {}",
            corpus.runs_per_case
        ));
    }
    Ok(vec![run_index])
}

fn live_output_root() -> PathBuf {
    std::env::var_os("A3S_DEEP_RESEARCH_EVAL_OUTPUT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("target/deep-research-eval/live")
        })
}

fn persist_json(path: PathBuf, value: &impl serde::Serialize) -> Result<(), String> {
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(value)
            .map_err(|error| format!("encode {}: {error}", path.display()))?,
    )
    .map_err(|error| format!("write {}: {error}", path.display()))
}

fn persist_measurement_error(output_dir: &Path, error: &str) {
    let _ = std::fs::write(output_dir.join("measurement-error.txt"), error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluator_invocation_uses_product_directory_and_explicit_config_semantics() {
        let default_cli = crate::cli::args::Cli::try_parse_from(live_evaluator_cli_args(None))
            .expect("default evaluator CLI");
        assert_eq!(
            default_cli.directory.as_deref(),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")))
        );
        assert_eq!(default_cli.config, None);

        let explicit = OsString::from("fixture-config.acl");
        let explicit_cli =
            crate::cli::args::Cli::try_parse_from(live_evaluator_cli_args(Some(explicit.clone())))
                .expect("explicit evaluator CLI");
        assert_eq!(
            explicit_cli.directory.as_deref(),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")))
        );
        assert_eq!(explicit_cli.config, Some(PathBuf::from(explicit)));
    }

    #[test]
    fn planner_visible_input_excludes_hidden_evaluator_expectations() {
        let corpus = corpus::load_live_corpus().expect("live corpus");
        for case in &corpus.cases {
            let input = case.planner_input("2026-07-21", "+08:00", &corpus.budget);
            let encoded = serde_json::to_string(&input).expect("planner input JSON");
            assert!(!encoded.contains("expectations"));
            assert!(!encoded.contains("guardrails"));
            assert!(!encoded.contains("source_requirements"));
            for strategy in [
                EvaluationStrategy::Minimal,
                EvaluationStrategy::Brief,
                EvaluationStrategy::Compiler,
            ] {
                let prompt =
                    planning::planner_prompt(&input, strategy, None).expect("planner prompt");
                for dimension in &case.expectations.dimensions {
                    assert!(
                        !prompt.contains(&dimension.question),
                        "{}/{} leaked evaluator dimension `{}`",
                        case.id,
                        strategy.label(),
                        dimension.id
                    );
                }
                for requirement in &case.expectations.source_requirements {
                    assert!(
                        !prompt.contains(&requirement.description),
                        "{}/{} leaked source requirement `{}`",
                        case.id,
                        strategy.label(),
                        requirement.id
                    );
                }
            }
        }
    }

    #[test]
    fn case_and_strategy_rotation_changes_order_without_changing_membership() {
        let corpus = corpus::load_live_corpus().expect("live corpus");
        let first = corpus
            .rotated_cases(1)
            .into_iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        let second = corpus
            .rotated_cases(2)
            .into_iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        assert_ne!(first, second);
        assert_eq!(
            first
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            second.iter().copied().collect()
        );
        assert_eq!(
            selected_strategies("both", 1).expect("odd strategy order"),
            [EvaluationStrategy::Minimal, EvaluationStrategy::Brief]
        );
        assert_eq!(
            selected_strategies("both", 2).expect("even strategy order"),
            [EvaluationStrategy::Brief, EvaluationStrategy::Minimal]
        );
        assert!(selected_strategies("compiler", 1).is_err());

        let pilot = "C01,C02,C05,C07,C08";
        let first = selected_cases(&corpus, pilot, 1)
            .expect("first pilot order")
            .into_iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        let second = selected_cases(&corpus, pilot, 2)
            .expect("second pilot order")
            .into_iter()
            .map(|case| case.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(first, ["C01", "C02", "C05", "C07", "C08"]);
        assert_eq!(second, ["C02", "C05", "C07", "C08", "C01"]);
        assert!(selected_cases(&corpus, "C01,C01", 1).is_err());
        assert!(selected_cases(&corpus, "C01,missing", 1).is_err());
    }
}
