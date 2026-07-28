//! Durable `/goal` execution: an Ultracode session backed by a Loop
//! Engineering workspace and a host-owned achievement latch.

use super::super::*;
use super::agent;
use super::loop_engineering::{self, LoopSpec, BUDGET_FILE, LOOP_CONFIG, RUN_LOG_FILE, STATE_FILE};
use a3s_code_core::planning::AgentGoal;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::Path;

const GOAL_RUNTIME_START: &str = "<!-- a3s-goal-runtime:start -->";
const GOAL_RUNTIME_END: &str = "<!-- a3s-goal-runtime:end -->";
const GOAL_BUDGET_TOKENS_PER_DAY: u64 = 500_000;
const GOAL_PROGRESS_PERSIST_STEP_PERCENT: u8 = 5;

#[derive(Clone, Debug)]
pub(crate) struct GoalRunState {
    pub(crate) generation: u64,
    pub(crate) spec: LoopSpec,
    pub(crate) iteration: usize,
    pub(crate) progress: f32,
    pub(crate) achieved: bool,
    pub(crate) failures: usize,
    accepting_achievement: bool,
    extracted_goal: Option<String>,
}

impl GoalRunState {
    fn new(generation: u64, spec: LoopSpec) -> Self {
        Self {
            generation,
            spec,
            iteration: 1,
            progress: 0.0,
            achieved: false,
            failures: 0,
            accepting_achievement: true,
            extracted_goal: None,
        }
    }

    pub(crate) fn is_generation(&self, generation: u64) -> bool {
        self.generation == generation
    }

    pub(crate) fn paused_state(&self) -> PausedGoalState {
        PausedGoalState {
            loop_id: self.spec.id.clone(),
            goal: self.spec.goal.clone(),
            iteration: self.iteration,
            progress: self.progress,
            failures: self.failures,
        }
    }

    pub(crate) fn from_paused(
        cwd: &str,
        generation: u64,
        paused: &PausedGoalState,
    ) -> Result<Self, String> {
        if !paused.progress.is_finite() {
            return Err("saved goal progress is not finite".to_string());
        }
        let spec = init_goal_loop(cwd, &paused.goal)?;
        if spec.id != paused.loop_id {
            return Err(format!(
                "saved loop id `{}` does not match goal loop `{}`",
                paused.loop_id, spec.id
            ));
        }
        Ok(Self {
            generation,
            spec,
            iteration: paused.iteration.max(1),
            progress: paused.progress.clamp(0.0, 1.0),
            achieved: false,
            failures: paused.failures,
            accepting_achievement: false,
            extracted_goal: None,
        })
    }

    fn record_extracted_goal(&mut self, goal: &AgentGoal) {
        if !self.accepting_achievement {
            return;
        }
        self.extracted_goal = Some(goal.description.trim().to_string());
        self.progress = goal.progress.clamp(0.0, 1.0);
    }

    fn record_progress(&mut self, progress: f32) {
        if !self.accepting_achievement {
            return;
        }
        self.progress = progress.clamp(0.0, 1.0);
    }

    fn record_achievement(&mut self, event_goal: &str) -> bool {
        if !self.accepting_achievement {
            return false;
        }
        let matches = self
            .extracted_goal
            .as_deref()
            .is_some_and(|goal| goal == event_goal.trim());
        if matches {
            self.achieved = true;
            self.progress = 1.0;
        }
        matches
    }

    fn begin_next_iteration(&mut self, failed: bool) {
        self.iteration = self.iteration.saturating_add(1);
        self.progress = 0.0;
        self.accepting_achievement = true;
        self.extracted_goal = None;
        if failed {
            self.failures = self.failures.saturating_add(1);
        } else {
            self.failures = 0;
        }
    }

    fn begin_resumed_iteration(&mut self) {
        self.iteration = self.iteration.saturating_add(1);
        self.progress = 0.0;
        self.accepting_achievement = true;
        self.extracted_goal = None;
    }

    pub(crate) fn pause_achievement_for_user_turn(&mut self) {
        self.accepting_achievement = false;
        self.extracted_goal = None;
    }

    pub(crate) fn pause_for_exit(&mut self) {
        self.accepting_achievement = false;
        self.extracted_goal = None;
        let _ = persist_runtime_state(self, "paused", "TUI exited; waiting for session resume");
        let _ = append_goal_log(self, "paused", "TUI exited; waiting for session resume");
    }
}

pub(crate) fn mark_paused_goal_cancelled(cwd: &str, paused: &PausedGoalState, reason: &str) {
    let Ok(run) = GoalRunState::from_paused(cwd, 0, paused) else {
        return;
    };
    let _ = persist_runtime_state(&run, "cancelled", reason);
    let _ = append_goal_log(&run, "cancelled", reason);
}

fn normalize_goal(goal: &str) -> String {
    goal.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn goal_loop_id(goal: &str) -> String {
    let digest = Sha256::digest(goal.as_bytes());
    let suffix = digest[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("goal-{}-{suffix}", loop_engineering::slug(goal))
}

fn write_if_missing(path: &Path, body: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    std::fs::write(path, body).map_err(|error| error.to_string())
}

fn initial_goal_state(spec: &LoopSpec) -> String {
    format!(
        "# Goal Loop State: {}\n\n\
         {GOAL_RUNTIME_START}\n\
         Status: ready\n\
         Iteration: 0\n\
         Last plan progress: 0%\n\
         Goal: {}\n\
         Last event: waiting to start\n\
         {GOAL_RUNTIME_END}\n\n\
         ## Verified Evidence\n\n\
         - None yet.\n\n\
         ## Remaining Work\n\n\
         - Establish the implementation and verification plan.\n\n\
         ## Human Handoff\n\n\
         - None.\n",
        spec.id, spec.goal
    )
}

fn maker_skill(goal: &str) -> String {
    format!(
        "# Goal Maker\n\n\
         Work toward this exact goal: {goal}\n\n\
         Read `../STATE.md` and `../RUN_LOG.md` first. Inspect the current workspace before editing. \
         Make the smallest coherent change that advances the goal, preserve unrelated user work, \
         and record concrete evidence and remaining gaps in `../STATE.md`. Never declare your own \
         change verified; hand it to the verifier.\n"
    )
}

fn verifier_skill(goal: &str) -> String {
    format!(
        "# Goal Verifier\n\n\
         Independently verify this exact goal: {goal}\n\n\
         Derive observable success criteria from the goal, inspect the maker's actual changes, run \
         proportionate tests, and reject completion when evidence is missing, stale, indirect, or \
         contradicted by the workspace. Record commands, outcomes, residual risks, and remaining \
         gaps in `../STATE.md`. Only fresh evidence can support completion.\n"
    )
}

pub(crate) fn init_goal_loop(cwd: &str, raw_goal: &str) -> Result<LoopSpec, String> {
    let goal = normalize_goal(raw_goal);
    if goal.is_empty() {
        return Err("goal cannot be empty".to_string());
    }
    let id = goal_loop_id(&goal);
    let dir = Path::new(cwd).join(".a3s").join("loops").join(&id);
    let config = dir.join(LOOP_CONFIG);

    if config.exists() {
        let spec = loop_engineering::read_spec(&config)?;
        if normalize_goal(&spec.goal) != goal {
            return Err(format!("goal loop id collision for `{id}`"));
        }
        std::fs::create_dir_all(dir.join("skills")).map_err(|error| error.to_string())?;
        std::fs::create_dir_all(dir.join("reports")).map_err(|error| error.to_string())?;
        write_if_missing(&dir.join(STATE_FILE), &initial_goal_state(&spec))?;
        write_if_missing(
            &dir.join(RUN_LOG_FILE),
            &format!("# Goal Run Log: {}\n\n", spec.id),
        )?;
        write_if_missing(
            &dir.join(BUDGET_FILE),
            &format!(
                "tokens_per_day = {GOAL_BUDGET_TOKENS_PER_DAY}\n\
                 max_iterations_per_run = 0\n\
                 completion_gate = \"goal_achieved_event\"\n\
                 kill_switch = false\n"
            ),
        )?;
        write_if_missing(&dir.join("skills").join("maker.md"), &maker_skill(&goal))?;
        write_if_missing(
            &dir.join("skills").join("verifier.md"),
            &verifier_skill(&goal),
        )?;
        return Ok(spec);
    }

    std::fs::create_dir_all(dir.join("skills")).map_err(|error| error.to_string())?;
    std::fs::create_dir_all(dir.join("reports")).map_err(|error| error.to_string())?;
    let spec = LoopSpec {
        id,
        pattern: "goal-engineering".to_string(),
        goal,
        level: "G1".to_string(),
        cadence: "continuous-until-achieved".to_string(),
        os_runtime: false,
        worktree: false,
        maker_agent: "goal-maker".to_string(),
        checker_agent: "goal-verifier".to_string(),
        budget_tokens_per_day: GOAL_BUDGET_TOKENS_PER_DAY,
        // Zero is deliberately unbounded: only GoalAchieved closes this loop.
        max_iterations_per_run: 0,
        denylist: vec![".env*".to_string(), "secrets/**".to_string()],
        connectors: Vec::new(),
        dir: dir.clone(),
    };
    std::fs::write(&config, loop_engineering::spec_text(&spec))
        .map_err(|error| error.to_string())?;
    std::fs::write(dir.join(STATE_FILE), initial_goal_state(&spec))
        .map_err(|error| error.to_string())?;
    std::fs::write(
        dir.join(RUN_LOG_FILE),
        format!("# Goal Run Log: {}\n\n", spec.id),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(
        dir.join(BUDGET_FILE),
        format!(
            "tokens_per_day = {GOAL_BUDGET_TOKENS_PER_DAY}\n\
             max_iterations_per_run = 0\n\
             completion_gate = \"goal_achieved_event\"\n\
             kill_switch = false\n"
        ),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(dir.join("skills").join("maker.md"), maker_skill(&spec.goal))
        .map_err(|error| error.to_string())?;
    std::fs::write(
        dir.join("skills").join("verifier.md"),
        verifier_skill(&spec.goal),
    )
    .map_err(|error| error.to_string())?;
    Ok(spec)
}

fn runtime_section(run: &GoalRunState, status: &str, event: &str) -> String {
    let event = event.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "{GOAL_RUNTIME_START}\n\
         Status: {status}\n\
         Iteration: {}\n\
         Last plan progress: {:.0}%\n\
         Goal: {}\n\
         Last event: {event}\n\
         {GOAL_RUNTIME_END}",
        run.iteration,
        run.progress * 100.0,
        run.spec.goal,
    )
}

fn goal_progress_checkpoint(progress: f32) -> u8 {
    let percent = (progress.clamp(0.0, 1.0) * 100.0).floor() as u8;
    if percent == 100 {
        return 100;
    }
    (percent / GOAL_PROGRESS_PERSIST_STEP_PERCENT) * GOAL_PROGRESS_PERSIST_STEP_PERCENT
}

fn runtime_document_with_section(mut current: String, loop_id: &str, replacement: &str) -> String {
    let runtime_range = current.find(GOAL_RUNTIME_START).and_then(|start| {
        current[start..].find(GOAL_RUNTIME_END).map(|relative_end| {
            let end = start + relative_end + GOAL_RUNTIME_END.len();
            start..end
        })
    });
    if let Some(range) = runtime_range {
        current.replace_range(range, replacement);
    } else if current.trim().is_empty() {
        current = format!("# Goal Loop State: {loop_id}\n\n{replacement}\n");
    } else {
        current.push_str("\n\n");
        current.push_str(replacement);
        current.push('\n');
    }
    current
}

fn persist_runtime_state(run: &GoalRunState, status: &str, event: &str) -> Result<(), String> {
    let path = run.spec.dir.join(STATE_FILE);
    let previous = std::fs::read_to_string(&path).unwrap_or_default();
    let replacement = runtime_section(run, status, event);
    let current = runtime_document_with_section(previous.clone(), &run.spec.id, &replacement);
    if current == previous {
        return Ok(());
    }
    std::fs::write(path, current).map_err(|error| error.to_string())
}

fn append_goal_log(run: &GoalRunState, status: &str, detail: &str) -> Result<(), String> {
    let detail = detail.split_whitespace().collect::<Vec<_>>().join(" ");
    let line = format!(
        "- {} iteration={} · status={} · {}\n",
        chrono::Utc::now().to_rfc3339(),
        run.iteration,
        status,
        detail
    );
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(run.spec.dir.join(RUN_LOG_FILE))
        .and_then(|mut file| file.write_all(line.as_bytes()))
        .map_err(|error| error.to_string())
}

pub(crate) fn goal_run_prompt(run: &GoalRunState) -> String {
    format!(
        "Run this durable A3S `/goal` Loop Engineering iteration.\n\n\
         Exact goal: {goal}\n\
         Loop id: {id}\n\
         Iteration: {iteration}\n\
         Workspace: {cwd}\n\n\
         Read these files before acting:\n\
         - {config}\n\
         - {state}\n\
         - {log}\n\
         - {maker}\n\
         - {verifier}\n\n\
         Completion contract:\n\
         1. Work on the exact goal, not a smaller substitute. Inspect the current workspace first and preserve unrelated user changes.\n\
         2. Use the maker/verifier split as a dependency-ordered maker -> verifier plan. The maker may implement; the verifier must run only after the maker phase and independently inspect and run fresh checks. Parallelize only genuinely independent work inside a phase; never run overlapping writes or maker and verifier concurrently.\n\
         3. Update STATE.md with concrete evidence, commands, outcomes, remaining gaps, and any genuine blocker. Append this iteration to RUN_LOG.md.\n\
         4. Do not treat a normal answer, the word DONE, a plan ending, or an iteration limit as goal completion. The host derives its GoalAchieved event from fresh evidence; never fabricate that event, but never list the host event itself as a success criterion or remaining user work.\n\
         5. If the goal is not yet fully verified, make the maximum safe progress and report exact remaining work; the host will start another iteration automatically.\n\
         6. Never fabricate test results or completion evidence. When the underlying goal is proven, report the concrete evidence and say that no underlying work remains; the host owns the completion signal.\n\
         7. Treat parallelism as a bounded admission window, not a target. Prefer 2-4 focused read-only branches per wave. For evidence fan-out use allow_partial_failure=true, retain successful results, and retry only failed branches; never replay a completed or potentially mutating branch.\n\
         8. Reuse still-valid evidence already recorded in STATE.md instead of rescanning unchanged areas. Spend this iteration on the highest-value unresolved gap.\n\n\
         Begin iteration {iteration} now.",
        goal = run.spec.goal,
        id = run.spec.id,
        iteration = run.iteration,
        cwd = run.spec.dir.parent().and_then(Path::parent).and_then(Path::parent).unwrap_or(Path::new(".")).display(),
        config = run.spec.dir.join(LOOP_CONFIG).display(),
        state = run.spec.dir.join(STATE_FILE).display(),
        log = run.spec.dir.join(RUN_LOG_FILE).display(),
        maker = run.spec.dir.join("skills").join("maker.md").display(),
        verifier = run.spec.dir.join("skills").join("verifier.md").display(),
    )
}

fn goal_continuation_prompt(run: &GoalRunState, failure: Option<&str>) -> String {
    let reason = failure
        .map(|failure| {
            format!(
                "The previous iteration failed before verification completed: {}\n\
                 Diagnose or work around that failure, then continue. An error is not completion.\n",
                failure.split_whitespace().collect::<Vec<_>>().join(" ")
            )
        })
        .unwrap_or_else(|| {
            "The previous iteration ended without a matching GoalAchieved event. The goal is still active.\n"
                .to_string()
        });
    format!(
        "Continue durable `/goal` loop `{}` at iteration {}.\n\n\
         Exact goal: {}\n\
         {reason}\n\
         Re-read {} and {}. Preserve verified work instead of repeating it, identify the highest-value remaining gap, and use a dependency-ordered maker pass followed by independent verification. Keep read-only fan-out to 2-4 focused branches with allow_partial_failure=true; retain successful branches and never replay completed or potentially mutating work. Update the loop artifacts and continue until the exact goal is proven. Do not use DONE as a completion signal. GoalAchieved is derived by the host after evaluating evidence: never fabricate it and never list that host event as a success criterion or remaining user work. If the underlying goal is proven, report the concrete evidence and state that no underlying work remains.",
        run.spec.id,
        run.iteration,
        run.spec.goal,
        run.spec.dir.join(STATE_FILE).display(),
        run.spec.dir.join(RUN_LOG_FILE).display(),
    )
}

fn retry_delay(failures: usize) -> Duration {
    let exponent = failures.saturating_sub(1).min(5) as u32;
    Duration::from_secs((1u64 << exponent).min(30))
}

impl App {
    pub(crate) fn start_goal_run(&mut self, raw_goal: &str) -> Option<Cmd<Msg>> {
        if self.state != State::Idle {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  finish the current turn before starting a goal"),
            );
            return None;
        }
        self.clear_paused_goal("replaced by a new /goal");
        let requested = normalize_goal(raw_goal);
        let effective_goal = self
            .agent_dev
            .as_ref()
            .map(|dev| agent::agent_goal_label(dev, &requested))
            .unwrap_or(requested);
        let spec = match init_goal_loop(&self.cwd, &effective_goal) {
            Ok(spec) => spec,
            Err(error) => {
                self.push_line(&Style::new().fg(TN_RED).render(&format!(
                    "  /goal could not create Loop Engineering: {error}"
                )));
                return None;
            }
        };

        let previous_effort = self.effort;
        let previous_goal = self.goal.clone();
        let previous_goal_since = self.goal_since;
        self.goal_generation = self.goal_generation.wrapping_add(1).max(1);
        let run = GoalRunState::new(self.goal_generation, spec);
        self.goal = Some(run.spec.goal.clone());
        self.goal_since = Some(Instant::now());
        self.goal_run = Some(run);

        let mut profile = self.session_rebuild_profile();
        profile.effort = ULTRACODE;
        self.start_session_rebuild(
            profile,
            SessionRebuildAction::GoalStart {
                generation: self.goal_generation,
                previous_effort,
                previous_goal,
                previous_goal_since,
            },
        )
    }

    pub(crate) fn finish_goal_start(&mut self) -> Option<Cmd<Msg>> {
        self.effort = ULTRACODE;
        self.set_composer_mode(Mode::Auto);
        self.autonomy_restore = None;
        self.loop_remaining = 0;
        self.gradient_until = Some(Instant::now());
        self.gradient_frame = 0;

        let run = self.goal_run.as_ref().expect("goal run initialized");
        let _ = persist_runtime_state(run, "running", "Ultracode goal run started");
        let _ = append_goal_log(run, "running", "Ultracode + forced planning enabled");
        let mut prompt = goal_run_prompt(run);
        let display = if let Some(dev) = &self.agent_dev {
            prompt = agent::agent_loop_prompt(dev, &prompt);
            format!("◇ {} goal: {}", dev.name, truncate(&run.spec.goal, 48))
        } else {
            format!("◎ goal: {}", truncate(&run.spec.goal, 54))
        };
        let id = run.spec.id.clone();
        let dir = run.spec.dir.display().to_string();
        self.push_line(&gutter(
            ACCENT,
            &format!("◎\u{200A}goal loop `{id}` · Ultracode · continues until verified"),
        ));
        self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
            "  Loop Engineering: {dir} · Esc or /goal clear cancels"
        )));
        self.start_stream_inner(prompt, display, true, true, false)
    }

    pub(crate) fn finish_goal_resume(&mut self) -> Option<Cmd<Msg>> {
        self.autonomy_restore = None;
        self.loop_remaining = 0;
        self.gradient_until = Some(Instant::now());
        self.gradient_frame = 0;

        let run = self.goal_run.as_mut().expect("goal run restored");
        run.begin_resumed_iteration();
        let _ = persist_runtime_state(run, "running", "paused goal resumed by user");
        let _ = append_goal_log(run, "running", "paused goal resumed by user");
        let prompt = goal_continuation_prompt(run, None);
        let display = format!(
            "◎ resumed goal iteration {}: {}",
            run.iteration,
            truncate(&run.spec.goal, 48)
        );
        let id = run.spec.id.clone();
        self.push_line(&gutter(
            ACCENT,
            &format!("◎\u{200A}resumed goal loop `{id}` · continues until verified"),
        ));
        self.start_stream_inner(prompt, display, true, true, false)
    }

    pub(crate) fn record_goal_extracted(&mut self, goal: &AgentGoal) {
        if let Some(run) = self.goal_run.as_mut() {
            run.record_extracted_goal(goal);
            let _ = persist_runtime_state(run, "running", "goal extracted; executing plan");
        }
    }

    pub(crate) fn record_goal_progress(&mut self, progress: f32) {
        if let Some(run) = self.goal_run.as_mut() {
            let previous_checkpoint = goal_progress_checkpoint(run.progress);
            run.record_progress(progress);
            if goal_progress_checkpoint(run.progress) != previous_checkpoint {
                let _ = persist_runtime_state(run, "running", "plan progress updated");
            }
        }
    }

    pub(crate) fn record_goal_achieved(&mut self, goal: &str) {
        if let Some(run) = self.goal_run.as_mut() {
            if run.record_achievement(goal) {
                let _ = persist_runtime_state(run, "verified", "matching GoalAchieved received");
                let _ = append_goal_log(run, "verified", "matching GoalAchieved received");
            }
        }
    }

    pub(crate) fn continue_goal_run(&mut self, failure: Option<String>) -> Option<Cmd<Msg>> {
        if self.goal_run.as_ref().is_some_and(|run| run.achieved) {
            return self.finish_achieved_goal();
        }
        let run = self.goal_run.as_mut()?;
        let failed = failure.is_some();
        run.begin_next_iteration(failed);
        let prompt = goal_continuation_prompt(run, failure.as_deref());
        let generation = run.generation;
        let iteration = run.iteration;
        let failures = run.failures;
        let _ = persist_runtime_state(
            run,
            if failed { "retrying" } else { "running" },
            failure
                .as_deref()
                .unwrap_or("GoalAchieved not received; continuing"),
        );
        let _ = append_goal_log(
            run,
            if failed { "retrying" } else { "continuing" },
            failure.as_deref().unwrap_or("GoalAchieved not received"),
        );
        let delay = failed.then(|| retry_delay(failures));
        let suffix = delay
            .map(|delay| format!(" · retry in {}s", delay.as_secs()))
            .unwrap_or_default();
        self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
            "  ↻ goal iteration {iteration} · verification still open{suffix} · Esc stops"
        )));
        Some(match delay {
            Some(delay) => cmd::cmd(move || async move {
                tokio::time::sleep(delay).await;
                Msg::GoalContinue { generation, prompt }
            }),
            None => cmd::msg(Msg::GoalContinue { generation, prompt }),
        })
    }

    pub(crate) fn handle_goal_continue(
        &mut self,
        generation: u64,
        prompt: String,
    ) -> Option<Cmd<Msg>> {
        let run = self.goal_run.as_mut()?;
        if !run.is_generation(generation) || run.achieved || self.state != State::Idle {
            return None;
        }
        run.accepting_achievement = true;
        run.extracted_goal = None;
        let display = format!(
            "◎ goal iteration {}: {}",
            run.iteration,
            truncate(&run.spec.goal, 48)
        );
        self.start_stream_inner(prompt, display, true, false, false)
    }

    pub(crate) fn finish_achieved_goal(&mut self) -> Option<Cmd<Msg>> {
        let run = self.goal_run.take()?;
        if !run.achieved {
            self.goal_run = Some(run);
            return None;
        }
        let _ = persist_runtime_state(&run, "achieved", "goal completed and run closed");
        let _ = append_goal_log(&run, "achieved", "goal completed and run closed");
        self.goal = None;
        self.goal_since = None;
        self.loop_remaining = 0;
        self.push_line(&gutter(
            TN_GREEN,
            &format!(
                "◎\u{200A}goal achieved · {} iterations · {}",
                run.iteration,
                run.spec.dir.display()
            ),
        ));
        self.restore_goal_planning_mode()
    }

    pub(crate) fn cancel_goal_state(&mut self, reason: &str) -> bool {
        self.goal_generation = self.goal_generation.wrapping_add(1).max(1);
        let Some(run) = self.goal_run.take() else {
            return false;
        };
        let _ = persist_runtime_state(&run, "cancelled", reason);
        let _ = append_goal_log(&run, "cancelled", reason);
        self.goal = None;
        self.goal_since = None;
        self.loop_remaining = 0;
        true
    }

    pub(crate) fn restore_goal_planning_mode(&mut self) -> Option<Cmd<Msg>> {
        if self.state != State::Idle {
            return None;
        }
        let profile = self.session_rebuild_profile();
        self.start_session_rebuild(profile, SessionRebuildAction::GoalRestore)
    }

    pub(crate) fn clear_goal_command(&mut self) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let active = self.cancel_goal_state("cleared by user");
        let paused = self.clear_paused_goal("cleared by user");
        if !active {
            self.goal = None;
            self.goal_since = None;
            self.push_line(&Style::new().fg(TN_GRAY).render(if paused {
                "  paused goal cleared"
            } else {
                "  goal cleared"
            }));
            return None;
        }
        self.push_line(&Style::new().fg(TN_GRAY).render("  goal loop cleared"));
        if self.session_rebuild_pending.is_some() {
            // The atomic build cannot be force-aborted through the TEA command
            // handle. Its completion observes the invalidated generation and
            // immediately restores ordinary Ultracode planning instead of
            // starting the cancelled goal.
            return None;
        }
        if self.state == State::Idle {
            return self.restore_goal_planning_mode();
        }

        self.interrupting = true;
        let session = self.session.clone();
        let join = self.stream_join.take();
        let host_abort = self.host_tool_abort.take();
        Some(cmd::cmd(move || async move {
            if let Some(host_abort) = host_abort {
                host_abort.abort();
            }
            let _ = session
                .cancel_and_settle(
                    Duration::from_millis(DEEP_RESEARCH_ABORT_GRACE_MS),
                    Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                )
                .await;
            if let Some(join) = join {
                let _ = settle_stream_join_for_quit(
                    join,
                    Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                )
                .await;
            }
            Msg::GoalCleared
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "a3s-goal-{name}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn goal_loop_scaffolds_complete_loop_engineering_workspace() {
        let root = temp_root("scaffold");
        let spec = init_goal_loop(
            &root.to_string_lossy(),
            "Implement the feature and verify it end to end",
        )
        .unwrap();

        assert_eq!(spec.level, "G1");
        assert_eq!(spec.max_iterations_per_run, 0);
        assert!(spec.id.starts_with("goal-implement-the-feature"));
        for path in [
            spec.dir.join(LOOP_CONFIG),
            spec.dir.join(STATE_FILE),
            spec.dir.join(RUN_LOG_FILE),
            spec.dir.join(BUDGET_FILE),
            spec.dir.join("skills/maker.md"),
            spec.dir.join("skills/verifier.md"),
        ] {
            assert!(path.is_file(), "missing {}", path.display());
        }
        assert!(spec.dir.join("reports").is_dir());
        assert_eq!(loop_engineering::audit_loop(&spec).score, 100);
        let budget = std::fs::read_to_string(spec.dir.join(BUDGET_FILE)).unwrap();
        assert!(budget.contains("completion_gate = \"goal_achieved_event\""));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reopening_goal_loop_preserves_agent_work_notes() {
        let root = temp_root("resume");
        let cwd = root.to_string_lossy();
        let spec = init_goal_loop(&cwd, "Keep this exact goal active").unwrap();
        let state = spec.dir.join(STATE_FILE);
        let mut body = std::fs::read_to_string(&state).unwrap();
        body.push_str("\n## Work Notes\n\n- Preserve this evidence.\n");
        std::fs::write(&state, body).unwrap();

        let reopened = init_goal_loop(&cwd, "Keep this exact goal active").unwrap();

        assert_eq!(reopened.id, spec.id);
        assert!(std::fs::read_to_string(state)
            .unwrap()
            .contains("Preserve this evidence"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn paused_goal_restores_state_and_resumes_at_the_next_iteration() {
        let root = temp_root("paused-resume");
        let cwd = root.to_string_lossy();
        let spec = init_goal_loop(&cwd, "Keep this resumable goal active").unwrap();
        let mut original = GoalRunState::new(7, spec);
        original.iteration = 4;
        original.progress = 0.65;
        original.failures = 2;
        let paused = original.paused_state();

        let mut restored = GoalRunState::from_paused(&cwd, 8, &paused).unwrap();

        assert_eq!(restored.generation, 8);
        assert_eq!(restored.spec.id, paused.loop_id);
        assert_eq!(restored.spec.goal, paused.goal);
        assert_eq!(restored.iteration, 4);
        assert_eq!(restored.progress, 0.65);
        assert_eq!(restored.failures, 2);
        assert!(!restored.accepting_achievement);

        restored.begin_resumed_iteration();
        assert_eq!(restored.iteration, 5);
        assert_eq!(restored.progress, 0.0);
        assert!(restored.accepting_achievement);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn only_the_current_extracted_goal_can_latch_achievement() {
        let root = temp_root("latch");
        let spec = init_goal_loop(&root.to_string_lossy(), "Verify the exact target").unwrap();
        let mut run = GoalRunState::new(7, spec);
        let goal = AgentGoal::new("Verify the exact target");
        run.record_extracted_goal(&goal);

        assert!(!run.record_achievement("Different target"));
        assert!(!run.achieved);
        assert!(run.record_achievement("Verify the exact target"));
        assert!(run.achieved);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn user_turn_goal_events_cannot_close_the_host_goal_iteration() {
        let root = temp_root("user-turn");
        let spec = init_goal_loop(&root.to_string_lossy(), "Keep the host goal open").unwrap();
        let mut run = GoalRunState::new(8, spec);
        run.pause_achievement_for_user_turn();
        let unrelated = AgentGoal::new("Answer the queued side request");
        run.record_extracted_goal(&unrelated);

        assert!(!run.record_achievement("Answer the queued side request"));
        assert!(!run.achieved);
        run.begin_next_iteration(false);
        let actual = AgentGoal::new("Keep the host goal open");
        run.record_extracted_goal(&actual);
        assert!(run.record_achievement("Keep the host goal open"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn goal_iterations_have_no_fixed_completion_cap() {
        let root = temp_root("iterations");
        let spec = init_goal_loop(&root.to_string_lossy(), "Continue until verified").unwrap();
        let mut run = GoalRunState::new(11, spec);

        for _ in 0..1_000 {
            run.begin_next_iteration(false);
        }

        assert_eq!(run.iteration, 1_001);
        assert!(!run.achieved);
        assert!(run.is_generation(11));
        assert!(!run.is_generation(12));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn unverified_first_iteration_continues_and_second_can_complete() {
        let root = temp_root("two-iterations");
        let spec =
            init_goal_loop(&root.to_string_lossy(), "Finish only after verification").unwrap();
        let mut run = GoalRunState::new(21, spec);
        let first = AgentGoal::new("Finish only after verification");
        run.record_extracted_goal(&first);
        run.record_progress(1.0);

        assert!(!run.achieved, "plan completion is not goal completion");
        run.begin_next_iteration(false);
        assert_eq!(run.iteration, 2);
        assert!(!run.achieved);

        let second = AgentGoal::new("Finish only after verification");
        run.record_extracted_goal(&second);
        assert!(run.record_achievement("Finish only after verification"));
        assert!(run.achieved);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn goal_prompt_uses_event_gate_instead_of_done_text() {
        let root = temp_root("prompt");
        let spec = init_goal_loop(&root.to_string_lossy(), "Prove the release is ready").unwrap();
        let run = GoalRunState::new(1, spec);
        let prompt = goal_run_prompt(&run);

        assert!(prompt.contains("Ultracode") || prompt.contains("`/goal`"));
        assert!(prompt.contains("The host derives its GoalAchieved event from fresh evidence"));
        assert!(prompt.contains("never list the host event itself as a success criterion"));
        assert!(prompt.contains("maker/verifier split"));
        assert!(prompt.contains("word DONE"));
        assert!(prompt.contains("maker -> verifier"));
        assert!(prompt.contains("2-4 focused read-only branches"));
        assert!(prompt.contains("allow_partial_failure=true"));
        let continuation = goal_continuation_prompt(&run, None);
        assert!(continuation.contains("GoalAchieved is derived by the host"));
        assert!(continuation.contains("never list that host event as a success criterion"));
        assert!(continuation.contains("Preserve verified work instead of repeating it"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn goal_progress_persistence_is_bounded_to_five_percent_checkpoints() {
        assert_eq!(goal_progress_checkpoint(0.0), 0);
        assert_eq!(goal_progress_checkpoint(0.049), 0);
        assert_eq!(goal_progress_checkpoint(0.051), 5);
        assert_eq!(goal_progress_checkpoint(0.10), 10);
        assert_eq!(goal_progress_checkpoint(0.994), 95);
        assert_eq!(goal_progress_checkpoint(1.0), 100);
    }

    #[test]
    fn unchanged_runtime_document_does_not_require_another_write() {
        let root = temp_root("runtime-dedup");
        let spec = init_goal_loop(&root.to_string_lossy(), "Keep runtime writes bounded").unwrap();
        let run = GoalRunState::new(31, spec);
        let replacement = runtime_section(&run, "running", "plan progress updated");
        let initial = runtime_document_with_section(String::new(), &run.spec.id, &replacement);
        let repeated = runtime_document_with_section(initial.clone(), &run.spec.id, &replacement);

        assert_eq!(repeated, initial);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn next_goal_iteration_resets_plan_progress_without_losing_failure_history() {
        let root = temp_root("progress-reset");
        let spec = init_goal_loop(&root.to_string_lossy(), "Reset each plan wave").unwrap();
        let mut run = GoalRunState::new(41, spec);
        run.record_progress(0.85);

        run.begin_next_iteration(true);

        assert_eq!(run.iteration, 2);
        assert_eq!(run.progress, 0.0);
        assert_eq!(run.failures, 1);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn goal_error_retries_back_off_without_becoming_terminal() {
        assert_eq!(retry_delay(1), Duration::from_secs(1));
        assert_eq!(retry_delay(2), Duration::from_secs(2));
        assert_eq!(retry_delay(6), Duration::from_secs(30));
        assert_eq!(retry_delay(100), Duration::from_secs(30));
    }
}
