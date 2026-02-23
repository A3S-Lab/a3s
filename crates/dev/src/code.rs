use std::io::{self, Write};
use std::path::Path;

use colored::Colorize;
use minijinja::{context, Environment};

use crate::error::{DevError, Result};

#[derive(Debug, Clone, Copy)]
pub enum Language {
    Python,
    TypeScript,
}

/// Simple TUI: arrow-key selection from a list of options.
pub fn prompt_language() -> Result<Language> {
    let options = ["Python", "TypeScript"];
    let mut selected = 0usize;

    // Enable raw mode manually via termios
    use_raw_mode(|| {
        print!("\r\n  Select language for your a3s-code agent:\r\n\r\n");
        loop {
            for (i, opt) in options.iter().enumerate() {
                if i == selected {
                    print!("  {} {}\r\n", "▶".cyan(), opt.cyan().bold());
                } else {
                    print!("    {}\r\n", opt.dimmed());
                }
            }

            io::stdout().flush().ok();

            // Read a key
            let key = read_key()?;
            match key {
                Key::Up => {
                    selected = selected.saturating_sub(1);
                }
                Key::Down => {
                    if selected < options.len() - 1 {
                        selected += 1;
                    }
                }
                Key::Enter => break,
                Key::Char('q') | Key::Ctrl('c') => {
                    return Err(DevError::Config("cancelled".into()));
                }
                _ => {}
            }

            // Move cursor up to redraw
            print!("\x1b[{}A", options.len());
        }
        Ok(())
    })?;

    println!("\r\n  {} {}\r\n", "✓".green(), options[selected].cyan());

    Ok(match selected {
        0 => Language::Python,
        _ => Language::TypeScript,
    })
}

// ── Key reading ──────────────────────────────────────────────────────────────

#[derive(Debug)]
enum Key {
    Up,
    Down,
    Enter,
    Char(char),
    Ctrl(char),
    Other,
}

fn read_key() -> Result<Key> {
    let mut buf = [0u8; 4];
    let n = unsafe { libc::read(0, buf.as_mut_ptr() as *mut libc::c_void, 4) };
    if n <= 0 {
        return Ok(Key::Other);
    }
    Ok(match &buf[..n as usize] {
        [0x1b, b'[', b'A', ..] => Key::Up,
        [0x1b, b'[', b'B', ..] => Key::Down,
        [b'\r'] | [b'\n'] => Key::Enter,
        [3] => Key::Ctrl('c'),
        [b] if *b >= 1 && *b <= 26 => Key::Ctrl((b'a' + b - 1) as char),
        [b] => Key::Char(*b as char),
        _ => Key::Other,
    })
}

fn use_raw_mode<F>(f: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    use std::mem::MaybeUninit;

    let mut orig = MaybeUninit::uninit();
    unsafe {
        libc::tcgetattr(0, orig.as_mut_ptr());
        let mut raw = orig.assume_init();
        libc::cfmakeraw(&mut raw);
        libc::tcsetattr(0, libc::TCSANOW, &raw);
    }

    let result = f();

    unsafe {
        libc::tcsetattr(0, libc::TCSANOW, orig.as_ptr());
    }

    result
}

// ── Scaffold ──────────────────────────────────────────────────────────────────

pub fn scaffold(dir: &Path, lang: Language, project_name: &str) -> Result<()> {
    std::fs::create_dir_all(dir).map_err(|e| DevError::Config(format!("create dir: {e}")))?;

    let mut env = Environment::new();
    let add = |env: &mut Environment, name: &'static str, src: &'static str| {
        env.add_template(name, src)
            .map_err(|e| DevError::Config(format!("template '{name}': {e}")))
    };
    add(&mut env, "python/config.hcl", PYTHON_CONFIG)?;
    add(&mut env, "python/skills/hello.py", PYTHON_SKILL)?;
    add(&mut env, "python/agents/demo.py", PYTHON_AGENT)?;
    add(&mut env, "python/requirements.txt", PYTHON_REQUIREMENTS)?;
    add(&mut env, "typescript/config.hcl", TS_CONFIG)?;
    add(&mut env, "typescript/skills/hello.ts", TS_SKILL)?;
    add(&mut env, "typescript/agents/demo.ts", TS_AGENT)?;
    add(&mut env, "typescript/package.json", TS_PACKAGE_JSON)?;
    add(&mut env, "typescript/tsconfig.json", TS_CONFIG_JSON)?;

    let ctx = context! { project_name };

    match lang {
        Language::Python => scaffold_python(dir, &env, ctx),
        Language::TypeScript => scaffold_typescript(dir, &env, ctx),
    }
}

fn scaffold_python(dir: &Path, env: &Environment, ctx: minijinja::Value) -> Result<()> {
    std::fs::create_dir_all(dir.join("skills")).map_err(|e| DevError::Config(e.to_string()))?;
    std::fs::create_dir_all(dir.join("agents")).map_err(|e| DevError::Config(e.to_string()))?;
    let get = |name: &str| {
        env.get_template(name)
            .map_err(|e| DevError::Config(format!("template '{name}': {e}")))
    };
    render_file(dir, "config.hcl", get("python/config.hcl")?.render(ctx.clone())?)?;
    render_file(dir, "skills/hello.py", get("python/skills/hello.py")?.render(ctx.clone())?)?;
    render_file(dir, "agents/demo.py", get("python/agents/demo.py")?.render(ctx.clone())?)?;
    render_file(dir, "requirements.txt", get("python/requirements.txt")?.render(ctx)?)?;
    Ok(())
}

fn scaffold_typescript(dir: &Path, env: &Environment, ctx: minijinja::Value) -> Result<()> {
    std::fs::create_dir_all(dir.join("skills")).map_err(|e| DevError::Config(e.to_string()))?;
    std::fs::create_dir_all(dir.join("agents")).map_err(|e| DevError::Config(e.to_string()))?;
    let get = |name: &str| {
        env.get_template(name)
            .map_err(|e| DevError::Config(format!("template '{name}': {e}")))
    };
    render_file(dir, "config.hcl", get("typescript/config.hcl")?.render(ctx.clone())?)?;
    render_file(dir, "skills/hello.ts", get("typescript/skills/hello.ts")?.render(ctx.clone())?)?;
    render_file(dir, "agents/demo.ts", get("typescript/agents/demo.ts")?.render(ctx.clone())?)?;
    render_file(dir, "package.json", get("typescript/package.json")?.render(ctx.clone())?)?;
    render_file(dir, "tsconfig.json", get("typescript/tsconfig.json")?.render(ctx)?)?;
    Ok(())
}

fn render_file(dir: &Path, name: &str, content: String) -> Result<()> {
    let path = dir.join(name);
    std::fs::write(&path, content)
        .map_err(|e| DevError::Config(format!("write {}: {e}", path.display())))
}

// ── Templates (embedded at compile time) ─────────────────────────────────────

const PYTHON_CONFIG: &str = include_str!("templates/python/config.hcl");
const PYTHON_SKILL: &str = include_str!("templates/python/skills/hello.py");
const PYTHON_AGENT: &str = include_str!("templates/python/agents/demo.py");
const PYTHON_REQUIREMENTS: &str = include_str!("templates/python/requirements.txt");

const TS_CONFIG: &str = include_str!("templates/typescript/config.hcl");
const TS_SKILL: &str = include_str!("templates/typescript/skills/hello.ts");
const TS_AGENT: &str = include_str!("templates/typescript/agents/demo.ts");
const TS_PACKAGE_JSON: &str = include_str!("templates/typescript/package.json");
const TS_CONFIG_JSON: &str = include_str!("templates/typescript/tsconfig.json");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scaffold_python() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), Language::Python, "my-agent").unwrap();
        assert!(dir.path().join("config.hcl").exists());
        assert!(dir.path().join("requirements.txt").exists());
        assert!(dir.path().join("skills/hello.py").exists());
        assert!(dir.path().join("agents/demo.py").exists());
        let config = std::fs::read_to_string(dir.path().join("config.hcl")).unwrap();
        assert!(config.contains("claude-sonnet-4-5"));
        assert!(config.contains("my-agent"));
    }

    #[test]
    fn test_scaffold_typescript() {
        let dir = tempfile::tempdir().unwrap();
        scaffold(dir.path(), Language::TypeScript, "my-agent").unwrap();
        assert!(dir.path().join("config.hcl").exists());
        assert!(dir.path().join("package.json").exists());
        assert!(dir.path().join("tsconfig.json").exists());
        assert!(dir.path().join("skills/hello.ts").exists());
        assert!(dir.path().join("agents/demo.ts").exists());
        let pkg = std::fs::read_to_string(dir.path().join("package.json")).unwrap();
        assert!(pkg.contains("@a3s-lab/code"));
        assert!(pkg.contains("my-agent"));
    }

    #[test]
    fn test_templates_not_empty() {
        assert!(!PYTHON_CONFIG.is_empty());
        assert!(!PYTHON_SKILL.is_empty());
        assert!(!PYTHON_AGENT.is_empty());
        assert!(!TS_CONFIG.is_empty());
        assert!(!TS_SKILL.is_empty());
        assert!(!TS_AGENT.is_empty());
        assert!(!TS_PACKAGE_JSON.is_empty());
        assert!(!TS_CONFIG_JSON.is_empty());
    }
}
