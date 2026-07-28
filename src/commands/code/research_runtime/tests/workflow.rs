use super::*;

#[test]
fn deepresearch_cli_reuses_the_minimal_tui_workflow_contract() {
    let args = deep_research_workflow_args(
        "Compare two general-purpose storage approaches using current evidence",
    );
    let source = args["source"].as_str().expect("workflow source");

    assert!(
        args["input"]["loop_contract"]["planner"].is_object(),
        "the shared workflow must automatically create its Loop Engineering contract"
    );
    assert!(args["input"].get("semantic_plan_contract").is_none());
    assert_eq!(args["input"]["loop_contract"]["quota"]["mode"], "bounded");
    assert_eq!(
        args["input"]["loop_contract"]["execution"]["mode"],
        "progressively_publishable"
    );
    assert!(
        args["input"]["loop_contract"].get("checker").is_none()
            && args["input"]["loop_contract"].get("maker").is_none()
            && args["input"]["loop_contract"].get("scout").is_none()
            && args["input"]["loop_contract"].get("perspective").is_none()
            && args["input"]["loop_contract"]
                .get("adaptive_routes")
                .is_none(),
        "closed-evidence review is host-owned and must not reintroduce a workflow control plane"
    );
    assert!(source.contains("web_search"), "{source}");
    assert!(source.contains("web_fetch"), "{source}");
    assert!(source.contains("semantic_chunk_ids"), "{source}");
    assert!(source.contains("select_evidence_chunks"), "{source}");
    assert!(
        !source.contains("research_planner")
            && !source.contains("checker")
            && !source.contains("maker")
            && !source.contains("followUp")
            && !source.contains("queryTerms"),
        "the CLI workflow must contain retrieval only"
    );
}

#[test]
fn deepresearch_cli_uses_the_shared_hard_safety_envelope() {
    let args = deep_research_workflow_args("Investigate a broad technical decision");
    let spec = crate::tui::deep_research_evidence_first_research_spec(&args);

    assert!(args["input"].get("os_runtime").is_none());
    assert_eq!(args["input"]["evidence_scope"], "web_and_workspace");
    assert!(args["input"].get("local_max_parallel_tasks").is_none());
    assert!(
        args["input"]["local_max_steps"]
            .as_u64()
            .is_some_and(|value| value == 4),
        "a child task must never inherit the old 80-240 turn budget"
    );
    assert!(args["input"]
        .get("local_parallel_task_timeout_ms")
        .is_none());
    assert_eq!(args["limits"]["timeoutMs"], 600_000);
    assert_eq!(
        spec.total_budget_ms,
        crate::tui::DEEP_RESEARCH_EVIDENCE_FIRST_HOST_TIMEOUT_MS
    );
    assert_eq!(spec.retrieval_stage_budget_ms, 12 * 60 * 1_000 + 30 * 1_000);
    assert_eq!(
        spec.question_review_stage_budget_ms,
        2 * (8 * 60 * 1_000 + 15 * 1_000)
    );
    assert_eq!(spec.finalization_reserve_ms, 15 * 1_000);
}

#[test]
fn deepresearch_cli_scope_is_query_agnostic_and_only_explicitly_selected() {
    let local_wording =
        deep_research_workflow_args("仅本地分析当前工作区，不要联网，并说明证据缺口");
    let web_wording = deep_research_workflow_args("Compare current public documentation");
    let explicit_local = deep_research_workflow_args_for_scope(
        "Compare current public documentation",
        Some(crate::tui::DeepResearchEvidenceScope::LocalOnly),
    );
    let explicit_web = deep_research_workflow_args_for_scope(
        "Do not use web; inspect only local files",
        Some(crate::tui::DeepResearchEvidenceScope::WebAndWorkspace),
    );

    assert_eq!(
        local_wording["input"]["evidence_scope"],
        "web_and_workspace"
    );
    assert_eq!(local_wording["limits"]["timeoutMs"], 600_000);
    assert_eq!(web_wording["input"]["evidence_scope"], "web_and_workspace");
    assert_eq!(web_wording["limits"]["timeoutMs"], 600_000);
    assert_eq!(explicit_local["input"]["evidence_scope"], "local_only");
    assert_eq!(explicit_local["limits"]["timeoutMs"], 210_000);
    assert_eq!(explicit_web["input"]["evidence_scope"], "web_and_workspace");
    assert_eq!(explicit_web["limits"]["timeoutMs"], 600_000);
    assert_eq!(local_wording["source"], web_wording["source"]);
}
