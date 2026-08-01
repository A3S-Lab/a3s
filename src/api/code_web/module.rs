use std::sync::Arc;

use a3s_boot::{Module, ProviderDefinition, ProviderToken, Result as BootResult};

use super::capabilities::CapabilitiesModule;
use super::code_intelligence::CodeIntelligenceModule;
use super::config::ConfigModule;
use super::context::ContextModule;
use super::evolution::EvolutionModule;
use super::health::HealthModule;
use super::kernel::KernelModule;
use super::knowledge::KnowledgeModule;
use super::loops::LoopsModule;
use super::os::OsModule;
use super::plugins::PluginsModule;
use super::previews::PreviewsModule;
use super::processes::ProcessesModule;
use super::state::CodeWebState;
use super::weixin::WeixinModule;
use super::work::WorkModule;
use super::workspace::WorkspaceModule;

pub(in crate::api) struct CodeWebModule {
    state: Arc<CodeWebState>,
}

impl CodeWebModule {
    pub(in crate::api) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }
}

impl Module for CodeWebModule {
    fn name(&self) -> &'static str {
        "a3s-code-web"
    }

    fn imports(&self) -> Vec<Arc<dyn Module>> {
        vec![
            Arc::new(CodeWebStateModule::new(Arc::clone(&self.state))),
            Arc::new(HealthModule),
            Arc::new(ConfigModule),
            Arc::new(WorkModule),
            Arc::new(WorkspaceModule),
            Arc::new(CodeIntelligenceModule),
            Arc::new(CapabilitiesModule),
            Arc::new(KnowledgeModule),
            Arc::new(ContextModule),
            Arc::new(EvolutionModule),
            Arc::new(KernelModule),
            Arc::new(ProcessesModule),
            Arc::new(PreviewsModule),
            Arc::new(LoopsModule),
            Arc::new(PluginsModule),
            Arc::new(OsModule),
            Arc::new(WeixinModule::configured()),
        ]
    }
}

struct CodeWebStateModule {
    state: Arc<CodeWebState>,
}

impl CodeWebStateModule {
    fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }
}

impl Module for CodeWebStateModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-state"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::from_arc(Arc::clone(&self.state))])
    }

    fn exports(&self) -> BootResult<Vec<ProviderToken>> {
        Ok(vec![ProviderToken::of::<CodeWebState>()])
    }

    fn is_global(&self) -> bool {
        true
    }

    fn on_application_shutdown(
        &self,
        _module_ref: a3s_boot::ModuleRef,
    ) -> a3s_boot::BoxFuture<'static, BootResult<()>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state.close().await;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use a3s_boot::{BootApplication, BootRequest, HttpMethod};

    use super::*;

    #[tokio::test]
    async fn complete_code_web_module_builds_with_nested_remote_kernel_imports() {
        let temporary = tempfile::tempdir().expect("create Code Web module fixture");
        let workspace = temporary.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create fixture workspace");
        let code_config = a3s_code_core::CodeConfig::from_acl(
            r#"
                default_model = "openai/test-model"
                providers "openai" {
                  apiKey = "sk-test"
                  baseUrl = "https://example.com/v1"
                  models "test-model" {}
                }
            "#,
        )
        .expect("parse fixture config");
        let agent = Arc::new(
            a3s_code_core::Agent::from_config(code_config.clone())
                .await
                .expect("create fixture agent"),
        );
        let repository = Arc::new(
            super::super::session_store::CodeWebSessionRepository::open(
                temporary.path().join("sessions"),
            )
            .await
            .expect("open fixture session repository"),
        );
        let state = Arc::new(CodeWebState::new(
            agent,
            temporary.path().join("config.acl"),
            workspace,
            code_config,
            repository,
        ));
        let app = BootApplication::builder()
            .global_prefix("/api")
            .import(CodeWebModule::new(Arc::clone(&state)))
            .build()
            .expect("build complete Code Web application");

        let capability = app
            .call(BootRequest::new(
                HttpMethod::Get,
                "/api/v1/weixin/capability",
            ))
            .await
            .expect("read built-in Weixin capability")
            .body_json::<serde_json::Value>()
            .expect("decode capability");
        assert_eq!(capability["state"], "unbound");
        assert_eq!(capability["protocolMode"], "tencent");
        assert_eq!(capability["schemaVersion"], 2);
        assert_eq!(capability["releaseBlockers"], serde_json::json!([]));

        let office = app
            .call(BootRequest::new(
                HttpMethod::Get,
                "/api/v1/capabilities/office",
            ))
            .await
            .expect("read Office automation capability")
            .body_json::<serde_json::Value>()
            .expect("decode Office automation capability");
        assert_eq!(office["schemaVersion"], 1);
        assert_eq!(office["status"], "preparing");
        assert_eq!(office["route"], "use/office");
        assert_eq!(office["cli"]["name"], "a3s-office");
        assert_eq!(office["skill"]["name"], "a3s-office");
        assert_eq!(office["editors"]["office"], true);
        assert_eq!(office["editors"]["code"], true);

        let targets = app
            .call(BootRequest::new(HttpMethod::Get, "/api/v1/weixin/targets"))
            .await
            .expect("read remote target snapshot")
            .body_json::<serde_json::Value>()
            .expect("decode target snapshot");
        assert_eq!(targets["schemaVersion"], 1);
        assert!(
            targets["items"].is_array(),
            "system-agent discovery is host-dependent but must return an item array"
        );
        assert_eq!(targets["warnings"], serde_json::json!([]));

        app.shutdown().await.expect("shutdown Code Web application");
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn complete_code_web_module_honors_explicit_weixin_enable() {
        let temporary = tempfile::tempdir().expect("create Code Web module fixture");
        let workspace = temporary.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create fixture workspace");
        let source = r#"
            default_model = "openai/test-model"
            providers "openai" {
              apiKey = "sk-test"
              baseUrl = "https://example.com/v1"
              models "test-model" {}
            }
            channels {
              weixin {
                enabled = true
              }
            }
        "#;
        let config_path = temporary.path().join("config.acl");
        std::fs::write(&config_path, source).expect("write configured fixture");
        let code_config =
            a3s_code_core::CodeConfig::from_acl(source).expect("parse configured fixture");
        let agent = Arc::new(
            a3s_code_core::Agent::from_config(code_config.clone())
                .await
                .expect("create fixture agent"),
        );
        let repository = Arc::new(
            super::super::session_store::CodeWebSessionRepository::open(
                temporary.path().join("sessions"),
            )
            .await
            .expect("open fixture session repository"),
        );
        let state = Arc::new(CodeWebState::new(
            agent,
            config_path,
            workspace,
            code_config,
            repository,
        ));
        let app = BootApplication::builder()
            .global_prefix("/api")
            .import(CodeWebModule::new(state))
            .build()
            .expect("build configured Code Web application");

        app.bootstrap()
            .await
            .expect("bootstrap configured Code Web application");
        let capability = app
            .call(BootRequest::new(
                HttpMethod::Get,
                "/api/v1/weixin/capability",
            ))
            .await
            .expect("read configured Weixin capability")
            .body_json::<serde_json::Value>()
            .expect("decode configured capability");
        assert_eq!(capability["state"], "unbound");
        assert_eq!(capability["protocolMode"], "tencent");
        assert_eq!(capability["schemaVersion"], 2);
        assert_eq!(capability["releaseBlockers"], serde_json::json!([]));

        let account = app
            .call(BootRequest::new(HttpMethod::Get, "/api/v1/weixin/account"))
            .await
            .expect("read configured Weixin account")
            .body_json::<serde_json::Value>()
            .expect("decode configured account");
        assert_eq!(account["bound"], false);
        assert_eq!(account["protocolMode"], "tencent");

        app.shutdown()
            .await
            .expect("shutdown configured Code Web application");
    }
}
