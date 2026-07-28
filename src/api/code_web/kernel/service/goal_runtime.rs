use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn update_goal_action(
        &self,
        session_id: &str,
        action: &str,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        let goal = {
            let mut controls_by_session = self.state.session_controls.lock().await;
            let controls = controls_by_session
                .entry(session_id.to_string())
                .or_default();
            let goal = controls.goal.clone().ok_or_else(|| {
                BootError::BadRequest("the session has no active goal".to_string())
            })?;
            let run = controls.goal_run.get_or_insert_with(|| {
                let now = chrono::Utc::now().timestamp_millis();
                CodeWebGoalRun {
                    started_at: now,
                    updated_at: now,
                    ..CodeWebGoalRun::default()
                }
            });
            let now = chrono::Utc::now().timestamp_millis();
            match action {
                "pause" => run.status = CodeWebGoalStatus::Paused,
                "resume" => {
                    if run.status == CodeWebGoalStatus::Achieved {
                        return Err(BootError::BadRequest(
                            "an achieved goal cannot be resumed; set a new goal instead"
                                .to_string(),
                        ));
                    }
                    run.status = CodeWebGoalStatus::Active;
                    run.completed_at = None;
                }
                "retry" => {
                    if run.status == CodeWebGoalStatus::Achieved {
                        return Err(BootError::BadRequest(
                            "an achieved goal cannot be retried; set a new goal instead"
                                .to_string(),
                        ));
                    }
                    run.status = CodeWebGoalStatus::Retrying;
                    run.completed_at = None;
                    run.last_error = None;
                }
                _ => {
                    return Err(BootError::BadRequest(format!(
                        "unsupported goal action `{action}`"
                    )))
                }
            }
            run.updated_at = now;
            goal
        };

        if action == "pause" {
            self.state
                .session_turn_queues
                .lock()
                .await
                .entry(session_id.to_string())
                .or_default()
                .remove_kind(CodeWebQueuedTurnKind::GoalContinuation);
        } else {
            self.state
                .session_turn_queues
                .lock()
                .await
                .entry(session_id.to_string())
                .or_default()
                .resume();
            let attempt = self
                .session_controls_snapshot(session_id)
                .await
                .goal_run
                .map(|run| run.attempts)
                .unwrap_or_default();
            self.enqueue_goal_continuation(session_id, &goal, attempt)
                .await?;
        }
        self.persist_session_state(session_id).await?;
        self.session_controls(session_id).await
    }

    pub(in crate::api::code_web) async fn cancel_session(
        &self,
        session_id: &str,
    ) -> BootResult<Value> {
        let session = self.kernel_session(session_id).await?;
        let research_cancelled = self
            .state
            .active_research_runs
            .lock()
            .await
            .get(session_id)
            .cloned()
            .is_some_and(|cancellation| {
                cancellation.cancel();
                true
            });
        let cancelled = session.cancel().await || research_cancelled;
        {
            let mut controls_by_session = self.state.session_controls.lock().await;
            if let Some(run) = controls_by_session
                .entry(session_id.to_string())
                .or_default()
                .goal_run
                .as_mut()
            {
                if run.status != CodeWebGoalStatus::Achieved {
                    run.status = CodeWebGoalStatus::Paused;
                    run.updated_at = chrono::Utc::now().timestamp_millis();
                }
            }
        }
        {
            let mut queues = self.state.session_turn_queues.lock().await;
            let queue = queues.entry(session_id.to_string()).or_default();
            queue.pause();
            queue.remove_kind(CodeWebQueuedTurnKind::GoalContinuation);
        }
        self.persist_session_state(session_id).await?;
        Ok(json!({
            "sessionId": session_id,
            "cancelled": cancelled,
        }))
    }

    pub(super) async fn begin_goal_attempt(&self, session_id: &str) {
        let mut controls_by_session = self.state.session_controls.lock().await;
        let Some(run) = controls_by_session
            .entry(session_id.to_string())
            .or_default()
            .goal_run
            .as_mut()
        else {
            return;
        };
        if !matches!(
            run.status,
            CodeWebGoalStatus::Active | CodeWebGoalStatus::Retrying
        ) {
            return;
        }
        run.status = CodeWebGoalStatus::Active;
        run.attempts = run.attempts.saturating_add(1);
        run.progress_percent = 0;
        run.completed_steps = 0;
        run.total_steps = 0;
        run.extracted_goal = None;
        run.updated_at = chrono::Utc::now().timestamp_millis();
    }

    pub(super) async fn observe_goal_event(&self, session_id: &str, event: &AgentEvent) {
        let mut controls_by_session = self.state.session_controls.lock().await;
        let Some(run) = controls_by_session
            .entry(session_id.to_string())
            .or_default()
            .goal_run
            .as_mut()
        else {
            return;
        };
        if run.status == CodeWebGoalStatus::Paused || run.status == CodeWebGoalStatus::Achieved {
            return;
        }
        apply_goal_event(run, event, chrono::Utc::now().timestamp_millis());
    }

    pub(super) async fn pending_goal_continuation(
        &self,
        session_id: &str,
    ) -> Option<(String, u32)> {
        let controls = self.session_controls_snapshot(session_id).await;
        let goal = controls.goal?;
        let run = controls.goal_run?;
        matches!(
            run.status,
            CodeWebGoalStatus::Active | CodeWebGoalStatus::Retrying
        )
        .then_some((goal, run.attempts))
    }
}

fn apply_goal_event(run: &mut CodeWebGoalRun, event: &AgentEvent, now: i64) {
    match event {
        AgentEvent::GoalExtracted { goal } => {
            run.extracted_goal = normalize_goal(&goal.description);
            run.updated_at = now;
        }
        AgentEvent::GoalProgress {
            progress,
            completed_steps,
            total_steps,
            ..
        } => {
            run.progress_percent = (progress.clamp(0.0, 1.0) * 100.0).round() as u8;
            run.completed_steps = *completed_steps;
            run.total_steps = *total_steps;
            run.updated_at = now;
        }
        AgentEvent::GoalAchieved {
            goal, total_steps, ..
        } if run.extracted_goal.as_deref() == normalize_goal(goal).as_deref() => {
            run.status = CodeWebGoalStatus::Achieved;
            run.progress_percent = 100;
            run.completed_steps = *total_steps;
            run.total_steps = *total_steps;
            run.completed_at = Some(now);
            run.updated_at = now;
            run.last_error = None;
        }
        AgentEvent::Error { message } => {
            run.status = CodeWebGoalStatus::Retrying;
            run.last_error = Some(message.clone());
            run.updated_at = now;
        }
        _ => {}
    }
}

pub(super) fn goal_continuation_prompt(goal: &str, attempt: u32) -> String {
    format!(
        "The previous goal attempt ended without a matching GoalAchieved event. The goal remains active.\n\
         Goal: {goal}\n\
         Continue with attempt {}. Preserve verified work, identify the highest-value remaining gap, implement it, and independently verify the result. Do not claim completion without fresh evidence.",
        attempt.saturating_add(1)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::planning::AgentGoal;

    #[test]
    fn only_matching_extracted_goal_closes_the_durable_goal() {
        let mut run = CodeWebGoalRun {
            status: CodeWebGoalStatus::Active,
            started_at: 1,
            updated_at: 1,
            ..CodeWebGoalRun::default()
        };
        apply_goal_event(
            &mut run,
            &AgentEvent::GoalExtracted {
                goal: AgentGoal::new("focused tests pass"),
            },
            2,
        );
        apply_goal_event(
            &mut run,
            &AgentEvent::GoalAchieved {
                goal: "different goal".to_string(),
                total_steps: 3,
                duration_ms: 10,
            },
            3,
        );
        assert_eq!(run.status, CodeWebGoalStatus::Active);

        apply_goal_event(
            &mut run,
            &AgentEvent::GoalAchieved {
                goal: "focused tests pass".to_string(),
                total_steps: 4,
                duration_ms: 20,
            },
            4,
        );
        assert_eq!(run.status, CodeWebGoalStatus::Achieved);
        assert_eq!(run.progress_percent, 100);
        assert_eq!(run.completed_at, Some(4));
    }

    #[test]
    fn continuation_prompt_keeps_verification_as_the_completion_gate() {
        let prompt = goal_continuation_prompt("ship the Web flow", 2);
        assert!(prompt.contains("attempt 3"));
        assert!(prompt.contains("GoalAchieved"));
        assert!(prompt.contains("fresh evidence"));
    }
}
