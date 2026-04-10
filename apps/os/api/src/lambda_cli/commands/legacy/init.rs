//! `a3s init` command - Initialize a new agent project.

use crate::commands::Command;
use crate::config::a3sfile::{A3sfile, Agent, Language, Model, Runtime};
use crate::config::template::{TemplateGenerator, TemplateVars};
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

/// Initialize a new agent project.
#[derive(clap::Parser, Debug)]
pub struct InitCommand {
    /// Project name (default: current directory name).
    #[arg(short, long)]
    name: Option<String>,

    /// Language: python or typescript.
    #[arg(short, long, value_parser = ["python", "typescript", "py", "ts"])]
    language: Option<String>,

    /// Target directory (default: current directory).
    #[arg(short, long)]
    directory: Option<PathBuf>,

    /// Skip interactive prompts and use defaults.
    #[arg(long)]
    non_interactive: bool,

    /// Force overwrite existing files.
    #[arg(short, long)]
    force: bool,

    /// Initialize git repository.
    #[arg(long)]
    git: bool,

    /// OpenAI API key (optional).
    #[arg(long)]
    openai_key: Option<String>,
}

impl InitCommand {
    /// Run the init command.
    pub async fn execute(&self) -> Result<()> {
        // Determine target directory
        let target_dir = self
            .directory
            .clone()
            .unwrap_or_else(|| std::env::current_dir().expect("failed to get current directory"));

        let agent_name = self.name.clone().unwrap_or_else(|| {
            target_dir
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        });

        // Determine language
        let language = match self.language.as_deref() {
            Some("python" | "py") => Language::Python,
            Some("typescript" | "ts") => Language::TypeScript,
            None => {
                if self.non_interactive {
                    Language::Python
                } else {
                    Self::prompt_language()?
                }
            }
            _ => return Err(A3sError::InvalidArgument("invalid language".to_string())),
        };

        // Build A3sfile configuration
        let a3sfile = self.build_a3sfile(&agent_name, language.clone())?;

        // Generate template vars
        let vars = TemplateVars::from_a3sfile(&a3sfile);

        // Generate project files
        let generated = TemplateGenerator::generate(&vars, &target_dir, self.force)?;

        println!(
            "\n✓ Generated {} files in {}",
            generated.len(),
            target_dir.display()
        );

        // Initialize git if requested
        if self.git {
            Self::init_git(&target_dir)?;
        }

        println!(
            "\nAgent '{}' created successfully!\n\nTo get started:",
            agent_name
        );
        println!("  cd {}", target_dir.display());
        println!("  cp .env.example .env");
        println!("  # Edit .env with your API keys");
        println!("  a3s agent run\n");

        Ok(())
    }

    fn build_a3sfile(&self, name: &str, language: Language) -> Result<A3sfile> {
        let runtime_image = if self.non_interactive {
            "a3s/agent-runtime:latest".to_string()
        } else {
            Self::prompt_runtime_image()?
        };

        let agent = Agent {
            name: name.to_string(),
            version: "0.1.0".to_string(),
            description: format!("{} agent", name),
            language,
            entrypoint: None,
            skills: vec![],
        };

        let runtime = Runtime {
            image: Some(runtime_image),
            workdir: Some("/app".to_string()),
            env: HashMap::new(),
            resources: None,
        };

        let mut models: HashMap<String, Model> = HashMap::new();

        // Add primary model if API key provided
        if self.openai_key.is_some() {
            let api_key = format!("env(\"OPENAI_API_KEY\")");
            models.insert(
                "primary".to_string(),
                Model {
                    provider: "openai".to_string(),
                    name: "gpt-4o".to_string(),
                    api_key: Some(api_key),
                    repository: None,
                    deployment: None,
                    env_prefix: Some("OPENAI_".to_string()),
                    endpoint: None,
                    extra: HashMap::new(),
                },
            );
        }

        let dependencies: HashMap<String, crate::config::a3sfile::Dependency> = HashMap::new();

        Ok(A3sfile {
            agent,
            runtime,
            models,
            dependencies,
        })
    }

    fn prompt_language() -> Result<Language> {
        print!("Select language [python/typescript] (python): ");
        std::io::stdout().flush().map_err(|e| A3sError::Io(e))?;

        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| A3sError::Io(e))?;

        match input.trim().to_lowercase().as_str() {
            "typescript" | "ts" => Ok(Language::TypeScript),
            _ => Ok(Language::Python),
        }
    }

    fn prompt_runtime_image() -> Result<String> {
        println!("Runtime image [a3s/agent-runtime:latest]: ");
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .map_err(|e| A3sError::Io(e))?;

        let image = input.trim();
        if image.is_empty() {
            Ok("a3s/agent-runtime:latest".to_string())
        } else {
            Ok(image.to_string())
        }
    }

    fn init_git(target_dir: &PathBuf) -> Result<()> {
        use std::process::Command;

        println!("\nInitializing git repository...");

        Command::new("git")
            .args(["init"])
            .current_dir(target_dir)
            .output()
            .map_err(|e| A3sError::Io(e))?;

        // Create .gitignore
        let gitignore = r#"# Python
__pycache__/
*.py[cod]
*$py.class
.venv/
env/
.env

# Node.js
node_modules/
dist/
*.log

# IDE
.vscode/
.idea/

# OS
.DS_Store
Thumbs.db

# Build
target/
build/
*.egg-info/
"#;

        let gitignore_path = target_dir.join(".gitignore");
        std::fs::write(&gitignore_path, gitignore).map_err(|e| A3sError::Io(e))?;

        println!("✓ Git repository initialized");

        Ok(())
    }
}

#[async_trait]
impl Command for InitCommand {
    async fn run(&self) -> crate::errors::Result<()> {
        self.execute().await
    }
}
