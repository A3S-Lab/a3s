use serde::{Deserialize, Serialize};

use a3s_boot::ilink::SecretValue;

const WEIXIN_CAPABILITY_SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum WeixinCapabilityState {
    Unavailable,
    Unbound,
    Binding,
    Active,
    Paused,
    Degraded,
    StaleCredential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum WeixinProtocolMode {
    Disabled,
    Mock,
    Tencent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum RemoteScope {
    #[serde(rename = "agents.read")]
    AgentsRead,
    #[serde(rename = "sessions.read")]
    SessionsRead,
    #[serde(rename = "sessions.content.read")]
    SessionsContentRead,
    #[serde(rename = "sessions.message.write")]
    SessionsMessageWrite,
    #[serde(rename = "sessions.create")]
    SessionsCreate,
    #[serde(rename = "sessions.archive")]
    SessionsArchive,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SafeBlocker {
    pub(super) code: String,
    pub(super) message: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WeixinCapabilityResponse {
    pub(super) schema_version: u32,
    pub(super) state: WeixinCapabilityState,
    pub(super) protocol_mode: WeixinProtocolMode,
    pub(super) supported_scopes: Vec<RemoteScope>,
    pub(super) release_blockers: Vec<SafeBlocker>,
}

impl WeixinCapabilityResponse {
    pub(super) fn unavailable() -> Self {
        Self {
            schema_version: WEIXIN_CAPABILITY_SCHEMA_VERSION,
            state: WeixinCapabilityState::Unavailable,
            protocol_mode: WeixinProtocolMode::Disabled,
            supported_scopes: Vec::new(),
            release_blockers: vec![SafeBlocker {
                code: "ilink_channel_unavailable".to_string(),
                message: "The Weixin iLink channel is not enabled in this runtime.".to_string(),
            }],
        }
    }

    pub(super) fn mock(state: WeixinCapabilityState) -> Self {
        Self {
            schema_version: WEIXIN_CAPABILITY_SCHEMA_VERSION,
            state,
            protocol_mode: WeixinProtocolMode::Mock,
            supported_scopes: vec![RemoteScope::AgentsRead, RemoteScope::SessionsRead],
            release_blockers: vec![SafeBlocker {
                code: "mock_runtime_only".to_string(),
                message: "The WeChat integration is using a local mock runtime.".to_string(),
            }],
        }
    }

    pub(super) fn unavailable_with(blocker: SafeBlocker) -> Self {
        Self {
            schema_version: WEIXIN_CAPABILITY_SCHEMA_VERSION,
            state: WeixinCapabilityState::Unavailable,
            protocol_mode: WeixinProtocolMode::Disabled,
            supported_scopes: Vec::new(),
            release_blockers: vec![blocker],
        }
    }

    pub(super) fn production(state: WeixinCapabilityState) -> Self {
        Self {
            schema_version: WEIXIN_CAPABILITY_SCHEMA_VERSION,
            state,
            protocol_mode: WeixinProtocolMode::Tencent,
            supported_scopes: vec![RemoteScope::AgentsRead, RemoteScope::SessionsRead],
            release_blockers: Vec::new(),
        }
    }

    pub(super) fn production_unavailable(blocker: SafeBlocker) -> Self {
        Self {
            schema_version: WEIXIN_CAPABILITY_SCHEMA_VERSION,
            state: WeixinCapabilityState::Unavailable,
            protocol_mode: WeixinProtocolMode::Tencent,
            supported_scopes: vec![RemoteScope::AgentsRead, RemoteScope::SessionsRead],
            release_blockers: vec![blocker],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum WeixinMonitorState {
    Disabled,
    Stopped,
    Starting,
    Paused,
    Running,
    Degraded,
    StaleCredential,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WeixinAccountResponse {
    pub(super) schema_version: u32,
    pub(super) state: WeixinCapabilityState,
    pub(super) protocol_mode: WeixinProtocolMode,
    pub(super) bound: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) owner_label: Option<String>,
    pub(super) monitor_state: WeixinMonitorState,
    pub(super) mutations_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_update_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_error: Option<SafeBlocker>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StartWeixinLoginRequest {
    pub(super) force: bool,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct SubmitWeixinVerificationRequest {
    pub(super) code: SecretValue,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct WeixinAccountActionRequest {}
