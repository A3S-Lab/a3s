use crate::error::Result;
use crate::model::registry::ModelRegistry;

/// Execute the `show` command: display detailed information about a model.
pub fn execute(model: &str, registry: &ModelRegistry) -> Result<()> {
    let manifest = registry.get(model)?;

    println!("Model: {}", manifest.name);
    println!("Format: {}", manifest.format);
    println!("Size: {}", manifest.size_display());
    println!("SHA256: {}", manifest.sha256);
    println!("Path: {}", manifest.path.display());
    println!(
        "Created: {}",
        manifest.created_at.format("%Y-%m-%d %H:%M:%S UTC")
    );

    if let Some(params) = &manifest.parameters {
        println!("\nParameters:");
        if let Some(ctx) = params.context_length {
            println!("  Context Length: {ctx}");
        }
        if let Some(emb) = params.embedding_length {
            println!("  Embedding Length: {emb}");
        }
        if let Some(count) = params.parameter_count {
            let display = if count >= 1_000_000_000 {
                format!("{:.1}B", count as f64 / 1_000_000_000.0)
            } else if count >= 1_000_000 {
                format!("{:.1}M", count as f64 / 1_000_000.0)
            } else {
                format!("{count}")
            };
            println!("  Parameter Count: {display}");
        }
        if let Some(quant) = &params.quantization {
            println!("  Quantization: {quant}");
        }
    }

    Ok(())
}
