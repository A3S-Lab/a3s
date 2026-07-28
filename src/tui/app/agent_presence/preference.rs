use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AgentIslandPreferenceCommand {
    Status,
    Enable,
    Disable,
}

pub(super) fn parse_agent_island_preference_command(
    value: &str,
) -> Result<AgentIslandPreferenceCommand, &'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "" | "status" => Ok(AgentIslandPreferenceCommand::Status),
        "on" => Ok(AgentIslandPreferenceCommand::Enable),
        "off" => Ok(AgentIslandPreferenceCommand::Disable),
        _ => Err("usage: /island [on|off|status]"),
    }
}

impl AgentIslandSupervisor {
    pub(super) fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.consecutive_failures = 0;
        self.request = None;
        if enabled {
            self.lifecycle = AgentIslandLifecycle::AwaitingSnapshot;
            return;
        }

        let lifecycle = std::mem::replace(&mut self.lifecycle, AgentIslandLifecycle::Stopped);
        if let AgentIslandLifecycle::Running(mut monitor) = lifecycle {
            monitor.stop();
        }
    }
}

impl AgentPresenceRuntime {
    fn apply_island_preference(&mut self, enabled: bool) {
        if self.island.enabled == enabled {
            return;
        }
        self.island.set_enabled(enabled);
    }
}

impl App {
    pub(in crate::tui) fn sync_agent_island_preference(&mut self) {
        let enabled = self.agent_presence.publisher.island_preference_enabled();
        self.agent_presence.apply_island_preference(enabled);
    }

    pub(in crate::tui) fn submit_agent_island_command(&mut self, value: &str) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let command = match parse_agent_island_preference_command(value) {
            Ok(command) => command,
            Err(usage) => {
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {usage}")));
                return None;
            }
        };

        if command == AgentIslandPreferenceCommand::Status {
            let enabled = self.agent_presence.publisher.island_preference_enabled();
            let message = if !enabled {
                "  Agent Island is off · /island on enables it".to_string()
            } else if let Some(reason) = AgentIslandEnvironment::current().skip_reason() {
                format!("  Agent Island preference is on · launch unavailable: {reason}")
            } else {
                "  Agent Island is on".to_string()
            };
            self.push_line(&Style::new().fg(TN_GRAY).render(&message));
            return None;
        }

        let enabled = command == AgentIslandPreferenceCommand::Enable;
        if let Err(error) = self
            .agent_presence
            .publisher
            .persist_island_enabled(enabled)
        {
            self.push_line(&Style::new().fg(TN_RED).render(&format!(
                "  could not save Agent Island preference: {error}"
            )));
            return None;
        }

        self.agent_presence.apply_island_preference(enabled);
        let message = if enabled {
            if let Some(reason) = AgentIslandEnvironment::current().skip_reason() {
                format!("  Agent Island preference is on · launch unavailable: {reason}")
            } else {
                "  Agent Island is on".to_string()
            }
        } else {
            "  Agent Island is off · /island on enables it".to_string()
        };
        self.push_line(&Style::new().fg(TN_GRAY).render(&message));
        None
    }
}
