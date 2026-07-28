use std::sync::Arc;

use a3s_boot::{controller, BootResponse, Result as BootResult};

use super::service::WorkspaceService;

pub(super) struct WorkspaceController {
    service: Arc<WorkspaceService>,
}

impl WorkspaceController {
    pub(super) fn new(service: Arc<WorkspaceService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/workspace")]
impl WorkspaceController {
    #[get("/default-root")]
    async fn workspace_default_root(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.default_root())
    }

    #[get("/readiness")]
    async fn inspect_workspace_readiness(
        &self,
        #[query("workspaceRoot")] workspace_root: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.inspect_readiness(workspace_root, false).await
    }

    #[post("/readiness")]
    async fn ensure_workspace_readiness(
        &self,
        #[query("workspaceRoot")] workspace_root: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.inspect_readiness(workspace_root, true).await
    }

    #[post("/init-agent")]
    async fn init_workspace_agent(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.init_agent(request).await
    }

    #[post("/actions/init-prompt")]
    async fn workspace_init_prompt(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.init_prompt(request).await
    }

    #[post("/actions/pick-directory")]
    async fn pick_workspace_directory(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.pick_directory(request).await
    }

    #[post("/actions/reveal")]
    async fn reveal_workspace_path(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.reveal_path(request).await
    }

    #[post("/mkdir")]
    async fn create_workspace_dir(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.create_dir(request).await
    }

    #[post("/create-file")]
    async fn create_workspace_file(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.create_file(request).await
    }

    #[post("/write")]
    async fn write_workspace_file(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.write_file(request).await
    }

    #[post("/write-binary")]
    async fn write_workspace_binary_file(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.write_binary_file(request).await
    }

    #[get("/read")]
    async fn read_workspace_file(
        &self,
        #[query("path")] path: String,
    ) -> BootResult<serde_json::Value> {
        self.service.read_file(path).await
    }

    #[get("/read-binary", raw)]
    async fn read_workspace_binary_file(
        &self,
        #[query("path")] path: String,
    ) -> BootResult<BootResponse> {
        let body = self.service.read_binary_file(path).await?;
        Ok(BootResponse::new(200, body).with_content_type("application/octet-stream"))
    }

    #[get("/exists")]
    async fn workspace_path_exists(
        &self,
        #[query("path")] path: String,
    ) -> BootResult<serde_json::Value> {
        self.service.path_exists(path).await
    }

    #[delete("/delete")]
    async fn delete_workspace_path(
        &self,
        #[query("path")] path: String,
    ) -> BootResult<serde_json::Value> {
        self.service.delete_path(path).await
    }

    #[get("/read-dir")]
    async fn read_workspace_dir(
        &self,
        #[query("path")] path: String,
    ) -> BootResult<Vec<serde_json::Value>> {
        self.service.read_dir(path).await
    }

    #[get("/watch", raw)]
    async fn watch_workspace(
        &self,
        #[query("rootPath")] root_path: String,
    ) -> BootResult<BootResponse> {
        self.service.watch(root_path).await
    }

    #[post("/rename")]
    async fn rename_workspace_path(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.rename_path(request).await
    }

    #[post("/copy")]
    async fn copy_workspace_path(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.copy_path(request).await
    }

    #[get("/files")]
    async fn list_workspace_files(
        &self,
        #[query("rootPath")] root_path: String,
        #[query("query")] query: Option<String>,
        #[query("maxResults")] max_results: Option<usize>,
    ) -> BootResult<serde_json::Value> {
        self.service
            .workspace_files(
                root_path,
                query.unwrap_or_default(),
                max_results.unwrap_or(120),
            )
            .await
    }

    #[get("/git-status")]
    async fn workspace_git_status(
        &self,
        #[query("rootPath")] root_path: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.git_status(root_path).await
    }

    #[get("/git-diff")]
    async fn workspace_git_diff(
        &self,
        #[query("rootPath")] root_path: String,
        #[query("path")] path: Option<String>,
        #[query("staged")] staged: Option<bool>,
    ) -> BootResult<serde_json::Value> {
        self.service
            .git_diff(root_path, path, staged.unwrap_or(false))
            .await
    }

    #[post("/git-stage")]
    async fn stage_workspace_files(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.git_stage(request).await
    }

    #[post("/git-unstage")]
    async fn unstage_workspace_files(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.git_unstage(request).await
    }

    #[post("/git-commit")]
    async fn commit_workspace_files(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.git_commit(request).await
    }

    #[allow(clippy::too_many_arguments)]
    #[get("/search")]
    async fn search_workspace_files(
        &self,
        #[query("rootPath")] root_path: String,
        #[query("query")] query: String,
        #[query("caseSensitive")] case_sensitive: Option<bool>,
        #[query("useRegex")] use_regex: Option<bool>,
        #[query("matchWholeWord")] match_whole_word: Option<bool>,
        #[query("includePattern")] include_pattern: Option<String>,
        #[query("excludePattern")] exclude_pattern: Option<String>,
        #[query("maxResults")] max_results: Option<usize>,
    ) -> BootResult<Vec<serde_json::Value>> {
        self.service
            .search_files(
                root_path,
                query,
                super::service::WorkspaceSearchOptions {
                    case_sensitive: case_sensitive.unwrap_or(false),
                    use_regex: use_regex.unwrap_or(false),
                    match_whole_word: match_whole_word.unwrap_or(false),
                    include_pattern,
                    exclude_pattern,
                    max_results: max_results.unwrap_or(1000),
                },
            )
            .await
    }

    #[post("/replace")]
    async fn replace_workspace_files(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.replace_in_files(request).await
    }
}
