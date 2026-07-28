//! Strict two-phase planning state and its compact review surface.
//!
//! A planning turn is read-only and ends at this host-owned boundary. The
//! implementation turn is created only after an explicit approval.

use a3s_code_core::planning::Task;
use a3s_tui::event::{KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use a3s_tui::style::{fit_visible, wrap_words, Style};
use a3s_tui::KeyCode;

use super::{
    App, Cmd, Mode, Msg, NoticeKind, PendingImage, Queued, State, TextareaMsg, TranscriptEntry,
    COMPOSER_CHROME, PLAN_REVIEW_PRIORITY, SURFACE_SELECTED, TN_GREEN, TN_RED, TN_SUBTLE,
};

const OPTION_COUNT: usize = 3;
const MAX_PLAN_TEXT_CHARS: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PlanDraftRequest {
    pub(super) original_prompt: String,
    pub(super) display: String,
    pub(super) revision: u32,
    previous_plan: Option<String>,
    feedback: Option<String>,
}

impl PlanDraftRequest {
    pub(super) fn initial(original_prompt: String, display: String) -> Self {
        Self {
            original_prompt,
            display,
            revision: 0,
            previous_plan: None,
            feedback: None,
        }
    }

    pub(super) fn revised(&self, previous_plan: String, feedback: String) -> Self {
        Self {
            original_prompt: self.original_prompt.clone(),
            display: self.display.clone(),
            revision: self.revision.saturating_add(1),
            previous_plan: Some(bounded_text(&previous_plan)),
            feedback: Some(feedback),
        }
    }

    pub(super) fn planning_prompt(&self) -> String {
        let mut prompt = format!(
            "[strict-plan]\n\
             This is a read-only planning turn. Inspect the workspace as needed, \
             but do not modify files, run mutating commands, create external \
             resources, or begin implementation. Produce a concrete, ordered, \
             verifiable implementation plan and then stop for user review.\n\n\
             Original request:\n{}",
            self.original_prompt
        );
        if let Some(previous_plan) = self.previous_plan.as_deref() {
            prompt.push_str("\n\nPrevious plan:\n");
            prompt.push_str(previous_plan);
        }
        if let Some(feedback) = self.feedback.as_deref() {
            prompt.push_str("\n\nRequired revision feedback:\n");
            prompt.push_str(feedback);
        }
        prompt
    }
}

#[derive(Clone, Debug)]
pub(super) struct PlanReviewState {
    pub(super) request: PlanDraftRequest,
    pub(super) tasks: Vec<Task>,
    narrative: String,
    pub(super) selected: usize,
    stashed_composer: Option<String>,
}

impl PlanReviewState {
    pub(super) fn new(request: PlanDraftRequest, tasks: Vec<Task>, narrative: String) -> Self {
        Self {
            request,
            tasks,
            narrative: bounded_text(&narrative),
            selected: 0,
            stashed_composer: None,
        }
    }

    pub(super) fn is_revising(&self) -> bool {
        self.stashed_composer.is_some()
    }

    pub(super) fn begin_revision(&mut self, composer: String) {
        if self.stashed_composer.is_none() {
            self.stashed_composer = Some(composer);
        }
        self.selected = 1;
    }

    pub(super) fn cancel_revision(&mut self) -> Option<String> {
        self.stashed_composer.take()
    }

    pub(super) fn take_stashed_composer(&mut self) -> Option<String> {
        self.stashed_composer.take()
    }

    pub(super) fn plan_text(&self) -> String {
        if !self.tasks.is_empty() {
            return self
                .tasks
                .iter()
                .enumerate()
                .map(|(index, task)| format!("{}. {}", index + 1, task.content.trim()))
                .collect::<Vec<_>>()
                .join("\n");
        }
        if self.narrative.trim().is_empty() {
            "No structured plan was returned. Revisions should request concrete steps.".to_string()
        } else {
            self.narrative.trim().to_string()
        }
    }

    pub(super) fn implementation_prompt(&self) -> String {
        format!(
            "[approved-plan]\n\
             The user explicitly approved the plan below. Implement it now, \
             verify the result, and report the outcome. Treat the original \
             request as authoritative if a plan step is ambiguous.\n\n\
             Original request:\n{}\n\n\
             Approved plan:\n{}",
            self.request.original_prompt,
            self.plan_text()
        )
    }
}

fn bounded_text(text: &str) -> String {
    let mut chars = text.chars();
    let bounded = chars.by_ref().take(MAX_PLAN_TEXT_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{bounded}\n…")
    } else {
        bounded
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PlanReviewPromptMsg {
    Selected(usize),
}

#[derive(Clone, Debug)]
pub(super) struct PlanReviewPrompt {
    selected: usize,
    steps: usize,
    revision: u32,
    revising: bool,
    y_offset: u16,
}

impl PlanReviewPrompt {
    pub(super) fn new(state: &PlanReviewState) -> Self {
        Self {
            selected: state.selected.min(OPTION_COUNT - 1),
            steps: state.tasks.len(),
            revision: state.request.revision,
            revising: state.is_revising(),
            y_offset: 0,
        }
    }

    pub(super) fn lines(&self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }
        if self.revising {
            return self.revision_lines(width);
        }

        let step_label = if self.steps == 1 {
            "1 step".to_string()
        } else {
            format!("{} steps", self.steps)
        };
        let mut lines = vec![fit_visible(
            &Style::new()
                .fg(COMPOSER_CHROME.active)
                .bold()
                .render(&format!(
                    "◆ Plan ready · {step_label} · revision {}",
                    self.revision + 1
                )),
            width,
        )];
        lines.extend(
            wrap_words(
                "Implementation is paused until you choose an action.",
                width.saturating_sub(2).max(1),
            )
            .into_iter()
            .map(|line| {
                fit_visible(
                    &Style::new()
                        .fg(COMPOSER_CHROME.secondary)
                        .render(&format!("  {line}")),
                    width,
                )
            }),
        );
        lines.extend((0..OPTION_COUNT).map(|index| self.option_line(index, width)));
        lines.push(fit_visible(
            &Style::new()
                .fg(TN_SUBTLE)
                .render("  Enter select · ↑↓ move · a approve · r revise · x abandon"),
            width,
        ));
        lines
    }

    pub(super) fn set_y_offset(&mut self, y_offset: u16) {
        self.y_offset = y_offset;
    }

    pub(super) fn selected_index(&self) -> usize {
        self.selected.min(OPTION_COUNT - 1)
    }

    pub(super) fn handle_mouse(
        &mut self,
        mouse: &MouseEvent,
        width: usize,
    ) -> Option<PlanReviewPromptMsg> {
        if width == 0 || self.revising {
            return None;
        }
        let local_row = mouse.row.checked_sub(self.y_offset)? as usize;
        let choice_start = self.choice_start_row(width);
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.selected = self.selected_index().saturating_sub(1);
                None
            }
            MouseEventKind::ScrollDown => {
                self.selected = (self.selected_index() + 1).min(OPTION_COUNT - 1);
                None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let index = local_row.checked_sub(choice_start)?;
                if index < OPTION_COUNT {
                    self.selected = index;
                    Some(PlanReviewPromptMsg::Selected(index))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn choice_start_row(&self, width: usize) -> usize {
        1 + wrap_words(
            "Implementation is paused until you choose an action.",
            width.saturating_sub(2).max(1),
        )
        .len()
    }

    fn option_line(&self, index: usize, width: usize) -> String {
        let (label, color) = match index {
            0 => ("Approve plan and implement", TN_GREEN),
            1 => ("Revise plan with feedback", COMPOSER_CHROME.primary),
            _ => ("Abandon plan", TN_RED),
        };
        let marker = if index == self.selected_index() {
            "›"
        } else {
            " "
        };
        let line = format!("  {marker} {}. {label}", index + 1);
        let styled = if index == self.selected_index() {
            Style::new().fg(color).bg(SURFACE_SELECTED).bold()
        } else {
            Style::new().fg(color)
        };
        fit_visible(&styled.render(&line), width)
    }

    fn revision_lines(&self, width: usize) -> Vec<String> {
        vec![
            fit_visible(
                &Style::new()
                    .fg(COMPOSER_CHROME.active)
                    .bold()
                    .render("↺ Revise plan"),
                width,
            ),
            fit_visible(
                &Style::new()
                    .fg(COMPOSER_CHROME.secondary)
                    .render("  Type concrete feedback below, then press Enter. Esc cancels."),
                width,
            ),
        ]
    }
}

impl App {
    /// Capture a completed read-only planning turn before normal turn cleanup
    /// clears its typed plan projection.
    pub(super) fn take_plan_review_candidate(&mut self) -> Option<PlanReviewState> {
        let request = self.active_plan_draft.take()?;
        Some(PlanReviewState::new(
            request,
            self.plan.tasks().to_vec(),
            self.turn_text.clone(),
        ))
    }

    /// Promote the staged review only after the stream worker has released the
    /// session lease. Queue draining remains blocked until a decision is made.
    pub(super) fn activate_pending_plan_review(&mut self) -> bool {
        let Some(review) = self.pending_plan_review.take() else {
            return false;
        };
        self.set_plan(&review.tasks);
        self.plan_review = Some(review);
        self.state = State::Idle;
        self.relayout();
        true
    }

    pub(super) fn plan_review_input_active(&self) -> bool {
        self.plan_review
            .as_ref()
            .is_some_and(PlanReviewState::is_revising)
    }

    pub(super) fn handle_plan_review_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if self.plan_review_input_active() {
            if key.code == KeyCode::Esc {
                let restored = self
                    .plan_review
                    .as_mut()
                    .and_then(PlanReviewState::cancel_revision);
                if let Some(restored) = restored {
                    self.textarea.set_value(&restored);
                }
                self.relayout();
                return None;
            }
            match self.textarea.handle_key(key) {
                Some(TextareaMsg::Submit(feedback)) if !feedback.trim().is_empty() => {
                    return self.submit_plan_revision(feedback);
                }
                Some(TextareaMsg::Submit(_)) => return None,
                Some(TextareaMsg::Changed(_)) => {
                    self.relayout();
                    return None;
                }
                None => return None,
            }
        }

        let selected = self.plan_review.as_ref()?.selected;
        match key.code {
            KeyCode::Up => {
                if let Some(review) = self.plan_review.as_mut() {
                    review.selected = review.selected.saturating_sub(1);
                }
                None
            }
            KeyCode::Down => {
                if let Some(review) = self.plan_review.as_mut() {
                    review.selected = (review.selected + 1).min(OPTION_COUNT - 1);
                }
                None
            }
            KeyCode::Enter => self.apply_plan_review_choice(selected),
            KeyCode::Char('a' | 'A') | KeyCode::Char('1') => self.approve_plan_review(),
            KeyCode::Char('r' | 'R') | KeyCode::Char('2') => {
                self.begin_plan_revision();
                None
            }
            KeyCode::Char('x' | 'X') | KeyCode::Char('3') | KeyCode::Esc => {
                self.abandon_plan_review()
            }
            _ => None,
        }
    }

    pub(super) fn handle_plan_review_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let review = self.plan_review.as_ref()?;
        if review.is_revising() {
            return None;
        }
        let width = self.width as usize;
        if width == 0 {
            return None;
        }
        let mut prompt = PlanReviewPrompt::new(review);
        let rows = prompt.lines(width).len();
        let y_offset = self
            .height
            .saturating_sub(self.approval_rows_below().min(u16::MAX as usize) as u16)
            .saturating_sub(rows.min(u16::MAX as usize) as u16);
        prompt.set_y_offset(y_offset);
        let before = prompt.selected_index();
        match prompt.handle_mouse(mouse, width) {
            Some(PlanReviewPromptMsg::Selected(index)) => self.apply_plan_review_choice(index),
            None => {
                let after = prompt.selected_index();
                if after != before {
                    if let Some(review) = self.plan_review.as_mut() {
                        review.selected = after;
                    }
                }
                None
            }
        }
    }

    pub(super) fn overlay_plan_review(&self, composed: String) -> String {
        let Some(review) = self.plan_review.as_ref() else {
            return composed;
        };
        let lines = PlanReviewPrompt::new(review).lines(self.width as usize);
        self.overlay_list_with_rows_below(composed, &lines, self.approval_rows_below())
    }

    pub(super) fn overlay_decision_modals(&self, composed: String) -> String {
        self.overlay_approval(self.overlay_plan_review(self.overlay_queue_menu(composed)))
    }

    fn apply_plan_review_choice(&mut self, choice: usize) -> Option<Cmd<Msg>> {
        match choice {
            0 => self.approve_plan_review(),
            1 => {
                self.begin_plan_revision();
                None
            }
            _ => self.abandon_plan_review(),
        }
    }

    fn begin_plan_revision(&mut self) {
        let draft = self.textarea.value();
        if let Some(review) = self.plan_review.as_mut() {
            review.begin_revision(draft);
            self.textarea.clear();
            self.relayout();
        }
    }

    fn approve_plan_review(&mut self) -> Option<Cmd<Msg>> {
        let mut review = self.plan_review.take()?;
        if let Some(stashed) = review.take_stashed_composer() {
            self.textarea.set_value(&stashed);
        }
        let prompt = review.implementation_prompt();
        let display = format!("Implement approved plan: {}", review.request.display);
        self.set_composer_mode(Mode::Default);
        self.plan.clear();
        self.push_notice(NoticeKind::Info, "Plan approved · implementation started");
        self.enqueue_turn(
            PLAN_REVIEW_PRIORITY,
            Queued {
                text: prompt,
                display,
                images: Vec::<PendingImage>::new(),
                runtime_expectation: None,
                deep_research: None,
            },
            Mode::Default,
        );
        self.drain_queue()
    }

    fn submit_plan_revision(&mut self, feedback: String) -> Option<Cmd<Msg>> {
        let feedback = feedback.trim().to_string();
        let mut review = self.plan_review.take()?;
        let stashed = review.take_stashed_composer().unwrap_or_default();
        let request = review.request.revised(review.plan_text(), feedback.clone());
        self.textarea.set_value(&stashed);
        self.messages
            .push(TranscriptEntry::user(format!("Plan feedback: {feedback}")));
        self.history.push(feedback);
        self.history_pos = None;
        self.history_draft = None;
        self.set_composer_mode(Mode::Plan);
        self.plan.clear();
        let display = format!("Revise plan: {}", request.display);
        self.enqueue_plan_turn(
            PLAN_REVIEW_PRIORITY,
            Queued {
                text: request.planning_prompt(),
                display,
                images: Vec::<PendingImage>::new(),
                runtime_expectation: None,
                deep_research: None,
            },
            request,
        );
        self.rebuild_viewport();
        self.drain_queue()
    }

    fn abandon_plan_review(&mut self) -> Option<Cmd<Msg>> {
        let mut review = self.plan_review.take()?;
        if let Some(stashed) = review.take_stashed_composer() {
            self.textarea.set_value(&stashed);
        }
        self.set_composer_mode(Mode::Default);
        self.plan.clear();
        self.push_notice(
            NoticeKind::Info,
            "Plan abandoned · no implementation was started",
        );
        self.relayout();
        self.drain_queue()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::planning::TaskStatus;
    use a3s_tui::style::strip_ansi;

    fn task(id: &str, content: &str) -> Task {
        let mut task = Task::new(id, content);
        task.status = TaskStatus::Pending;
        task
    }

    #[test]
    fn revision_prompt_keeps_original_request_and_feedback() {
        let original = PlanDraftRequest::initial("Implement the feature".into(), "feature".into());
        let revised = original.revised("1. Inspect\n2. Implement".into(), "Add tests".into());
        let prompt = revised.planning_prompt();

        assert!(prompt.contains("[strict-plan]"));
        assert!(prompt.contains("Implement the feature"));
        assert!(prompt.contains("1. Inspect"));
        assert!(prompt.contains("Add tests"));
        assert_eq!(revised.revision, 1);
    }

    #[test]
    fn approved_implementation_uses_typed_plan() {
        let review = PlanReviewState::new(
            PlanDraftRequest::initial("Fix Auto mode".into(), "Auto mode".into()),
            vec![
                task("one", "Bind mode to the queued turn"),
                task("two", "Verify permissions"),
            ],
            "ignored narrative".into(),
        );
        let prompt = review.implementation_prompt();

        assert!(prompt.contains("[approved-plan]"));
        assert!(prompt.contains("1. Bind mode to the queued turn"));
        assert!(prompt.contains("2. Verify permissions"));
    }

    #[test]
    fn review_surface_has_three_explicit_actions() {
        let state = PlanReviewState::new(
            PlanDraftRequest::initial("Plan it".into(), "Plan it".into()),
            vec![task("one", "Inspect")],
            String::new(),
        );
        let plain = PlanReviewPrompt::new(&state)
            .lines(80)
            .into_iter()
            .map(|line| strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("Approve plan and implement"));
        assert!(plain.contains("Revise plan with feedback"));
        assert!(plain.contains("Abandon plan"));
    }
}
