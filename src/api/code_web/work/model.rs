use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum WorkArtifactKind {
    Document,
    Spreadsheet,
    Presentation,
    Pdf,
}

impl WorkArtifactKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Spreadsheet => "spreadsheet",
            Self::Presentation => "presentation",
            Self::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkSourceFile {
    pub(super) name: String,
    pub(super) content_type: String,
    pub(super) size: u64,
    pub(super) updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkArtifact {
    pub(super) id: String,
    pub(super) kind: WorkArtifactKind,
    pub(super) title: String,
    pub(super) favorite: bool,
    pub(super) created_at: u64,
    pub(super) updated_at: u64,
    pub(super) last_opened_at: u64,
    pub(super) revision: u64,
    pub(super) content: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) folder_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) trashed_at: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source: Option<WorkSourceFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkFolder {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) parent_id: Option<String>,
    pub(super) created_at: u64,
    pub(super) updated_at: u64,
    pub(super) revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) trashed_at: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveArtifactRequest {
    pub(super) artifact: WorkArtifact,
    pub(super) expected_revision: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CopyArtifactRequest {
    pub(super) id: String,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) folder_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SaveFolderRequest {
    pub(super) folder: WorkFolder,
    pub(super) expected_revision: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RevisionRequest {
    pub(super) expected_revision: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RestoreVersionRequest {
    pub(super) version: u64,
    pub(super) expected_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct WorkArtifactVersion {
    pub(super) revision: u64,
    pub(super) updated_at: u64,
    pub(super) current: bool,
    pub(super) artifact: WorkArtifact,
}
