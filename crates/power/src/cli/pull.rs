use indicatif::{ProgressBar, ProgressStyle};

use crate::error::Result;
use crate::model::pull::pull_model;
use crate::model::registry::ModelRegistry;

/// Execute the `pull` command: download a model from a URL.
pub async fn execute(model: &str, registry: &ModelRegistry) -> Result<()> {
    if registry.exists(model) {
        println!("Model '{model}' already exists locally.");
        return Ok(());
    }

    // For now, treat the model name as a direct URL if it starts with http
    let url = if model.starts_with("http://") || model.starts_with("https://") {
        model.to_string()
    } else {
        // In the future, this would resolve a model name to a registry URL
        println!("Model registry resolution not yet implemented.");
        println!("Please provide a direct URL to a model file.");
        println!("Example: a3s-power pull https://example.com/model.gguf");
        return Ok(());
    };

    let pb = ProgressBar::new(0);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .unwrap()
            .progress_chars("=>-"),
    );

    let progress_bar = pb.clone();
    let progress = Box::new(move |downloaded: u64, total: u64| {
        if total > 0 {
            progress_bar.set_length(total);
        }
        progress_bar.set_position(downloaded);
    });

    let name = extract_name_from_url(&url);
    println!("Pulling '{name}' from {url}");

    let manifest = pull_model(&name, &url, Some(progress)).await?;
    pb.finish_with_message("Download complete");

    registry.register(manifest)?;
    println!("Successfully pulled '{name}'");

    Ok(())
}

/// Extract a reasonable model name from a URL.
fn extract_name_from_url(url: &str) -> String {
    url.rsplit('/')
        .next()
        .unwrap_or("unknown")
        .trim_end_matches(".gguf")
        .trim_end_matches(".safetensors")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name_from_url() {
        assert_eq!(
            extract_name_from_url("https://example.com/model.gguf"),
            "model"
        );
        assert_eq!(
            extract_name_from_url("https://example.com/llama3-8b-q4.gguf"),
            "llama3-8b-q4"
        );
        assert_eq!(
            extract_name_from_url("https://example.com/model.safetensors"),
            "model"
        );
    }
}
