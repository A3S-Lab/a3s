#![cfg(unix)]

mod support;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use support::{a3s_bin, TempWorkspace};

#[tokio::test]
#[ignore = "hits the real configured OS API; set A3S_REAL_OS_LIFECYCLE=1"]
async fn real_os_asset_lifecycle_smoke() -> Result<(), Box<dyn Error>> {
    if std::env::var("A3S_REAL_OS_LIFECYCLE").ok().as_deref() != Some("1") {
        eprintln!("skipping real OS lifecycle smoke; set A3S_REAL_OS_LIFECYCLE=1");
        return Ok(());
    }

    let tmp = TempWorkspace::new("real-os-lifecycle");
    let root = tmp.path("assets");
    std::fs::create_dir_all(&root)?;
    let suffix = unique_suffix();
    write_lifecycle_fixtures(&root, &suffix)?;
    assert_agent_project_contracts(&root)?;

    let mut remote_assets = Vec::new();
    let scenario = run_real_os_lifecycle(&root, &suffix, &mut remote_assets).await;
    let cleanup = cleanup_real_os_assets(&remote_assets).await;
    let post_cleanup = verify_real_os_cleanup(&suffix);

    cleanup?;
    post_cleanup?;
    scenario?;
    Ok(())
}

async fn run_real_os_lifecycle(
    root: &Path,
    suffix: &str,
    remote_assets: &mut Vec<String>,
) -> Result<(), Box<dyn Error>> {
    let agentic = root.join("agentic/agent.md");
    let tool = root.join("tool/agent.md");
    let app = root.join("app/agent.md");
    let mcp = root.join("mcp");
    let skill = root.join("skill/SKILL.md");
    let flow = root.join("flow/flow.json");
    let okf = root.join("okf");

    remote_assets.push(asset_id(&run_ok(&[
        "code",
        "agent",
        "publish",
        path(&agentic),
        "--kind",
        "agentic",
    ])?)?);
    remote_assets.push(asset_id(&run_ok(&[
        "code",
        "agent",
        "publish",
        path(&tool),
        "--kind",
        "tool",
    ])?)?);
    remote_assets.push(asset_id(&run_ok(&[
        "code",
        "agent",
        "publish",
        path(&app),
        "--kind",
        "application",
    ])?)?);
    remote_assets.push(asset_id(&run_ok(&["code", "mcp", "publish", path(&mcp)])?)?);
    remote_assets.push(asset_id(&run_ok(&[
        "code",
        "skill",
        "publish",
        path(&skill),
    ])?)?);
    remote_assets.push(asset_id(&run_ok(&[
        "code",
        "flow",
        "publish",
        path(&flow),
    ])?)?);
    remote_assets.push(asset_id(&run_ok(&["code", "okf", "publish", path(&okf)])?)?);

    for args in [
        vec![
            "code",
            "agent",
            "status",
            path(&agentic),
            "--kind",
            "agentic",
        ],
        vec!["code", "agent", "status", path(&tool), "--kind", "tool"],
        vec![
            "code",
            "agent",
            "status",
            path(&app),
            "--kind",
            "application",
        ],
        vec!["code", "agent", "run", path(&agentic)],
        vec!["code", "agent", "deploy", path(&app)],
        vec!["code", "agent", "open", path(&agentic), "--kind", "agentic"],
        vec!["code", "agent", "open", path(&tool), "--kind", "tool"],
        vec!["code", "agent", "open", path(&app), "--kind", "application"],
        vec!["code", "agent", "logs", path(&agentic), "--kind", "agentic"],
        vec!["code", "agent", "logs", path(&tool), "--kind", "tool"],
        vec!["code", "agent", "logs", path(&app), "--kind", "application"],
        vec!["code", "mcp", "status", path(&mcp)],
        vec!["code", "mcp", "deploy", path(&mcp)],
        vec!["code", "mcp", "open", path(&mcp)],
        vec!["code", "mcp", "logs", path(&mcp)],
        vec!["code", "skill", "status", path(&skill)],
        vec!["code", "skill", "deploy", path(&skill)],
        vec!["code", "flow", "status", path(&flow)],
        vec!["code", "flow", "deploy", path(&flow)],
        vec!["code", "flow", "run", path(&flow)],
        vec!["code", "flow", "open", path(&flow)],
        vec!["code", "flow", "logs", path(&flow)],
        vec!["code", "okf", "status", path(&okf)],
        vec!["code", "okf", "deploy", path(&okf)],
    ] {
        run_ok(&args)?;
    }

    let skill_open = run_ok(&["code", "skill", "open", path(&skill)])?;
    assert!(
        !skill_open.contains("IssueController_reopenIssue"),
        "skill open must not execute issue reopen capability:\n{skill_open}"
    );

    expect_mcp_runner_result(
        &["code", "mcp", "run", path(&mcp)],
        "runnable MCP capability",
    )?;
    expect_mcp_runner_result(&["code", "mcp", "test", path(&mcp)], "MCP test capability")?;

    for args in [
        vec!["code", "agent", "deploy", path(&agentic)],
        vec!["code", "agent", "deploy", path(&tool)],
        vec!["code", "agent", "debug", path(&agentic)],
        vec!["code", "mcp", "debug", path(&mcp)],
        vec!["code", "mcp", "invoke", path(&mcp)],
        vec!["code", "mcp", "batch", path(&mcp)],
        vec!["code", "skill", "run", path(&skill)],
        vec!["code", "skill", "debug", path(&skill)],
        vec!["code", "skill", "logs", path(&skill)],
        vec!["code", "flow", "debug", path(&flow)],
        vec!["code", "okf", "open", path(&okf)],
        vec!["code", "okf", "debug", path(&okf)],
        vec!["code", "okf", "logs", path(&okf)],
    ] {
        run_fail(&args)?;
    }

    for args in [
        vec!["code", "agent", "list", "--location", "os", suffix],
        vec!["code", "mcp", "list", "--location", "os", suffix],
        vec!["code", "skill", "list", "--location", "os", suffix],
        vec!["code", "flow", "list", "--location", "os", suffix],
        vec!["code", "okf", "list", "--location", "os", suffix],
        vec!["code", "agent", "activity", suffix],
        vec!["code", "mcp", "activity", suffix],
        vec!["code", "skill", "activity", suffix],
        vec!["code", "flow", "activity", suffix],
        vec!["code", "okf", "activity", suffix],
    ] {
        run_ok(&args)?;
    }

    assert_agent_project_contracts(root)?;
    assert_no_legacy_generated_configs(root)?;
    Ok(())
}

async fn cleanup_real_os_assets(asset_ids: &[String]) -> Result<(), Box<dyn Error>> {
    if asset_ids.is_empty() {
        return Ok(());
    }
    let origin = configured_os_origin()?;
    let token = stored_os_token(&origin)?;
    let client = reqwest::Client::builder().build()?;
    for id in asset_ids {
        let url = format!("{}/api/v1/assets/{id}", origin.trim_end_matches('/'));
        let response = client.delete(url).bearer_auth(&token).send().await?;
        let status = response.status();
        if !(status.as_u16() == 204 || status.as_u16() == 404) {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("delete real OS test asset {id} failed: {status} {body}").into());
        }
    }
    Ok(())
}

fn verify_real_os_cleanup(suffix: &str) -> Result<(), Box<dyn Error>> {
    for family in ["agent", "mcp", "skill", "flow", "okf"] {
        let output = run_ok(&["code", family, "list", suffix])?;
        assert!(
            output.contains("0 asset(s)"),
            "{family} cleanup should leave no matching assets:\n{output}"
        );
    }
    Ok(())
}

fn write_lifecycle_fixtures(root: &Path, suffix: &str) -> Result<(), Box<dyn Error>> {
    write_agent_project(
        root,
        "agentic",
        "agentic",
        &format!("a3s-real-agentic-{suffix}"),
        "Real OS agentic lifecycle smoke autonomous agent project",
        "Return a concise success note after validating the requested input.",
    )?;
    write_agent_project(
        root,
        "tool",
        "tool",
        &format!("a3s-real-tool-{suffix}"),
        "Real OS tool-agent lifecycle smoke autonomous agent project",
        "Expose a small tool-agent workflow that validates input and reports health.",
    )?;
    write_agent_project(
        root,
        "app",
        "application",
        &format!("a3s-real-app-{suffix}"),
        "Real OS application-agent lifecycle smoke autonomous agent project",
        "Coordinate a production-style application agent and report deployment readiness.",
    )?;
    write_file(
        root.join("mcp/server.js"),
        format!(
            "#!/usr/bin/env node\n\nif (process.argv.includes(\"--smoke\")) {{\n  console.log(JSON.stringify({{ ok: true, tool: \"health\" }}));\n  process.exit(0);\n}}\nconsole.log(\"a3s-real-mcp-{suffix}\");\n"
        ),
    )?;
    write_file(
        root.join("mcp/README.md"),
        format!("# a3s-real-mcp-{suffix}\n\nReal OS MCP lifecycle smoke asset.\n"),
    )?;
    write_file(
        root.join("mcp/.a3s/asset.acl"),
        format!(
            "version = \"a3s.asset.v1\"\ncategory = \"mcp\"\nname = \"a3s-real-mcp-{suffix}\"\nentrypoint = \"server.js\"\n"
        ),
    )?;
    write_file(
        root.join("skill/SKILL.md"),
        format!(
            "---\nname: a3s-real-skill-{suffix}\ndescription: Real OS skill lifecycle smoke\nkind: instruction\nallowed-tools: \"Read(*)\"\n---\n\nValidate the real OS skill lifecycle path.\n"
        ),
    )?;
    write_file(
        root.join("skill/README.md"),
        format!("# a3s-real-skill-{suffix}\n\nReal OS skill lifecycle smoke asset.\n"),
    )?;
    write_file(
        root.join("skill/.a3s/asset.acl"),
        format!(
            "version = \"a3s.asset.v1\"\ncategory = \"skill\"\nname = \"a3s-real-skill-{suffix}\"\ndefinition_path = \"SKILL.md\"\n"
        ),
    )?;
    write_file(
        root.join("flow/flow.json"),
        format!(
            "{{\n  \"version\": \"1.0\",\n  \"name\": \"a3s-real-flow-{suffix}\",\n  \"description\": \"Real OS workflow lifecycle smoke\",\n  \"nodes\": [\n    {{ \"id\": \"start\", \"type\": \"start\" }},\n    {{ \"id\": \"finish\", \"type\": \"end\" }}\n  ],\n  \"edges\": [\n    {{ \"from\": \"start\", \"to\": \"finish\" }}\n  ]\n}}\n"
        ),
    )?;
    write_file(
        root.join("flow/README.md"),
        format!("# a3s-real-flow-{suffix}\n\nReal OS workflow lifecycle smoke asset.\n"),
    )?;
    write_file(
        root.join("flow/.a3s/asset.acl"),
        format!(
            "version = \"a3s.asset.v1\"\ncategory = \"workflow\"\nname = \"a3s-real-flow-{suffix}\"\ndesign_document_path = \"flow.json\"\n"
        ),
    )?;
    write_file(
        root.join("okf/README.md"),
        format!("# a3s-real-okf-{suffix}\n\nReal OS OKF lifecycle smoke package.\n"),
    )?;
    write_file(
        root.join("okf/wiki/index.md"),
        "Real OS OKF lifecycle smoke knowledge page.\n",
    )?;
    write_file(
        root.join("okf/.a3s/asset.acl"),
        format!(
            "version = \"a3s.asset.v1\"\ncategory = \"knowledge\"\nname = \"a3s-real-okf-{suffix}\"\nreadme_path = \"README.md\"\n"
        ),
    )?;
    Ok(())
}

fn write_agent_project(
    root: &Path,
    dir: &str,
    kind: &str,
    name: &str,
    description: &str,
    system_prompt: &str,
) -> Result<(), Box<dyn Error>> {
    let package = root.join(dir);
    write_file(
        package.join("README.md"),
        format!(
            "# {name}\n\nA3S Code autonomous agent project for real OS lifecycle testing.\n\n## Layout\n\n- `agent.md` is the visible A3S Code entrypoint.\n- `prompts/system.md` contains the expanded system prompt.\n- `workflows/operating-procedure.md` describes the agent runbook.\n- `examples/`, `eval/`, and `tests/` keep visible source assets.\n- `.a3s/asset.acl` is metadata only.\n"
        ),
    )?;
    write_file(
        package.join("agent.md"),
        format!(
            "---\nname: {name}\ndescription: {description}\ntools: Read, Grep, Glob, Bash\nmax_steps: 3\n---\n\n# Role\n\n{system_prompt}\n\n# Success Criteria\n\n- Use the visible package files as the agent source of truth.\n- Keep generated metadata inside `.a3s/asset.acl` only.\n"
        ),
    )?;
    write_file(
        package.join("prompts/system.md"),
        format!(
            "# System Prompt\n\n{system_prompt}\n\nYou are packaged as a complete A3S Code autonomous agent project, not a single Markdown file.\n"
        ),
    )?;
    write_file(
        package.join("workflows/operating-procedure.md"),
        format!(
            "# Operating Procedure\n\n1. Inspect the request.\n2. Plan the minimal safe action.\n3. Execute with the allowed A3S Code tools.\n4. Summarize the result and evidence.\n\nAgent kind: `{kind}`.\n"
        ),
    )?;
    write_file(
        package.join("examples/example-input.md"),
        "# Example Input\n\nValidate the real OS lifecycle path.\n",
    )?;
    write_file(
        package.join("examples/example-output.md"),
        "# Example Output\n\nLifecycle validation completed successfully.\n",
    )?;
    write_file(
        package.join("eval/smoke.md"),
        "# Smoke Eval\n\nThe agent should return a concise success note and preserve package structure.\n",
    )?;
    write_file(
        package.join("tests/smoke.md"),
        "# Smoke Test\n\nPublish, run/status/open/logs, and deploy where the lifecycle supports it.\n",
    )?;
    write_file(
        package.join(".a3s/asset.acl"),
        agent_asset_acl(kind, name, description),
    )?;
    Ok(())
}

fn agent_asset_acl(kind: &str, name: &str, description: &str) -> String {
    let service = if kind == "tool" {
        "Function as a Service"
    } else {
        "Agent as a Service"
    };
    let runtime_kind = if kind == "tool" {
        "a3s-function-service"
    } else {
        "a3s-agent-service"
    };
    let isolation = if kind == "application" {
        "container"
    } else {
        "serving"
    };
    let protocol = if kind == "tool" {
        "  protocol = \"agent-tool\"\n"
    } else {
        ""
    };

    format!(
        "version = \"a3s.asset.v1\"\n\
         category = \"agent\"\n\
         kind = \"{kind}\"\n\
         name = \"{name}\"\n\
         description = \"{description}\"\n\
         service = \"{service}\"\n\
         created_by = \"a3s-code-tui\"\n\n\
         source {{\n\
           package_path = \".\"\n\
           entrypoint = \"agent.md\"\n\
           definition_path = \"agent.md\"\n\
         }}\n\n\
         metadata {{\n\
           asset_acl_path = \".a3s/asset.acl\"\n\
         }}\n\n\
         runtime {{\n\
           kind = \"{runtime_top_kind}\"\n\
           isolation = \"{isolation}\"\n\
           runtime_kind = \"{runtime_kind}\"\n\
         {protocol}  agent_kind = \"{kind}\"\n\
         }}\n",
        runtime_top_kind = if kind == "tool" { "tool" } else { "agent" },
    )
}

fn assert_agent_project_contracts(root: &Path) -> Result<(), Box<dyn Error>> {
    for (dir, kind) in [
        ("agentic", "agentic"),
        ("tool", "tool"),
        ("app", "application"),
    ] {
        assert_complete_agent_project(&root.join(dir), kind)?;
    }
    Ok(())
}

fn assert_complete_agent_project(
    package: &Path,
    expected_kind: &str,
) -> Result<(), Box<dyn Error>> {
    for rel in [
        "README.md",
        "agent.md",
        "prompts/system.md",
        "workflows/operating-procedure.md",
        "examples/example-input.md",
        "examples/example-output.md",
        "eval/smoke.md",
        "tests/smoke.md",
        ".a3s/asset.acl",
    ] {
        assert!(
            package.join(rel).is_file(),
            "missing agent project file: {rel}"
        );
    }

    let mut metadata_entries = Vec::new();
    for entry in std::fs::read_dir(package.join(".a3s"))? {
        let entry = entry?;
        metadata_entries.push(entry.file_name().to_string_lossy().to_string());
    }
    metadata_entries.sort();
    assert_eq!(
        metadata_entries,
        vec!["asset.acl".to_string()],
        ".a3s must be metadata-only for {}",
        package.display()
    );

    for rel in [
        ".a3s/agent.md",
        ".a3s/prompts/system.md",
        ".a3s/workflows/operating-procedure.md",
        ".a3s/examples/example-input.md",
        ".a3s/eval/smoke.md",
        ".a3s/tests/smoke.md",
    ] {
        assert!(
            !package.join(rel).exists(),
            "agent source must stay visible, not under {rel}"
        );
    }

    let acl = std::fs::read_to_string(package.join(".a3s/asset.acl"))?;
    for expected in [
        "category = \"agent\"",
        &format!("kind = \"{expected_kind}\""),
        "package_path = \".\"",
        "entrypoint = \"agent.md\"",
        "definition_path = \"agent.md\"",
    ] {
        assert!(
            acl.contains(expected),
            "agent asset.acl missing {expected:?}:\n{acl}"
        );
    }
    Ok(())
}

fn assert_no_legacy_generated_configs(root: &Path) -> Result<(), Box<dyn Error>> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let rel = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            assert!(
                file_name == "asset.acl"
                    || file_name == "flow.json"
                    || !file_name.ends_with(".json"),
                "unexpected generated JSON config: {rel}"
            );
            assert!(
                !rel.contains("runtime-binding")
                    && !rel.ends_with(".asset.json")
                    && !rel.ends_with(".config.json")
                    && !rel.ends_with("mcp.server.json"),
                "unexpected legacy generated config: {rel}"
            );
        }
    }
    Ok(())
}

fn run_ok(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = run_command(args)?;
    if !output.status.success() {
        return Err(format!(
            "command failed: a3s {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(command_text(&output))
}

fn run_fail(args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = run_command(args)?;
    if output.status.success() {
        return Err(format!(
            "command unexpectedly succeeded: a3s {}\n{}",
            args.join(" "),
            command_text(&output)
        )
        .into());
    }
    Ok(command_text(&output))
}

fn expect_mcp_runner_result(args: &[&str], missing_capability: &str) -> Result<(), Box<dyn Error>> {
    let output = run_command(args)?;
    let text = command_text(&output);
    if output.status.success() {
        return Ok(());
    }
    assert!(
        text.contains(missing_capability),
        "MCP runner/test failure should be the known OS capability gap:\n{text}"
    );
    Ok(())
}

fn run_command(args: &[&str]) -> Result<std::process::Output, Box<dyn Error>> {
    let mut command = Command::new(a3s_bin());
    command.args(args);
    if let Some(webview) = true_bin() {
        command.env("A3S_WEBVIEW_BIN", webview);
    }
    Ok(command.output()?)
}

fn command_text(output: &std::process::Output) -> String {
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

fn configured_os_origin() -> Result<String, Box<dyn Error>> {
    let output = run_ok(&["code", "auth", "status"])?;
    for line in output.lines() {
        if let Some(value) = line.strip_prefix("os: ") {
            return Ok(value.trim().trim_end_matches('/').to_string());
        }
    }
    Err(format!("could not determine OS origin from auth status:\n{output}").into())
}

fn stored_os_token(origin: &str) -> Result<String, Box<dyn Error>> {
    let home = std::env::var("HOME")?;
    let store = Path::new(&home).join(".a3s/os-auth.json");
    let body = std::fs::read_to_string(&store)?;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    let sessions = json
        .get("sessions")
        .and_then(|value| value.as_array())
        .ok_or("os-auth.json does not contain sessions[]")?;
    for session in sessions {
        if session
            .get("address")
            .and_then(|value| value.as_str())
            .is_some_and(|address| address.trim_end_matches('/') == origin)
        {
            return session
                .get("access_token")
                .and_then(|value| value.as_str())
                .map(str::to_string)
                .ok_or_else(|| "matching OS session has no access_token".into());
        }
    }
    Err(format!("no stored OS session for {origin} in {}", store.display()).into())
}

fn asset_id(output: &str) -> Result<String, Box<dyn Error>> {
    for line in output.lines() {
        let Some(open) = line.rfind('(') else {
            continue;
        };
        let Some(close) = line[open + 1..].find(')') else {
            continue;
        };
        let candidate = &line[open + 1..open + 1 + close];
        if looks_like_uuid(candidate) {
            return Ok(candidate.to_string());
        }
    }
    Err(format!("could not parse asset id from output:\n{output}").into())
}

fn looks_like_uuid(value: &str) -> bool {
    value.len() == 36
        && value.chars().enumerate().all(|(idx, ch)| {
            if matches!(idx, 8 | 13 | 18 | 23) {
                ch == '-'
            } else {
                ch.is_ascii_hexdigit()
            }
        })
}

fn write_file(path: PathBuf, body: impl AsRef<[u8]>) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, body)?;
    Ok(())
}

fn path(path: &Path) -> &str {
    path.to_str().expect("test paths must be utf-8")
}

fn true_bin() -> Option<&'static str> {
    ["/usr/bin/true", "/bin/true"]
        .into_iter()
        .find(|path| Path::new(path).is_file())
}

fn unique_suffix() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{}-{secs}", std::process::id())
}
