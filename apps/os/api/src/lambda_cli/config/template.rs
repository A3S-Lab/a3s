//! Template generator for a3s init command.
//!
//! Generates agent project scaffold from A3sfile.hcl template.

use crate::config::a3sfile::{A3sfile, Language};
use crate::errors::Result;
use std::path::Path;

/// Template variables for agent project generation.
#[derive(Debug, Clone)]
pub struct TemplateVars {
    pub agent_name: String,
    pub agent_version: String,
    pub agent_description: String,
    pub language: Language,
    pub runtime_image: String,
    pub runtime_workdir: String,
    pub models: Vec<ModelTemplateVar>,
    pub dependencies: Vec<DependencyTemplateVar>,
    pub env_vars: Vec<String>,
    pub skills: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ModelTemplateVar {
    pub name: String,
    pub provider: String,
    pub endpoint: String,
    pub env_prefix: String,
}

#[derive(Debug, Clone)]
pub struct DependencyTemplateVar {
    pub name: String,
    pub host: String,
    pub port: u16,
}

impl TemplateVars {
    pub fn from_a3sfile(a3sfile: &A3sfile) -> Self {
        let mut models = Vec::new();
        let mut env_vars = Vec::new();

        for (name, model) in &a3sfile.models {
            let env_prefix = model
                .env_prefix
                .clone()
                .unwrap_or_else(|| format!("{}_", name.to_uppercase()));

            let endpoint = model
                .endpoint
                .clone()
                .unwrap_or_else(|| "http://localhost".to_string());

            models.push(ModelTemplateVar {
                name: name.clone(),
                provider: model.provider.clone(),
                endpoint: endpoint.clone(),
                env_prefix: env_prefix.clone(),
            });

            // Add endpoint env var
            env_vars.push(format!("{}API_URL={}", env_prefix, endpoint));
            env_vars.push(format!("{}MODEL_NAME={}", env_prefix, model.name));
        }

        let mut dependencies = Vec::new();

        for (name, dep) in &a3sfile.dependencies {
            let port = dep.ports.values().next().copied().unwrap_or(0);
            dependencies.push(DependencyTemplateVar {
                name: name.clone(),
                host: name.clone(), // Docker service name
                port,
            });

            // Add env vars for dependency
            for (key, val) in &dep.env {
                env_vars.push(format!(
                    "{}_{}={}",
                    name.to_uppercase(),
                    key.to_uppercase(),
                    val
                ));
            }
        }

        Self {
            agent_name: a3sfile.agent.name.clone(),
            agent_version: a3sfile.agent.version.clone(),
            agent_description: a3sfile.agent.description.clone(),
            language: a3sfile.agent.language.clone(),
            runtime_image: a3sfile.runtime.image.clone().unwrap_or_default(),
            runtime_workdir: a3sfile
                .runtime
                .workdir
                .clone()
                .unwrap_or_else(|| "/app".to_string()),
            models,
            dependencies,
            env_vars,
            skills: a3sfile.agent.skills.clone(),
        }
    }
}

/// Generate agent project template files.
pub struct TemplateGenerator;

impl TemplateGenerator {
    /// Generate all template files into the target directory.
    pub fn generate(
        vars: &TemplateVars,
        target_dir: &Path,
        overwrite: bool,
    ) -> Result<Vec<GeneratedFile>> {
        let mut generated = Vec::new();

        let base_files = match vars.language {
            Language::Python => Self::python_base_files(vars),
            Language::TypeScript => Self::typescript_base_files(vars),
            Language::Other => Self::python_base_files(vars),
        };

        for file in base_files {
            let path = target_dir.join(&file.path);
            generated.push(file.clone());

            if path.exists() && !overwrite {
                tracing::warn!("skipping existing file: {}", path.display());
                continue;
            }

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::write(&path, file.content.as_bytes())?;
            tracing::info!("generated: {}", path.display());
        }

        Ok(generated)
    }

    fn python_base_files(vars: &TemplateVars) -> Vec<GeneratedFile> {
        vec![
            GeneratedFile {
                path: "A3sfile.hcl".to_string(),
                content: generate_a3sfile_hcl(vars),
            },
            GeneratedFile {
                path: "src/main.py".to_string(),
                content: PYTHON_MAIN_TEMPLATE.to_string(),
            },
            GeneratedFile {
                path: "skills/.gitkeep".to_string(),
                content: String::new(),
            },
            GeneratedFile {
                path: "prompts/.gitkeep".to_string(),
                content: String::new(),
            },
            GeneratedFile {
                path: ".env.example".to_string(),
                content: generate_env_example(vars),
            },
            GeneratedFile {
                path: "requirements.txt".to_string(),
                content: PYTHON_REQUIREMENTS.to_string(),
            },
            GeneratedFile {
                path: "README.md".to_string(),
                content: generate_readme(vars),
            },
        ]
    }

    fn typescript_base_files(vars: &TemplateVars) -> Vec<GeneratedFile> {
        vec![
            GeneratedFile {
                path: "A3sfile.hcl".to_string(),
                content: generate_a3sfile_hcl(vars),
            },
            GeneratedFile {
                path: "src/main.ts".to_string(),
                content: TYPESCRIPT_MAIN_TEMPLATE.to_string(),
            },
            GeneratedFile {
                path: "skills/.gitkeep".to_string(),
                content: String::new(),
            },
            GeneratedFile {
                path: "prompts/.gitkeep".to_string(),
                content: String::new(),
            },
            GeneratedFile {
                path: ".env.example".to_string(),
                content: generate_env_example(vars),
            },
            GeneratedFile {
                path: "package.json".to_string(),
                content: generate_package_json(vars),
            },
            GeneratedFile {
                path: "tsconfig.json".to_string(),
                content: TYPESCRIPT_TSCONFIG.to_string(),
            },
            GeneratedFile {
                path: "README.md".to_string(),
                content: generate_readme(vars),
            },
        ]
    }
}

#[derive(Debug, Clone)]
pub struct GeneratedFile {
    pub path: String,
    pub content: String,
}

fn generate_a3sfile_hcl(vars: &TemplateVars) -> String {
    let mut models_blocks = String::new();
    for model in &vars.models {
        let extra = if !model.endpoint.is_empty() && model.endpoint != "http://localhost" {
            format!("\n    endpoint = \"{}\"", model.endpoint)
        } else {
            String::new()
        };

        let env_prefix_line = if !model.env_prefix.is_empty() {
            format!("\n    env_prefix = \"{}\"", model.env_prefix)
        } else {
            String::new()
        };

        models_blocks.push_str(&format!(
            r#"  {name} = {{
    provider = "{provider}"{env_prefix}{extra}
  }}
"#,
            name = model.name,
            provider = model.provider,
            env_prefix = env_prefix_line,
            extra = extra
        ));
    }

    let mut dep_blocks = String::new();
    for dep in &vars.dependencies {
        dep_blocks.push_str(&format!(
            r#"  {name} = {{
    image = "service:latest"
    ports = {{ "8080" = {port} }}
  }}
"#,
            name = dep.name,
            port = dep.port
        ));
    }

    format!(
        r#"agent {{
  name        = "{name}"
  version     = "{version}"
  description = "{description}"
  language    = "{language}"
  entrypoint  = "src/main.{ext}"
  skills      = []
}}

runtime {{
  image   = "{image}"
  workdir = "{workdir}"
  env     = {{}}
}}

models {{
{model_blocks}
}}

dependencies {{
{dep_blocks}}}"#,
        name = vars.agent_name,
        version = vars.agent_version,
        description = vars.agent_description,
        language = vars.language,
        image = vars.runtime_image,
        workdir = vars.runtime_workdir,
        model_blocks = models_blocks,
        dep_blocks = dep_blocks,
        ext = match vars.language {
            Language::Python => "py",
            _ => "ts",
        }
    )
}

fn generate_env_example(vars: &TemplateVars) -> String {
    let mut lines = vec![
        "# a3s agent environment variables".to_string(),
        "# Copy this file to .env and fill in your values".to_string(),
        String::new(),
    ];

    // Add model env vars
    for model in &vars.models {
        if model.provider == "openai" {
            lines.push(format!("{}API_KEY=sk-your-api-key", model.env_prefix));
        } else if model.provider == "anthropic" {
            lines.push(format!("{}API_KEY=sk-ant-your-api-key", model.env_prefix));
        }
    }

    // Add dependency env vars
    for dep in &vars.env_vars {
        lines.push(format!("# {}", dep));
    }

    lines.join("\n")
}

fn generate_readme(vars: &TemplateVars) -> String {
    format!(
        r#"# {name}

{description}

## Getting Started

1. Copy `.env.example` to `.env` and configure your API keys
2. Install dependencies:

```bash
{}

## Development

Run the agent:

```bash
cargo run -- run
```

## A3sfile.hcl

This project is configured via `A3sfile.hcl`. Key configurations:

- **Runtime**: `{image}`
- **Language**: {language}
- **Dependencies**: {deps}
- **Models**: {models}
"#,
        name = vars.agent_name,
        description = vars.agent_description,
        image = vars.runtime_image,
        language = vars.language,
        deps = vars
            .dependencies
            .iter()
            .map(|d| d.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
        models = vars
            .models
            .iter()
            .map(|m| m.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn generate_package_json(vars: &TemplateVars) -> String {
    format!(
        r#"{{
  "name": "{}",
  "version": "{}",
  "description": "{}",
  "main": "src/main.ts",
  "scripts": {{
    "build": "tsc",
    "start": "node dist/main.js",
    "dev": "ts-node src/main.ts"
  }},
  "dependencies": {{}},
  "devDependencies": {{
    "@types/node": "^20.0.0",
    "ts-node": "^10.9.0",
    "typescript": "^5.0.0"
  }}
}}"#,
        vars.agent_name.replace('-', "_"),
        vars.agent_version,
        vars.agent_description
    )
}

// ============================================================================
// Language-specific templates
// ============================================================================

const PYTHON_MAIN_TEMPLATE: &str = r#"#!/usr/bin/env python3
"""a3s-agent - Agent entry point."""

import os
import json
import sys
from typing import Any


def main() -> None:
    """Main agent entry point."""
    print("a3s-agent starting...")

    # Load input from environment or stdin
    input_data = os.environ.get("A3S_INPUT", "{}")

    try:
        input_json = json.loads(input_data)
    except json.JSONDecodeError as e:
        print(f"Failed to parse input: {e}", file=sys.stderr)
        sys.exit(1)

    # TODO: Implement agent logic here
    result = {
        "status": "ok",
        "input": input_json,
        "message": "Agent executed successfully"
    }

    # Output result to stdout
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
"#;

const TYPESCRIPT_MAIN_TEMPLATE: &str = r#"#!/usr/bin/env node
/**
 * a3s-agent - Agent entry point.
 */

interface AgentInput {
  // Define your input schema here
  [key: string]: unknown;
}

interface AgentResult {
  status: string;
  input: AgentInput;
  message: string;
}

async function main(): Promise<void> {
  console.log("a3s-agent starting...");

  // Load input from environment or stdin
  const inputData = process.env.A3S_INPUT || "{}";

  let inputJson: AgentInput;
  try {
    inputJson = JSON.parse(inputData);
  } catch (e) {
    console.error(`Failed to parse input: ${e}`);
    process.exit(1);
  }

  // TODO: Implement agent logic here
  const result: AgentResult = {
    status: "ok",
    input: inputJson,
    message: "Agent executed successfully",
  };

  // Output result to stdout
  console.log(JSON.stringify(result, null, 2));
}

main().catch((e) => {
  console.error(`Agent error: ${e}`);
  process.exit(1);
});
"#;

const TYPESCRIPT_TSCONFIG: &str = r#"{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
"#;

const PYTHON_REQUIREMENTS: &str = r#"# a3s-agent dependencies
# Add your Python dependencies here

aiohttp>=3.9.0
pydantic>=2.0.0
"#;
