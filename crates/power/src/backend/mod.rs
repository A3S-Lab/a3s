pub mod llamacpp;
pub mod types;

use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;

use crate::error::{PowerError, Result};
use crate::model::manifest::{ModelFormat, ModelManifest};

use types::{
    ChatRequest, ChatResponseChunk, CompletionRequest, CompletionResponseChunk, EmbeddingRequest,
    EmbeddingResponse,
};

/// Trait for inference backends that can load models and run inference.
#[async_trait]
pub trait Backend: Send + Sync {
    /// Human-readable name of this backend.
    fn name(&self) -> &str;

    /// Check if this backend can serve the given model format.
    fn supports(&self, format: &ModelFormat) -> bool;

    /// Load a model into memory, ready for inference.
    async fn load(&self, manifest: &ModelManifest) -> Result<()>;

    /// Unload a model from memory.
    async fn unload(&self, model_name: &str) -> Result<()>;

    /// Run chat completion inference, returning a stream of token chunks.
    async fn chat(
        &self,
        model_name: &str,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponseChunk>> + Send>>>;

    /// Run text completion inference, returning a stream of token chunks.
    async fn complete(
        &self,
        model_name: &str,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<CompletionResponseChunk>> + Send>>>;

    /// Generate embeddings for the given input texts.
    async fn embed(&self, model_name: &str, request: EmbeddingRequest)
        -> Result<EmbeddingResponse>;
}

/// Registry of available inference backends.
pub struct BackendRegistry {
    backends: Vec<Arc<dyn Backend>>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// Register a new backend.
    pub fn register(&mut self, backend: Arc<dyn Backend>) {
        self.backends.push(backend);
    }

    /// Find a backend that supports the given model format.
    pub fn find_for_format(&self, format: &ModelFormat) -> Result<Arc<dyn Backend>> {
        self.backends
            .iter()
            .find(|b| b.supports(format))
            .cloned()
            .ok_or_else(|| {
                PowerError::BackendNotAvailable(format!(
                    "No backend available for format: {format}"
                ))
            })
    }

    /// List all registered backend names.
    pub fn list_names(&self) -> Vec<&str> {
        self.backends.iter().map(|b| b.name()).collect()
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a `BackendRegistry` with all available backends pre-registered.
pub fn default_backends() -> BackendRegistry {
    let mut registry = BackendRegistry::new();
    registry.register(Arc::new(llamacpp::LlamaCppBackend::new()));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_registry_find() {
        let registry = default_backends();
        let backend = registry.find_for_format(&ModelFormat::Gguf);
        assert!(backend.is_ok());
        assert_eq!(backend.unwrap().name(), "llama.cpp");
    }

    #[test]
    fn test_backend_registry_list() {
        let registry = default_backends();
        let names = registry.list_names();
        assert!(names.contains(&"llama.cpp"));
    }

    #[test]
    fn test_backend_supports() {
        let backend = llamacpp::LlamaCppBackend::new();
        assert!(backend.supports(&ModelFormat::Gguf));
        assert!(!backend.supports(&ModelFormat::SafeTensors));
    }
}
