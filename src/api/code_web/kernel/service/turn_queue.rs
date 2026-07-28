use a3s_code_core::host_env::HostEnv;

use super::*;

const MAX_PENDING_TURNS: usize = 100;

impl KernelService {
    pub(in crate::api::code_web) async fn session_turn_queue(
        &self,
        session_id: &str,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        Ok(self.session_turn_queue_json(session_id).await)
    }

    pub(in crate::api::code_web) async fn enqueue_session_turn(
        &self,
        session_id: &str,
        request: Value,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        let content = required_queue_content(&request)?;
        let context_files = string_array(&request, "contextFiles")?;
        let skill_names = string_array(&request, "skillNames")?;
        let mode = queued_turn_mode(&request)?;
        if mode == CodeWebQueuedTurnMode::DeepResearch {
            super::research::validate_code_web_research_turn(
                &content,
                &context_files,
                &skill_names,
            )
            .map_err(BootError::BadRequest)?;
        }
        let turn_id = HostEnv::default().next_id();
        let research_run_id = (mode == CodeWebQueuedTurnMode::DeepResearch)
            .then(|| super::research::code_web_research_run_id(&turn_id));
        let turn = CodeWebQueuedTurn {
            id: turn_id,
            kind: CodeWebQueuedTurnKind::User,
            content,
            context_files,
            skill_names,
            mode,
            research_run_id,
            priority: USER_TURN_PRIORITY,
            enqueued_at: chrono::Utc::now().timestamp_millis(),
        };
        let accepted_item_id = turn.id.clone();
        {
            let mut queues = self.state.session_turn_queues.lock().await;
            let queue = queues.entry(session_id.to_string()).or_default();
            if queue.snapshot().items.len() >= MAX_PENDING_TURNS {
                return Err(BootError::BadRequest(format!(
                    "the turn queue cannot exceed {MAX_PENDING_TURNS} pending items"
                )));
            }
            queue.enqueue(turn);
        }
        self.persist_session_state(session_id).await?;
        let mut response = self.session_turn_queue_json(session_id).await;
        response["acceptedItemId"] = Value::String(accepted_item_id);
        Ok(response)
    }

    pub(in crate::api::code_web) async fn update_session_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        request: Value,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        let content = required_queue_content(&request)?;
        let context_files = string_array(&request, "contextFiles")?;
        let skill_names = string_array(&request, "skillNames")?;
        let updated = {
            let mut queues = self.state.session_turn_queues.lock().await;
            let queue = queues.entry(session_id.to_string()).or_default();
            let deep_research =
                queue.snapshot().items.iter().any(|turn| {
                    turn.id == turn_id && turn.mode == CodeWebQueuedTurnMode::DeepResearch
                });
            if deep_research {
                super::research::validate_code_web_research_turn(
                    &content,
                    &context_files,
                    &skill_names,
                )
                .map_err(BootError::BadRequest)?;
            }
            queue.update_user_turn(turn_id, content, context_files, skill_names)
        };
        if !updated {
            return Err(BootError::NotFound(format!(
                "queued user turn `{turn_id}` was not found"
            )));
        }
        self.persist_session_state(session_id).await?;
        Ok(self.session_turn_queue_json(session_id).await)
    }

    pub(in crate::api::code_web) async fn delete_session_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        let removed = self
            .state
            .session_turn_queues
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .remove(turn_id);
        if !removed {
            return Err(BootError::NotFound(format!(
                "queued turn `{turn_id}` was not found"
            )));
        }
        self.persist_session_state(session_id).await?;
        Ok(self.session_turn_queue_json(session_id).await)
    }

    pub(in crate::api::code_web) async fn reorder_session_turns(
        &self,
        session_id: &str,
        request: Value,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        let ordered_ids = string_array(&request, "orderedIds")?;
        let reordered = self
            .state
            .session_turn_queues
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .reorder(&ordered_ids);
        if !reordered {
            return Err(BootError::BadRequest(
                "orderedIds must contain every pending turn exactly once".to_string(),
            ));
        }
        self.persist_session_state(session_id).await?;
        Ok(self.session_turn_queue_json(session_id).await)
    }

    pub(in crate::api::code_web) async fn update_session_turn_queue_action(
        &self,
        session_id: &str,
        action: &str,
    ) -> BootResult<Value> {
        self.kernel_session(session_id).await?;
        let mut queues = self.state.session_turn_queues.lock().await;
        let queue = queues.entry(session_id.to_string()).or_default();
        match action {
            "pause" => queue.pause(),
            "resume" => queue.resume(),
            _ => {
                return Err(BootError::BadRequest(format!(
                    "unsupported turn queue action `{action}`"
                )))
            }
        }
        drop(queues);
        self.persist_session_state(session_id).await?;
        Ok(self.session_turn_queue_json(session_id).await)
    }

    pub(super) async fn session_turn_queue_snapshot(
        &self,
        session_id: &str,
    ) -> CodeWebStoredTurnQueue {
        self.state
            .session_turn_queues
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .snapshot()
    }

    pub(super) async fn begin_queued_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> BootResult<CodeWebQueuedTurn> {
        let turn = self
            .state
            .session_turn_queues
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .begin(turn_id, chrono::Utc::now().timestamp_millis())
            .map_err(|message| BootError::BadRequest(message.to_string()))?;
        self.persist_session_state(session_id).await?;
        Ok(turn)
    }

    pub(super) async fn restore_queued_turn(
        &self,
        session_id: &str,
        turn_id: &str,
    ) -> BootResult<()> {
        self.state
            .session_turn_queues
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .restore_active(turn_id);
        self.persist_session_state(session_id).await
    }

    pub(super) async fn finish_queued_turn(
        &self,
        session_id: &str,
        turn_id: &str,
        pause: bool,
    ) -> BootResult<()> {
        self.state
            .session_turn_queues
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .finish_active(turn_id, pause);
        self.persist_session_state(session_id).await
    }

    pub(super) async fn enqueue_goal_continuation(
        &self,
        session_id: &str,
        goal: &str,
        attempt: u32,
    ) -> BootResult<()> {
        let mut queues = self.state.session_turn_queues.lock().await;
        let queue = queues.entry(session_id.to_string()).or_default();
        if queue.contains_kind(CodeWebQueuedTurnKind::GoalContinuation) {
            return Ok(());
        }
        queue.enqueue(CodeWebQueuedTurn {
            id: HostEnv::default().next_id(),
            kind: CodeWebQueuedTurnKind::GoalContinuation,
            content: format!(
                "Continue goal attempt {}: {}",
                attempt.saturating_add(1),
                goal
            ),
            context_files: Vec::new(),
            skill_names: Vec::new(),
            mode: CodeWebQueuedTurnMode::Standard,
            research_run_id: None,
            priority: GOAL_CONTINUATION_PRIORITY,
            enqueued_at: chrono::Utc::now().timestamp_millis(),
        });
        drop(queues);
        self.persist_session_state(session_id).await
    }

    async fn session_turn_queue_json(&self, session_id: &str) -> Value {
        let snapshot = self.session_turn_queue_snapshot(session_id).await;
        let active_research = match snapshot.active.as_ref().and_then(|active| {
            (active.turn.mode == CodeWebQueuedTurnMode::DeepResearch)
                .then_some(active.turn.research_run_id.as_deref())
                .flatten()
        }) {
            Some(run_id) => match self.kernel_session(session_id).await {
                Ok(session) => match crate::research::read_code_deep_research_journal(
                    session.workspace(),
                    run_id,
                )
                .await
                {
                    Ok(Some(research)) => Some(json!({
                        "schemaVersion": research.schema_version,
                        "runId": research.run_id,
                        "sequence": research.sequence,
                        "recordedAt": research.recorded_at,
                        "lifecycle": research.lifecycle,
                        "stage": research.stage,
                        "publication": research.publication,
                        "quality": research.quality,
                    })),
                    Ok(None) => Some(json!({
                        "schemaVersion": 2,
                        "runId": run_id,
                        "lifecycle": "running",
                    })),
                    Err(error) => {
                        tracing::warn!(%error, %session_id, %run_id, "could not restore DeepResearch journal projection");
                        None
                    }
                },
                Err(_) => None,
            },
            None => None,
        };
        let total = snapshot.items.len();
        let next_item_id = snapshot.items.first().map(|turn| turn.id.clone());
        let status = if snapshot.active.is_some() {
            "running"
        } else if snapshot.paused && !snapshot.items.is_empty() {
            "paused"
        } else if snapshot.items.is_empty() {
            "idle"
        } else {
            "pending"
        };
        json!({
            "sessionId": session_id,
            "status": status,
            "paused": snapshot.paused,
            "active": snapshot.active,
            "research": active_research,
            "items": snapshot.items,
            "total": total,
            "nextItemId": next_item_id,
        })
    }
}

fn required_queue_content(request: &Value) -> BootResult<String> {
    request
        .get("content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|content| !content.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| BootError::BadRequest("content is required".to_string()))
}

fn string_array(request: &Value, key: &str) -> BootResult<Vec<String>> {
    let Some(value) = request.get(key) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| BootError::BadRequest(format!("{key} must be an array of strings")))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .ok_or_else(|| BootError::BadRequest(format!("{key} must contain strings")))
        })
        .collect()
}

pub(super) fn queued_turn_mode(request: &Value) -> BootResult<CodeWebQueuedTurnMode> {
    match request.get("mode") {
        None | Some(Value::Null) => Ok(CodeWebQueuedTurnMode::Standard),
        Some(Value::String(mode)) if mode == "standard" => Ok(CodeWebQueuedTurnMode::Standard),
        Some(Value::String(mode)) if mode == "deepResearch" => {
            Ok(CodeWebQueuedTurnMode::DeepResearch)
        }
        Some(_) => Err(BootError::BadRequest(
            "mode must be `standard` or `deepResearch`".to_string(),
        )),
    }
}
