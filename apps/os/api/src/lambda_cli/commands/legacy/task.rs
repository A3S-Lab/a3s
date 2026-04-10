//! `a3s task` command - Run agent tasks.

use crate::commands::Command;
use crate::config::a3sfile::A3sfile;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

/// Run an agent task.
#[derive(clap::Parser, Debug)]
pub struct TaskCommands {
    #[command(subcommand)]
    pub command: TaskSubcommand,
}

#[derive(clap::Parser, Debug)]
pub enum TaskSubcommand {
    /// Run the agent task.
    Run {
        /// Task input as JSON string.
        #[arg(short, long)]
        input: Option<String>,

        /// Input from file.
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// A3sfile path (default: ./A3sfile.hcl).
        #[arg(short, long)]
        a3sfile: Option<PathBuf>,

        /// Watch task output.
        #[arg(short, long)]
        watch: bool,
    },
    /// List available tasks.
    List,
}

/// Task input structure.
#[derive(serde::Deserialize, serde::Serialize)]
struct TaskInput {
    #[serde(default)]
    task: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
}

impl TaskCommands {
    /// Find A3sfile in current directory or parent directories.
    fn find_a3sfile() -> Option<PathBuf> {
        let current = std::env::current_dir().ok()?;
        for dir in current.ancestors() {
            let a3sfile = dir.join("A3sfile.hcl");
            if a3sfile.exists() {
                return Some(a3sfile);
            }
            let a3sfile_json = dir.join("A3sfile.json");
            if a3sfile_json.exists() {
                return Some(a3sfile_json);
            }
        }
        None
    }

    /// Load A3sfile configuration.
    fn load_a3sfile(path: Option<PathBuf>) -> Result<Option<A3sfile>> {
        let a3sfile_path = if let Some(p) = path {
            p
        } else {
            match Self::find_a3sfile() {
                Some(p) => p,
                None => return Ok(None),
            }
        };

        let content = std::fs::read_to_string(&a3sfile_path).map_err(|e| A3sError::Io(e))?;
        let a3sfile = if a3sfile_path
            .extension()
            .map(|e| e == "json")
            .unwrap_or(false)
        {
            serde_json::from_str(&content).map_err(|e| A3sError::Project(e.to_string()))?
        } else {
            A3sfile::parse(&content).map_err(|e| A3sError::Project(e))?
        };
        Ok(Some(a3sfile))
    }

    /// Execute a task with the given input.
    async fn execute_task(
        input_data: &str,
        a3sfile_path: Option<PathBuf>,
        _watch: bool,
    ) -> Result<()> {
        // Parse input
        let task_input: TaskInput = serde_json::from_str(input_data)
            .map_err(|e| A3sError::Project(format!("invalid task input JSON: {}", e)))?;

        // Load A3sfile if available
        let a3sfile = Self::load_a3sfile(a3sfile_path)?;

        let agent_name = a3sfile
            .as_ref()
            .map(|a| a.agent.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let agent_lang = a3sfile
            .as_ref()
            .map(|a| format!("{:?}", a.agent.language))
            .unwrap_or_else(|| "python".to_string());

        println!("Agent: {}", agent_name);
        println!("Language: {}", agent_lang);
        println!();

        // Display task information
        if let Some(ref task) = task_input.task {
            println!("Task: {}", task);
        } else if let Some(ref prompt) = task_input.prompt {
            println!("Prompt: {}", prompt);
        } else {
            println!("Task: (no task specified)");
        }

        if !task_input.params.is_empty() {
            println!("Parameters:");
            for (key, value) in &task_input.params {
                println!("  {}: {}", key, value);
            }
        }

        println!();

        // Execute task via agent runtime if A3sfile exists
        if let Some(ref a3sfile) = a3sfile {
            let runtime_image = a3sfile
                .runtime
                .image
                .as_deref()
                .unwrap_or("a3s/agent-runtime:latest");

            println!("Executing via {}...", runtime_image);

            // Build environment variables
            let mut env_vars: Vec<(String, String)> = vec![
                ("A3S_TASK_INPUT".to_string(), input_data.to_string()),
                ("A3S_AGENT_NAME".to_string(), agent_name.clone()),
            ];

            // Add model API keys from A3sfile
            for (_name, model) in &a3sfile.models {
                if let Some(ref api_key) = model.api_key {
                    let key_name = format!(
                        "{}_{}",
                        model.env_prefix.as_deref().unwrap_or(""),
                        "API_KEY"
                    );
                    env_vars.push((key_name, api_key.clone()));
                }
            }

            // Try to run via Docker if available
            if Self::docker_is_available() {
                Self::run_via_docker(runtime_image, &env_vars, &agent_name).await?;
            } else {
                println!("(Docker not available - task execution simulated)");
                println!();
                println!("To run the task:");
                println!("  1. Ensure Docker is running");
                println!(
                    "  2. Run: docker run -e A3S_TASK_INPUT='{}' {}",
                    input_data, runtime_image
                );
            }
        } else {
            println!("No A3sfile found - task execution simulated");
            println!();
            println!("Create an A3sfile to enable actual task execution:");
            println!("  a3s init");
        }

        println!();
        println!("Task completed successfully.");

        Ok(())
    }

    /// Check if Docker is available.
    fn docker_is_available() -> bool {
        std::process::Command::new("docker")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Run task via Docker container.
    async fn run_via_docker(
        image: &str,
        env_vars: &[(String, String)],
        agent_name: &str,
    ) -> Result<()> {
        let mut cmd = std::process::Command::new("docker");
        cmd.arg("run")
            .arg("--rm")
            .arg("-e")
            .arg(format!(
                "A3S_TASK_INPUT={}",
                env_vars
                    .iter()
                    .find(|(k, _)| k == "A3S_TASK_INPUT")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("{}")
            ))
            .arg("-e")
            .arg(format!("A3S_AGENT_NAME={}", agent_name));

        // Add any additional environment variables
        for (key, value) in env_vars {
            if key.starts_with("OPENAI_") || key.starts_with("ANTHROPIC_") {
                cmd.arg("-e").arg(format!("{}={}", key, value));
            }
        }

        cmd.arg(image);

        let output = cmd.output().map_err(|e| A3sError::Io(e))?;

        if output.status.success() {
            if !output.stdout.is_empty() {
                print!("{}", String::from_utf8_lossy(&output.stdout));
            }
        } else {
            if !output.stderr.is_empty() {
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
            }
            return Err(A3sError::Project(format!(
                "task failed with exit code: {:?}",
                output.status.code()
            )));
        }

        Ok(())
    }

    pub async fn execute(&self) -> Result<()> {
        match &self.command {
            TaskSubcommand::Run {
                input,
                file,
                a3sfile,
                watch,
            } => {
                let input_data = if let Some(ref f) = file {
                    std::fs::read_to_string(f).map_err(|e| A3sError::Io(e))?
                } else {
                    input.clone().unwrap_or_else(|| "{}".to_string())
                };

                Self::execute_task(&input_data, a3sfile.clone(), *watch).await
            }
            TaskSubcommand::List => {
                println!("Available tasks:");
                println!("  run   - Run the agent task");
                println!();
                println!("Task input format (JSON):");
                println!(r#"  {{"task": "task-name", "params": {{"key": "value"}}}}"#);
                Ok(())
            }
        }
    }
}

#[async_trait]
impl Command for TaskCommands {
    async fn run(&self) -> crate::errors::Result<()> {
        self.execute().await
    }
}
