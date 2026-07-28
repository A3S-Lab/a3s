pub(crate) mod asset_runtime;
pub(crate) mod asset_types;
mod assets;
mod context_history;
mod exec;
mod exec_policy;
mod knowledge;
mod memory;
pub(crate) mod naming;
pub(crate) mod research_runtime;
mod session;

use std::ffi::OsString;

use anyhow::{bail, Context};
use serde_json::json;

use crate::cli::args::{
    AuthArgs, AuthCommand, AuthLoginArgs, AuthProviderArgs, CodeArgs, CodeCommand,
    CodeResearchArgs, ConfigArgs, ConfigCommand, ConfigScope, ConfigScopeArgs, ConfigValidateArgs,
    ModelArgs, ModelCommand, OutputMode, SelfUpdateArgs,
};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, usage_error, write_jsonl, CliError, ExitClass};

pub(crate) async fn run(args: CodeArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    match args.command {
        None => launch_tui(Vec::new(), context).await,
        Some(CodeCommand::Exec(args)) => exec::run(args, context).await,
        Some(CodeCommand::Resume(args)) => {
            let mut argv = vec!["resume".to_string()];
            if let Some(session_id) = args.session_id {
                argv.push(session_id);
            }
            launch_tui(argv, context).await
        }
        Some(CodeCommand::Research(args)) => run_research(args, context).await,
        Some(CodeCommand::Session(args)) => session::run(args, context).await,
        Some(CodeCommand::Agent(args)) => assets::run_agent(args, context).await,
        Some(CodeCommand::Mcp(args)) => assets::run_mcp(args, context).await,
        Some(CodeCommand::Skill(args)) => assets::run_skill(args, context).await,
        Some(CodeCommand::Flow(args)) => assets::run_flow(args, context).await,
        Some(CodeCommand::Okf(args)) => assets::run_okf(args, context).await,
        Some(CodeCommand::Kb(args)) => knowledge::run(args, context),
        Some(CodeCommand::Context(args)) => context_history::run(args, context).await,
        Some(CodeCommand::Memory(args)) => memory::run(args, context),
        Some(CodeCommand::LegacyLogin(args)) => legacy_login(args.values, context).await,
        Some(CodeCommand::LegacyLogout) => {
            warn(output, "`a3s code logout`", "`a3s auth logout os`");
            crate::commands::auth::run(
                AuthArgs {
                    command: AuthCommand::Logout(AuthProviderArgs::default()),
                },
                context,
            )
            .await
        }
        Some(CodeCommand::LegacyAuth(args)) => legacy_auth(args.args, context).await,
        Some(CodeCommand::LegacyConfig(args)) => legacy_config(args.args, context).await,
        Some(CodeCommand::LegacyDirs) => {
            warn(output, "`a3s code dirs`", "`a3s config paths`");
            crate::commands::config::run(
                ConfigArgs {
                    command: ConfigCommand::Paths,
                },
                context,
            )
            .await
        }
        Some(CodeCommand::LegacyModels) => {
            warn(output, "`a3s code models`", "`a3s model list`");
            crate::commands::model::run(
                ModelArgs {
                    command: ModelCommand::List,
                },
                context,
            )
            .await
        }
        Some(CodeCommand::LegacyModel(args)) => legacy_model(args.args, context).await,
        Some(CodeCommand::LegacyTop(args)) => {
            warn(output, "`a3s code top`", "`a3s top`");
            crate::commands::top::run(args, context).await
        }
        Some(CodeCommand::LegacyUpdate) => {
            warn(output, "`a3s code update`", "`a3s self update`");
            crate::cli::run_self_update(SelfUpdateArgs::default(), context).await?;
            Ok(())
        }
        Some(CodeCommand::RemovedServe(_)) => Err(usage_error(
            "`a3s code serve` has been removed; use `a3s web start`",
        )),
    }
}

async fn run_research(args: CodeResearchArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let mut argv = Vec::new();
    if context.network.offline && args.web {
        return Err(usage_error(
            "`a3s code research --web` conflicts with the global `--offline` policy",
        ));
    }
    if context.network.offline || args.local_only {
        argv.push("--local-only".to_string());
    } else if args.web {
        argv.push("--web".to_string());
    }
    argv.extend(args.query);
    let (_, code_config) = crate::commands::config::load_active_config(context)?;
    let memory_dir = code_config
        .memory_dir
        .clone()
        .map(|path| context.resolve_path(path))
        .unwrap_or_else(|| context.directory.join(".a3s/memory"));
    let mut synthesis = research_runtime::execute_deepresearch_in(
        &argv,
        &context.directory,
        code_config,
        memory_dir,
    )
    .await?;
    if let Some(report_dir) = args.report_dir {
        let report_dir = context.resolve_path(report_dir);
        synthesis.artifacts = relocate_research_artifacts(&synthesis.artifacts, &report_dir)?;
    }
    let (status, incomplete) = match synthesis.status {
        research_runtime::DeepResearchReportStatus::Synthesized => ("synthesized", false),
        research_runtime::DeepResearchReportStatus::Qualified => ("qualified", false),
        research_runtime::DeepResearchReportStatus::SourceBacked => ("source_backed", true),
        research_runtime::DeepResearchReportStatus::NoEvidence => ("no_evidence", true),
    };
    let data = json!({
        "runId": synthesis.run_id,
        "status": status,
        "text": synthesis.text,
        "quality": synthesis.quality,
        "artifacts": {
            "markdown": synthesis.artifacts.markdown,
            "html": synthesis.artifacts.html,
        },
    });
    if incomplete {
        if output == OutputMode::Human {
            print_research_result(&synthesis);
        }
        return Err(CliError::new(
            "research.incomplete",
            format!(
                "DeepResearch published a {status} result; report written at {}",
                synthesis.artifacts.html.display()
            ),
            ExitClass::Failure,
        )
        .with_details(data)
        .into());
    }
    match output {
        OutputMode::Human => {
            print_research_result(&synthesis);
            Ok(())
        }
        OutputMode::Json => render_value(output, "code.research", data, || {}),
        OutputMode::Jsonl => {
            write_jsonl(&json!({
                "schemaVersion": 1,
                "command": "code.research",
                "type": "result",
                "sequence": 1,
                "ok": true,
                "data": data,
            }))?;
            Ok(())
        }
    }
}

fn print_research_result(synthesis: &research_runtime::DeepResearchReportSynthesis) {
    print!("{}", synthesis.text);
    if !synthesis.text.ends_with('\n') {
        println!();
    }
    println!("report.md: {}", synthesis.artifacts.markdown.display());
    println!("index.html: {}", synthesis.artifacts.html.display());
}

fn relocate_research_artifacts(
    artifacts: &research_runtime::ResearchReportArtifacts,
    report_dir: &std::path::Path,
) -> anyhow::Result<research_runtime::ResearchReportArtifacts> {
    if std::fs::symlink_metadata(report_dir).is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        bail!("--report-dir must not be a symbolic link");
    }
    if report_dir.exists() && !report_dir.is_dir() {
        bail!("--report-dir must be a directory");
    }
    std::fs::create_dir_all(report_dir)
        .with_context(|| format!("could not create report directory {}", report_dir.display()))?;
    let markdown = report_dir.join("report.md");
    let html = report_dir.join("index.html");
    copy_report_artifact(&artifacts.markdown, &markdown)?;
    copy_report_artifact(&artifacts.html, &html)?;
    Ok(research_runtime::ResearchReportArtifacts { markdown, html })
}

fn copy_report_artifact(source: &std::path::Path, target: &std::path::Path) -> anyhow::Result<()> {
    if source == target {
        return Ok(());
    }
    let contents = std::fs::read(source)
        .with_context(|| format!("could not read generated report {}", source.display()))?;
    crate::api::code_web::config::persistence::write_atomic(target, &contents)
        .map_err(|error| anyhow::anyhow!("could not write report {}: {error}", target.display()))
}

async fn launch_tui(args: Vec<String>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    if output != OutputMode::Human {
        return Err(usage_error(
            "interactive `a3s code` requires human output; use `a3s code exec` for automation",
        ));
    }
    crate::tui::run_in(args, &context.directory, context).await
}

async fn legacy_login(values: Vec<OsString>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    if !values.is_empty() {
        bail!("positional credentials are not accepted; use `a3s auth login os --token-stdin`");
    }
    warn(output, "`a3s code login`", "`a3s auth login os`");
    crate::commands::auth::run(
        AuthArgs {
            command: AuthCommand::Login(AuthLoginArgs {
                provider_or_legacy: Some(OsString::from("os")),
                token_stdin: false,
                token_file: None,
                legacy_values: Vec::new(),
            }),
        },
        context,
    )
    .await
}

async fn legacy_auth(values: Vec<OsString>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let command = first_utf8(&values, "auth")?.unwrap_or("status");
    match command {
        "status" if values.len() == 1 || values.is_empty() => {
            warn(output, "`a3s code auth status`", "`a3s auth status os`");
            crate::commands::auth::run(
                AuthArgs {
                    command: AuthCommand::Status(AuthProviderArgs::default()),
                },
                context,
            )
            .await
        }
        "login" if values.len() == 1 => legacy_login(Vec::new(), context).await,
        "logout" if values.len() == 1 => {
            warn(output, "`a3s code auth logout`", "`a3s auth logout os`");
            crate::commands::auth::run(
                AuthArgs {
                    command: AuthCommand::Logout(AuthProviderArgs::default()),
                },
                context,
            )
            .await
        }
        "login" => {
            bail!("positional credentials are not accepted; use `a3s auth login os --token-stdin`")
        }
        _ => bail!("deprecated `a3s code auth` accepts only status, login, or logout"),
    }
}

async fn legacy_config(values: Vec<OsString>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let command = first_utf8(&values, "config")?.unwrap_or("path");
    let no_extra = values.len() <= 1;
    let command = match command {
        "path" if no_extra => ConfigCommand::Path,
        "dirs" if no_extra => ConfigCommand::Paths,
        "cat" if no_extra => ConfigCommand::Show,
        "check" if no_extra => ConfigCommand::Validate(ConfigValidateArgs::default()),
        "edit" if no_extra => ConfigCommand::Edit(ConfigScopeArgs {
            scope: ConfigScope::User,
        }),
        "init" if no_extra => ConfigCommand::Init(crate::cli::args::ConfigInitArgs {
            scope: ConfigScope::User,
            force: false,
        }),
        "init" => {
            bail!("positional config paths are deprecated; use `a3s --config <path> config init`")
        }
        _ => bail!("deprecated `a3s code config` accepts path, dirs, cat, check, edit, or init"),
    };
    warn(output, "`a3s code config …`", "`a3s config …`");
    crate::commands::config::run(ConfigArgs { command }, context).await
}

async fn legacy_model(values: Vec<OsString>, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let command = first_utf8(&values, "model")?.unwrap_or("list");
    if command != "list" || values.len() > 1 {
        bail!("deprecated `a3s code model` accepts only `list`; use `a3s model --help`");
    }
    warn(output, "`a3s code model list`", "`a3s model list`");
    crate::commands::model::run(
        ModelArgs {
            command: ModelCommand::List,
        },
        context,
    )
    .await
}

fn first_utf8<'a>(values: &'a [OsString], label: &str) -> anyhow::Result<Option<&'a str>> {
    values
        .first()
        .map(|value| {
            value
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("{label} command must be valid UTF-8"))
        })
        .transpose()
}

fn warn(output: OutputMode, old: &str, replacement: &str) {
    if output == OutputMode::Human {
        eprintln!("warning: {old} is deprecated; use {replacement}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_relocation_writes_only_the_requested_artifacts() {
        let temp = tempfile::tempdir().unwrap();
        let generated = temp.path().join("generated");
        let destination = temp.path().join("destination");
        std::fs::create_dir_all(&generated).unwrap();
        std::fs::write(generated.join("report.md"), "# Report\n").unwrap();
        std::fs::write(generated.join("index.html"), "<h1>Report</h1>").unwrap();
        let artifacts = research_runtime::ResearchReportArtifacts {
            markdown: generated.join("report.md"),
            html: generated.join("index.html"),
        };

        let relocated = relocate_research_artifacts(&artifacts, &destination).unwrap();

        assert_eq!(relocated.markdown, destination.join("report.md"));
        assert_eq!(relocated.html, destination.join("index.html"));
        assert_eq!(
            std::fs::read_to_string(relocated.markdown).unwrap(),
            "# Report\n"
        );
        assert_eq!(
            std::fs::read_to_string(relocated.html).unwrap(),
            "<h1>Report</h1>"
        );
    }
}
