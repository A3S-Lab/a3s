use super::*;

#[test]
fn parses_deepresearch_cli_options() {
    let opts =
        parse_deepresearch_args(&["compare".into(), "runtimes".into()]).expect("deepresearch args");
    assert_eq!(opts.query, "compare runtimes");
    assert_eq!(opts.evidence_scope, None);

    let local_only = parse_deepresearch_args(&[
        "--local-only".into(),
        "compare".into(),
        "public".into(),
        "sources".into(),
    ])
    .expect("explicit local-only evidence scope");
    assert_eq!(
        local_only.evidence_scope,
        Some(crate::tui::DeepResearchEvidenceScope::LocalOnly)
    );

    let web = parse_deepresearch_args(&[
        "--web".into(),
        "do".into(),
        "not".into(),
        "use".into(),
        "web".into(),
    ])
    .expect("explicit web evidence scope");
    assert_eq!(
        web.evidence_scope,
        Some(crate::tui::DeepResearchEvidenceScope::WebAndWorkspace)
    );

    let conflict =
        parse_deepresearch_args(&["--local-only".into(), "--web".into(), "conflict".into()])
            .expect_err("conflicting evidence scopes");
    assert!(conflict.to_string().contains("conflicts"), "{conflict}");
}

#[tokio::test]
async fn deepresearch_cli_rejects_removed_runtime_selection() {
    let err = execute_deepresearch_in(
        &["--os".into(), "market".into()],
        Path::new("."),
        CodeConfig::default(),
        PathBuf::from(".a3s/memory"),
    )
    .await
    .expect_err("--os should be rejected before building a DeepResearch session");
    let message = err.to_string();
    assert!(
        message.contains("runtime selection has been removed"),
        "{message}"
    );
    assert!(message.contains("--web or --local-only"), "{message}");

    let local = parse_deepresearch_args(&["--local".into(), "market".into()])
        .expect_err("--local must not create a second runtime route");
    assert!(
        local
            .to_string()
            .contains("runtime selection has been removed"),
        "{local}"
    );
}

#[tokio::test]
async fn deepresearch_cli_resolves_account_model_before_building_the_session() {
    use a3s_code_core::permissions::PermissionDecision;

    let workspace = tempfile::tempdir().expect("DeepResearch workspace");
    let config = CodeConfig::from_acl(
        r#"
            default_model = "codex/account-model"
            memory {
              llmExtraction = false
            }
        "#,
    )
    .expect("account-model config");
    let scripted: Arc<dyn LlmClient> = Arc::new(ScriptedLlmClient::new(Vec::new()));
    let resolved_route = Arc::new(Mutex::new(None));
    let captured_route = Arc::clone(&resolved_route);

    let session = crate::research::build_isolated_research_session_with_resolver(
        workspace.path(),
        config,
        workspace.path().join("memory"),
        EvidenceScope::LocalOnly,
        "deep-research-session-test",
        move |config, options, session_id| {
            let permission_policy = options
                .permission_policy
                .as_ref()
                .expect("typed DeepResearch permission policy");
            *captured_route.lock().unwrap() = Some((
                config.default_model.clone(),
                options.session_id.clone(),
                session_id.to_string(),
                options.continuation_enabled,
                options.max_continuation_turns,
                options.max_tool_rounds,
                options.max_parallel_tasks,
                options.auto_delegation.as_ref().map(|delegation| {
                    (
                        delegation.enabled,
                        delegation.auto_parallel,
                        delegation.allow_manual_delegation,
                    )
                }),
                options.manual_delegation_enabled,
                options.auto_parallel_delegation,
                options.workspace_services.is_some(),
                options.skill_dirs.is_empty(),
                options
                    .skill_registry
                    .as_ref()
                    .is_some_and(|registry| registry.is_empty()),
                permission_policy.check("read", &serde_json::json!({"file_path": "src/lib.rs"})),
                permission_policy.check("web_search", &serde_json::json!({"query": "a3s"})),
            ));
            Ok(scripted)
        },
    )
    .await
    .expect("account-backed DeepResearch session");

    let (
        model,
        option_session_id,
        resolver_session_id,
        continuation_enabled,
        max_continuation_turns,
        max_tool_rounds,
        max_parallel_tasks,
        auto_delegation,
        manual_delegation_enabled,
        auto_parallel_delegation,
        has_workspace_services,
        has_no_skill_dirs,
        has_empty_skill_registry,
        read_decision,
        web_search_decision,
    ) = resolved_route
        .lock()
        .unwrap()
        .clone()
        .expect("DeepResearch model resolver call");
    assert_eq!(model.as_deref(), Some("codex/account-model"));
    assert_eq!(
        option_session_id.as_deref(),
        Some(resolver_session_id.as_str())
    );
    assert_eq!(session.id(), resolver_session_id);
    assert_eq!(continuation_enabled, Some(false));
    assert_eq!(max_continuation_turns, None);
    assert_eq!(max_tool_rounds, None);
    assert_eq!(max_parallel_tasks, Some(1));
    assert_eq!(auto_delegation, Some((false, false, true)));
    assert_eq!(manual_delegation_enabled, Some(true));
    assert_eq!(auto_parallel_delegation, Some(false));
    assert!(has_workspace_services);
    assert!(has_no_skill_dirs);
    assert!(has_empty_skill_registry);
    assert_eq!(read_decision, PermissionDecision::Allow);
    assert_eq!(web_search_decision, PermissionDecision::Deny);
}

#[test]
fn deepresearch_cli_preserves_all_publication_outcomes() {
    let outcomes = [
        DeepResearchReportStatus::Synthesized,
        DeepResearchReportStatus::Qualified,
        DeepResearchReportStatus::SourceBacked,
        DeepResearchReportStatus::NoEvidence,
    ];
    let encoded = outcomes
        .into_iter()
        .map(|outcome| serde_json::to_value(outcome).expect("publication outcome"))
        .collect::<Vec<_>>();

    assert_eq!(
        encoded,
        vec!["synthesized", "qualified", "source_backed", "no_evidence"]
    );
}
