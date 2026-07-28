use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::config::OsConfig;
use serde_json::{json, Value};

use super::controller::OsTokenLoginRequest;
use crate::api::code_web::session_runtime::{code_web_os_status, rebuild_code_web_sessions};
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct OsService {
    state: Arc<CodeWebState>,
}

impl OsService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) async fn account(&self) -> BootResult<Value> {
        code_web_os_status(self.state.as_ref()).await
    }

    pub(in crate::api::code_web) async fn login_with_token(
        &self,
        request: OsTokenLoginRequest,
    ) -> BootResult<Value> {
        let config = self.os_config()?;
        let token = request.token.trim();
        if token.is_empty() {
            return Err(BootError::BadRequest("token is required".to_string()));
        }
        let session = crate::a3s_os::login_with_token(&config, token)
            .await
            .map_err(|error| BootError::BadRequest(error.to_string()))?;
        self.finish_login(&config, session, "token").await
    }

    pub(in crate::api::code_web) async fn login_with_browser(&self) -> BootResult<Value> {
        let config = self.os_config()?;
        let session = crate::a3s_os::login_via_browser(config.clone())
            .await
            .map_err(|error| BootError::BadRequest(error.to_string()))?;
        self.finish_login(&config, session, "browser").await
    }

    pub(in crate::api::code_web) async fn logout(&self) -> BootResult<Value> {
        let config = self.os_config()?;
        let removed = crate::a3s_os::logout(&config)
            .map_err(|error| BootError::Internal(error.to_string()))?;
        crate::a3s_os::clear_os_env();
        crate::a3s_os::remove_capability_skill_dir();
        let rebuilt_sessions = rebuild_code_web_sessions(self.state.as_ref()).await?;
        let mut response = code_web_os_status(self.state.as_ref()).await?;
        insert_extra(
            &mut response,
            json!({
                "operation": "logout",
                "removed": removed,
                "rebuiltSessions": rebuilt_sessions,
            }),
        );
        Ok(response)
    }

    async fn finish_login(
        &self,
        config: &OsConfig,
        session: crate::a3s_os::StoredOsSession,
        method: &str,
    ) -> BootResult<Value> {
        crate::a3s_os::export_os_env(&session);
        let capability_skill_active = crate::a3s_os::ensure_capability_skill_dir(config).is_some();
        let ssh_key = ssh_key_outcome_json(crate::a3s_os::sync_ssh_key(session.clone()).await);
        let rebuilt_sessions = rebuild_code_web_sessions(self.state.as_ref()).await?;
        let mut response = code_web_os_status(self.state.as_ref()).await?;
        insert_extra(
            &mut response,
            json!({
                "operation": "login",
                "method": method,
                "capabilitySkillPrepared": capability_skill_active,
                "sshKey": ssh_key,
                "rebuiltSessions": rebuilt_sessions,
            }),
        );
        Ok(response)
    }

    fn os_config(&self) -> BootResult<OsConfig> {
        self.state
            .code_config_snapshot()
            .os
            .ok_or_else(|| BootError::BadRequest("OS endpoint is not configured".to_string()))
    }
}

fn insert_extra(target: &mut Value, extra: Value) {
    let Some(target) = target.as_object_mut() else {
        return;
    };
    let Some(extra) = extra.as_object() else {
        return;
    };
    for (key, value) in extra {
        target.insert(key.clone(), value.clone());
    }
}

fn ssh_key_outcome_json(outcome: crate::a3s_os::SshKeyOutcome) -> Value {
    match outcome {
        crate::a3s_os::SshKeyOutcome::Registered(fingerprint) => json!({
            "status": "registered",
            "fingerprint": fingerprint,
        }),
        crate::a3s_os::SshKeyOutcome::AlreadyRegistered => json!({
            "status": "alreadyRegistered",
        }),
        crate::a3s_os::SshKeyOutcome::NoLocalKey => json!({
            "status": "noLocalKey",
        }),
        crate::a3s_os::SshKeyOutcome::Failed(error) => json!({
            "status": "failed",
            "error": error,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_key_outcome_status_is_stable() {
        assert_eq!(
            ssh_key_outcome_json(crate::a3s_os::SshKeyOutcome::AlreadyRegistered)["status"],
            "alreadyRegistered"
        );
        assert_eq!(
            ssh_key_outcome_json(crate::a3s_os::SshKeyOutcome::NoLocalKey)["status"],
            "noLocalKey"
        );
    }
}
