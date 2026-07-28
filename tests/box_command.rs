#![cfg(unix)]

mod support;

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

use support::{
    a3s_bin, box_release_target, configure_component_env, make_executable, sh_quote,
    start_fake_box_release, TempWorkspace,
};

static PROCESS_TEST_LOCK: Mutex<()> = Mutex::new(());

fn process_test_guard() -> MutexGuard<'static, ()> {
    PROCESS_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[test]
fn box_command_delegates_to_configured_a3s_box() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("delegate");
    let bin_dir = temp.path("bin");
    let args_log = temp.path("args.log");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-box 2.5.2\\n'\n  exit 0\nfi\nprintf '%s\\n' \"$@\" > {}\nprintf 'delegated:%s\\n' \"$*\"\nexit 0\n",
        sh_quote(&args_log)
    );
    make_executable(&bin_dir.join("a3s-box"), &script);

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["box", "ps", "--format", "json"])
        .env("A3S_BOX_INSTALL_DIR", &bin_dir)
        .output()
        .expect("failed to run a3s box");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "delegated:ps --format json\n"
    );
    assert_eq!(
        std::fs::read_to_string(args_log).expect("args log should be written"),
        "ps\n--format\njson\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
}

#[test]
fn box_command_propagates_a3s_box_exit_status() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("exit-status");
    let bin_dir = temp.path("bin");
    make_executable(
        &bin_dir.join("a3s-box"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-box 2.5.2\\n'\n  exit 0\nfi\nprintf 'failing-box:%s\\n' \"$*\"\nexit 7\n",
    );

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["box", "run", "bad"])
        .env("A3S_BOX_INSTALL_DIR", &bin_dir)
        .output()
        .expect("failed to run a3s box");

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "failing-box:run bad\n"
    );
}

#[test]
fn use_box_routes_through_use_with_one_resolved_box_executable() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("use-box-delegate");
    let use_bin = temp.path("use-bin");
    let box_bin = temp.path("box-bin");
    let use_log = temp.path("use.log");
    let box_log = temp.path("box.log");
    make_executable(
        &use_bin.join("a3s-use"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-use 0.1.0\\n'\n  exit 0\nfi\nprintf '%s\\n' \"$@\" > {}\nprintf '%s\\n' \"$A3S_USE_BOX_EXECUTABLE\" >> {}\nif [ \"$1\" = \"box\" ]; then\n  shift\n  exec \"$A3S_USE_BOX_EXECUTABLE\" \"$@\"\nfi\nexit 2\n",
            sh_quote(&use_log),
            sh_quote(&use_log)
        ),
    );
    make_executable(
        &box_bin.join("a3s-box"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-box 2.5.2\\n'\n  exit 0\nfi\nprintf '%s\\n' \"$@\" > {}\nprintf 'use-box:%s\\n' \"$*\"\nexit 7\n",
            sh_quote(&box_log)
        ),
    );

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["use", "box", "compose", "up", "--detach"])
        .env("A3S_USE_INSTALL_DIR", &use_bin)
        .env("A3S_BOX_INSTALL_DIR", &box_bin)
        .env("A3S_USE_BOX_EXECUTABLE", "/tmp/untrusted-box")
        .output()
        .expect("failed to run a3s use box");

    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "use-box:compose up --detach\n"
    );
    let use_lines = std::fs::read_to_string(use_log).unwrap();
    assert!(use_lines.starts_with("box\ncompose\nup\n--detach\n"));
    let resolved_box = use_lines.lines().last().unwrap();
    assert_eq!(
        PathBuf::from(resolved_box),
        std::fs::canonicalize(box_bin.join("a3s-box")).unwrap()
    );
    assert_eq!(
        std::fs::read_to_string(box_log).unwrap(),
        "compose\nup\n--detach\n"
    );
}

#[test]
fn non_box_use_routes_do_not_install_box() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("use-without-box");
    let use_bin = temp.path("use-bin");
    make_executable(
        &use_bin.join("a3s-use"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-use 0.1.0\\n'\n  exit 0\nfi\nif [ -n \"$A3S_USE_BOX_EXECUTABLE\" ]; then\n  printf 'unexpected-box\\n'\n  exit 3\nfi\nprintf 'use:%s\\n' \"$*\"\n",
    );

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["use", "capabilities", "--json"])
        .env("A3S_USE_INSTALL_DIR", &use_bin)
        .env("A3S_USE_BOX_EXECUTABLE", "/tmp/untrusted-box")
        .output()
        .expect("failed to run non-Box Use route");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "use:capabilities --json\n"
    );
    assert!(!temp.path("state/components/box.json").exists());
    assert!(!temp.path("data/components/box").exists());
}

#[test]
fn use_box_auto_install_creates_only_the_authoritative_box_receipt() {
    let _guard = process_test_guard();
    if box_release_target().is_none() {
        eprintln!("skipping release install test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("use-box-auto-install");
    let use_bin = temp.path("use-bin");
    make_executable(
        &use_bin.join("a3s-use"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-use 0.1.0\\n'\n  exit 0\nfi\nif [ \"$1\" = \"box\" ]; then\n  shift\n  exec \"$A3S_USE_BOX_EXECUTABLE\" \"$@\"\nfi\nexit 2\n",
    );
    let server = start_fake_box_release(&temp, "2.5.2", None);

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["use", "box", "version"])
        .env("A3S_USE_INSTALL_DIR", &use_bin)
        .env("A3S_UPDATER_GITHUB_API_BASE", server.api_base())
        .output()
        .expect("failed to auto-install Box through Use");

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "installed-box:version\n"
    );
    let receipt_root = temp.path("state/components");
    let receipts = std::fs::read_dir(&receipt_root)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(receipts, ["box.json"]);
    assert!(!temp.path("state/components/use/box.json").exists());
}

#[test]
fn compose_namespace_delegates_to_a3s_box_compose_without_reparsing_arguments() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("compose-delegate");
    let bin_dir = temp.path("bin");
    let args_log = temp.path("args.log");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-box 2.5.2\\n'\n  exit 0\nfi\nprintf '%s\\n' \"$@\" > {}\nexit 0\n",
        sh_quote(&args_log)
    );
    make_executable(&bin_dir.join("a3s-box"), &script);

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args([
            "compose",
            "--file",
            "compose.acl",
            "--project-name",
            "demo",
            "up",
            "--detach",
        ])
        .env("A3S_BOX_INSTALL_DIR", &bin_dir)
        .output()
        .expect("failed to run a3s compose");

    assert!(output.status.success());
    assert_eq!(
        std::fs::read_to_string(args_log).expect("args log should be written"),
        "compose\n--file\ncompose.acl\n--project-name\ndemo\nup\n--detach\n"
    );
}

#[test]
fn compose_shortcuts_prepend_the_box_compose_command() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("compose-shortcuts");
    let bin_dir = temp.path("bin");
    let args_log = temp.path("args.log");
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'a3s-box 2.5.2\\n'\n  exit 0\nfi\nprintf '%s\\n' \"$@\" >> {}\nprintf '%s\\n' --- >> {}\nexit 0\n",
        sh_quote(&args_log),
        sh_quote(&args_log)
    );
    make_executable(&bin_dir.join("a3s-box"), &script);

    for args in [
        &[
            "up",
            "--file",
            "stack.yaml",
            "--project-name=demo",
            "--detach",
        ][..],
        &["down", "-f", "stack.yaml", "--volumes"][..],
        &["ps"][..],
        &["logs", "--file", "stack.yaml", "-f", "api"][..],
    ] {
        let mut command = Command::new(a3s_bin());
        configure_component_env(&mut command, &temp);
        let output = command
            .args(args)
            .env("A3S_BOX_INSTALL_DIR", &bin_dir)
            .output()
            .expect("failed to run a3s Compose shortcut");
        assert!(output.status.success(), "shortcut {args:?} failed");
    }

    assert_eq!(
        std::fs::read_to_string(args_log).expect("args log should be written"),
        concat!(
            "compose\n--file\nstack.yaml\n--project-name=demo\nup\n--detach\n---\n",
            "compose\n-f\nstack.yaml\ndown\n--volumes\n---\n",
            "compose\nps\n---\n",
            "compose\n--file\nstack.yaml\nlogs\n-f\napi\n---\n"
        )
    );
}

#[test]
fn box_command_auto_installs_a_verified_release() {
    let _guard = process_test_guard();
    if box_release_target().is_none() {
        eprintln!("skipping release install test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("auto-install");
    let server = start_fake_box_release(&temp, "2.5.2", None);

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["box", "version"])
        .env("A3S_UPDATER_GITHUB_API_BASE", server.api_base())
        .output()
        .expect("failed to run a3s box");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "installed-box:version\n"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.is_empty(),
        "captured non-TTY invocations must not emit progress: {stderr}"
    );

    let receipt = temp.path("state/components/box.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(receipt).unwrap()).unwrap();
    assert_eq!(receipt["componentId"], "box");
    assert_eq!(receipt["version"], "2.5.2");
    assert_eq!(receipt["provenance"], "github-release");
    let executable = PathBuf::from(receipt["executablePath"].as_str().unwrap());
    assert!(executable.is_file());
    assert!(executable.starts_with(temp.path("data/components/box/2.5.2")));

    let requests = server.requests();
    assert!(requests
        .iter()
        .any(|path| path.ends_with("/repos/A3S-Lab/Box/releases/latest")));
    assert!(requests
        .iter()
        .any(|path| path.contains("/assets/a3s-box-v2.5.2-")));
}

#[test]
fn box_command_respects_no_auto_install() {
    let _guard = process_test_guard();
    let temp = TempWorkspace::new("no-auto-install");
    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .args(["box", "version"])
        .env("A3S_NO_AUTO_INSTALL", "1")
        .output()
        .expect("failed to run a3s box");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("first-use installation is disabled"),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains("a3s install box"),
        "unexpected stderr: {stderr}"
    );
}
