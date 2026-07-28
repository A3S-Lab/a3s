#![cfg(unix)]

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use support::{a3s_bin, configure_component_env, TempWorkspace};

#[test]
#[ignore = "requires A3S_BOX_E2E_BIN pointing to a real a3s-box binary"]
fn real_a3s_to_box_compose_acl_boundary() {
    let box_binary = real_box_binary();
    let box_install_dir = box_binary
        .parent()
        .expect("a3s-box binary must have a parent directory");

    let temp = TempWorkspace::new("compose-acl-e2e");
    let project = temp.path("project");
    std::fs::create_dir_all(&project).expect("create Compose ACL project");
    std::fs::write(
        project.join("compose.acl"),
        r#"service "api" {
  image = "ghcr.io/a3s-lab/api:${IMAGE_TAG:-latest}"
  environment = {
    FROM_DOTENV = env("FROM_DOTENV")
    SHELL_WINS = env("SHELL_WINS")
  }
  ports = ["18080:8080"]
}
"#,
    )
    .expect("write compose.acl");
    std::fs::write(
        project.join(".env"),
        "IMAGE_TAG=dotenv\nFROM_DOTENV=loaded\nSHELL_WINS=dotenv\n",
    )
    .expect("write Compose .env");
    std::fs::write(
        project.join("compose.yaml"),
        "invalid YAML proves compose.acl has discovery priority: [\n",
    )
    .expect("write conflicting Compose YAML");

    let mut config = Command::new(a3s_bin());
    configure_component_env(&mut config, &temp);
    let output = config
        .arg("-C")
        .arg(&project)
        .args(["compose", "config"])
        .env("A3S_BOX_INSTALL_DIR", box_install_dir)
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_NO_PROGRESS", "1")
        .env("IMAGE_TAG", "shell")
        .env("SHELL_WINS", "shell")
        .output()
        .expect("run real a3s compose config");

    assert!(
        output.status.success(),
        "real Compose ACL boundary failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ghcr.io/a3s-lab/api:shell"));
    assert!(stdout.contains("FROM_DOTENV=loaded"));
    assert!(stdout.contains("SHELL_WINS=shell"));
    assert!(stdout.contains("Configuration is valid."));

    std::fs::write(
        project.join("invalid.acl"),
        "service \"api\" { image = \"api:latest\" unsupported_field = true }\n",
    )
    .expect("write invalid Compose ACL");
    let mut invalid = Command::new(a3s_bin());
    configure_component_env(&mut invalid, &temp);
    let output = invalid
        .arg("-C")
        .arg(&project)
        .args(["compose", "-f", "invalid.acl", "config"])
        .env("A3S_BOX_INSTALL_DIR", box_install_dir)
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_NO_PROGRESS", "1")
        .output()
        .expect("run invalid real a3s compose config");

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported_field"));
}

#[test]
#[ignore = "requires a real a3s-box binary, registry access, and a working MicroVM runtime"]
fn real_a3s_compose_acl_lifecycle() {
    let box_binary = real_box_binary();
    let box_install_dir = box_binary
        .parent()
        .expect("a3s-box binary must have a parent directory");
    let temp = TempWorkspace::new("compose-acl-lifecycle-e2e");
    let project = temp.path("project");
    std::fs::create_dir_all(&project).expect("create Compose ACL lifecycle project");
    let image = std::env::var("A3S_BOX_E2E_IMAGE")
        .unwrap_or_else(|_| "docker.io/library/alpine:latest".to_string());
    std::fs::write(
        project.join("compose.acl"),
        format!(
            r#"service "worker" {{
  image = "{image}"
  command = ["sleep", "3600"]
  environment = {{ A3S_CLI_E2E = "ready" }}
}}
"#,
        ),
    )
    .expect("write lifecycle compose.acl");

    run_a3s(&temp, &project, box_install_dir, &["box", "pull", &image]);
    run_a3s(
        &temp,
        &project,
        box_install_dir,
        &["up", "--project-name", "cliacl", "--detach"],
    );
    let mut cleanup = ComposeCleanup::new(&temp, &project, box_install_dir, "cliacl");

    run_a3s(
        &temp,
        &project,
        box_install_dir,
        &["up", "--project-name", "cliacl", "--detach"],
    );
    let ps = run_a3s(
        &temp,
        &project,
        box_install_dir,
        &["ps", "--project-name", "cliacl"],
    );
    let ps = String::from_utf8_lossy(&ps.stdout);
    let service_rows = ps
        .lines()
        .filter(|line| line.split_whitespace().next() == Some("worker"))
        .count();
    assert_eq!(
        service_rows, 1,
        "convergent up must leave exactly one service instance:\n{ps}"
    );
    let box_ps = run_a3s(&temp, &project, box_install_dir, &["box", "ps"]);
    assert_eq!(
        String::from_utf8_lossy(&box_ps.stdout)
            .matches("cliacl-worker")
            .count(),
        1,
        "convergent up must leave exactly one Box instance"
    );
    run_a3s(
        &temp,
        &project,
        box_install_dir,
        &["logs", "--project-name", "cliacl", "--tail", "20"],
    );
    let exec = run_a3s(
        &temp,
        &project,
        box_install_dir,
        &[
            "box",
            "exec",
            "cliacl-worker",
            "--",
            "sh",
            "-c",
            "echo $A3S_CLI_E2E",
        ],
    );
    assert_eq!(String::from_utf8_lossy(&exec.stdout).trim(), "ready");

    run_a3s(
        &temp,
        &project,
        box_install_dir,
        &["down", "--project-name", "cliacl"],
    );
    cleanup.disarm();

    let all_boxes = run_a3s(&temp, &project, box_install_dir, &["box", "ps", "-a"]);
    assert!(!String::from_utf8_lossy(&all_boxes.stdout).contains("cliacl-worker"));
    let boxes_dir = temp.path("box-home/boxes");
    let remaining_boxes = std::fs::read_dir(&boxes_dir)
        .map(|entries| entries.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(
        remaining_boxes.is_empty(),
        "a3s down left box storage or a mounted rootfs under {}",
        boxes_dir.display()
    );
}

fn real_box_binary() -> PathBuf {
    let configured = PathBuf::from(
        std::env::var_os("A3S_BOX_E2E_BIN")
            .expect("A3S_BOX_E2E_BIN must point to a real a3s-box binary"),
    );
    let binary = std::fs::canonicalize(&configured).unwrap_or_else(|error| {
        panic!(
            "failed to resolve A3S_BOX_E2E_BIN {}: {error}",
            configured.display()
        )
    });
    assert_eq!(
        binary.file_name().and_then(|name| name.to_str()),
        Some("a3s-box"),
        "A3S_BOX_E2E_BIN must end in a3s-box"
    );
    binary
}

fn a3s_command(workspace: &TempWorkspace, project: &Path, box_install_dir: &Path) -> Command {
    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, workspace);
    command
        .arg("-C")
        .arg(project)
        .env("A3S_HOME", workspace.path("box-home"))
        .env("A3S_BOX_INSTALL_DIR", box_install_dir)
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_NO_PROGRESS", "1")
        .env("PATH", std::env::var_os("PATH").unwrap_or_default());
    command
}

fn run_a3s(
    workspace: &TempWorkspace,
    project: &Path,
    box_install_dir: &Path,
    args: &[&str],
) -> std::process::Output {
    let output = a3s_command(workspace, project, box_install_dir)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run a3s {args:?}: {error}"));
    assert!(
        output.status.success(),
        "a3s {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

struct ComposeCleanup<'a> {
    workspace: &'a TempWorkspace,
    project: &'a Path,
    box_install_dir: &'a Path,
    project_name: &'a str,
    armed: bool,
}

impl<'a> ComposeCleanup<'a> {
    fn new(
        workspace: &'a TempWorkspace,
        project: &'a Path,
        box_install_dir: &'a Path,
        project_name: &'a str,
    ) -> Self {
        Self {
            workspace,
            project,
            box_install_dir,
            project_name,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ComposeCleanup<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = a3s_command(self.workspace, self.project, self.box_install_dir)
                .args(["down", "--project-name", self.project_name])
                .output();
        }
    }
}
