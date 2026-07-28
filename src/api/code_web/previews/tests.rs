use std::sync::Arc;

use a3s_boot::{
    BootApplication, BootError, BootRequest, ControllerDefinition, HttpMethod, Module, ModuleRef,
    Result as BootResult,
};
use axum::body::to_bytes;
use axum::http::{header::CONTENT_SECURITY_POLICY, StatusCode};

use super::controller::PreviewsController;
use super::http::serve_content;
use super::model::PreviewKind;
use super::registry::PreviewRegistry;
use super::service::PreviewsService;

#[tokio::test]
async fn registers_static_sites_documents_and_loopback_urls() {
    let temporary = tempfile::tempdir().expect("temporary preview workspace");
    let site = temporary.path().join("site");
    tokio::fs::create_dir_all(&site).await.expect("create site");
    tokio::fs::write(site.join("index.html"), "<h1>Live</h1>")
        .await
        .expect("write index");
    tokio::fs::write(temporary.path().join("notes.md"), "# Notes")
        .await
        .expect("write notes");
    tokio::fs::write(temporary.path().join("icon.svg"), "<svg></svg>")
        .await
        .expect("write SVG");
    tokio::fs::write(temporary.path().join("legacy.doc"), "unsupported")
        .await
        .expect("write legacy document");
    let registry = PreviewRegistry::new(temporary.path().to_path_buf());

    let static_site = registry
        .create("site".to_string())
        .await
        .expect("register static site");
    assert_eq!(static_site.kind, PreviewKind::StaticSite);
    assert!(static_site.content_url.starts_with("/preview/"));
    let canonical_site = tokio::fs::canonicalize(&site)
        .await
        .expect("canonical site path");
    assert_eq!(
        static_site.watch_root.as_deref(),
        Some(canonical_site.to_string_lossy().as_ref())
    );
    assert!(static_site.capabilities.live_reload);
    assert!(static_site.capabilities.responsive);

    let text = registry
        .create("notes.md".to_string())
        .await
        .expect("register text preview");
    assert_eq!(text.kind, PreviewKind::Text);
    assert!(!text.capabilities.responsive);

    let svg = registry
        .create("icon.svg".to_string())
        .await
        .expect("register SVG source preview");
    assert_eq!(svg.kind, PreviewKind::Text);

    let legacy = registry
        .create("legacy.doc".to_string())
        .await
        .expect_err("unsupported legacy Office formats must be rejected");
    assert!(matches!(legacy, BootError::BadRequest(message) if message.contains("not supported")));

    let local = registry
        .create("http://localhost:3000/app#section".to_string())
        .await
        .expect("register local URL");
    assert_eq!(local.kind, PreviewKind::LocalUrl);
    assert_eq!(local.content_url, "http://localhost:3000/app");
    assert!(!local.capabilities.live_reload);
}

#[tokio::test]
async fn expires_preview_descriptors_and_content_at_the_declared_deadline() {
    let workspace = tempfile::tempdir().expect("temporary preview workspace");
    tokio::fs::write(workspace.path().join("notes.md"), "# Notes")
        .await
        .expect("write notes");
    let registry = PreviewRegistry::new(workspace.path().to_path_buf());
    let preview = registry
        .create("notes.md".to_string())
        .await
        .expect("register preview");

    registry.expire_for_test(&preview.id).await;

    assert!(matches!(
        registry.get(&preview.id).await,
        Err(BootError::NotFound(_))
    ));
    assert!(registry.content(&preview.id).await.is_none());
    assert!(matches!(
        registry.remove(&preview.id).await,
        Err(BootError::NotFound(_))
    ));
}

#[tokio::test]
async fn rejects_targets_outside_the_workspace_and_non_loopback_urls() {
    let workspace = tempfile::tempdir().expect("temporary preview workspace");
    let outside = tempfile::NamedTempFile::new().expect("outside file");
    let registry = PreviewRegistry::new(workspace.path().to_path_buf());

    let error = registry
        .create(outside.path().display().to_string())
        .await
        .expect_err("outside path must be rejected");
    assert!(
        matches!(error, BootError::BadRequest(message) if message.contains("active workspace"))
    );

    let error = registry
        .create("https://example.com".to_string())
        .await
        .expect_err("public URL must be rejected");
    assert!(matches!(error, BootError::BadRequest(message) if message.contains("loopback")));
}

#[tokio::test]
async fn serves_assets_with_isolation_headers_and_blocks_sensitive_paths() {
    let workspace = tempfile::tempdir().expect("temporary preview workspace");
    let site = workspace.path().join("site");
    tokio::fs::create_dir_all(&site).await.expect("create site");
    tokio::fs::write(
        site.join("index.html"),
        "<link rel=\"stylesheet\" href=\"style.css\"><h1>Live</h1>",
    )
    .await
    .expect("write index");
    tokio::fs::write(site.join("style.css"), "h1{color:red}")
        .await
        .expect("write stylesheet");
    tokio::fs::write(site.join(".env"), "SECRET=never")
        .await
        .expect("write secret fixture");
    let registry = PreviewRegistry::new(workspace.path().to_path_buf());
    let preview = registry
        .create(site.display().to_string())
        .await
        .expect("register site");

    let index = serve_content(&registry, &preview.id, "").await;
    assert_eq!(index.status(), StatusCode::OK);
    let csp = index
        .headers()
        .get(CONTENT_SECURITY_POLICY)
        .expect("HTML isolation policy")
        .to_str()
        .expect("CSP text");
    assert!(csp.contains("sandbox allow-scripts"));
    assert!(!csp.contains("allow-same-origin"));
    let body = to_bytes(index.into_body(), 1024).await.expect("read index");
    assert!(String::from_utf8_lossy(&body).contains("Live"));

    let stylesheet = serve_content(&registry, &preview.id, "style.css").await;
    assert_eq!(stylesheet.status(), StatusCode::OK);
    assert_eq!(
        stylesheet.headers()["content-type"],
        "text/css; charset=utf-8"
    );

    let hidden = serve_content(&registry, &preview.id, ".env").await;
    assert_eq!(hidden.status(), StatusCode::FORBIDDEN);
    let traversal = serve_content(&registry, &preview.id, "../.env").await;
    assert_eq!(traversal.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
#[cfg(unix)]
async fn blocks_symlinks_that_escape_the_preview_root() {
    use std::os::unix::fs::symlink;

    let workspace = tempfile::tempdir().expect("temporary preview workspace");
    let outside = tempfile::tempdir().expect("outside directory");
    let site = workspace.path().join("site");
    tokio::fs::create_dir_all(&site).await.expect("create site");
    tokio::fs::write(site.join("index.html"), "<h1>Live</h1>")
        .await
        .expect("write index");
    tokio::fs::write(outside.path().join("secret.txt"), "never")
        .await
        .expect("write outside secret");
    symlink(outside.path().join("secret.txt"), site.join("linked.txt")).expect("create symlink");
    let registry = PreviewRegistry::new(workspace.path().to_path_buf());
    let preview = registry
        .create(site.display().to_string())
        .await
        .expect("register site");

    let response = serve_content(&registry, &preview.id, "linked.txt").await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn controller_creates_reads_and_stops_preview_sessions() {
    let workspace = tempfile::tempdir().expect("temporary preview workspace");
    tokio::fs::write(workspace.path().join("index.html"), "<h1>Live</h1>")
        .await
        .expect("write page");
    let service = Arc::new(PreviewsService::new(Arc::new(PreviewRegistry::new(
        workspace.path().to_path_buf(),
    ))));
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(TestPreviewsModule {
            service: Arc::clone(&service),
        })
        .build()
        .expect("build preview test app");

    let created = app
        .call(
            BootRequest::new(HttpMethod::Post, "/api/v1/previews")
                .with_content_type("application/json")
                .with_body(r#"{"target":"index.html"}"#),
        )
        .await
        .expect("create preview through controller")
        .body_json::<serde_json::Value>()
        .expect("decode descriptor");
    assert_eq!(created["kind"], "staticSite");
    assert_eq!(created["source"]["type"], "path");
    assert!(created["source"]["rootPath"].is_string());
    assert!(created["source"]["mtimeMs"].is_number());
    assert_eq!(created["source"]["isDirectory"], false);
    assert_eq!(created["source"]["isBinary"], false);
    assert!(created["source"].get("root_path").is_none());
    let id = created["id"].as_str().expect("preview id");

    let fetched = app
        .call(BootRequest::new(
            HttpMethod::Get,
            format!("/api/v1/previews/{id}"),
        ))
        .await
        .expect("fetch preview");
    assert_eq!(fetched.status(), 200);

    let stopped = app
        .call(BootRequest::new(
            HttpMethod::Delete,
            format!("/api/v1/previews/{id}"),
        ))
        .await
        .expect("stop preview")
        .body_json::<serde_json::Value>()
        .expect("decode stop response");
    assert_eq!(stopped["stopped"], true);
}

struct TestPreviewsModule {
    service: Arc<PreviewsService>,
}

impl Module for TestPreviewsModule {
    fn name(&self) -> &'static str {
        "test-previews"
    }

    fn controllers(&self, _module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        Ok(vec![Arc::new(PreviewsController::new(Arc::clone(
            &self.service,
        )))
        .controller()?])
    }
}
