use std::io::{self, Write};
use std::path::Path;

use colored::Colorize;

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
                    if selected > 0 {
                        selected -= 1;
                    }
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

// ── Template generation ───────────────────────────────────────────────────────

pub fn scaffold(dir: &Path, lang: Language) -> Result<()> {
    std::fs::create_dir_all(dir)
        .map_err(|e| DevError::Config(format!("create dir: {e}")))?;

    match lang {
        Language::Python => scaffold_python(dir),
        Language::TypeScript => scaffold_typescript(dir),
    }
}

fn scaffold_python(dir: &Path) -> Result<()> {
    write_file(dir, "config.hcl", PYTHON_CONFIG)?;
    std::fs::create_dir_all(dir.join("skills"))
        .map_err(|e| DevError::Config(e.to_string()))?;
    std::fs::create_dir_all(dir.join("agents"))
        .map_err(|e| DevError::Config(e.to_string()))?;
    write_file(dir, "skills/hello.py", PYTHON_SKILL)?;
    write_file(dir, "agents/demo.py", PYTHON_AGENT)?;
    write_file(dir, "requirements.txt", "a3s-code\n")?;
    Ok(())
}

fn scaffold_typescript(dir: &Path) -> Result<()> {
    write_file(dir, "config.hcl", TS_CONFIG)?;
    std::fs::create_dir_all(dir.join("skills"))
        .map_err(|e| DevError::Config(e.to_string()))?;
    std::fs::create_dir_all(dir.join("agents"))
        .map_err(|e| DevError::Config(e.to_string()))?;
    write_file(dir, "skills/hello.ts", TS_SKILL)?;
    write_file(dir, "agents/demo.ts", TS_AGENT)?;
    write_file(dir, "package.json", TS_PACKAGE_JSON)?;
    write_file(dir, "tsconfig.json", TS_CONFIG_JSON)?;
    Ok(())
}

fn write_file(dir: &Path, name: &str, content: &str) -> Result<()> {
    let path = dir.join(name);
    std::fs::write(&path, content)
        .map_err(|e| DevError::Config(format!("write {}: {e}", path.display())))
}

// ── Templates ─────────────────────────────────────────────────────────────────

const PYTHON_CONFIG: &str = r#"# a3s-code agent configuration
agent "demo" {
  model       = "claude-sonnet-4-5"
  max_turns   = 10
  system      = "You are a helpful assistant."

  skills = [
    "./skills/hello.py",
  ]
}
"#;

const PYTHON_SKILL: &str = r#"# skills/hello.py — example a3s-code skill
from a3s_code import skill, ToolResult

@skill(
    name="hello",
    description="Say hello to someone",
    parameters={
        "name": {"type": "string", "description": "Name to greet"},
    },
)
async def hello(name: str) -> ToolResult:
    return ToolResult(content=f"Hello, {name}!")
"#;

const PYTHON_AGENT: &str = r#"# agents/demo.py — example a3s-code agent runner
import asyncio
from a3s_code import Agent

async def main():
    agent = Agent.from_config("config.hcl", agent="demo")
    async with agent.session() as session:
        result = await session.run("Say hello to the world")
        print(result)

if __name__ == "__main__":
    asyncio.run(main())
"#;

const TS_CONFIG: &str = r#"# a3s-code agent configuration
agent "demo" {
  model       = "claude-sonnet-4-5"
  max_turns   = 10
  system      = "You are a helpful assistant."

  skills = [
    "./skills/hello.ts",
  ]
}
"#;

const TS_SKILL: &str = r#"// skills/hello.ts — example a3s-code skill
import { skill, ToolResult } from "@a3s-lab/code";

export const hello = skill({
  name: "hello",
  description: "Say hello to someone",
  parameters: {
    name: { type: "string", description: "Name to greet" },
  },
  async execute({ name }: { name: string }): Promise<ToolResult> {
    return { content: `Hello, ${name}!` };
  },
});
"#;

const TS_AGENT: &str = r#"// agents/demo.ts — example a3s-code agent runner
import { Agent } from "@a3s-lab/code";

async function main() {
  const agent = await Agent.fromConfig("config.hcl", { agent: "demo" });
  const session = await agent.session();
  const result = await session.run("Say hello to the world");
  console.log(result);
  await session.close();
}

main().catch(console.error);
"#;

const TS_PACKAGE_JSON: &str = r#"{
  "name": "my-a3s-agent",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "start": "ts-node agents/demo.ts"
  },
  "dependencies": {
    "@a3s-lab/code": "latest"
  },
  "devDependencies": {
    "typescript": "^5",
    "ts-node": "^10",
    "@types/node": "^20"
  }
}
"#;

const TS_CONFIG_JSON: &str = r#"{
  "compilerOptions": {
    "target": "ES2022",
    "module": "commonjs",
    "strict": true,
    "esModuleInterop": true,
    "outDir": "dist"
  },
  "include": ["agents/**/*", "skills/**/*"]
}
"#;
