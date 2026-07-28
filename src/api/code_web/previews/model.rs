use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CreatePreviewRequest {
    pub(in crate::api::code_web) target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api) enum PreviewKind {
    StaticSite,
    LocalUrl,
    Pdf,
    Image,
    Office,
    Text,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub(in crate::api) enum PreviewSource {
    Path {
        path: String,
        root_path: String,
        name: String,
        size: u64,
        mtime_ms: Option<u64>,
        is_directory: bool,
        is_binary: bool,
    },
    Url {
        url: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api) struct PreviewCapabilities {
    pub(in crate::api) live_reload: bool,
    pub(in crate::api) responsive: bool,
    pub(in crate::api) navigation: bool,
    pub(in crate::api) open_external: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api) struct PreviewDescriptor {
    pub(in crate::api) id: String,
    pub(in crate::api) kind: PreviewKind,
    pub(in crate::api) title: String,
    pub(in crate::api) source: PreviewSource,
    pub(in crate::api) content_url: String,
    pub(in crate::api) watch_root: Option<String>,
    pub(in crate::api) created_at: i64,
    pub(in crate::api) expires_at: i64,
    pub(in crate::api) capabilities: PreviewCapabilities,
}

#[derive(Debug, Clone)]
pub(in crate::api) enum PreviewContentSource {
    Directory { root: PathBuf, entry: PathBuf },
    File { path: PathBuf },
}

#[derive(Debug, Clone)]
pub(in crate::api) struct PreviewSession {
    pub(in crate::api) descriptor: PreviewDescriptor,
    pub(in crate::api) content: Option<PreviewContentSource>,
}
