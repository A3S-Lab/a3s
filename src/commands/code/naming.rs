//! Naming helpers shared by A3S Code asset and research surfaces.

/// Filesystem-and-URL-safe asset slug: ASCII lowercase with `-` separators.
pub(crate) fn asset_slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let slug = out.trim_matches('-').to_string();
    if slug.is_empty() {
        "asset".to_string()
    } else {
        slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_slug_is_ascii_lowercase_and_stable() {
        assert_eq!(asset_slug("Review Captain"), "review-captain");
        assert_eq!(asset_slug("agent_app.v2"), "agent-app-v2");
        assert_eq!(asset_slug("  "), "asset");
    }
}
