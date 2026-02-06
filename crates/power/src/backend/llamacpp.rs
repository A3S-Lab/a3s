// llama.cpp backend implementation.
//
// This module provides inference via the `llama-cpp-2` crate (Rust bindings for llama.cpp).
// It is currently a placeholder; the actual implementation requires the `llamacpp` feature flag
// and the llama.cpp C++ build toolchain.
//
// When enabled, this backend supports GGUF model files and provides:
// - Chat completion with streaming
// - Text completion with streaming
// - Embedding generation

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;

use crate::error::{PowerError, Result};
use crate::model::manifest::{ModelFormat, ModelManifest};

use super::types::{
    ChatRequest, ChatResponseChunk, CompletionRequest, CompletionResponseChunk, EmbeddingRequest,
    EmbeddingResponse,
};
use super::Backend;

/// llama.cpp backend for GGUF model inference.
pub struct LlamaCppBackend;

impl LlamaCppBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LlamaCppBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Backend for LlamaCppBackend {
    fn name(&self) -> &str {
        "llama.cpp"
    }

    fn supports(&self, format: &ModelFormat) -> bool {
        matches!(format, ModelFormat::Gguf)
    }

    async fn load(&self, manifest: &ModelManifest) -> Result<()> {
        tracing::info!(model = %manifest.name, "Loading model with llama.cpp backend (stub)");
        Err(PowerError::BackendNotAvailable(
            "llama.cpp backend is not yet implemented. Enable the `llamacpp` feature flag."
                .to_string(),
        ))
    }

    async fn unload(&self, model_name: &str) -> Result<()> {
        tracing::info!(
            model = model_name,
            "Unloading model from llama.cpp backend (stub)"
        );
        Ok(())
    }

    async fn chat(
        &self,
        _model_name: &str,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponseChunk>> + Send>>> {
        Err(PowerError::BackendNotAvailable(
            "llama.cpp chat not yet implemented".to_string(),
        ))
    }

    async fn complete(
        &self,
        _model_name: &str,
        _request: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<CompletionResponseChunk>> + Send>>> {
        Err(PowerError::BackendNotAvailable(
            "llama.cpp completion not yet implemented".to_string(),
        ))
    }

    async fn embed(
        &self,
        _model_name: &str,
        _request: EmbeddingRequest,
    ) -> Result<EmbeddingResponse> {
        Err(PowerError::BackendNotAvailable(
            "llama.cpp embeddings not yet implemented".to_string(),
        ))
    }
}
