use std::time::Instant;

#[derive(Debug, Clone)]
pub enum ServiceState {
    #[allow(dead_code)]
    Pending,
    #[allow(dead_code)]
    Starting,
    Running { pid: u32, since: Instant },
    #[allow(dead_code)]
    Unhealthy { pid: u32, failures: u32 },
    Stopped,
    #[allow(dead_code)]
    Failed { exit_code: Option<i32> },
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
