use crate::error::Result;
use crate::model::registry::ModelRegistry;
use crate::model::storage;

/// Execute the `delete` command: remove a model from local storage.
pub fn execute(model: &str, registry: &ModelRegistry) -> Result<()> {
    let manifest = registry.remove(model)?;
    storage::delete_blob(&manifest)?;

    println!("Deleted model '{}'", manifest.name);
    println!("  Freed {}", manifest.size_display());

    Ok(())
}
