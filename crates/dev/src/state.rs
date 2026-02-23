use std::time::Instant;

#[derive(Debug, Clone)]
pub enum ServiceState {
    #[allow(dead_code)]
    Pending,
    #[allow(dead_code)]
    Starting,
    Running {
        pid: u32,
        since: Instant,
    },
    #[allow(dead_code)]
    Unhealthy {
        pid: u32,
        failures: u32,
    },
    Stopped,
    #[allow(dead_code)]
    Failed {
        exit_code: Option<i32>,
    },
}

impl ServiceState {
    pub fn pid(&self) -> Option<u32> {
        match self {
            ServiceState::Running { pid, .. } | ServiceState::Unhealthy { pid, .. } => Some(*pid),
            _ => None,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ServiceState::Pending => "pending",
            ServiceState::Starting => "starting",
            ServiceState::Running { .. } => "running",
            ServiceState::Unhealthy { .. } => "unhealthy",
            ServiceState::Stopped => "stopped",
            ServiceState::Failed { .. } => "failed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_all_variants() {
        assert_eq!(ServiceState::Pending.label(), "pending");
        assert_eq!(ServiceState::Starting.label(), "starting");
        assert_eq!(
            ServiceState::Running {
                pid: 1,
                since: Instant::now()
            }
            .label(),
            "running"
        );
        assert_eq!(
            ServiceState::Unhealthy { pid: 1, failures: 2 }.label(),
            "unhealthy"
        );
        assert_eq!(ServiceState::Stopped.label(), "stopped");
        assert_eq!(ServiceState::Failed { exit_code: None }.label(), "failed");
        assert_eq!(
            ServiceState::Failed { exit_code: Some(1) }.label(),
            "failed"
        );
    }

    #[test]
    fn test_pid_running() {
        let s = ServiceState::Running {
            pid: 42,
            since: Instant::now(),
        };
        assert_eq!(s.pid(), Some(42));
    }

    #[test]
    fn test_pid_unhealthy() {
        let s = ServiceState::Unhealthy { pid: 99, failures: 3 };
        assert_eq!(s.pid(), Some(99));
    }

    #[test]
    fn test_pid_none_for_non_running() {
        assert_eq!(ServiceState::Pending.pid(), None);
        assert_eq!(ServiceState::Starting.pid(), None);
        assert_eq!(ServiceState::Stopped.pid(), None);
        assert_eq!(ServiceState::Failed { exit_code: Some(1) }.pid(), None);
    }
}
