use std::sync::Arc;

use a3s_boot::Result as BootResult;
use serde_json::{json, Value};

use super::model::{CreatePreviewRequest, PreviewDescriptor};
use super::registry::PreviewRegistry;

pub(in crate::api::code_web) struct PreviewsService {
    registry: Arc<PreviewRegistry>,
}

impl PreviewsService {
    pub(in crate::api::code_web) fn new(registry: Arc<PreviewRegistry>) -> Self {
        Self { registry }
    }

    pub(in crate::api::code_web) async fn create(
        &self,
        request: CreatePreviewRequest,
    ) -> BootResult<PreviewDescriptor> {
        self.registry.create(request.target).await
    }

    pub(in crate::api::code_web) async fn get(&self, id: &str) -> BootResult<PreviewDescriptor> {
        self.registry.get(id).await
    }

    pub(in crate::api::code_web) async fn remove(&self, id: &str) -> BootResult<Value> {
        self.registry.remove(id).await?;
        Ok(json!({ "id": id, "stopped": true }))
    }
}
