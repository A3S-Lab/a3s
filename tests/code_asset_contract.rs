use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct AssetWorkspace {
    _root: tempfile::TempDir,
    launch: PathBuf,
    workspace: PathBuf,
    home: PathBuf,
}

impl AssetWorkspace {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temp directory");
        let launch = root.path().join("launch");
        let workspace = root.path().join("workspace");
        let home = root.path().join("home");
        std::fs::create_dir_all(&launch).expect("launch directory");
        std::fs::create_dir_all(workspace.join(".a3s")).expect("workspace config directory");
        std::fs::create_dir_all(&home).expect("home directory");
        std::fs::write(
            workspace.join(".a3s/config.acl"),
            r#"agent_dir = "acl-assets/agents"
skill_dir = "acl-assets/skills"
mcp_dir = "acl-assets/mcps"
flow_dir = "acl-assets/flows"
"#,
        )
        .expect("asset ACL");

        create_assets(&workspace.join("acl-assets"), "acl");
        create_assets(&workspace.join("env-assets"), "env");
        std::fs::create_dir_all(workspace.join("okf/acl-ops/sources")).expect("OKF sources");
        std::fs::write(
            workspace.join("okf/acl-ops/README.md"),
            "# ACL Operations\n",
        )
        .expect("OKF asset");

        Self {
            _root: root,
            launch,
            workspace,
            home,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new(a3s_binary());
        command
            .current_dir(&self.launch)
            .env("HOME", &self.home)
            .env_remove("A3S_CONFIG_FILE")
            .arg("-C")
            .arg(&self.workspace);
        for name in [
            "A3S_AGENT_DIR",
            "A3S_MCP_DIR",
            "A3S_SKILL_DIR",
            "A3S_FLOW_DIR",
        ] {
            command.env_remove(name);
        }
        command
    }

    fn list(
        &self,
        family: &str,
        output: Option<&str>,
        environment_override: Option<(&str, &str)>,
    ) -> Output {
        let mut command = self.command();
        if let Some(output) = output {
            command.args(["--output", output]);
        }
        if let Some((name, value)) = environment_override {
            command.env(name, value);
        }
        command
            .args(["code", family, "list", "--location", "local"])
            .output()
            .expect("list local Code assets")
    }

    fn canonical_workspace(&self) -> PathBuf {
        self.workspace.canonicalize().expect("canonical workspace")
    }
}

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

fn create_assets(base: &Path, prefix: &str) {
    let agent = base.join(format!("agents/{prefix}-reviewer/agent.md"));
    let mcp = base.join(format!("mcps/{prefix}-weather/server.py"));
    let skill = base.join(format!("skills/{prefix}-auditor/SKILL.md"));
    let flow = base.join(format!("flows/{prefix}-daily/flow.json"));
    for path in [&agent, &mcp, &skill, &flow] {
        std::fs::create_dir_all(path.parent().unwrap()).expect("asset directory");
    }
    std::fs::write(
        agent,
        format!("---\nname: {prefix}-reviewer\ndescription: Review changes\n---\nReview.\n"),
    )
    .expect("agent asset");
    std::fs::write(mcp, "# MCP fixture\n").expect("MCP asset");
    std::fs::write(
        skill,
        format!("---\nname: {prefix}-auditor\ndescription: Audit changes\n---\nAudit.\n"),
    )
    .expect("skill asset");
    std::fs::write(flow, "{}\n").expect("flow asset");
}

#[test]
fn asset_commands_resolve_acl_and_environment_roots_from_the_effective_directory() {
    let fixture = AssetWorkspace::new();
    let process_directory = std::env::current_dir().expect("process directory");
    let canonical_workspace = fixture.canonical_workspace();

    for (family, relative_root, marker) in [
        ("agent", "acl-assets/agents", "acl-reviewer"),
        ("mcp", "acl-assets/mcps", "acl-weather"),
        ("skill", "acl-assets/skills", "acl-auditor"),
        ("flow", "acl-assets/flows", "acl-daily/flow.json"),
        ("okf", "okf", "acl-ops"),
    ] {
        let output = fixture.list(family, None, None);
        assert!(
            output.status.success(),
            "{family} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(marker), "{family} stdout: {stdout}");
        assert!(
            stdout.contains(
                &canonical_workspace
                    .join(relative_root)
                    .display()
                    .to_string()
            ),
            "{family} stdout: {stdout}"
        );
    }

    for (family, variable, relative_root, marker, acl_marker) in [
        (
            "agent",
            "A3S_AGENT_DIR",
            "env-assets/agents",
            "env-reviewer",
            "acl-reviewer",
        ),
        (
            "mcp",
            "A3S_MCP_DIR",
            "env-assets/mcps",
            "env-weather",
            "acl-weather",
        ),
        (
            "skill",
            "A3S_SKILL_DIR",
            "env-assets/skills",
            "env-auditor",
            "acl-auditor",
        ),
        (
            "flow",
            "A3S_FLOW_DIR",
            "env-assets/flows",
            "env-daily/flow.json",
            "acl-daily/flow.json",
        ),
    ] {
        let output = fixture.list(family, None, Some((variable, relative_root)));
        assert!(
            output.status.success(),
            "{family} override stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains(marker), "{family} stdout: {stdout}");
        assert!(!stdout.contains(acl_marker), "{family} stdout: {stdout}");
        assert!(
            stdout.contains(
                &canonical_workspace
                    .join(relative_root)
                    .display()
                    .to_string()
            ),
            "{family} stdout: {stdout}"
        );
    }

    let paths = fixture
        .command()
        .args(["--output", "json", "config", "paths"])
        .output()
        .expect("show effective asset paths");
    assert!(
        paths.status.success(),
        "paths stderr: {}",
        String::from_utf8_lossy(&paths.stderr)
    );
    let paths: serde_json::Value = serde_json::from_slice(&paths.stdout).expect("paths JSON");
    assert_eq!(
        paths["data"]["assets"]["mcp"],
        canonical_workspace
            .join("acl-assets/mcps")
            .display()
            .to_string()
    );
    assert_eq!(
        paths["data"]["assets"]["flow"],
        canonical_workspace
            .join("acl-assets/flows")
            .display()
            .to_string()
    );
    assert_eq!(
        std::env::current_dir().expect("process directory after commands"),
        process_directory
    );
}

#[test]
fn asset_commands_emit_one_structured_document_for_local_discovery_and_review() {
    let fixture = AssetWorkspace::new();
    let canonical_workspace = fixture.canonical_workspace();

    for (family, command_name, relative_root, marker) in [
        (
            "agent",
            "code.agent.list",
            "acl-assets/agents",
            "acl-reviewer",
        ),
        ("mcp", "code.mcp.list", "acl-assets/mcps", "acl-weather"),
        (
            "skill",
            "code.skill.list",
            "acl-assets/skills",
            "acl-auditor",
        ),
        (
            "flow",
            "code.flow.list",
            "acl-assets/flows",
            "acl-daily/flow.json",
        ),
        ("okf", "code.okf.list", "okf", "acl-ops"),
    ] {
        let output = fixture.list(family, Some("json"), None);
        assert!(
            output.status.success(),
            "{family} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty(), "{family} wrote machine stderr");
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("asset JSON document");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["command"], command_name);
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["family"], family);
        assert_eq!(value["data"]["location"], "local");
        assert_eq!(
            value["data"]["root"],
            canonical_workspace
                .join(relative_root)
                .display()
                .to_string()
        );
        assert!(
            value["data"]["assets"].to_string().contains(marker),
            "{family} JSON: {value:#}"
        );
    }

    let review = fixture
        .command()
        .args([
            "--output",
            "json",
            "code",
            "agent",
            "review",
            "acl-assets/agents/acl-reviewer",
        ])
        .output()
        .expect("review local Agent asset");
    assert!(
        review.status.success(),
        "review stderr: {}",
        String::from_utf8_lossy(&review.stderr)
    );
    let review: serde_json::Value = serde_json::from_slice(&review.stdout).expect("review JSON");
    assert_eq!(review["command"], "code.agent.review");
    assert_eq!(review["data"]["family"], "agent");
    assert!(review["data"]["path"]
        .as_str()
        .unwrap()
        .ends_with("acl-reviewer/agent.md"));
    assert!(review["data"]["prompt"]
        .as_str()
        .unwrap()
        .contains("acl-reviewer"));

    let jsonl = fixture.list("agent", Some("jsonl"), None);
    assert_eq!(jsonl.status.code(), Some(2));
    assert!(jsonl.stderr.is_empty());
    let jsonl: serde_json::Value = serde_json::from_slice(&jsonl.stdout).expect("JSONL error");
    assert_eq!(jsonl["command"], "code.agent.list");
    assert_eq!(jsonl["type"], "error");
    assert_eq!(jsonl["error"]["code"], "usage.invalid");
}

#[test]
fn asset_roots_accept_core_plural_scan_directory_keys() {
    let fixture = AssetWorkspace::new();
    std::fs::write(
        fixture.workspace.join(".a3s/config.acl"),
        r#"agent_dirs = ["acl-assets/agents"]
skill_dirs = ["acl-assets/skills"]
"#,
    )
    .expect("plural asset ACL");

    for (family, marker) in [("agent", "acl-reviewer"), ("skill", "acl-auditor")] {
        let output = fixture.list(family, Some("json"), None);
        assert!(
            output.status.success(),
            "{family} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("asset JSON document");
        assert!(
            value["data"]["assets"].to_string().contains(marker),
            "{family} JSON: {value:#}"
        );
    }

    std::fs::write(
        fixture.workspace.join(".a3s/config.acl"),
        "agent_dirs = []\n",
    )
    .expect("empty plural asset ACL");
    let output = fixture.list("agent", Some("json"), None);
    assert!(output.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("fallback asset JSON");
    assert_eq!(
        value["data"]["root"],
        fixture.home.join(".a3s/agents").display().to_string()
    );
}

#[test]
fn asset_clone_rejects_secret_bearing_git_urls_without_echoing_them() {
    let fixture = AssetWorkspace::new();
    let secret_url = "https://secret-token@github.com/acme/private-agent.git";
    let output = fixture
        .command()
        .args(["--output", "json", "code", "agent", "clone", secret_url])
        .output()
        .expect("reject secret-bearing clone URL");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let rendered = String::from_utf8(output.stdout).expect("clone error UTF-8");
    assert!(
        !rendered.contains(secret_url),
        "secret URL leaked: {rendered}"
    );
    assert!(
        !rendered.contains("secret-token"),
        "secret leaked: {rendered}"
    );
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("clone error JSON");
    assert_eq!(value["command"], "code.agent.clone");
    assert_eq!(value["error"]["code"], "operation.failed");
    assert_eq!(
        value["error"]["message"],
        "git URL must not contain credentials"
    );
}

#[cfg(unix)]
#[test]
fn asset_requests_preserve_non_utf8_paths_across_the_clap_boundary() {
    use std::ffi::OsString;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let fixture = AssetWorkspace::new();
    let package_name = OsString::from_vec(b"native-path-\xff".to_vec());
    let package = fixture
        .workspace
        .join("acl-assets/agents")
        .join(package_name);
    let directory = std::fs::create_dir_all(&package);
    if let Err(error) = directory {
        let output = fixture
            .command()
            .args(["--output", "json", "code", "agent", "review"])
            .arg(&package)
            .output()
            .expect("pass a non-UTF-8 path through Clap");

        assert_eq!(output.status.code(), Some(1), "filesystem error: {error}");
        assert!(output.stderr.is_empty());
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("structured filesystem error");
        assert_eq!(value["command"], "code.agent.review");
        assert_eq!(value["error"]["code"], "operation.failed");
        assert!(
            value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("could not resolve"),
            "{value:#}"
        );
        assert!(
            !value["error"]["message"]
                .as_str()
                .unwrap()
                .contains("valid UTF-8"),
            "{value:#}"
        );
        return;
    }
    std::fs::write(
        package.join("agent.md"),
        "---\nname: native-path-reviewer\ndescription: Review native paths\n---\nReview.\n",
    )
    .expect("non-UTF-8 Agent asset");

    let output = fixture
        .command()
        .args(["--output", "json", "code", "agent", "review"])
        .arg(&package)
        .output()
        .expect("review Agent asset at a non-UTF-8 path");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let value: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("review JSON document");
    assert_eq!(value["command"], "code.agent.review");
    assert_eq!(value["data"]["path"]["encoding"], "unix-bytes-hex");
    assert_eq!(
        value["data"]["path"]["value"],
        package
            .join("agent.md")
            .as_os_str()
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert!(value["data"]["prompt"]
        .as_str()
        .unwrap()
        .contains("native-path-reviewer"));
}
