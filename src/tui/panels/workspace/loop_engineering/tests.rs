#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "a3s-loop-{name}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn file_text(path: impl AsRef<Path>) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    fn agent_dev_session(root: &Path) -> AgentDevSession {
        AgentDevSession {
            name: "code-reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "review/code-reviewer".into(),
            definition_rel: "agent.md".into(),
            path: root.join("agents/review/code-reviewer/agent.md"),
            package_path: root.join("agents/review/code-reviewer"),
            root: root.join("agents"),
        }
    }

    #[test]
    fn loop_columns_keep_narrow_panels_usable() {
        let (left, right) = loop_columns(40);

        assert!(left > 0, "left column should not collapse");
        assert!(right > 0, "right column should not collapse");
        assert!(left + 1 + right <= 40);
    }

    #[test]
    fn loop_lines_are_width_bounded_with_styles() {
        let line = loop_line(
            &Style::new()
                .fg(TN_GRAY)
                .render("  Enter/r run · a audit · l logs · p runtime · i init · Esc"),
            22,
        );

        assert!(
            a3s_tui::style::visible_len(&line) <= 22,
            "{}",
            a3s_tui::style::strip_ansi(&line)
        );
    }

    #[test]
    fn loop_header_lines_use_shared_section_header_and_fit_width() {
        let lines = loop_header_lines(false, 34);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(lines.len(), 3);
        assert!(plain.contains("loop engineering"), "{plain}");
        assert!(plain.contains("no loops"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 34),
            "{plain}"
        );
    }

    #[test]
    fn loop_detail_lines_use_shared_key_value_and_fit_width() {
        let root = temp_root("detail-lines");
        let summary = LoopSummary {
            spec: LoopSpec {
                id: "daily-triage".into(),
                pattern: "daily-triage".into(),
                goal: "Inspect workspace state, tests, and risks before reporting".into(),
                level: "L1".into(),
                cadence: "1d".into(),
                os_runtime: true,
                worktree: true,
                maker_agent: "triage".into(),
                checker_agent: "verifier".into(),
                budget_tokens_per_day: 120_000,
                max_iterations_per_run: 1,
                denylist: vec![".env*".into()],
                connectors: vec!["os-runtime".into()],
                dir: root.join(".a3s/loops/daily-triage"),
            },
            audit: LoopAudit {
                score: 72,
                level: "L1-ready".into(),
                passed: vec![],
                missing: vec![
                    "budget.toml is missing a daily cap".into(),
                    "os_runtime connector has not been verified".into(),
                ],
                warnings: vec![],
            },
            last_run: "never".into(),
        };

        let lines = loop_detail_lines(Some(&summary), "ready", 38);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("daily-triage"), "{plain}");
        assert!(plain.contains("pattern"), "{plain}");
        assert!(plain.contains("Missing"), "{plain}");
        assert!(plain.contains("- budget.toml"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 38),
            "{plain}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn parse_loop_command_keeps_quick_loop_form() {
        assert_eq!(parse_loop_command(""), LoopCommand::Dashboard);
        assert_eq!(
            parse_loop_command("run daily-triage"),
            LoopCommand::Run("daily-triage".into())
        );
        assert_eq!(
            parse_loop_command("fix the tests"),
            LoopCommand::Quick("fix the tests".into())
        );
    }

    #[test]
    fn parse_loop_command_requires_names_for_targeted_subcommands() {
        assert_eq!(
            parse_loop_command("run"),
            LoopCommand::Usage("usage: /loop run <name>")
        );
        assert_eq!(
            parse_loop_command("audit"),
            LoopCommand::Usage("usage: /loop audit <name>")
        );
        assert_eq!(
            parse_loop_command("logs"),
            LoopCommand::Usage("usage: /loop logs <name>")
        );
        assert_eq!(
            parse_loop_command("log daily-triage"),
            LoopCommand::Quick("log daily-triage".into()),
            "/loop log must not stay as a hidden /loop logs alias"
        );
        assert_eq!(
            parse_loop_command("help"),
            LoopCommand::Quick("help".into()),
            "/loop help must not stay as a hidden dashboard alias"
        );
    }

    #[test]
    fn init_loop_scaffolds_state_budget_skills_and_audits_l1_ready() {
        let root = temp_root("init");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();
        assert!(spec.dir.join(LOOP_CONFIG).is_file());
        assert!(spec.dir.join(STATE_FILE).is_file());
        assert!(spec.dir.join(RUN_LOG_FILE).is_file());
        assert!(spec.dir.join(BUDGET_FILE).is_file());
        assert!(spec.dir.join("skills").join("triage.md").is_file());
        let audit = audit_loop(&spec);
        assert!(audit.score >= 75, "{audit:?}");
        assert!(audit.passed.iter().any(|p| p.contains("OS Runtime")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_loop_rejects_duplicate_id_without_overwriting_existing_files() {
        let root = temp_root("duplicate");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();
        let config = spec.dir.join(LOOP_CONFIG);
        let state = spec.dir.join(STATE_FILE);
        let original_config = file_text(&config);
        std::fs::write(&state, "sentinel state\n").unwrap();

        let err = init_loop(&cwd, "daily-triage").unwrap_err();

        assert!(err.contains("already exists"), "{err}");
        assert_eq!(file_text(&config), original_config);
        assert_eq!(file_text(&state), "sentinel state\n");
        assert_eq!(list_loops(&cwd).len(), 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_loop_custom_name_uses_default_pattern() {
        let root = temp_root("custom-name");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "nightly-check").unwrap();

        assert_eq!(spec.id, "nightly-check");
        assert_eq!(spec.pattern, DEFAULT_PATTERN);
        assert_eq!(spec.cadence, "1d");
        assert!(spec.dir.ends_with(".a3s/loops/nightly-check"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_loop_custom_name_can_select_known_pattern() {
        let root = temp_root("custom-pattern");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "ci-watch ci-sweeper").unwrap();

        assert_eq!(spec.id, "ci-watch");
        assert_eq!(spec.pattern, "ci-sweeper");
        assert_eq!(spec.cadence, "15m");
        assert!(spec.goal.contains("CI/test failures"));
        assert!(spec.budget_tokens_per_day > 120_000);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_agent_loop_scopes_to_active_agent_and_disables_os_runtime() {
        let root = temp_root("agent-init");
        let cwd = root.to_string_lossy();
        let agent = agent_dev_session(&root);
        let spec = init_agent_loop(&cwd, "", &agent).unwrap();

        assert_eq!(spec.id, "agent-code-reviewer");
        assert_eq!(spec.pattern, "agent-dev");
        assert_eq!(spec.level, "A2");
        assert!(!spec.os_runtime);
        assert!(!spec.worktree);
        assert!(spec.connectors.is_empty());
        assert!(spec.goal.contains("code-reviewer"));
        assert!(spec.goal.contains("review/code-reviewer"));
        assert!(spec.goal.contains("agent.md"));
        assert!(file_text(spec.dir.join(LOOP_CONFIG)).contains("os_runtime = false"));
        assert!(file_text(spec.dir.join(STATE_FILE)).contains("Target Agent"));

        let audit = audit_loop(&spec);
        assert_eq!(audit.score, 100, "{audit:?}");
        assert!(audit.passed.iter().any(|p| p == "local agent loop runtime"));
        assert!(audit
            .passed
            .iter()
            .any(|p| p == "agent asset scope requested"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_agent_loop_accepts_custom_id_and_pattern() {
        let root = temp_root("agent-custom");
        let cwd = root.to_string_lossy();
        let agent = agent_dev_session(&root);
        let spec = init_agent_loop(&cwd, "review-watch ci-sweeper", &agent).unwrap();

        assert_eq!(spec.id, "review-watch");
        assert_eq!(spec.pattern, "ci-sweeper");
        assert_eq!(spec.level, "A2");
        assert!(spec.goal.contains("code-reviewer"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn list_and_find_loops_sort_and_match_slug() {
        let root = temp_root("list");
        let cwd = root.to_string_lossy();
        init_loop(&cwd, "zeta-check").unwrap();
        init_loop(&cwd, "nightly-check").unwrap();
        init_loop(&cwd, "alpha-check").unwrap();

        let ids = list_loops(&cwd)
            .into_iter()
            .map(|summary| summary.spec.id)
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["alpha-check", "nightly-check", "zeta-check"]);

        let spec = find_loop(&cwd, "Nightly Check").unwrap();
        assert_eq!(spec.id, "nightly-check");
        assert!(find_loop(&cwd, "missing")
            .unwrap_err()
            .contains("not found"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn audit_loop_downgrades_when_required_artifacts_are_missing() {
        let root = temp_root("audit-missing");
        let cwd = root.to_string_lossy();
        let mut spec = init_loop(&cwd, "daily-triage").unwrap();
        std::fs::remove_file(spec.dir.join(BUDGET_FILE)).unwrap();
        std::fs::remove_file(spec.dir.join("skills").join("verifier.md")).unwrap();
        spec.connectors.clear();

        let audit = audit_loop(&spec);

        assert!(audit.score < 90, "{audit:?}");
        assert!(audit.level == "L2-ready" || audit.level == "L1-ready");
        assert!(
            audit
                .missing
                .iter()
                .any(|m| m.contains("budget.toml") || m.contains("daily caps")),
            "{audit:?}"
        );
        assert!(
            audit
                .missing
                .iter()
                .any(|m| m.contains("os_runtime") || m.contains("os-runtime")),
            "{audit:?}"
        );
        assert!(
            audit
                .missing
                .iter()
                .any(|m| m.contains("triage.md") && m.contains("verifier.md")),
            "{audit:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn append_run_start_records_os_runtime_flag_and_last_run() {
        let root = temp_root("run-log");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();

        append_run_start(&spec, true).unwrap();

        let log = file_text(spec.dir.join(RUN_LOG_FILE));
        assert!(log.contains("start · level=L1 · os_runtime=true"), "{log}");
        let summaries = list_loops(&cwd);
        assert_eq!(summaries.len(), 1);
        assert!(
            summaries[0]
                .last_run
                .contains("start · level=L1 · os_runtime=true"),
            "{summaries:?}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn parse_spec_reads_lists_and_flags() {
        let spec = parse_spec(
            r#"
id = "ci"
goal = "Fix CI"
level = "L2"
os_runtime = true
worktree = false
denylist = [".env*", "infra/**"]
connectors = ["os-runtime"]
"#,
            PathBuf::from("/tmp/ci"),
        )
        .unwrap();
        assert_eq!(spec.id, "ci");
        assert_eq!(spec.level, "L2");
        assert!(!spec.worktree);
        assert_eq!(spec.denylist, vec![".env*", "infra/**"]);
    }

    #[test]
    fn loop_run_prompt_falls_back_locally_without_os() {
        let root = temp_root("prompt-local");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();
        let p = loop_run_prompt(&spec, &cwd, false);

        assert!(p.contains("OS is not signed in"), "{p}");
        assert!(
            p.contains("`/login` enables Runtime parallelism and RemoteUI"),
            "{p}"
        );
        assert!(p.contains("Do not claim an OS RemoteUI view exists"), "{p}");
        assert!(!p.contains("OS IS AVAILABLE AND MUST BE USED"), "{p}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loop_run_prompt_stays_local_for_agent_dev_runtime_mode() {
        let root = temp_root("prompt-agent");
        let cwd = root.to_string_lossy();
        let agent = agent_dev_session(&root);
        let spec = init_agent_loop(&cwd, "", &agent).unwrap();
        let p = loop_run_prompt_with_runtime(&spec, &cwd, LoopRuntimeMode::LocalAgentDev);

        assert!(p.contains("local /agent development mode"), "{p}");
        assert!(p.contains("Stay local even if OS is signed in"), "{p}");
        assert!(p.contains("Do not open OS, WebIDE, RemoteUI"), "{p}");
        assert!(p.contains("Local-only runtime policy"), "{p}");
        assert!(p.contains("A2 agent-development loop"), "{p}");
        assert!(!p.contains("OS IS AVAILABLE AND MUST BE USED"), "{p}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loop_run_prompt_requires_os_runtime_and_remoteui_when_available() {
        let root = temp_root("prompt");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();
        let p = loop_run_prompt(&spec, &cwd, true);
        assert!(p.contains("OS IS AVAILABLE AND MUST BE USED"), "{p}");
        assert!(p.contains("Runtime evidence is required"), "{p}");
        assert!(p.contains("parallel_task") && p.contains("RemoteUI"), "{p}");
        assert!(p.contains("A3S Runtime"), "{p}");
        assert!(p.contains("shaped:true"), "{p}");
        assert!(p.contains(".view") && p.contains("viewUrl"), "{p}");
        assert!(p.contains("must include both fan-out"), "{p}");
        assert!(
            p.contains("Markdown report") && p.contains("HTML report"),
            "{p}"
        );
        assert!(
            p.contains("visible through the asset-scoped runtime activity panel"),
            "{p}"
        );
        assert!(p.contains("STATE.md") && p.contains("RUN_LOG.md"), "{p}");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn loop_run_prompt_enforces_l1_report_only_policy() {
        let root = temp_root("prompt-l1");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();
        let p = loop_run_prompt(&spec, &cwd, true);

        assert!(p.contains("L1 report-only"), "{p}");
        assert!(p.contains("do not modify project source files"), "{p}");
        assert!(
            p.contains("Only update this loop's STATE.md, RUN_LOG.md, and reports/ artifacts"),
            "{p}"
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
