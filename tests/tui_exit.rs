#![cfg(target_os = "macos")]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct TestDirectory {
    path: PathBuf,
}

impl TestDirectory {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("a3s-tui-exit-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).expect("create TUI exit test directory");
        Self { path }
    }

    fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_executable(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create executable parent");
    }
    fs::write(path, contents).expect("write executable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("make executable");
}

fn process_exists(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn process_group_exists(pid: &str) -> bool {
    Command::new("/bin/kill")
        .args(["-0", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn kill_process_group(pid: &str) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &format!("-{pid}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn kill_process(pid: &str) {
    let _ = Command::new("/bin/kill")
        .args(["-KILL", pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn wait_for_process_exit(pid: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(1);
    while process_exists(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
    !process_exists(pid)
}

#[test]
fn code_exit_completes_after_session_saved_with_a_blocked_workspace_scan() {
    let directory = TestDirectory::new();
    let workspace = directory.join("workspace");
    let home = directory.join("home");
    let bin = directory.join("bin");
    let config = directory.join("config.acl");
    let block_git = directory.join("block-git");
    let git_started = directory.join("git-started");
    let sleep_started = directory.join("sleep-started");
    let trigger = workspace.join("trigger.txt");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&home).expect("create home");
    fs::write(workspace.join("README.md"), "# Exit test\n").expect("write workspace file");
    fs::write(
        &config,
        r#"default_model = "openai/test"
providers "openai" {
  apiKey = "test"
  baseUrl = "http://127.0.0.1:1"
  models "test" {
    name = "Test"
    toolCall = true
  }
}
memory { llmExtraction = false }
"#,
    )
    .expect("write test config");
    write_executable(
        &bin.join("curl"),
        "#!/bin/sh\nprintf '[{\"tag_name\":\"v0.8.3\",\"draft\":false,\"prerelease\":false}]'\n",
    );
    write_executable(
        &bin.join("git"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'git version test\\n'\n  exit 0\nfi\nif [ -f '{}' ]; then\n  printf '%s\\n' \"$$\" > '{}'\n  /bin/sleep 30 &\n  sleep_pid=$!\n  printf '%s\\n' \"$sleep_pid\" > '{}'\n  wait \"$sleep_pid\"\nfi\n",
            block_git.display(),
            git_started.display(),
            sleep_started.display()
        ),
    );

    let expect_script = r#"
log_user 0
set timeout 60
spawn $env(A3S_EXIT_TEST_BIN) code -C $env(A3S_EXIT_TEST_WORKSPACE) --config $env(A3S_EXIT_TEST_CONFIG)
expect {
    -exact "\033\[?1049h" {}
    eof {
        set result [wait]
        puts "a3s exited before the TUI became ready: [lindex $result 3]"
        exit 120
    }
    timeout {
        catch {exec kill -TERM [exp_pid]}
        catch {wait}
        puts "TUI event loop did not become ready"
        exit 121
    }
}

set block [open $env(A3S_EXIT_TEST_BLOCK_GIT) w]
close $block
set trigger [open $env(A3S_EXIT_TEST_TRIGGER) w]
puts $trigger "trigger"
close $trigger

set scan_deadline [expr {[clock milliseconds] + 5000}]
while {(![file exists $env(A3S_EXIT_TEST_GIT_STARTED)] || ![file exists $env(A3S_EXIT_TEST_SLEEP_STARTED)]) && [clock milliseconds] < $scan_deadline} {
    after 50
}
if {![file exists $env(A3S_EXIT_TEST_GIT_STARTED)] || ![file exists $env(A3S_EXIT_TEST_SLEEP_STARTED)]} {
    catch {exec kill -TERM [exp_pid]}
    catch {wait}
    puts "blocked Git scan was not observed"
    exit 122
}

set started [clock milliseconds]
send -- "/exit\r"
set timeout 12
expect {
    -glob "*session saved*" {
        set saved [clock milliseconds]
        set timeout 5
        expect {
            eof {
                set finished [clock milliseconds]
                set elapsed [expr {$finished - $started}]
                set after_saved [expr {$finished - $saved}]
                set result [wait]
                set status [lindex $result 3]
                puts "exit_ms=$elapsed after_session_saved_ms=$after_saved exit_status=$status"
                if {$status != 0 || $elapsed >= 10000 || $after_saved >= 4000} {
                    exit 123
                }
                exit 0
            }
            timeout {
                catch {exec kill -TERM [exp_pid]}
                after 500
                catch {exec kill -KILL [exp_pid]}
                catch {wait}
                puts "process remained alive after the session-saved message"
                exit 127
            }
        }
    }
    eof {
        set result [wait]
        puts "a3s exited without the session-saved message: [lindex $result 3]"
        exit 128
    }
    timeout {
        catch {exec kill -TERM [exp_pid]}
        after 500
        catch {exec kill -KILL [exp_pid]}
        catch {wait}
        puts "TUI exit exceeded its deadline"
        exit 124
    }
}
"#;
    let path = format!("{}:/usr/local/bin:/usr/bin:/bin", bin.to_string_lossy());
    let output = Command::new("/usr/bin/expect")
        .args(["-c", expect_script])
        .env("HOME", &home)
        .env("PATH", path)
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_EXIT_TEST_BIN", env!("CARGO_BIN_EXE_a3s"))
        .env("A3S_EXIT_TEST_WORKSPACE", &workspace)
        .env("A3S_EXIT_TEST_CONFIG", &config)
        .env("A3S_EXIT_TEST_BLOCK_GIT", &block_git)
        .env("A3S_EXIT_TEST_GIT_STARTED", &git_started)
        .env("A3S_EXIT_TEST_SLEEP_STARTED", &sleep_started)
        .env("A3S_EXIT_TEST_TRIGGER", &trigger)
        .output()
        .expect("run PTY exit probe");

    let git_pid = fs::read_to_string(&git_started)
        .ok()
        .map(|value| value.trim().to_owned());
    let sleep_pid = fs::read_to_string(&sleep_started)
        .ok()
        .map(|value| value.trim().to_owned());
    let git_exited = git_pid.as_deref().is_some_and(wait_for_process_exit);
    let sleep_exited = sleep_pid.as_deref().is_some_and(wait_for_process_exit);
    let git_group_still_running = git_pid.as_deref().is_some_and(process_group_exists);
    if let Some(pid) = git_pid.as_deref().filter(|_| git_group_still_running) {
        kill_process_group(pid);
    }
    if let Some(pid) = git_pid.as_deref().filter(|_| !git_exited) {
        kill_process(pid);
    }
    if let Some(pid) = sleep_pid.as_deref().filter(|_| !sleep_exited) {
        kill_process(pid);
    }

    assert!(
        output.status.success(),
        "PTY exit probe failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        git_exited && sleep_exited && !git_group_still_running,
        "workspace scan processes survived TUI shutdown: git={git_pid:?}, sleep={sleep_pid:?}, \
         git_alive={}, sleep_alive={}, group_alive={git_group_still_running}",
        !git_exited,
        !sleep_exited
    );
}
