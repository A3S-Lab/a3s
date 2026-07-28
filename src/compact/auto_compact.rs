#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AutoCompactState {
    Armed,
    Triggered,
    Compacting,
    WaitingForBelowThreshold,
}

#[derive(Clone, Debug)]
pub(crate) struct AutoCompactController {
    state: AutoCompactState,
    threshold: f64,
    context_limit: u32,
}

impl AutoCompactController {
    pub(crate) fn new(threshold: f64, context_limit: u32) -> Self {
        Self {
            state: AutoCompactState::Armed,
            threshold,
            context_limit,
        }
    }

    #[cfg(test)]
    pub(crate) fn state(&self) -> AutoCompactState {
        self.state
    }

    pub(crate) fn observe_prompt_tokens(&mut self, prompt_tokens: usize) -> bool {
        let above_threshold = self.is_above_threshold(prompt_tokens);
        match self.state {
            AutoCompactState::Armed if above_threshold => {
                self.state = AutoCompactState::Triggered;
                true
            }
            AutoCompactState::WaitingForBelowThreshold if !above_threshold => {
                self.state = AutoCompactState::Armed;
                false
            }
            _ => false,
        }
    }

    pub(crate) fn start(&mut self) -> bool {
        if self.state != AutoCompactState::Triggered {
            return false;
        }
        self.state = AutoCompactState::Compacting;
        true
    }

    pub(crate) fn finish_success(&mut self, prompt_tokens: usize) {
        self.state = if self.is_above_threshold(prompt_tokens) {
            AutoCompactState::WaitingForBelowThreshold
        } else {
            AutoCompactState::Armed
        };
    }

    pub(crate) fn finish_failure(&mut self) {
        self.state = AutoCompactState::WaitingForBelowThreshold;
    }

    pub(crate) fn update_policy(&mut self, threshold: f64, context_limit: u32) {
        self.threshold = threshold;
        self.context_limit = context_limit;
    }

    fn is_above_threshold(&self, prompt_tokens: usize) -> bool {
        self.context_limit > 0 && prompt_tokens as f64 / self.context_limit as f64 >= self.threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triggers_once_during_one_high_water_generation() {
        let mut controller = AutoCompactController::new(0.85, 200_000);

        assert!(!controller.observe_prompt_tokens(160_000));
        assert_eq!(controller.state(), AutoCompactState::Armed);
        assert!(controller.observe_prompt_tokens(170_000));
        assert_eq!(controller.state(), AutoCompactState::Triggered);
        assert!(!controller.observe_prompt_tokens(174_000));
        assert!(controller.start());
        assert_eq!(controller.state(), AutoCompactState::Compacting);
        assert!(!controller.start());

        controller.finish_failure();
        assert_eq!(
            controller.state(),
            AutoCompactState::WaitingForBelowThreshold
        );
        assert!(!controller.observe_prompt_tokens(180_000));
        assert!(!controller.observe_prompt_tokens(140_000));
        assert_eq!(controller.state(), AutoCompactState::Armed);
        assert!(controller.observe_prompt_tokens(170_000));
    }

    #[test]
    fn successful_compact_rearms_only_when_projected_usage_is_below_threshold() {
        let mut controller = AutoCompactController::new(0.85, 200_000);
        assert!(controller.observe_prompt_tokens(170_000));
        assert!(controller.start());

        controller.finish_success(0);
        assert_eq!(controller.state(), AutoCompactState::Armed);
        assert!(controller.observe_prompt_tokens(170_000));

        assert!(controller.start());
        controller.finish_success(180_000);
        assert_eq!(
            controller.state(),
            AutoCompactState::WaitingForBelowThreshold
        );
        assert!(!controller.observe_prompt_tokens(175_000));
    }

    #[test]
    fn zero_context_limit_never_triggers() {
        let mut controller = AutoCompactController::new(0.85, 0);
        assert!(!controller.observe_prompt_tokens(usize::MAX));
        assert_eq!(controller.state(), AutoCompactState::Armed);
    }
}
