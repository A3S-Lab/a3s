#![cfg(unix)]

mod support;

use std::process::Command;

use support::{a3s_bin, configure_component_env, make_executable, sh_quote, TempWorkspace};

#[test]
fn search_proxy_forwards_native_arguments_and_exit_status() {
    let workspace = TempWorkspace::new("search-proxy");
    let bin_dir = workspace.path("bin");
    let args_log = workspace.path("args.log");
    make_executable(
        &bin_dir.join("a3s-search"),
        &format!(
            "#!/bin/sh\n\
             if [ \"$1\" = \"--version\" ]; then\n\
               printf 'a3s-search 2.0.0\\n'\n\
               exit 0\n\
             fi\n\
             printf '%s\\n' \"$@\" > {}\n\
             if [ \"$1\" = \"fail\" ]; then exit 9; fi\n\
             printf '{{\"engines\":[\"ddg\",\"chrome\"]}}\\n'\n",
            sh_quote(&args_log)
        ),
    );

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &workspace);
    let output = command
        .args(["search", "engines", "--format", "json"])
        .env("A3S_SEARCH_INSTALL_DIR", &bin_dir)
        .output()
        .expect("failed to run a3s search");

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "{\"engines\":[\"ddg\",\"chrome\"]}\n"
    );
    assert_eq!(
        std::fs::read_to_string(&args_log).expect("argument log"),
        "engines\n--format\njson\n"
    );
    assert!(String::from_utf8_lossy(&output.stderr).is_empty());
    assert!(!workspace.path("state/components/search.json").exists());

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &workspace);
    let failed = command
        .args(["search", "fail"])
        .env("A3S_SEARCH_INSTALL_DIR", &bin_dir)
        .output()
        .expect("failed to run a3s search failure fixture");
    assert_eq!(failed.status.code(), Some(9));
}

#[test]
fn search_proxy_requires_an_explicit_registered_installation() {
    let workspace = TempWorkspace::new("search-missing");
    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &workspace);
    let output = command
        .args(["search", "engines"])
        .output()
        .expect("failed to run a3s search");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("component 'search' is not installed"));
    assert!(stderr.contains("a3s install search"));
    assert!(!workspace.path("state/components/search.json").exists());
}
