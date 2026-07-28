//! Config-file discovery and the first-launch starter template.

pub(crate) const DEFAULT_AUTO_COMPACT_THRESHOLD: f64 = 0.85;

/// A starter A3S ACL `config.acl` with placeholders, generated on first
/// launch so a new user has something to edit instead of an error.
pub(crate) fn config_template() -> &'static str {
    r#"# A3S coding-agent config (A3S ACL).
# Fill in your provider apiKey/baseUrl + a model, set default_model, then save
# with Ctrl+S. Docs: https://a3s-lab.github.io/a3s/

default_model = "openai/my-model"

# Compact automatically when the last prompt reaches this share of the model context window.
# auto_compact_threshold = 0.85

# Optional OS endpoint. When set, a3s code enables /login and /logout.
# os = "https://os.example.com"

# Optional: where /flow DAG JSONs are stored (default ~/.a3s/flows).
# flow_dir = "~/.a3s/flows"

# Optional: where /agent agent definitions are stored (default ~/.a3s/agents).
# agent_dir = "~/.a3s/agents"

# Optional: where /mcp MCP server assets are stored (default ~/.a3s/mcps).
# mcp_dir = "~/.a3s/mcps"

# Optional: where /skill local skill assets are stored (default ~/.a3s/skills).
# skill_dir = "~/.a3s/skills"

# Optional: where long-term memory is stored (default ~/.a3s/memory).
# memory_dir = "~/.a3s/memory"
#
# Optional: tune memory extraction. LLM extraction is enabled by default and
# runs only after significant completed turns.
# memory {
#   llmExtraction = true
#   llmExtractionMaxItems = 5
#   llmExtractionMaxInputChars = 8000
# }

# Optional: a3s-search configuration. Without explicit engine entries,
# web_search uses DuckDuckGo and Wikipedia. Engine entries replace that
# default, so AnySearch is used only when it is explicitly enabled here.
# Enable the headless block only for google or baidu; manage browser runtimes
# with `a3s search browser ...` and verify them with `a3s search doctor`.
# search {
#   timeout = 20
#   engine {
#     ddg   { enabled = true  weight = 1.0 }
#     brave { enabled = true  weight = 1.0 }
#     wiki  { enabled = true  weight = 0.8 }
#     # anysearch { enabled = true weight = 1.0 } # opt in; ANYSEARCH_API_KEY is optional
#     # baidu  { enabled = true weight = 1.0 }
#     # bing_cn { enabled = true weight = 1.0 }
#   }
#   # headless { backend = "chrome" maxTabs = 4 }
# }

providers "openai" {
  apiKey  = "sk-REPLACE-ME"
  baseUrl = "https://api.openai.com/v1/"   # or any OpenAI-compatible endpoint

  models "my-model" {
    name        = "My Model"
    toolCall    = true
    temperature = true
    modalities  = { input = ["text"], output = ["text"] }
    limit       = { context = 200000, output = 4096 }
  }
}

# Optional: use the local Codex CLI / ChatGPT account login as a provider.
# Run `codex login`, then set `default_model` to a slug from `a3s code models`.
# default_model = "codex/model-slug"
# providers "codex" {
#   models "model-slug" { name = "Codex model"; toolCall = true }
# }
"#
}

/// `~/.a3s/config.acl` — the default user-global config location.
pub(crate) fn default_config_path() -> Option<std::path::PathBuf> {
    crate::user_paths::user_home_dir().map(|home| home.join(".a3s/config.acl"))
}

/// Where the interactive `/model` picker stores the last successful choice.
pub(crate) use crate::model::route::ModelSource as ModelSelectionSource;
pub(crate) use crate::model::selection::ModelSelection as ModelSelectionPreference;

/// Load the last successful `/model` choice. Invalid or empty preferences are
/// ignored so a broken cache never prevents the TUI from launching.
pub(crate) fn load_model_selection_preference() -> Option<ModelSelectionPreference> {
    crate::model::selection::load()
}

/// Persist the last successful `/model` choice without mutating config.acl.
pub(crate) fn save_model_selection_preference(
    preference: &ModelSelectionPreference,
) -> std::io::Result<()> {
    crate::model::selection::save(preference)
}

/// Load the last successfully applied TUI effort profile.
/// Unknown values are ignored so older or manually edited state is harmless.
pub(crate) fn load_tui_effort_preference() -> Option<usize> {
    let path = tui_effort_preference_path()?;
    let id = if path.exists() {
        std::fs::read_to_string(&path).ok()?
    } else {
        let legacy = path.parent()?.parent()?.join("tui-effort");
        let id = std::fs::read_to_string(legacy).ok()?;
        if effort_index(&id).is_some() {
            let _ = save_tui_effort_id(&path, id.trim());
        }
        id
    };
    effort_index(&id)
}

fn effort_index(id: &str) -> Option<usize> {
    crate::budget::EFFORT_LEVELS
        .iter()
        .position(|profile| profile.id == id.trim())
}

/// Persist a successfully applied TUI effort profile by stable profile ID.
pub(crate) fn save_tui_effort_preference(index: usize) -> std::io::Result<()> {
    let profile = crate::budget::EFFORT_LEVELS.get(index).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid TUI effort index")
    })?;
    let path = tui_effort_preference_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "user home directory is unavailable",
        )
    })?;
    save_tui_effort_id(&path, profile.id)
}

fn save_tui_effort_id(path: &std::path::Path, id: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&temporary, id)?;
    std::fs::rename(temporary, path)
}

fn tui_effort_preference_path() -> Option<std::path::PathBuf> {
    crate::user_paths::user_home_dir().map(|home| home.join(".a3s/tui/effort"))
}

/// Where long-term memory is stored: `$A3S_MEMORY_DIR`, else a top-level
/// `memory_dir = "..."` / `memoryDir = "..."` in config.acl, else
/// `~/.a3s/memory`. Read at use time so `/config` edits take effect without a
/// restart, and so the TUI session and `/memory` panel browse the same store.
pub(crate) fn memory_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("A3S_MEMORY_DIR") {
        if !d.is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    if let Some(path) = find_config() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(d) =
                top_level_str(&text, "memory_dir").or_else(|| top_level_str(&text, "memoryDir"))
            {
                return expand_home(&d);
            }
        }
    }
    crate::user_paths::user_home_dir()
        .map(|home| home.join(".a3s/memory"))
        .unwrap_or_else(|| std::path::PathBuf::from(".a3s/memory"))
}

/// Where `/flow` DAG JSONs are stored: `$A3S_FLOW_DIR`, else a top-level
/// `flow_dir = "…"` in config.acl, else `~/.a3s/flows`. Read at use time so a
/// `/config` edit takes effect without a restart.
pub(crate) fn flow_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("A3S_FLOW_DIR") {
        if !d.is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    if let Some(path) = find_config() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(d) = top_level_str(&text, "flow_dir") {
                return expand_home(&d);
            }
        }
    }
    crate::user_paths::user_home_dir()
        .map(|home| home.join(".a3s/flows"))
        .unwrap_or_else(|| std::path::PathBuf::from(".a3s/flows"))
}

/// Where `/agent` definitions are stored: `$A3S_AGENT_DIR`, else a top-level
/// `agent_dir = "…"` in config.acl, else `~/.a3s/agents`. Read at use time so
/// a `/config` edit takes effect without a restart.
pub(crate) fn agent_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("A3S_AGENT_DIR") {
        if !d.is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    if let Some(path) = find_config() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(d) = top_level_str(&text, "agent_dir") {
                return expand_home(&d);
            }
        }
    }
    crate::user_paths::user_home_dir()
        .map(|home| home.join(".a3s/agents"))
        .unwrap_or_else(|| std::path::PathBuf::from(".a3s/agents"))
}

/// Where `/mcp` assets are stored: `$A3S_MCP_DIR`, else a top-level
/// `mcp_dir = "..."` in config.acl, else `~/.a3s/mcps`. Read at use time so
/// a `/config` edit takes effect without a restart.
pub(crate) fn mcp_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("A3S_MCP_DIR") {
        if !d.is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    if let Some(path) = find_config() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(d) = top_level_str(&text, "mcp_dir") {
                return expand_home(&d);
            }
        }
    }
    crate::user_paths::user_home_dir()
        .map(|home| home.join(".a3s/mcps"))
        .unwrap_or_else(|| std::path::PathBuf::from(".a3s/mcps"))
}

/// Where `/skill` skill assets are stored: `$A3S_SKILL_DIR`, else a top-level
/// `skill_dir = "..."` in config.acl, else `~/.a3s/skills`. Read at use time so
/// a `/config` edit takes effect without a restart.
pub(crate) fn skill_dir() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("A3S_SKILL_DIR") {
        if !d.is_empty() {
            return std::path::PathBuf::from(d);
        }
    }
    if let Some(path) = find_config() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Some(d) = top_level_str(&text, "skill_dir") {
                return expand_home(&d);
            }
        }
    }
    crate::user_paths::user_home_dir()
        .map(|home| home.join(".a3s/skills"))
        .unwrap_or_else(|| std::path::PathBuf::from(".a3s/skills"))
}

pub(crate) fn auto_compact_threshold_for_path(path: &std::path::Path) -> f64 {
    let Ok(text) = std::fs::read_to_string(path) else {
        return DEFAULT_AUTO_COMPACT_THRESHOLD;
    };
    match auto_compact_threshold_from_text(&text) {
        Ok(Some(threshold)) => threshold,
        Ok(None) => DEFAULT_AUTO_COMPACT_THRESHOLD,
        Err(value) => {
            eprintln!(
                "warning: invalid auto compact threshold {value:?}; using {DEFAULT_AUTO_COMPACT_THRESHOLD}"
            );
            DEFAULT_AUTO_COMPACT_THRESHOLD
        }
    }
}

fn auto_compact_threshold_from_text(text: &str) -> Result<Option<f64>, String> {
    let value = top_level_str(text, "auto_compact_threshold")
        .or_else(|| top_level_str(text, "autoCompactThreshold"));
    let Some(value) = value else {
        return Ok(None);
    };
    let threshold = value.parse::<f64>().map_err(|_| value.clone())?;
    if threshold > 0.0 && threshold <= 1.0 {
        Ok(Some(threshold))
    } else {
        Err(value)
    }
}

/// Extract a top-level `key = "value"` scalar from A3S ACL text. Only lines at
/// brace depth 0 count, so a same-named key inside a `providers { … }` block
/// can't shadow it. The core's CodeConfig ignores unknown keys, so the option
/// lives in the same config.acl without breaking its typed parse.
fn top_level_str(text: &str, key: &str) -> Option<String> {
    let mut depth = 0i64;
    for line in text.lines() {
        let t = line.trim();
        if depth == 0 && !t.starts_with('#') {
            if let Some(rest) = t.strip_prefix(key) {
                if let Some(v) = rest.trim_start().strip_prefix('=') {
                    let v = v.trim().trim_matches('"');
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
        depth += t.matches('{').count() as i64 - t.matches('}').count() as i64;
    }
    None
}

/// Expand a leading `~/` to the native user home (config values are user-typed paths).
fn expand_home(p: &str) -> std::path::PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = crate::user_paths::user_home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(p)
}

/// Write the starter config to `path` (creating parent dirs). Never overwrites.
pub(crate) fn write_template_config(path: &std::path::Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, config_template())
}

/// Find the A3S config: `$A3S_CONFIG_FILE`, then `.a3s/config.acl` walking up
/// from the current directory (project-local), then `~/.a3s/config.acl`
/// (user-global) — so `a3s code` works from anywhere once a global config exists.
pub(crate) fn find_config() -> Option<String> {
    if let Ok(p) = std::env::var("A3S_CONFIG_FILE") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        let mut dir: Option<&std::path::Path> = Some(cwd.as_path());
        while let Some(d) = dir {
            let candidate = d.join(".a3s/config.acl");
            if candidate.is_file() {
                return Some(candidate.to_string_lossy().into_owned());
            }
            dir = d.parent();
        }
    }
    if let Some(home) = crate::user_paths::user_home_dir() {
        let candidate = home.join(".a3s/config.acl");
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().into_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_compact_threshold_parses_snake_and_camel_case_top_level_values() {
        assert_eq!(
            auto_compact_threshold_from_text("auto_compact_threshold = 0.9"),
            Ok(Some(0.9))
        );
        assert_eq!(
            auto_compact_threshold_from_text("autoCompactThreshold = 0.75"),
            Ok(Some(0.75))
        );
        assert_eq!(
            auto_compact_threshold_from_text("default_model = \"x\""),
            Ok(None)
        );
        assert_eq!(
            auto_compact_threshold_from_text(
                "providers \"x\" { auto_compact_threshold = 0.2 }\nauto_compact_threshold = 0.8"
            ),
            Ok(Some(0.8))
        );
    }

    #[test]
    fn auto_compact_threshold_rejects_values_outside_ratio_range() {
        for value in ["0", "-0.1", "1.01", "not-a-number"] {
            let text = format!("auto_compact_threshold = {value}");
            assert!(
                auto_compact_threshold_from_text(&text).is_err(),
                "{value} should be invalid"
            );
        }
    }

    #[test]
    fn top_level_str_reads_only_depth_zero_keys() {
        let text = r#"
# flow_dir = "/commented/out"
providers "x" {
  flow_dir = "/inside/a/block"
}
flow_dir = "~/flows"
memory_dir = "~/memories"
memoryDir = "~/camel-memories"
"#;
        assert_eq!(top_level_str(text, "flow_dir").as_deref(), Some("~/flows"));
        assert_eq!(
            top_level_str(text, "memory_dir").as_deref(),
            Some("~/memories")
        );
        assert_eq!(
            top_level_str(text, "memoryDir").as_deref(),
            Some("~/camel-memories")
        );
        assert_eq!(top_level_str(text, "missing"), None);
        // A longer identifier sharing the prefix does not match.
        assert_eq!(top_level_str("flow_dirx = \"/y\"", "flow_dir"), None);
    }

    #[test]
    fn expand_home_resolves_tilde() {
        let home = std::env::var("HOME").expect("HOME set in tests");
        assert_eq!(
            expand_home("~/clones"),
            std::path::Path::new(&home).join("clones")
        );
        assert_eq!(expand_home("/abs/path"), std::path::Path::new("/abs/path"));
    }

    #[test]
    fn model_selection_preference_round_trips_under_home() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old_home = std::env::var_os("HOME");
        let home = temp_home("model-selection-round-trip");
        std::env::set_var("HOME", &home);

        let preference = ModelSelectionPreference {
            source: ModelSelectionSource::Codex,
            model: "gpt-5.5".to_string(),
        };
        save_model_selection_preference(&preference).expect("preference should save");

        assert_eq!(load_model_selection_preference(), Some(preference));
        assert!(home.join(".a3s/tui/model-selection.json").is_file());

        restore_var("HOME", old_home);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn invalid_model_selection_preference_is_ignored() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old_home = std::env::var_os("HOME");
        let home = temp_home("model-selection-invalid");
        let path = home.join(".a3s/tui/model-selection.json");
        std::fs::create_dir_all(path.parent().expect("path has parent")).unwrap();
        std::fs::write(&path, "{not-json").unwrap();
        std::env::set_var("HOME", &home);

        assert_eq!(load_model_selection_preference(), None);

        restore_var("HOME", old_home);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn tui_effort_preference_round_trips_and_rejects_invalid_values() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old_home = std::env::var_os("HOME");
        let home = temp_home("tui-effort-round-trip");
        std::env::set_var("HOME", &home);

        assert_eq!(load_tui_effort_preference(), None);
        save_tui_effort_preference(4).expect("preference should save");
        assert_eq!(load_tui_effort_preference(), Some(4));
        assert_eq!(
            std::fs::read_to_string(home.join(".a3s/tui/effort")).unwrap(),
            crate::budget::EFFORT_LEVELS[4].id
        );
        std::fs::write(home.join(".a3s/tui/effort"), "unknown").unwrap();
        assert_eq!(load_tui_effort_preference(), None);
        assert!(save_tui_effort_preference(usize::MAX).is_err());

        restore_var("HOME", old_home);
        let _ = std::fs::remove_dir_all(home);
    }

    fn temp_home(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("a3s-{name}-{}-{nanos}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn restore_var(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
