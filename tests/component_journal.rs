#![cfg(unix)]

mod support;

use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use support::{a3s_bin, configure_component_env, make_executable, TempWorkspace};

#[test]
fn interrupted_batch_recovers_completed_components_and_retries_pending_work() {
    let temp = TempWorkspace::new("component-journal-recovery");
    let use_bin = temp.path("use-bin");
    let fixture = temp.path("fixture");
    std::fs::create_dir_all(&fixture).unwrap();
    make_executable(
        &use_bin.join("a3s-use"),
        r#"#!/bin/sh
fixture=${A3S_JOURNAL_TEST_ROOT:?}

if [ "$1" = "--version" ]; then
  printf 'a3s-use 0.1.1\n'
  exit 0
fi

if [ "$1" = "component" ] && [ "$2" = "list" ]; then
  printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"components":[]}}'
  exit 0
fi

if [ "$1" = "component" ] && [ "$2" = "status" ]; then
  case "$3" in
    browser)
      if [ -f "$fixture/browser-installed" ]; then
        printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"component":{"id":"browser","presence":"managed","health":"ready","version":"1.0.0"}}}'
      else
        printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"component":{"id":"browser","presence":"missing","health":"unknown"}}}'
      fi
      exit 0
      ;;
    office)
      if [ -f "$fixture/office-installed" ]; then
        printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"component":{"id":"office","presence":"managed","health":"ready","version":"1.0.0"}}}'
      else
        printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"component":{"id":"office","presence":"missing","health":"unknown"}}}'
      fi
      exit 0
      ;;
  esac
fi

if [ "$1" = "component" ] && [ "$2" = "install" ] && [ "$3" = "browser" ]; then
  printf 'browser\n' >> "$fixture/install-calls.log"
  : > "$fixture/browser-installed"
  printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"changed":true,"component":{"id":"browser","presence":"managed","health":"ready","version":"1.0.0"}}}'
  exit 0
fi

if [ "$1" = "component" ] && [ "$2" = "install" ] && [ "$3" = "office" ]; then
  printf 'office\n' >> "$fixture/install-calls.log"
  : > "$fixture/office-started"
  while [ ! -f "$fixture/allow-office" ]; do
    /bin/sleep 0.02
  done
  : > "$fixture/office-installed"
  printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"changed":true,"component":{"id":"office","presence":"managed","health":"ready","version":"1.0.0"}}}'
  exit 0
fi

exit 2
"#,
    );

    let active = temp.path("state/component-operations/active.json");
    let mut first = Command::new(a3s_bin());
    configure_component_env(&mut first, &temp);
    first
        .args(["install", "use/browser", "use/office", "--json"])
        .env("A3S_USE_INSTALL_DIR", &use_bin)
        .env("A3S_JOURNAL_TEST_ROOT", &fixture)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let mut first = ProcessGroup::spawn(&mut first);

    wait_until(Duration::from_secs(10), || {
        fixture.join("office-started").is_file()
            && read_json(&active).is_some_and(|record| {
                step(&record, "use/browser").is_some_and(|step| step["status"] == "succeeded")
                    && step(&record, "use/office").is_some_and(|step| step["status"] == "pending")
            })
    });
    let first_record = read_json(&active).expect("active journal disappeared before interruption");
    let first_digest = first_record["planDigest"].as_str().unwrap().to_string();
    let interrupted_output = first.kill_and_collect();
    assert!(!interrupted_output.status.success());
    assert!(
        active.is_file(),
        "interrupted journal must remain recoverable"
    );

    std::fs::write(fixture.join("allow-office"), b"continue").unwrap();
    let mut resumed = Command::new(a3s_bin());
    configure_component_env(&mut resumed, &temp);
    let resumed = resumed
        .args(["install", "use/browser", "use/office", "--json"])
        .env("A3S_USE_INSTALL_DIR", &use_bin)
        .env("A3S_JOURNAL_TEST_ROOT", &fixture)
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        resumed.status,
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );

    let result: serde_json::Value = serde_json::from_slice(&resumed.stdout).unwrap();
    let operations = result["data"]["operations"].as_array().unwrap();
    let browser = operations
        .iter()
        .find(|operation| operation["component"] == "use/browser")
        .unwrap();
    let office = operations
        .iter()
        .find(|operation| operation["component"] == "use/office")
        .unwrap();
    assert_eq!(browser["recovered"], true);
    assert_eq!(browser["action"], "install");
    assert!(browser["message"]
        .as_str()
        .unwrap()
        .starts_with("Recovered completed checkpoint:"));
    assert!(office.get("recovered").is_none());

    let calls = std::fs::read_to_string(fixture.join("install-calls.log")).unwrap();
    assert_eq!(calls.lines().filter(|call| *call == "browser").count(), 1);
    assert_eq!(calls.lines().filter(|call| *call == "office").count(), 2);
    assert!(!active.exists());

    let last = read_json(&temp.path("state/component-operations/last.json")).unwrap();
    assert_eq!(last["phase"], "completed");
    assert_eq!(last["recoveredFromPlanDigest"], first_digest);
    assert_eq!(step(&last, "use/browser").unwrap()["recovered"], true);
    assert_eq!(step(&last, "use/office").unwrap()["status"], "succeeded");

    let interrupted =
        read_json(&temp.path("state/component-operations/last-interrupted.json")).unwrap();
    assert_eq!(interrupted["phase"], "interrupted");
    assert_eq!(interrupted["planDigest"], first_digest);
    assert_eq!(
        step(&interrupted, "use/browser").unwrap()["status"],
        "succeeded"
    );
    assert_eq!(
        step(&interrupted, "use/office").unwrap()["status"],
        "pending"
    );
}

fn read_json(path: &std::path::Path) -> Option<serde_json::Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn step<'a>(record: &'a serde_json::Value, component: &str) -> Option<&'a serde_json::Value> {
    record["steps"]
        .as_array()?
        .iter()
        .find(|step| step["component"] == component)
}

fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("condition was not satisfied within {timeout:?}");
}

struct ProcessGroup {
    child: Option<Child>,
    process_group: i32,
}

impl ProcessGroup {
    fn spawn(command: &mut Command) -> Self {
        let child = command.spawn().unwrap();
        Self {
            process_group: child.id() as i32,
            child: Some(child),
        }
    }

    fn kill_and_collect(&mut self) -> Output {
        let result = unsafe { libc::kill(-self.process_group, libc::SIGKILL) };
        assert_eq!(
            result,
            0,
            "failed to kill component command process group: {}",
            std::io::Error::last_os_error()
        );
        self.child.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        unsafe {
            libc::kill(-self.process_group, libc::SIGKILL);
        }
        let _ = child.wait();
    }
}
