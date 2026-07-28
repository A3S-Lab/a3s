#![cfg(unix)]

mod support;

use std::process::Command;

use support::{
    a3s_bin, box_release_target, configure_component_env, start_fake_box_release, TempWorkspace,
};

#[test]
#[ignore = "soak test; run with `cargo test --test box_command_soak -- --ignored`"]
fn box_command_soak_installs_once_then_reuses_receipt() {
    if box_release_target().is_none() {
        eprintln!("skipping release install soak test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("soak");
    let args_log = temp.path("installed-args.log");
    let server = start_fake_box_release(&temp, "2.5.2", Some(&args_log));

    for index in 0..50 {
        let mut command = Command::new(a3s_bin());
        configure_component_env(&mut command, &temp);
        let output = command
            .args(["box", "run", "--iteration"])
            .arg(index.to_string())
            .env("A3S_UPDATER_GITHUB_API_BASE", server.api_base())
            .output()
            .expect("failed to run a3s box soak iteration");

        assert!(
            output.status.success(),
            "iteration {index} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("installed-box:run --iteration {index}\n")
        );
    }

    let requests = server.requests();
    assert_eq!(
        requests
            .iter()
            .filter(|path| path.ends_with("/repos/A3S-Lab/Box/releases/latest"))
            .count(),
        1
    );
    assert_eq!(
        requests
            .iter()
            .filter(|path| path.contains("/assets/a3s-box-v2.5.2-"))
            .count(),
        1
    );
    let delegated_args = std::fs::read_to_string(args_log).expect("args log should be written");
    assert_eq!(delegated_args.lines().count(), 50 * 3);
    assert!(delegated_args.contains("--iteration"));
    assert!(temp.path("state/components/box.json").is_file());
}
