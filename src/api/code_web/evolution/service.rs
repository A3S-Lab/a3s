use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};

use crate::api::code_web::session_runtime::rebuild_code_web_sessions_for_workspace;
use crate::api::code_web::state::CodeWebState;
use crate::config;
use crate::evolution::WorkspaceEvolution;

pub(in crate::api::code_web) struct EvolutionService {
    state: Arc<CodeWebState>,
}

impl EvolutionService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) async fn overview(&self) -> BootResult<Value> {
        serialize(self.evolution().overview().await.map_err(internal_error)?)
    }

    pub(in crate::api::code_web) async fn scan(&self) -> BootResult<Value> {
        let evolution = self.evolution();
        let observed = evolution
            .synchronize_memory_store(config::memory_dir())
            .await
            .map_err(internal_error)?;
        let overview = evolution.overview().await.map_err(internal_error)?;
        Ok(json!({
            "observed": observed,
            "overview": overview,
        }))
    }

    pub(in crate::api::code_web) async fn materialize(
        &self,
        id: String,
        request: Value,
    ) -> BootResult<Value> {
        let _refresh = self.state.evolution_refresh_lock.lock().await;
        let force = request
            .get("force")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let evolution = self.evolution();
        let result = evolution
            .materialize(id, force)
            .await
            .map_err(action_error)?;
        let rebuilt_sessions = if result.requires_session_reload {
            let rebuilt = rebuild_code_web_sessions_for_workspace(
                &self.state,
                Some(&self.state.default_workspace),
            )
            .await?;
            if !rebuilt.is_empty() {
                evolution
                    .mark_session_assets_activated()
                    .await
                    .map_err(internal_error)?;
            }
            rebuilt
        } else {
            Vec::new()
        };
        Ok(json!({
            "result": result,
            "rebuiltSessions": rebuilt_sessions,
        }))
    }

    pub(in crate::api::code_web) async fn reject(
        &self,
        id: String,
        request: Value,
    ) -> BootResult<Value> {
        let reason = request
            .get("reason")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        serialize(
            self.evolution()
                .reject(id, reason)
                .await
                .map_err(action_error)?,
        )
    }

    pub(in crate::api::code_web) async fn reopen(&self, id: String) -> BootResult<Value> {
        serialize(self.evolution().reopen(id).await.map_err(action_error)?)
    }

    pub(in crate::api::code_web) async fn rollback(
        &self,
        id: String,
        request: Value,
    ) -> BootResult<Value> {
        let _refresh = self.state.evolution_refresh_lock.lock().await;
        let target_version = request
            .get("targetVersion")
            .and_then(Value::as_u64)
            .map(u32::try_from)
            .transpose()
            .map_err(|_| BootError::BadRequest("targetVersion is too large".to_string()))?;
        let evolution = self.evolution();
        let result = evolution
            .rollback(id, target_version)
            .await
            .map_err(action_error)?;
        let rebuilt_sessions = if result.requires_session_reload {
            let rebuilt = rebuild_code_web_sessions_for_workspace(
                &self.state,
                Some(&self.state.default_workspace),
            )
            .await?;
            if !rebuilt.is_empty() {
                evolution
                    .mark_session_assets_activated()
                    .await
                    .map_err(internal_error)?;
            }
            rebuilt
        } else {
            Vec::new()
        };
        Ok(json!({
            "result": result,
            "rebuiltSessions": rebuilt_sessions,
        }))
    }

    fn evolution(&self) -> WorkspaceEvolution {
        WorkspaceEvolution::new(&self.state.default_workspace)
    }
}

fn serialize(value: impl serde::Serialize) -> BootResult<Value> {
    serde_json::to_value(value).map_err(|error| BootError::Internal(error.to_string()))
}

fn internal_error(error: anyhow::Error) -> BootError {
    BootError::Internal(error.to_string())
}

fn action_error(error: anyhow::Error) -> BootError {
    BootError::BadRequest(error.to_string())
}
