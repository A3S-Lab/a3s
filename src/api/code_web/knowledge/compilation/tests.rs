use super::*;

#[test]
fn manual_jobs_preempt_automatic_jobs_and_only_one_is_claimed() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();
    let auto = create_fixture_base(workspace, "Auto Base", CompilationPolicy::SmartAuto);
    let manual = create_fixture_base(workspace, "Manual Base", CompilationPolicy::Manual);
    let now = Utc::now();
    let auto_path = PathBuf::from(auto.knowledge_base.path);
    let manual_path = PathBuf::from(manual.knowledge_base.path);
    let mut auto_state = read_state(&auto_path).expect("auto state");
    let mut manual_state = read_state(&manual_path).expect("manual state");
    let auto_digest = auto_state.source_digest.clone().expect("auto digest");
    let manual_digest = manual_state.source_digest.clone().expect("manual digest");
    enqueue_job(
        &auto_path,
        &auto.knowledge_base.id,
        CompilationTrigger::SmartAuto,
        &auto_digest,
        now,
        &mut auto_state,
    )
    .expect("queue automatic job");
    enqueue_job(
        &manual_path,
        &manual.knowledge_base.id,
        CompilationTrigger::Manual,
        &manual_digest,
        now + chrono::Duration::seconds(1),
        &mut manual_state,
    )
    .expect("queue manual job");

    let claim = claim_next(workspace, now + chrono::Duration::seconds(2)).expect("claim job");
    assert!(claim.claimed);
    assert_eq!(
        claim.job.expect("claimed job").trigger,
        CompilationTrigger::Manual
    );
    assert!(
        !claim_next(workspace, now + chrono::Duration::seconds(3))
            .expect("second claim")
            .claimed
    );
}

#[test]
fn one_running_job_blocks_claims_across_all_monitored_workspaces() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let first_workspace = temporary.path().join("first");
    let second_workspace = temporary.path().join("second");
    let first = create_fixture_base(&first_workspace, "First", CompilationPolicy::Manual);
    let second = create_fixture_base(&second_workspace, "Second", CompilationPolicy::Manual);
    request_compilation(&first_workspace, &first.knowledge_base.id, Utc::now())
        .expect("queue first compilation");
    request_compilation(&second_workspace, &second.knowledge_base.id, Utc::now())
        .expect("queue second compilation");
    let workspaces = vec![first_workspace.clone(), second_workspace.clone()];

    let claimed = claim_next_in_workspaces(&workspaces, Utc::now()).expect("claim global job");
    assert!(claimed.claimed);
    assert!(claimed.workspace_root.is_some());
    assert!(
        !claim_next_in_workspaces(&workspaces, Utc::now())
            .expect("block second global claim")
            .claimed
    );
}

#[test]
fn changes_during_compilation_become_a_manual_follow_up_generation() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();
    let mutation = create_fixture_base(workspace, "Follow Up", CompilationPolicy::Manual);
    let id = mutation.knowledge_base.id;
    let source = workspace.join("source/Follow-Up.md");
    let first = request_compilation(workspace, &id, Utc::now()).expect("request first compilation");
    let first_job = first.job.expect("first job");
    let claim = claim_next(workspace, Utc::now()).expect("claim first compilation");
    assert_eq!(
        claim.job.as_ref().map(|job| job.id.as_str()),
        Some(first_job.id.as_str())
    );
    std::fs::write(source, "changed").expect("change source during compilation");

    let pending = request_compilation(workspace, &id, Utc::now()).expect("request follow up");
    assert!(pending.knowledge_base.compilation.pending_changes);
    let output = PathBuf::from(first_job.output_path).join("wiki");
    std::fs::write(
        output.join("index.md"),
        "---\ntype: Note\n---\n# Compiled\n",
    )
    .expect("write compiler output");
    let completed = complete_job(
        workspace,
        &id,
        &first_job.id,
        CompilationOutcome::Succeeded,
        false,
        None,
        Some(COMPILER_CONTRACT_VERSION),
        Utc::now(),
    )
    .expect("complete first compilation");

    assert_eq!(
        completed.job.expect("follow-up job").trigger,
        CompilationTrigger::Manual
    );
    assert_eq!(
        completed.knowledge_base.compilation.phase,
        CompilationPhase::Queued
    );
}

#[test]
fn failed_compilation_preserves_last_good_wiki_and_schedules_only_transient_retries() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();
    let mutation = create_fixture_base(workspace, "Failure", CompilationPolicy::Manual);
    let id = mutation.knowledge_base.id;
    let base = PathBuf::from(mutation.knowledge_base.path);
    std::fs::create_dir_all(base.join("wiki")).expect("create last good wiki directory");
    std::fs::write(base.join("wiki/index.md"), "last good").expect("write last good wiki");
    let requested = request_compilation(workspace, &id, Utc::now()).expect("request compilation");
    let job = requested.job.expect("queued job");
    claim_next(workspace, Utc::now()).expect("claim compilation");

    complete_job(
        workspace,
        &id,
        &job.id,
        CompilationOutcome::Failed,
        true,
        Some("compiler unavailable"),
        None,
        Utc::now(),
    )
    .expect("record transient failure");

    assert_eq!(
        std::fs::read_to_string(base.join("wiki/index.md")).expect("read last good wiki"),
        "last good"
    );
    let state = read_state(&base).expect("failed state");
    let retry_at = state
        .retry_at
        .as_deref()
        .and_then(parse_time)
        .expect("transient retry time");
    assert_eq!(
        poll_compilations(workspace, retry_at).expect("poll manual retry"),
        1
    );
    let retried = read_state(&base).expect("retried state");
    assert_eq!(retried.phase, CompilationPhase::Queued);
    let retry_job = read_job(
        &base,
        retried.active_job_id.as_deref().expect("retry job ID"),
    )
    .expect("retry job");
    assert_eq!(retry_job.trigger, CompilationTrigger::Retry);

    let permanent = create_fixture_base(workspace, "Permanent", CompilationPolicy::Manual);
    let permanent_id = permanent.knowledge_base.id;
    let permanent_base = PathBuf::from(permanent.knowledge_base.path);
    let permanent_job = request_compilation(workspace, &permanent_id, Utc::now())
        .expect("request permanent failure compilation")
        .job
        .expect("permanent failure job");
    cancel_job(workspace, &id, &retry_job.id, Utc::now()).expect("clear retry fixture");
    claim_next(workspace, Utc::now()).expect("claim permanent failure compilation");
    complete_job(
        workspace,
        &permanent_id,
        &permanent_job.id,
        CompilationOutcome::Failed,
        false,
        Some("invalid source"),
        None,
        Utc::now(),
    )
    .expect("record permanent failure");
    assert!(read_state(&permanent_base)
        .expect("permanent failure state")
        .retry_at
        .is_none());
}

#[test]
fn smart_auto_waits_for_quiet_window_and_minimum_interval() {
    let now = Utc::now();
    let digest = "sha256:pending".to_string();
    let mut state = CompilationState {
        policy: CompilationPolicy::SmartAuto,
        pending_source_digest: Some(digest),
        change_detected_at: Some(now.to_rfc3339()),
        ..CompilationState::default()
    };
    assert!(!auto_compile_due(
        &state,
        now + chrono::Duration::seconds(29)
    ));
    assert!(auto_compile_due(
        &state,
        now + chrono::Duration::seconds(30)
    ));
    state.last_auto_requested_at = Some((now + chrono::Duration::seconds(30)).to_rfc3339());
    assert!(!auto_compile_due(
        &state,
        now + chrono::Duration::seconds(629)
    ));
    assert!(auto_compile_due(
        &state,
        now + chrono::Duration::seconds(630)
    ));
}

#[test]
fn enabling_smart_auto_keeps_an_unchanged_successful_generation_current() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();
    let mutation = create_fixture_base(workspace, "Current", CompilationPolicy::Manual);
    let id = mutation.knowledge_base.id;
    let base = PathBuf::from(mutation.knowledge_base.path);
    let requested = request_compilation(workspace, &id, Utc::now())
        .expect("request current generation compilation");
    let job = requested.job.expect("queued current generation");
    claim_next(workspace, Utc::now()).expect("claim current generation");
    std::fs::write(
        PathBuf::from(&job.output_path).join("wiki/index.md"),
        "---\ntype: Note\n---\n# Current\n",
    )
    .expect("write compiler output");
    let completed = complete_job(
        workspace,
        &id,
        &job.id,
        CompilationOutcome::Succeeded,
        false,
        None,
        Some("fixture-compiler/1.0.0"),
        Utc::now(),
    )
    .expect("complete current generation");
    assert!(!completed.knowledge_base.compilation.recompile_recommended);

    let updated = set_policy(workspace, &id, CompilationPolicy::SmartAuto, Utc::now())
        .expect("enable smart automatic compilation");

    assert_eq!(
        updated.knowledge_base.compilation.phase,
        CompilationPhase::Succeeded
    );
    assert!(!updated.knowledge_base.compilation.pending_changes);
    assert!(updated
        .knowledge_base
        .compilation
        .next_auto_compile_at
        .is_none());
    let state = read_state(&base).expect("read updated compilation state");
    assert_eq!(
        state.last_compiled_digest.as_deref(),
        state.source_digest.as_deref()
    );
}

fn create_fixture_base(
    workspace: &Path,
    name: &str,
    policy: CompilationPolicy,
) -> personal_bases::KnowledgeBaseMutation {
    let source_root = workspace.join("source");
    std::fs::create_dir_all(&source_root).expect("create source root");
    let source = source_root.join(format!("{}.md", name.replace(' ', "-")));
    std::fs::write(&source, "fixture").expect("write source fixture");
    let plan = source_packages::plan_source_selection(
        workspace,
        &[source],
        None,
        Duration::ZERO,
        SystemTime::now(),
    )
    .expect("source fixture plan");
    personal_bases::materialize_personal_base(
        workspace,
        name,
        Some("Compilation test fixture"),
        "selection",
        |staging| {
            source_packages::materialize_source_package(staging, &plan)?;
            initialize_for_source_package(
                staging,
                &plan.snapshot.content_digest,
                policy,
                Utc::now(),
            )
        },
    )
    .expect("create fixture base")
}
