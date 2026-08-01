//! Agent skill discovery, loading, and the disabled-skills persistence.

/// Discover agent skill directories — personal (`~/.agents/skills`,
/// `~/.codex/skills`, `~/.claude/skills`), project (`<ws>/<root>/skills`),
/// and plugin-bundled (`~/<root>/plugins/**/skills`) — so a3s can load
/// `SKILL.md` skills directly. a3s's skill loader already understands the
/// `<name>/SKILL.md` layout and YAML frontmatter.
/// Parse a SKILL.md's YAML frontmatter for `name` + `description`.
fn parse_skill_meta(path: &std::path::Path) -> Option<(String, String)> {
    let content = std::fs::read_to_string(path).ok()?;
    let rest = content.trim_start().strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let (mut name, mut desc) = (None, None);
    for line in rest[..end].lines() {
        if let Some(v) = line.strip_prefix("name:") {
            name = Some(v.trim().trim_matches(['"', '\'']).to_string());
        } else if let Some(v) = line.strip_prefix("description:") {
            desc = Some(v.trim().trim_matches(['"', '\'']).to_string());
        }
    }
    let name = name?;
    if name.is_empty() {
        return None;
    }
    Some((name, desc.unwrap_or_default()))
}

/// `~/.a3s/disabled_skills` — names the user has turned off via `/plugin`.
fn disabled_skills_path() -> Option<std::path::PathBuf> {
    crate::user_paths::user_home_dir().map(|home| home.join(".a3s/disabled_skills"))
}

pub(crate) fn load_disabled_skills() -> std::collections::HashSet<String> {
    disabled_skills_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn save_disabled_skills(set: &std::collections::HashSet<String>) {
    if let Some(p) = disabled_skills_path() {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut names: Vec<&String> = set.iter().collect();
        names.sort();
        let body = names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(p, body);
    }
}

/// Load skill (name, description) pairs from the skill dirs, for the `$` menu.
pub(crate) fn load_skills(dirs: &[std::path::PathBuf]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let md = if p.is_dir() {
                p.join("SKILL.md")
            } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                p.clone()
            } else {
                continue;
            };
            if md.is_file() {
                if let Some(meta) = parse_skill_meta(&md) {
                    out.push(meta);
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Count discoverable Claude skills (`<name>/SKILL.md` dirs + flat `*.md`)
/// across the skill dirs — shown on the start screen so compatibility is visible.
pub(crate) fn count_skill_files(dirs: &[std::path::PathBuf]) -> usize {
    let mut n = 0;
    for d in dirs {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                let is_skill_dir = p.is_dir() && p.join("SKILL.md").is_file();
                let is_flat_md = p.extension().and_then(|x| x.to_str()) == Some("md");
                if is_skill_dir || is_flat_md {
                    n += 1;
                }
            }
        }
    }
    n
}

/// Bundled built-in skills the cli always ships (not gated, not project-local).
const OKF_SKILL: &str = include_str!("../../../skills/okf.md");

/// Materialize the always-available built-in cli skill (`okf`) under
/// `~/.a3s/cli/skills/<name>/` and return that root so the session can add it to
/// its skill dirs. Best-effort — returns `None` on any I/O error.
pub(crate) fn ensure_builtin_skills_dir() -> Option<std::path::PathBuf> {
    let a3s_root = crate::user_paths::user_home_dir()?.join(".a3s");
    let root = a3s_root.join("cli").join("skills");
    ensure_builtin_skills_dir_at(&root).ok()?;
    // `~/.a3s/cli-skills` was an early flat layout and was always CLI-owned.
    // Built-ins are regenerated above, so remove the obsolete duplicate instead
    // of keeping two sources that can drift or surface twice in skill discovery.
    let _ = remove_legacy_builtin_skills_dir(&a3s_root.join("cli-skills"));
    Some(root)
}

/// The built-in cli skills materialized under `~/.a3s/cli/skills/`. Any other
/// directory there is a stale leftover from an earlier version (e.g. the old
/// `kb-compile`, which was renamed to `okf`) and is pruned below.
const BUILTIN_SKILLS: &[(&str, &str)] = &[("okf", OKF_SKILL)];

fn ensure_builtin_skills_dir_at(root: &std::path::Path) -> std::io::Result<()> {
    for (name, body) in BUILTIN_SKILLS {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("SKILL.md"), body)?;
    }
    // The cli-skills root is cli-owned (only built-ins materialize here), so prune
    // any directory that isn't a current built-in — otherwise a renamed skill like
    // the old `kb-compile` lingers and resurfaces as a duplicate `/` command.
    if let Ok(rd) = std::fs::read_dir(root) {
        for entry in rd.flatten() {
            let p = entry.path();
            let is_builtin = p
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| BUILTIN_SKILLS.iter().any(|(name, _)| *name == n));
            if p.is_dir() && !is_builtin {
                let _ = std::fs::remove_dir_all(&p);
            }
        }
    }
    Ok(())
}

fn remove_legacy_builtin_skills_dir(path: &std::path::Path) -> std::io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

pub(crate) fn agent_skill_dirs(workspace: &str) -> Vec<std::path::PathBuf> {
    let configured_a3s = crate::config::skill_dir();
    agent_skill_dirs_with_configured(workspace, &configured_a3s)
}

pub(crate) fn agent_skill_dirs_with_configured(
    workspace: &str,
    configured_a3s: &std::path::Path,
) -> Vec<std::path::PathBuf> {
    let mut dirs: Vec<std::path::PathBuf> = Vec::new();
    let project_a3s = std::path::Path::new(workspace).join(".a3s").join("skills");
    if project_a3s.is_dir() {
        dirs.push(project_a3s);
    }
    if configured_a3s.is_dir() {
        dirs.push(configured_a3s.to_path_buf());
    }
    // Agents, Claude Code, and Codex all keep skills under `<root>/skills`
    // (same SKILL.md layout); load from any of them so a skill written for one
    // works in the others.
    for root in [".agents", ".claude", ".codex"] {
        let project = std::path::Path::new(workspace).join(root).join("skills");
        if project.is_dir() {
            dirs.push(project);
        }
    }
    if let Some(home) = crate::user_paths::user_home_dir() {
        for root in [".agents", ".claude", ".codex"] {
            let personal = home.join(root).join("skills");
            if personal.is_dir() {
                dirs.push(personal);
            }
        }
        // Depth 6 covers nested plugin layouts: plugins/cache/<plugin>/<plugin>/
        // <version>/skills and marketplaces/<mkt>/external_plugins/<plugin>/skills.
        collect_skills_dirs(&home.join(".agents/plugins"), 0, 6, &mut dirs);
        collect_skills_dirs(&home.join(".claude/plugins"), 0, 6, &mut dirs);
        collect_skills_dirs(&home.join(".codex/plugins"), 0, 6, &mut dirs);
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// Recursively collect directories literally named `skills` (Claude plugins
/// bundle their skills there), bounded in depth and skipping dotfiles.
fn collect_skills_dirs(
    dir: &std::path::Path,
    depth: usize,
    max: usize,
    out: &mut Vec<std::path::PathBuf>,
) {
    if depth > max {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "skills" {
            out.push(p);
        } else if !name.starts_with('.') && name != "node_modules" {
            collect_skills_dirs(&p, depth + 1, max, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("a3s-{name}-{}-{nanos}", std::process::id()))
    }

    fn restore_env_var(name: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }

    #[test]
    fn okf_skill_materializes_and_parses() {
        let dir = std::env::temp_dir().join(format!("a3s-okf-skill-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // Seed a stale leftover (the pre-rename `kb-compile`) — it must be pruned.
        std::fs::create_dir_all(dir.join("kb-compile")).unwrap();
        std::fs::write(dir.join("kb-compile/SKILL.md"), "stale").unwrap();
        ensure_builtin_skills_dir_at(&dir).unwrap();
        assert!(
            !dir.join("kb-compile").exists(),
            "stale kb-compile dir should be pruned so /kb-compile can't resurface"
        );

        // The cli loader discovers it by name → it shows in the `$` menu as `$okf`.
        let skills = load_skills(std::slice::from_ref(&dir));
        assert!(
            skills.iter().any(|(n, _)| n == "okf"),
            "okf skill not discovered: {skills:?}"
        );
        assert!(
            !skills.iter().any(|(n, _)| n == "kb-compile"),
            "stale kb-compile must not be discovered: {skills:?}"
        );

        // The stricter CORE loader (validates kind + fail-secure allowed-tools +
        // 10KiB body cap) must accept it, else it would silently fail to load.
        let md = std::fs::read_to_string(dir.join("okf/SKILL.md")).unwrap();
        let skill = a3s_code_core::skills::Skill::parse(&md)
            .expect("core skill loader must accept the bundled okf SKILL.md");
        assert_eq!(skill.name, "okf");
        assert!(
            skill.allowed_tools.is_some(),
            "allowed-tools must parse (fail-secure) so the skill is usable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agent_skill_dirs_include_agents_codex_and_claude_roots() {
        let _guard = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old_home = std::env::var_os("HOME");
        let old_config = std::env::var_os("A3S_CONFIG_FILE");
        let old_skill_dir = std::env::var_os("A3S_SKILL_DIR");

        let root = temp_root("skill-roots");
        let home = root.join("home");
        let workspace = root.join("workspace");
        let a3s_skills = home.join(".a3s/skills");
        let agents_personal = home.join(".agents/skills");
        let agents_plugin = home.join(".agents/plugins/cache/example/1.0.0/skills");
        let codex_personal = home.join(".codex/skills");
        let claude_personal = home.join(".claude/skills");
        let project_agents = workspace.join(".agents/skills");
        let project_codex = workspace.join(".codex/skills");
        let project_claude = workspace.join(".claude/skills");

        for dir in [
            &a3s_skills,
            &agents_personal,
            &agents_plugin,
            &codex_personal,
            &claude_personal,
            &project_agents,
            &project_codex,
            &project_claude,
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }

        std::env::set_var("HOME", &home);
        std::env::remove_var("A3S_CONFIG_FILE");
        std::env::set_var("A3S_SKILL_DIR", &a3s_skills);

        let dirs = agent_skill_dirs(workspace.to_str().unwrap());

        for expected in [
            &a3s_skills,
            &agents_personal,
            &agents_plugin,
            &codex_personal,
            &claude_personal,
            &project_agents,
            &project_codex,
            &project_claude,
        ] {
            assert!(
                dirs.iter().any(|dir| dir == expected),
                "missing skill dir {} in {dirs:?}",
                expected.display()
            );
        }

        restore_env_var("HOME", old_home);
        restore_env_var("A3S_CONFIG_FILE", old_config);
        restore_env_var("A3S_SKILL_DIR", old_skill_dir);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_flat_builtin_skill_directory_is_removed() {
        let base =
            std::env::temp_dir().join(format!("a3s-legacy-builtin-skills-{}", std::process::id()));
        let legacy = base.join("cli-skills");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(legacy.join("retired-skill")).unwrap();
        std::fs::write(legacy.join("retired-skill/SKILL.md"), "stale").unwrap();

        remove_legacy_builtin_skills_dir(&legacy).unwrap();

        assert!(!legacy.exists());
        let _ = std::fs::remove_dir_all(&base);
    }
}
