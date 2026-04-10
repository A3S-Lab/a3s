//! Tests for the deployment module.

#[cfg(test)]
mod tests {
    use crate::deployment::provider::ProviderType;
    use std::path::PathBuf;

    /// Test ProviderType parsing.
    #[test]
    fn test_provider_type_parsing() {
        let microvm: ProviderType = "microvm".parse().unwrap();
        assert_eq!(microvm, ProviderType::Microvm);

        let firecracker: ProviderType = "firecracker".parse().unwrap();
        assert_eq!(firecracker, ProviderType::Microvm);

        let micro_vm: ProviderType = "micro-vm".parse().unwrap();
        assert_eq!(micro_vm, ProviderType::Microvm);

        let invalid: Result<ProviderType, _> = "docker".parse();
        assert!(invalid.is_err());
    }

    /// Test ProviderType Display.
    #[test]
    fn test_provider_type_display() {
        assert_eq!(format!("{}", ProviderType::Microvm), "microvm");
    }

    /// Test DeploymentConfig creation.
    #[test]
    fn test_deployment_config() {
        use crate::deployment::provider::DeploymentConfig;

        let config = DeploymentConfig::new(
            ProviderType::Microvm,
            "my-project".to_string(),
            PathBuf::from("/tmp"),
        );

        assert_eq!(config.project_name, "my-project");
        assert_eq!(config.provider, ProviderType::Microvm);
    }
}
