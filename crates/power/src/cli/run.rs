use std::sync::Arc;

use crate::backend::BackendRegistry;
use crate::error::Result;
use crate::model::registry::ModelRegistry;

/// Execute the `run` command: load a model and start interactive chat.
pub async fn execute(
    model: &str,
    prompt: Option<&str>,
    registry: &ModelRegistry,
    backends: &BackendRegistry,
) -> Result<()> {
    // Ensure model is available locally
    let manifest = match registry.get(model) {
        Ok(m) => m,
        Err(_) => {
            println!("Model '{model}' not found locally.");
            println!("Use `a3s-power pull {model}` to download it first.");
            return Ok(());
        }
    };

    // Find a backend that supports this model format
    let backend = backends.find_for_format(&manifest.format)?;
    tracing::info!(
        model = %manifest.name,
        backend = backend.name(),
        "Selected backend for model"
    );

    // Load the model
    println!("Loading model '{}'...", manifest.name);
    if let Err(e) = backend.load(&manifest).await {
        println!("Failed to load model: {e}");
        println!("Note: The llama.cpp backend is not yet fully implemented.");
        return Ok(());
    }

    if let Some(prompt_text) = prompt {
        // Non-interactive: send a single prompt
        println!("Prompt: {prompt_text}");
        println!("(Inference not yet implemented)");
    } else {
        // Interactive chat mode
        println!("Interactive chat mode (type 'exit' or Ctrl+C to quit)");
        println!("(Interactive mode not yet implemented)");
    }

    // Unload model
    let _ = backend.unload(&manifest.name).await;

    Ok(())
}

/// Ensure model is available, pulling if necessary.
pub async fn ensure_model(model: &str, registry: &Arc<ModelRegistry>) -> Result<()> {
    if !registry.exists(model) {
        println!("Model '{model}' not found locally. Pull it first with `a3s-power pull`.");
    }
    Ok(())
}
