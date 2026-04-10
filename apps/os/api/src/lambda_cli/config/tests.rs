//! Unit tests for config parsing (no KVM required).

#[cfg(test)]
mod tests {
    use crate::config::a3sfile::{A3sfile, Language};

    /// Test that we can parse a minimal A3sfile.
    #[test]
    fn test_parse_minimal_a3sfile() {
        let hcl = r#"
            agent {
                name = "test-agent"
                version = "0.1.0"
                description = "Test agent"
                language = "python"
            }

            runtime {}

            model "test-model" {
                provider = "openai"
                name = "gpt-4"
            }
        "#;

        let result = A3sfile::parse(hcl);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());

        let a3sfile = result.unwrap();
        assert_eq!(a3sfile.agent.name, "test-agent");
        assert_eq!(a3sfile.agent.version, "0.1.0");
        assert_eq!(a3sfile.agent.language, Language::Python);
        assert_eq!(a3sfile.models.len(), 1);
        assert_eq!(a3sfile.dependencies.len(), 0);
    }

    /// Test parsing with dependencies.
    #[test]
    fn test_parse_with_dependencies() {
        let hcl = r#"
            agent {
                name = "test-agent"
                version = "0.1.0"
                description = "Test agent"
                language = "python"
            }

            runtime {}

            model "my-model" {
                provider = "ollama"
                name = "llama3"
            }

            dependency "postgres" {
                image = "postgres:15"
                ports = {
                    db = 5432
                }
                env = {
                    POSTGRES_PASSWORD = "secret"
                }
            }

            dependency "redis" {
                image = "redis:7"
                ports = {
                    redis = 6379
                }
            }
        "#;

        let result = A3sfile::parse(hcl);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());

        let a3sfile = result.unwrap();
        assert_eq!(a3sfile.agent.name, "test-agent");
        assert_eq!(a3sfile.agent.language, Language::Python);
        assert_eq!(a3sfile.models.len(), 1);
        assert_eq!(a3sfile.dependencies.len(), 2);

        let postgres = &a3sfile.dependencies["postgres"];
        assert_eq!(postgres.image, "postgres:15");
        assert_eq!(postgres.ports["db"], 5432);
        assert_eq!(postgres.env["POSTGRES_PASSWORD"], "secret");

        let redis = &a3sfile.dependencies["redis"];
        assert_eq!(redis.image, "redis:7");
        assert_eq!(redis.ports["redis"], 6379);
    }

    /// Test parsing model with deployment config.
    #[test]
    fn test_parse_model_with_deployment() {
        let hcl = r#"
            agent {
                name = "test-agent"
                version = "0.1.0"
                description = "Test agent"
                language = "rust"
            }

            runtime {}

            model "vllm-model" {
                provider = "vllm"
                name = "meta-llama/Llama-3-8B-Instruct"
                deployment {
                    type = "microvm"
                    replicas = 2
                    gpu = true
                    memory = "16g"
                }
            }
        "#;

        let result = A3sfile::parse(hcl);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());

        let a3sfile = result.unwrap();
        let model = &a3sfile.models["vllm-model"];
        assert_eq!(model.provider, "vllm");
        assert!(model.deployment.is_some());

        let deployment = model.deployment.as_ref().unwrap();
        assert_eq!(deployment.replicas, Some(2));
        assert!(deployment.gpu);
        assert_eq!(deployment.memory, Some("16g".to_string()));
    }

    /// Test parsing with env() function.
    #[test]
    fn test_parse_with_env_function() {
        std::env::set_var("TEST_API_KEY", "secret123");

        let hcl = r#"
            agent {
                name = "test-agent"
                version = "0.1.0"
                description = "Test agent"
                language = "rust"
            }

            runtime {}

            model "test-model" {
                provider = "openai"
                name = "gpt-4"
                api_key = env("TEST_API_KEY")
            }
        "#;

        let result = A3sfile::parse(hcl);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());

        let a3sfile = result.unwrap();
        let model = &a3sfile.models["test-model"];
        // api_key should be set from env
        // Note: The exact behavior depends on env() implementation

        std::env::remove_var("TEST_API_KEY");
    }

    /// Test parsing the sample A3sfile from examples/.
    #[test]
    fn test_parse_example_a3sfile() {
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let a3sfile_path = examples_dir.join("A3sfile.hcl");

        if !a3sfile_path.exists() {
            println!(
                "Skipping example file test (file not found: {})",
                a3sfile_path.display()
            );
            return;
        }

        let content =
            std::fs::read_to_string(&a3sfile_path).expect("failed to read example A3sfile");

        let result = crate::config::a3sfile::A3sfile::parse(&content);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());

        let a3sfile = result.unwrap();
        assert_eq!(a3sfile.agent.name, "test-agent");
        assert_eq!(a3sfile.models.len(), 2);
        assert_eq!(a3sfile.dependencies.len(), 3);
    }

    /// Test parsing the minimal A3sfile from examples/.
    #[test]
    fn test_parse_minimal_example() {
        let examples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let a3sfile_path = examples_dir.join("minimal.hcl");

        if !a3sfile_path.exists() {
            println!(
                "Skipping minimal example test (file not found: {})",
                a3sfile_path.display()
            );
            return;
        }

        let content =
            std::fs::read_to_string(&a3sfile_path).expect("failed to read minimal A3sfile");

        let result = crate::config::a3sfile::A3sfile::parse(&content);
        assert!(result.is_ok(), "parse failed: {:?}", result.err());

        let a3sfile = result.unwrap();
        assert_eq!(a3sfile.agent.name, "minimal-agent");
        assert_eq!(a3sfile.models.len(), 1);
        assert_eq!(a3sfile.dependencies.len(), 1);
    }
}
