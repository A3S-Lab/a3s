use std::collections::{HashMap, HashSet};

use a3s_lane::{Priority, PriorityQueue};
use serde::{Deserialize, Serialize};

pub(in crate::api::code_web) const USER_TURN_PRIORITY: Priority = 0;
pub(in crate::api::code_web) const GOAL_CONTINUATION_PRIORITY: Priority = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum CodeWebQueuedTurnKind {
    User,
    GoalContinuation,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum CodeWebQueuedTurnMode {
    #[default]
    Standard,
    DeepResearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebQueuedTurn {
    pub(in crate::api::code_web) id: String,
    pub(in crate::api::code_web) kind: CodeWebQueuedTurnKind,
    pub(in crate::api::code_web) content: String,
    #[serde(default)]
    pub(in crate::api::code_web) context_files: Vec<String>,
    #[serde(default)]
    pub(in crate::api::code_web) skill_names: Vec<String>,
    #[serde(default)]
    pub(in crate::api::code_web) mode: CodeWebQueuedTurnMode,
    #[serde(default)]
    pub(in crate::api::code_web) research_run_id: Option<String>,
    pub(in crate::api::code_web) priority: Priority,
    pub(in crate::api::code_web) enqueued_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebActiveTurn {
    pub(in crate::api::code_web) turn: CodeWebQueuedTurn,
    pub(in crate::api::code_web) started_at: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebStoredTurnQueue {
    pub(in crate::api::code_web) items: Vec<CodeWebQueuedTurn>,
    pub(in crate::api::code_web) active: Option<CodeWebActiveTurn>,
    pub(in crate::api::code_web) paused: bool,
}

#[derive(Debug, Default)]
pub(in crate::api::code_web) struct CodeWebSessionTurnQueue {
    pending: PriorityQueue<CodeWebQueuedTurn>,
    active: Option<CodeWebActiveTurn>,
    paused: bool,
}

impl CodeWebSessionTurnQueue {
    pub(in crate::api::code_web) fn restore(stored: CodeWebStoredTurnQueue) -> Self {
        let mut queue = Self::default();
        let interrupted = stored.active.map(|active| active.turn);
        let recovered_interrupted = interrupted.is_some();
        if let Some(turn) = interrupted {
            queue.pending.push(turn.priority, turn);
        }
        for turn in stored.items {
            queue.pending.push(turn.priority, turn);
        }
        queue.paused = stored.paused || recovered_interrupted;
        queue
    }

    pub(in crate::api::code_web) fn snapshot(&self) -> CodeWebStoredTurnQueue {
        CodeWebStoredTurnQueue {
            items: self
                .pending
                .ordered()
                .into_iter()
                .map(|item| item.value().clone())
                .collect(),
            active: self.active.clone(),
            paused: self.paused,
        }
    }

    pub(in crate::api::code_web) fn enqueue(&mut self, turn: CodeWebQueuedTurn) {
        self.pending.push(turn.priority, turn);
    }

    pub(in crate::api::code_web) fn begin(
        &mut self,
        expected_id: &str,
        started_at: i64,
    ) -> Result<CodeWebQueuedTurn, &'static str> {
        if self.active.is_some() {
            return Err("another queued turn is already active");
        }
        if self.paused {
            return Err("the queued turn is paused");
        }
        let Some(next) = self.pending.pop() else {
            return Err("the queued turn does not exist");
        };
        if next.value().id != expected_id {
            self.pending.restore(next);
            return Err("the queued turn is not next");
        }
        let turn = next.into_value();
        self.active = Some(CodeWebActiveTurn {
            turn: turn.clone(),
            started_at,
        });
        Ok(turn)
    }

    pub(in crate::api::code_web) fn finish_active(&mut self, turn_id: &str, pause: bool) -> bool {
        if self
            .active
            .as_ref()
            .is_none_or(|active| active.turn.id != turn_id)
        {
            return false;
        }
        self.active = None;
        self.paused |= pause;
        true
    }

    pub(in crate::api::code_web) fn restore_active(&mut self, turn_id: &str) -> bool {
        let Some(active) = self.active.take_if(|active| active.turn.id == turn_id) else {
            return false;
        };
        let mut turns = vec![active.turn];
        turns.extend(
            self.pending
                .ordered()
                .into_iter()
                .map(|item| item.value().clone()),
        );
        self.rebuild(turns);
        true
    }

    pub(in crate::api::code_web) fn pause(&mut self) {
        self.paused = true;
    }

    pub(in crate::api::code_web) fn resume(&mut self) {
        self.paused = false;
    }

    pub(in crate::api::code_web) fn update_user_turn(
        &mut self,
        turn_id: &str,
        content: String,
        context_files: Vec<String>,
        skill_names: Vec<String>,
    ) -> bool {
        let mut turns = self.pending_turns();
        let Some(turn) = turns
            .iter_mut()
            .find(|turn| turn.id == turn_id && turn.kind == CodeWebQueuedTurnKind::User)
        else {
            return false;
        };
        turn.content = content;
        turn.context_files = context_files;
        turn.skill_names = skill_names;
        self.rebuild(turns);
        true
    }

    pub(in crate::api::code_web) fn remove(&mut self, turn_id: &str) -> bool {
        let mut turns = self.pending_turns();
        let original_len = turns.len();
        turns.retain(|turn| turn.id != turn_id);
        if turns.len() == original_len {
            return false;
        }
        self.rebuild(turns);
        true
    }

    pub(in crate::api::code_web) fn remove_kind(&mut self, kind: CodeWebQueuedTurnKind) -> usize {
        let mut turns = self.pending_turns();
        let original_len = turns.len();
        turns.retain(|turn| turn.kind != kind);
        let removed = original_len.saturating_sub(turns.len());
        if removed > 0 {
            self.rebuild(turns);
        }
        removed
    }

    pub(in crate::api::code_web) fn reorder(&mut self, ordered_ids: &[String]) -> bool {
        let turns = self.pending_turns();
        if turns.len() != ordered_ids.len() {
            return false;
        }
        let expected = turns
            .iter()
            .map(|turn| turn.id.as_str())
            .collect::<HashSet<_>>();
        let requested = ordered_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        if expected != requested {
            return false;
        }
        let rank = ordered_ids
            .iter()
            .enumerate()
            .map(|(index, id)| (id.as_str(), index))
            .collect::<HashMap<_, _>>();
        let mut turns = turns;
        turns.sort_by_key(|turn| rank[turn.id.as_str()]);
        self.rebuild(turns);
        true
    }

    pub(in crate::api::code_web) fn contains_kind(&self, kind: CodeWebQueuedTurnKind) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.turn.kind == kind)
            || self
                .pending
                .ordered()
                .into_iter()
                .any(|item| item.value().kind == kind)
    }

    fn pending_turns(&self) -> Vec<CodeWebQueuedTurn> {
        self.pending
            .ordered()
            .into_iter()
            .map(|item| item.value().clone())
            .collect()
    }

    fn rebuild(&mut self, turns: Vec<CodeWebQueuedTurn>) {
        self.pending.clear();
        for turn in turns {
            self.pending.push(turn.priority, turn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(id: &str, kind: CodeWebQueuedTurnKind, priority: Priority) -> CodeWebQueuedTurn {
        CodeWebQueuedTurn {
            id: id.to_string(),
            kind,
            content: id.to_string(),
            context_files: Vec::new(),
            skill_names: Vec::new(),
            mode: CodeWebQueuedTurnMode::Standard,
            research_run_id: None,
            priority,
            enqueued_at: 1,
        }
    }

    #[test]
    fn user_turns_preempt_goal_continuations_and_remain_fifo() {
        let mut queue = CodeWebSessionTurnQueue::default();
        queue.enqueue(turn(
            "goal",
            CodeWebQueuedTurnKind::GoalContinuation,
            GOAL_CONTINUATION_PRIORITY,
        ));
        queue.enqueue(turn(
            "user-1",
            CodeWebQueuedTurnKind::User,
            USER_TURN_PRIORITY,
        ));
        queue.enqueue(turn(
            "user-2",
            CodeWebQueuedTurnKind::User,
            USER_TURN_PRIORITY,
        ));

        assert_eq!(
            queue
                .snapshot()
                .items
                .into_iter()
                .map(|turn| turn.id)
                .collect::<Vec<_>>(),
            ["user-1", "user-2", "goal"]
        );
    }

    #[test]
    fn deep_research_mode_survives_queue_snapshot_and_restore() {
        let mut research = turn("research", CodeWebQueuedTurnKind::User, USER_TURN_PRIORITY);
        research.mode = CodeWebQueuedTurnMode::DeepResearch;
        research.research_run_id = Some("web-research".to_string());
        let mut queue = CodeWebSessionTurnQueue::default();
        queue.enqueue(research);

        let restored = CodeWebSessionTurnQueue::restore(queue.snapshot()).snapshot();

        assert_eq!(restored.items[0].mode, CodeWebQueuedTurnMode::DeepResearch);
        assert_eq!(
            restored.items[0].research_run_id.as_deref(),
            Some("web-research")
        );
    }

    #[test]
    fn failed_admission_restores_the_claim_to_the_front() {
        let mut queue = CodeWebSessionTurnQueue::default();
        queue.enqueue(turn(
            "first",
            CodeWebQueuedTurnKind::User,
            USER_TURN_PRIORITY,
        ));
        queue.enqueue(turn(
            "second",
            CodeWebQueuedTurnKind::User,
            USER_TURN_PRIORITY,
        ));

        queue.begin("first", 10).expect("claim first turn");
        assert!(queue.restore_active("first"));

        assert_eq!(
            queue
                .snapshot()
                .items
                .into_iter()
                .map(|turn| turn.id)
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    #[test]
    fn interrupted_active_turn_is_recovered_paused_after_restart() {
        let restored = CodeWebSessionTurnQueue::restore(CodeWebStoredTurnQueue {
            items: vec![turn(
                "next",
                CodeWebQueuedTurnKind::User,
                USER_TURN_PRIORITY,
            )],
            active: Some(CodeWebActiveTurn {
                turn: turn(
                    "interrupted",
                    CodeWebQueuedTurnKind::User,
                    USER_TURN_PRIORITY,
                ),
                started_at: 20,
            }),
            paused: false,
        });
        let snapshot = restored.snapshot();

        assert!(snapshot.paused);
        assert!(snapshot.active.is_none());
        assert_eq!(snapshot.items[0].id, "interrupted");
        assert_eq!(snapshot.items[1].id, "next");
    }

    #[test]
    fn reorder_cannot_move_a_goal_continuation_ahead_of_user_turns() {
        let mut queue = CodeWebSessionTurnQueue::default();
        queue.enqueue(turn(
            "goal",
            CodeWebQueuedTurnKind::GoalContinuation,
            GOAL_CONTINUATION_PRIORITY,
        ));
        queue.enqueue(turn(
            "user",
            CodeWebQueuedTurnKind::User,
            USER_TURN_PRIORITY,
        ));

        assert!(queue.reorder(&["goal".to_string(), "user".to_string()]));
        assert_eq!(queue.snapshot().items[0].id, "user");
    }
}
