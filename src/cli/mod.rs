pub(crate) mod args;
pub(crate) mod context;
pub(crate) mod output;

use std::ffi::OsString;
use std::path::Path;
use std::process::{ExitCode, ExitStatus};

use anyhow::{bail, Context};
use clap::{error::ErrorKind, CommandFactory, Parser};

use self::args::{
    Cli, ComponentKindArg, DoctorArgs, HelpArgs, InfoArgs, InstallArgs, InstallScopeArg, ListArgs,
    OutputMode, ReleaseChannelArg, RootCommand, SelfCommand, SelfUpdateArgs, UninstallArgs,
    UpgradeArgs,
};
use self::context::InvocationContext;

pub(crate) async fn run(args: impl IntoIterator<Item = OsString>) -> ExitCode {
    let args = args.into_iter().collect::<Vec<_>>();
    let requested_output = preparse_output_mode(&args);
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            let exit = error.exit_code();
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
            ) || requested_output == OutputMode::Human
            {
                let _ = error.print();
                return ExitCode::from(exit.clamp(0, u8::MAX as i32) as u8);
            }
            let error: anyhow::Error = output::CliError::new(
                "usage.invalid",
                "command-line arguments are invalid",
                output::ExitClass::Usage,
            )
            .with_suggestion("Run `a3s help` or `a3s help <command>` to inspect valid usage.")
            .with_details(serde_json::json!({"kind": format!("{:?}", error.kind())}))
            .into();
            return output::render_error(requested_output, "a3s", &error);
        }
    };

    let output = cli.output_mode();
    let command_name = cli.command.as_ref().map(root_command_name).unwrap_or("a3s");
    let context = match InvocationContext::build(&cli) {
        Ok(context) => context,
        Err(error) => return output::render_error(output, command_name, &error),
    };

    let cancellation = context.cancellation.clone();
    let signal_task = tokio::spawn(async move {
        loop {
            if tokio::signal::ctrl_c().await.is_err() {
                break;
            }
            if cancellation.is_cancelled() {
                eprintln!("Second Ctrl+C received; forcing shutdown.");
                std::process::exit(130);
            }
            cancellation.cancel();
        }
    });

    let result = match cli.command {
        None => print_root_help(output),
        Some(command) => dispatch(command, &context).await,
    };
    signal_task.abort();
    match result {
        Ok(exit) => exit,
        Err(error) => output::render_error(output, command_name, &error),
    }
}

fn preparse_output_mode(args: &[OsString]) -> OutputMode {
    let mut output = OutputMode::Human;
    let mut index = 1usize;
    while index < args.len() {
        let Some(argument) = args[index].to_str() else {
            index += 1;
            continue;
        };
        if argument == "--"
            || matches!(
                argument,
                "box" | "compose" | "up" | "down" | "ps" | "logs" | "bench" | "search" | "use"
            )
        {
            break;
        }
        match argument {
            "--json" => output = OutputMode::Json,
            "--output" => {
                if let Some(value) = args.get(index + 1).and_then(|value| value.to_str()) {
                    output = match value {
                        "json" => OutputMode::Json,
                        "jsonl" => OutputMode::Jsonl,
                        _ => output,
                    };
                    index += 1;
                }
            }
            _ => {
                if let Some(value) = argument.strip_prefix("--output=") {
                    output = match value {
                        "json" => OutputMode::Json,
                        "jsonl" => OutputMode::Jsonl,
                        _ => output,
                    };
                }
            }
        }
        index += 1;
    }
    output
}

fn root_command_name(command: &RootCommand) -> &'static str {
    use self::args::{
        AgentCommand, AuthCommand, CacheCommand, CodeCommand, CodeSessionCommand, ConfigCommand,
        ContextCommand, ContextShowCommand, FlowCommand, KbCommand, McpCommand, MemoryCommand,
        ModelCommand, OkfCommand, RegistryCommand, SkillCommand, WebCommand,
    };

    match command {
        RootCommand::Code(args) => match &args.command {
            None => "code",
            Some(CodeCommand::Exec(_)) => "code.exec",
            Some(CodeCommand::Resume(_)) => "code.resume",
            Some(CodeCommand::Research(_)) => "code.research",
            Some(CodeCommand::Session(args)) => match &args.command {
                CodeSessionCommand::List => "code.session.list",
                CodeSessionCommand::Show(_) => "code.session.show",
                CodeSessionCommand::Export(_) => "code.session.export",
                CodeSessionCommand::Delete(_) => "code.session.delete",
            },
            Some(CodeCommand::Agent(args)) => match &args.command {
                AgentCommand::List(_) => "code.agent.list",
                AgentCommand::Clone(_) => "code.agent.clone",
                AgentCommand::Review(_) => "code.agent.review",
                AgentCommand::Activity(_) => "code.agent.activity",
                AgentCommand::Publish(_) => "code.agent.publish",
                AgentCommand::Run(_) => "code.agent.run",
                AgentCommand::Deploy(_) => "code.agent.deploy",
                AgentCommand::Open(_) => "code.agent.open",
                AgentCommand::Logs(_) => "code.agent.logs",
                AgentCommand::Status(_) => "code.agent.status",
            },
            Some(CodeCommand::Mcp(args)) => match &args.command {
                McpCommand::List(_) => "code.mcp.list",
                McpCommand::Clone(_) => "code.mcp.clone",
                McpCommand::Review(_) => "code.mcp.review",
                McpCommand::Activity(_) => "code.mcp.activity",
                McpCommand::Publish(_) => "code.mcp.publish",
                McpCommand::Run(_) => "code.mcp.run",
                McpCommand::Test(_) => "code.mcp.test",
                McpCommand::Deploy(_) => "code.mcp.deploy",
                McpCommand::Open(_) => "code.mcp.open",
                McpCommand::Logs(_) => "code.mcp.logs",
                McpCommand::Status(_) => "code.mcp.status",
            },
            Some(CodeCommand::Skill(args)) => match &args.command {
                SkillCommand::List(_) => "code.skill.list",
                SkillCommand::Clone(_) => "code.skill.clone",
                SkillCommand::Review(_) => "code.skill.review",
                SkillCommand::Activity(_) => "code.skill.activity",
                SkillCommand::Publish(_) => "code.skill.publish",
                SkillCommand::Deploy(_) => "code.skill.deploy",
                SkillCommand::Open(_) => "code.skill.open",
                SkillCommand::Status(_) => "code.skill.status",
            },
            Some(CodeCommand::Flow(args)) => match &args.command {
                FlowCommand::List(_) => "code.flow.list",
                FlowCommand::Clone(_) => "code.flow.clone",
                FlowCommand::Review(_) => "code.flow.review",
                FlowCommand::Activity(_) => "code.flow.activity",
                FlowCommand::Publish(_) => "code.flow.publish",
                FlowCommand::Run(_) => "code.flow.run",
                FlowCommand::Deploy(_) => "code.flow.deploy",
                FlowCommand::Open(_) => "code.flow.open",
                FlowCommand::Logs(_) => "code.flow.logs",
                FlowCommand::Status(_) => "code.flow.status",
            },
            Some(CodeCommand::Okf(args)) => match &args.command {
                OkfCommand::List(_) => "code.okf.list",
                OkfCommand::Clone(_) => "code.okf.clone",
                OkfCommand::Review(_) => "code.okf.review",
                OkfCommand::Activity(_) => "code.okf.activity",
                OkfCommand::Publish(_) => "code.okf.publish",
                OkfCommand::Deploy(_) => "code.okf.deploy",
                OkfCommand::Status(_) => "code.okf.status",
            },
            Some(CodeCommand::Kb(args)) => match &args.command {
                KbCommand::Stats => "code.kb.stats",
                KbCommand::Add(_) => "code.kb.add",
                KbCommand::Import(_) => "code.kb.import",
                KbCommand::Search(_) => "code.kb.search",
                KbCommand::Path => "code.kb.path",
            },
            Some(CodeCommand::Context(args)) => match &args.command {
                ContextCommand::Search(_) => "code.context.search",
                ContextCommand::Show(args) => match &args.command {
                    ContextShowCommand::Event(_) => "code.context.show.event",
                    ContextShowCommand::Session(_) => "code.context.show.session",
                },
            },
            Some(CodeCommand::Memory(args)) => match &args.command {
                MemoryCommand::List(_) => "code.memory.list",
                MemoryCommand::Stats => "code.memory.stats",
                MemoryCommand::Path => "code.memory.path",
            },
            Some(CodeCommand::LegacyLogin(_))
            | Some(CodeCommand::LegacyLogout)
            | Some(CodeCommand::LegacyAuth(_)) => "auth",
            Some(CodeCommand::LegacyConfig(_)) | Some(CodeCommand::LegacyDirs) => "config",
            Some(CodeCommand::LegacyModels) | Some(CodeCommand::LegacyModel(_)) => "model",
            Some(CodeCommand::LegacyTop(_)) => "top",
            Some(CodeCommand::LegacyUpdate) => "self.update",
            Some(CodeCommand::RemovedServe(_)) => "code.serve",
        },
        RootCommand::Web(args) => match &args.command {
            Some(WebCommand::Start(_)) | None => "web.start",
            Some(WebCommand::Stop(_)) => "web.stop",
            Some(WebCommand::Status(_)) => "web.status",
            Some(WebCommand::Logs(_)) => "web.logs",
            Some(WebCommand::Open(_)) => "web.open",
        },
        RootCommand::Top(_) => "top",
        RootCommand::Box(_) => "box",
        RootCommand::Compose(_) => "compose",
        RootCommand::Up(_) => "compose.up",
        RootCommand::Down(_) => "compose.down",
        RootCommand::Ps(_) => "compose.ps",
        RootCommand::Logs(_) => "compose.logs",
        RootCommand::Bench(_) => "bench",
        RootCommand::Search(_) => "search",
        RootCommand::Use(_) => "use",
        RootCommand::Auth(args) => match &args.command {
            AuthCommand::List => "auth.list",
            AuthCommand::Status(_) => "auth.status",
            AuthCommand::Login(_) => "auth.login",
            AuthCommand::Logout(_) => "auth.logout",
        },
        RootCommand::Model(args) => match &args.command {
            ModelCommand::List => "model.list",
            ModelCommand::Current => "model.current",
            ModelCommand::Use(_) => "model.use",
            ModelCommand::Reset(_) => "model.reset",
        },
        RootCommand::Config(args) => match &args.command {
            ConfigCommand::Path => "config.path",
            ConfigCommand::Paths => "config.paths",
            ConfigCommand::Show => "config.show",
            ConfigCommand::Init(_) => "config.init",
            ConfigCommand::Edit(_) => "config.edit",
            ConfigCommand::Validate(_) => "config.validate",
        },
        RootCommand::List(_) => "component.list",
        RootCommand::Info(_) => "component.info",
        RootCommand::Install(_) => "component.install",
        RootCommand::Upgrade(_) => "component.upgrade",
        RootCommand::Uninstall(_) => "component.uninstall",
        RootCommand::Doctor(_) => "component.doctor",
        RootCommand::Registry(args) => match &args.command {
            RegistryCommand::List => "registry.list",
            RegistryCommand::Show(_) => "registry.show",
            RegistryCommand::Add(_) => "registry.add",
            RegistryCommand::Remove(_) => "registry.remove",
            RegistryCommand::Refresh(_) => "registry.refresh",
        },
        RootCommand::Cache(args) => match &args.command {
            CacheCommand::Path => "cache.path",
            CacheCommand::Status => "cache.status",
            CacheCommand::Prune(_) => "cache.prune",
            CacheCommand::Clean(_) => "cache.clean",
        },
        RootCommand::Self_(_) => "self.update",
        RootCommand::Version => "version",
        RootCommand::Completion(_) => "completion",
        RootCommand::Help(_) => "help",
        RootCommand::LegacyUpdate(_) => "update",
    }
}

async fn dispatch(command: RootCommand, context: &InvocationContext) -> anyhow::Result<ExitCode> {
    let output = context.output_mode();
    match command {
        RootCommand::Code(args) => {
            crate::commands::code::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Web(args) => {
            crate::commands::web::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Top(args) => {
            crate::commands::top::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Box(args) => run_proxy("box", args.args, context).await,
        RootCommand::Compose(args) => run_box_compose(None, args.args, context).await,
        RootCommand::Up(args) => run_box_compose(Some("up"), args.args, context).await,
        RootCommand::Down(args) => run_box_compose(Some("down"), args.args, context).await,
        RootCommand::Ps(args) => run_box_compose(Some("ps"), args.args, context).await,
        RootCommand::Logs(args) => run_box_compose(Some("logs"), args.args, context).await,
        RootCommand::Bench(args) => run_proxy("bench", args.args, context).await,
        RootCommand::Search(args) => run_proxy("search", args.args, context).await,
        RootCommand::Use(args) => run_use_proxy(args.args, context).await,
        RootCommand::Auth(args) => {
            crate::commands::auth::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Model(args) => {
            crate::commands::model::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Config(args) => {
            crate::commands::config::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::List(args) => {
            let registries = crate::commands::registry::store(context)?;
            a3s::components::run_list_with_registries(
                list_argv(args, output)?,
                &context.component_paths,
                context.network.offline,
                &registries,
            )
            .await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Info(args) => {
            a3s::components::run_info_with(
                info_argv(args, output)?,
                &context.component_paths,
                context.network.offline,
            )
            .await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Install(args) => {
            let registries = crate::commands::registry::store(context)?;
            a3s::components::run_install_with_registries(
                install_argv(args, output)?,
                &context.component_paths,
                context.network.offline,
                context.output.progress,
                &registries,
            )
            .await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Upgrade(args) => run_upgrade(args, context).await,
        RootCommand::Uninstall(args) => {
            a3s::components::run_uninstall_with(
                uninstall_argv(args, output)?,
                &context.component_paths,
            )?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Doctor(args) => {
            let healthy = a3s::components::run_doctor_with(
                doctor_argv(args, output)?,
                &context.component_paths,
            )?;
            Ok(if healthy {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            })
        }
        RootCommand::Registry(args) => {
            crate::commands::registry::run(args, context).await?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Cache(args) => {
            crate::commands::cache::run(args, context)?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Self_(args) => match args.command {
            SelfCommand::Update(args) => run_self_update(args, context).await,
        },
        RootCommand::Version => run_version(context),
        RootCommand::Completion(args) => {
            crate::commands::completion::run(args, output)?;
            Ok(ExitCode::SUCCESS)
        }
        RootCommand::Help(args) => print_command_help(args, output),
        RootCommand::LegacyUpdate(args) => run_legacy_update(args.args, context).await,
    }
}

async fn run_box_compose(
    shortcut: Option<&str>,
    args: Vec<OsString>,
    context: &InvocationContext,
) -> anyhow::Result<ExitCode> {
    let mut forwarded = Vec::with_capacity(args.len() + 2);
    forwarded.push(OsString::from("compose"));
    if let Some(command) = shortcut {
        let (global_args, command_args) = partition_compose_shortcut_args(command, args)?;
        forwarded.extend(global_args);
        forwarded.push(OsString::from(command));
        forwarded.extend(command_args);
    } else {
        forwarded.extend(args);
    }
    run_proxy("box", forwarded, context).await
}

fn partition_compose_shortcut_args(
    command: &str,
    args: Vec<OsString>,
) -> anyhow::Result<(Vec<OsString>, Vec<OsString>)> {
    let mut global = Vec::new();
    let mut local = Vec::new();
    let mut args = args.into_iter();
    let mut positional_only = false;

    while let Some(argument) = args.next() {
        if positional_only {
            local.push(argument);
            continue;
        }
        if argument == "--" {
            positional_only = true;
            local.push(argument);
            continue;
        }

        let is_file = argument == "--file" || (argument == "-f" && command != "logs");
        let is_project = argument == "--project-name" || argument == "-p";
        if is_file || is_project {
            let option = argument.to_string_lossy().into_owned();
            let value = args.next().ok_or_else(|| {
                output::usage_error(format!("{option} requires a value for `a3s {command}`"))
            })?;
            global.push(argument);
            global.push(value);
            continue;
        }

        let long_value = argument.to_str().is_some_and(|value| {
            value.starts_with("--file=") || value.starts_with("--project-name=")
        });
        if long_value {
            global.push(argument);
        } else {
            local.push(argument);
        }
    }

    Ok((global, local))
}

async fn run_proxy(
    component: &str,
    args: Vec<OsString>,
    context: &InvocationContext,
) -> anyhow::Result<ExitCode> {
    let output = context.output_mode();
    if output != OutputMode::Human {
        return Err(output::usage_error(format!(
            "root `--output` is not translated for the `{component}` proxy; pass the child CLI's native output option after `{component}`"
        )));
    }
    let executable = a3s::components::resolve_or_install_with(
        component,
        &context.component_paths,
        context.network.allow_first_use_install,
        context.output.progress,
    )
    .await?;
    let mut command = tokio::process::Command::new(&executable);
    command.args(args);
    context.configure_child(&mut command);
    let status = command
        .status()
        .await
        .with_context(|| format!("failed to run {}", executable.display()))?;
    Ok(exit_code_from_status(status))
}

async fn run_use_proxy(
    args: Vec<OsString>,
    context: &InvocationContext,
) -> anyhow::Result<ExitCode> {
    if context.output_mode() != OutputMode::Human {
        return Err(output::usage_error(
            "root `--output` is not translated for the `use` proxy; pass the child CLI's native output option after `use`",
        ));
    }

    let box_requested = args
        .first()
        .is_some_and(|argument| argument.as_os_str() == "box");
    let box_executable = if box_requested {
        Some(
            a3s::components::resolve_or_install_with(
                "box",
                &context.component_paths,
                context.network.allow_first_use_install,
                context.output.progress,
            )
            .await?,
        )
    } else {
        a3s::components::find_ready_executable_with("box", &context.component_paths)?
    }
    .map(|path| canonical_component_executable("box", &path))
    .transpose()?;

    let executable = a3s::components::resolve_or_install_with(
        "use",
        &context.component_paths,
        context.network.allow_first_use_install,
        context.output.progress,
    )
    .await?;
    let mut command = tokio::process::Command::new(&executable);
    command.args(args);
    command.env_remove("A3S_USE_BOX_EXECUTABLE");
    if let Some(box_executable) = box_executable {
        command.env("A3S_USE_BOX_EXECUTABLE", box_executable);
    }
    context.configure_child(&mut command);
    let status = command
        .status()
        .await
        .with_context(|| format!("failed to run {}", executable.display()))?;
    Ok(exit_code_from_status(status))
}

fn canonical_component_executable(
    component: &str,
    path: &Path,
) -> anyhow::Result<std::path::PathBuf> {
    let canonical = std::fs::canonicalize(path).with_context(|| {
        format!(
            "failed to resolve the '{}' component executable at {}",
            component,
            path.display()
        )
    })?;
    let metadata = std::fs::metadata(&canonical).with_context(|| {
        format!(
            "failed to inspect the '{}' component executable at {}",
            component,
            canonical.display()
        )
    })?;
    if !metadata.is_file() {
        bail!(
            "the '{}' component executable is not a regular file: {}",
            component,
            canonical.display()
        );
    }
    Ok(canonical)
}

async fn run_upgrade(args: UpgradeArgs, context: &InvocationContext) -> anyhow::Result<ExitCode> {
    let output = context.output_mode();
    if args.components.is_empty() && !args.all {
        let list = ListArgs {
            updates: true,
            ..ListArgs::default()
        };
        let registries = crate::commands::registry::store(context)?;
        a3s::components::run_upgrade_list_with_registries(
            list_argv(list, output)?,
            &context.component_paths,
            context.network.offline,
            &registries,
        )
        .await?;
        return Ok(ExitCode::SUCCESS);
    }
    let mut argv = args
        .components
        .into_iter()
        .map(|component| component.to_string())
        .collect::<Vec<_>>();
    if args.all {
        argv.push("--all".to_string());
    }
    if args.yes {
        argv.push("--yes".to_string());
    }
    if args.dry_run {
        argv.push("--dry-run".to_string());
    }
    if let Some(plan_digest) = args.plan_digest {
        argv.push(format!("--plan-digest={plan_digest}"));
    }
    append_json_flag(&mut argv, output)?;
    let registries = crate::commands::registry::store(context)?;
    a3s::components::run_update_with_registries(
        argv,
        &context.component_paths,
        context.network.offline,
        context.output.progress,
        &registries,
    )
    .await?;
    Ok(ExitCode::SUCCESS)
}

async fn run_legacy_update(
    args: Vec<OsString>,
    context: &InvocationContext,
) -> anyhow::Result<ExitCode> {
    let args = utf8_args(args, "update")?;
    let output = if context.output_mode() == OutputMode::Human
        && args.iter().any(|argument| argument == "--json")
    {
        OutputMode::Json
    } else {
        context.output_mode()
    };
    if args.is_empty() {
        if output == OutputMode::Human {
            eprintln!("warning: `a3s update` is deprecated; use `a3s self update`");
        }
        run_self_update_with_output(SelfUpdateArgs::default(), context, output).await
    } else {
        if output == OutputMode::Human {
            eprintln!(
                "warning: `a3s update <component>` is deprecated; use `a3s upgrade <component>`"
            );
        }
        let mut args = args;
        append_json_flag(&mut args, output)?;
        let registries = crate::commands::registry::store(context)?;
        a3s::components::run_update_with_registries(
            args,
            &context.component_paths,
            context.network.offline,
            output == OutputMode::Human && context.output.progress,
            &registries,
        )
        .await?;
        Ok(ExitCode::SUCCESS)
    }
}

pub(crate) async fn run_self_update(
    args: SelfUpdateArgs,
    context: &InvocationContext,
) -> anyhow::Result<ExitCode> {
    run_self_update_with_output(args, context, context.output_mode()).await
}

async fn run_self_update_with_output(
    args: SelfUpdateArgs,
    context: &InvocationContext,
    output: OutputMode,
) -> anyhow::Result<ExitCode> {
    if output == OutputMode::Jsonl {
        return Err(output::usage_error(
            "`a3s self update` does not support JSONL output",
        ));
    }
    if context.network.offline {
        bail!("self-update is unavailable in offline mode");
    }
    let current = tokio::task::spawn_blocking(crate::update::current_version)
        .await
        .context("self-update version probe failed")?;
    if output == OutputMode::Human {
        eprintln!("a3s {current} — checking for updates…");
    }
    let latest = tokio::task::spawn_blocking(crate::update::fetch_latest)
        .await
        .context("self-update release check failed")?
        .context("could not reach the release server; try again later")?;
    if crate::update::version_ge(&current, &latest) {
        let mut repaired = Vec::new();
        let mut warnings = Vec::new();
        if !args.check && !args.dry_run {
            match tokio::task::spawn_blocking(crate::update::repair_installation)
                .await
                .context("self-update installation repair task failed")?
            {
                Ok(items) => repaired = items,
                Err(error) => warnings.push(format!("install repair failed: {error}")),
            }
        }
        output::render_value_with_warnings(
            output,
            "self.update",
            serde_json::json!({
                "status": "current",
                "currentVersion": current,
                "latestVersion": latest,
                "checkOnly": args.check,
                "dryRun": args.dry_run,
                "accepted": args.yes,
                "repaired": repaired,
            }),
            warnings.clone(),
            || {
                println!("a3s {current} is up to date");
                for item in &repaired {
                    eprintln!("repaired: {item}");
                }
                for warning in &warnings {
                    eprintln!("warning: {warning}");
                }
            },
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    if args.check || args.dry_run {
        output::render_value(
            output,
            "self.update",
            serde_json::json!({
                "status": "available",
                "currentVersion": current,
                "latestVersion": latest,
                "checkOnly": args.check,
                "dryRun": args.dry_run,
                "accepted": args.yes,
            }),
            || println!("a3s {latest} is available (current: {current})"),
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    let can_self_update = tokio::task::spawn_blocking(crate::update::can_self_update)
        .await
        .context("self-update capability probe failed")?;
    if !can_self_update {
        let release_url = "https://github.com/A3S-Lab/a3s/releases";
        output::render_value(
            output,
            "self.update",
            serde_json::json!({
                "status": "manual-update-required",
                "currentVersion": current,
                "latestVersion": latest,
                "releaseUrl": release_url,
                "accepted": args.yes,
            }),
            || {
                println!("a3s {latest} is available (current: {current})");
                println!("get the new build from {release_url}");
            },
        )?;
        return Ok(ExitCode::SUCCESS);
    }
    let install_version = latest.clone();
    tokio::task::spawn_blocking(move || crate::update::perform_upgrade(&install_version))
        .await
        .context("self-update installation task failed")?
        .map_err(|error| {
            anyhow::anyhow!(
            "upgrade failed: {error}; get the latest from https://github.com/A3S-Lab/a3s/releases"
        )
        })?;
    output::render_value(
        output,
        "self.update",
        serde_json::json!({
            "status": "updated",
            "currentVersion": current,
            "latestVersion": latest,
            "accepted": args.yes,
        }),
        || {
            println!("a3s {latest} is available (current: {current})");
            println!("updated to a3s {latest}");
        },
    )?;
    Ok(ExitCode::SUCCESS)
}

fn list_argv(args: ListArgs, output: OutputMode) -> anyhow::Result<Vec<String>> {
    let mut argv = Vec::new();
    if args.installed {
        argv.push("--installed".to_string());
    }
    if args.available {
        argv.push("--available".to_string());
    }
    if args.updates {
        argv.push("--updates".to_string());
    }
    if let Some(kind) = args.kind {
        argv.push(format!("--kind={}", component_kind_name(kind)));
    }
    append_json_flag(&mut argv, output)?;
    Ok(argv)
}

fn info_argv(args: InfoArgs, output: OutputMode) -> anyhow::Result<Vec<String>> {
    let mut argv = vec![args.component.to_string()];
    if args.versions {
        argv.push("--versions".to_string());
    }
    if args.sources {
        argv.push("--sources".to_string());
    }
    append_json_flag(&mut argv, output)?;
    Ok(argv)
}

fn doctor_argv(args: DoctorArgs, output: OutputMode) -> anyhow::Result<Vec<String>> {
    let mut argv = args
        .component
        .into_iter()
        .map(|component| component.to_string())
        .collect::<Vec<_>>();
    append_json_flag(&mut argv, output)?;
    Ok(argv)
}

fn install_argv(args: InstallArgs, output: OutputMode) -> anyhow::Result<Vec<String>> {
    let mut argv = args
        .components
        .into_iter()
        .map(|component| component.to_string())
        .collect::<Vec<_>>();
    if let Some(version) = args.version {
        argv.extend(["--version".to_string(), version]);
    }
    if let Some(source) = args.source {
        argv.extend(["--source".to_string(), source]);
    }
    if args.channel != ReleaseChannelArg::Stable {
        argv.extend([
            "--channel".to_string(),
            release_channel_name(args.channel).to_string(),
        ]);
    }
    if args.scope != InstallScopeArg::User {
        argv.extend([
            "--scope".to_string(),
            install_scope_name(args.scope).to_string(),
        ]);
    }
    if let Some(package) = args.package {
        argv.extend([
            "--from".to_string(),
            path_to_utf8(&package, "install package path")?,
        ]);
    }
    if args.force {
        argv.push("--force".to_string());
    }
    if args.migrate {
        argv.push("--migrate".to_string());
    }
    if args.dry_run {
        argv.push("--dry-run".to_string());
    }
    if let Some(plan_digest) = args.plan_digest {
        argv.push(format!("--plan-digest={plan_digest}"));
    }
    if args.allow_unsigned {
        argv.push("--allow-unsigned".to_string());
    }
    if args.yes {
        argv.push("--yes".to_string());
    }
    append_json_flag(&mut argv, output)?;
    Ok(argv)
}

fn uninstall_argv(args: UninstallArgs, output: OutputMode) -> anyhow::Result<Vec<String>> {
    let mut argv = args
        .components
        .into_iter()
        .map(|component| component.to_string())
        .collect::<Vec<_>>();
    if args.cascade {
        argv.push("--cascade".to_string());
    }
    if args.purge {
        argv.push("--purge".to_string());
    }
    if args.yes {
        argv.push("--yes".to_string());
    }
    if args.dry_run {
        argv.push("--dry-run".to_string());
    }
    if let Some(plan_digest) = args.plan_digest {
        argv.push(format!("--plan-digest={plan_digest}"));
    }
    append_json_flag(&mut argv, output)?;
    Ok(argv)
}

fn component_kind_name(kind: ComponentKindArg) -> &'static str {
    match kind {
        ComponentKindArg::BuiltIn => "built-in",
        ComponentKindArg::Product => "product",
        ComponentKindArg::Capability => "capability",
        ComponentKindArg::Extension => "extension",
    }
}

fn release_channel_name(channel: ReleaseChannelArg) -> &'static str {
    match channel {
        ReleaseChannelArg::Stable => "stable",
        ReleaseChannelArg::Beta => "beta",
        ReleaseChannelArg::Nightly => "nightly",
    }
}

fn install_scope_name(scope: InstallScopeArg) -> &'static str {
    match scope {
        InstallScopeArg::User => "user",
        InstallScopeArg::System => "system",
    }
}

fn append_output_flag(
    args: &mut Vec<String>,
    output: OutputMode,
    supports_jsonl: bool,
) -> anyhow::Result<()> {
    match output {
        OutputMode::Human => Ok(()),
        OutputMode::Json => {
            if !args.iter().any(|arg| arg == "--json") {
                args.push("--json".to_string());
            }
            Ok(())
        }
        OutputMode::Jsonl if supports_jsonl => {
            if !args.iter().any(|arg| arg == "--json") {
                args.push("--json".to_string());
            }
            Ok(())
        }
        OutputMode::Jsonl => Err(output::usage_error(
            "this command does not support JSONL output",
        )),
    }
}

fn append_json_flag(args: &mut Vec<String>, output: OutputMode) -> anyhow::Result<()> {
    append_output_flag(args, output, false)
}

fn utf8_args(args: Vec<OsString>, command: &str) -> anyhow::Result<Vec<String>> {
    args.into_iter()
        .map(|arg| {
            arg.into_string()
                .map_err(|_| anyhow::anyhow!("{command} arguments must be valid UTF-8"))
        })
        .collect()
}

fn path_to_utf8(path: &Path, label: &str) -> anyhow::Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("{label} must be valid UTF-8"))
}

fn print_root_help(output: OutputMode) -> anyhow::Result<ExitCode> {
    if output != OutputMode::Human {
        return Err(output::usage_error(
            "machine output requires an explicit command",
        ));
    }
    Cli::command().print_help()?;
    println!();
    Ok(ExitCode::SUCCESS)
}

fn print_command_help(args: HelpArgs, output: OutputMode) -> anyhow::Result<ExitCode> {
    if output != OutputMode::Human {
        return Err(output::usage_error(
            "command help is available only with human output",
        ));
    }

    let mut argv = Vec::<OsString>::with_capacity(args.command.len() + 2);
    argv.push("a3s".into());
    argv.extend(args.command.into_iter().map(OsString::from));
    argv.push("--help".into());
    match Cli::command().try_get_matches_from(argv) {
        Err(error) => {
            let kind = error.kind();
            let exit = error.exit_code().clamp(0, u8::MAX as i32) as u8;
            let _ = error.print();
            if kind == ErrorKind::DisplayHelp {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::from(exit))
            }
        }
        Ok(_) => bail!("generated help request did not produce help output"),
    }
}

fn run_version(context: &InvocationContext) -> anyhow::Result<ExitCode> {
    let output = context.output_mode();
    let version = env!("CARGO_PKG_VERSION");
    let verbose = context.output.verbosity > 0;
    let executable = verbose.then(|| context.component_paths.current_exe.clone());
    let data = serde_json::json!({
        "version": version,
        "verbose": verbose,
        "target": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "family": std::env::consts::FAMILY,
        },
        "executable": &executable,
    });
    output::render_value(output, "version", data, || {
        println!("a3s {version}");
        if verbose {
            println!(
                "target: {}-{}",
                std::env::consts::OS,
                std::env::consts::ARCH
            );
            if let Some(executable) = executable {
                println!("executable: {}", executable.display());
            }
        }
    })?;
    Ok(ExitCode::SUCCESS)
}

fn exit_code_from_status(status: ExitStatus) -> ExitCode {
    if status.success() {
        return ExitCode::SUCCESS;
    }
    if let Some(code) = status.code() {
        return ExitCode::from(code.clamp(1, u8::MAX as i32) as u8);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return ExitCode::from((128 + signal).clamp(1, u8::MAX as i32) as u8);
        }
    }
    ExitCode::FAILURE
}
