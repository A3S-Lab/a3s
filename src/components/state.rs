use std::path::PathBuf;

use a3s_updater::InstallProvenance;
use serde::{Deserialize, Serialize};

use super::catalog::ComponentKind;
use super::id::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Presence {
    Bundled,
    Managed,
    External,
    System,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Health {
    Ready,
    Broken,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateState {
    Current,
    Available,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Trust {
    FirstParty,
    LocalExplicit,
    ReleaseBundle,
    RegistryTuf,
    Untrusted,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentState {
    pub id: ComponentId,
    pub kind: ComponentKind,
    pub description: String,
    pub presence: Presence,
    pub health: Health,
    pub update: UpdateState,
    pub trust: Trust,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<InstallProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ComponentState {
    pub fn is_ready(&self) -> bool {
        self.health == Health::Ready && self.presence != Presence::Missing
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalTool {
    pub command: String,
    pub binary: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentReport {
    pub schema_version: u32,
    pub components: Vec<ComponentState>,
    pub external_tools: Vec<ExternalTool>,
}
