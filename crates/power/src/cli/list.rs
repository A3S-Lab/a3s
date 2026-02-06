use crate::error::Result;
use crate::model::registry::ModelRegistry;

/// Execute the `list` command: display all locally available models.
pub fn execute(registry: &ModelRegistry) -> Result<()> {
    let models = registry.list()?;

    if models.is_empty() {
        println!("No models found locally.");
        println!("Use `a3s-power pull <url>` to download a model.");
        return Ok(());
    }

    println!("{:<30} {:<12} {:<12} MODIFIED", "NAME", "FORMAT", "SIZE");
    for model in &models {
        let modified = model.created_at.format("%Y-%m-%d %H:%M");
        println!(
            "{:<30} {:<12} {:<12} {}",
            model.name,
            model.format.to_string(),
            model.size_display(),
            modified,
        );
    }

    println!("\n{} model(s) total", models.len());
    Ok(())
}
