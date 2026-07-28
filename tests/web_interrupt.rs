#![cfg(target_os = "macos")]

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
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
            std::env::temp_dir().join(format!("a3s-web-interrupt-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&path).expect("create Web interrupt test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
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

#[test]
fn foreground_web_exits_on_ctrl_c_from_raw_terminal() {
    run_foreground_web_probe(false);
}

#[test]
fn foreground_web_exits_when_terminal_disables_signals_after_startup() {
    run_foreground_web_probe(true);
}

#[test]
fn foreground_web_exits_on_ctrl_c_with_open_workspace_event_stream() {
    let directory = TestDirectory::new();
    let config_path = directory.join("config.acl");
    fs::write(&config_path, test_config()).expect("write Web interrupt test config");

    let port = reserve_loopback_port();
    let mut server = ChildGuard::spawn(
        Command::new(env!("CARGO_BIN_EXE_a3s"))
            .args([
                "web",
                "--api-only",
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--workspace",
                directory.path().to_str().expect("UTF-8 test directory"),
                "--config",
                config_path.to_str().expect("UTF-8 test config path"),
            ])
            .env("HOME", directory.path())
            .env("A3S_NO_AUTO_INSTALL", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    );

    wait_for_server(port, Duration::from_secs(30));
    let _workspace_stream = open_workspace_stream(port, directory.path());

    let signal_result = unsafe { libc::kill(server.id() as libc::pid_t, libc::SIGINT) };
    assert_eq!(
        signal_result,
        0,
        "send SIGINT to foreground Web process: {}",
        std::io::Error::last_os_error()
    );
    let status = server
        .wait_timeout(Duration::from_secs(3))
        .expect("Ctrl+C did not stop a3s web while an SSE connection was open");
    assert!(status.success(), "a3s web exited with status {status}");
}

fn run_foreground_web_probe(disable_signals_after_startup: bool) {
    let directory = TestDirectory::new();
    let config_path = directory.join("config.acl");
    fs::write(&config_path, test_config()).expect("write Web interrupt test config");

    let expect_script = r#"
log_user 0
set timeout 45
spawn /bin/sh -c {stty raw -echo -isig; exec "$A3S_WEB_INTERRUPT_BIN" web --api-only --host 127.0.0.1 --port 0 --workspace "$A3S_WEB_INTERRUPT_ROOT" --config "$A3S_WEB_INTERRUPT_CONFIG"}
expect {
    -exact "Press Ctrl+C to stop.\n" {}
    eof {
        set result [wait]
        puts "a3s web exited before becoming ready: [lindex $result 3]"
        exit 120
    }
    timeout {
        catch {exec kill -TERM [exp_pid]}
        catch {wait}
        puts "a3s web did not become ready"
        exit 121
    }
}

if {$env(A3S_WEB_INTERRUPT_DISABLE_SIGNALS) == "1"} {
    if {[catch {stty -isig < $spawn_out(slave,name)} error]} {
        catch {exec kill -TERM [exp_pid]}
        catch {wait}
        puts "failed to disable terminal signals after startup: $error"
        exit 124
    }
}
send -- "\003"
set timeout 12
expect {
    eof {
        set result [wait]
        set status [lindex $result 3]
        if {$status != 0} {
            puts "a3s web exited with status $status"
            exit 122
        }
        exit 0
    }
    timeout {
        catch {exec kill -TERM [exp_pid]}
        after 500
        catch {exec kill -KILL [exp_pid]}
        catch {wait}
        puts "Ctrl+C did not stop a3s web"
        exit 123
    }
}
"#;
    let output = Command::new("/usr/bin/expect")
        .args(["-c", expect_script])
        .env("HOME", directory.path())
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_WEB_INTERRUPT_BIN", env!("CARGO_BIN_EXE_a3s"))
        .env("A3S_WEB_INTERRUPT_ROOT", directory.path())
        .env("A3S_WEB_INTERRUPT_CONFIG", &config_path)
        .env(
            "A3S_WEB_INTERRUPT_DISABLE_SIGNALS",
            if disable_signals_after_startup {
                "1"
            } else {
                "0"
            },
        )
        .output()
        .expect("run foreground Web PTY probe");

    assert!(
        output.status.success(),
        "foreground Web PTY probe (disable signals after startup: \
         {disable_signals_after_startup}) failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct ChildGuard {
    child: Child,
}

impl ChildGuard {
    fn spawn(command: &mut Command) -> Self {
        Self {
            child: command.spawn().expect("start foreground Web process"),
        }
    }

    fn id(&self) -> u32 {
        self.child.id()
    }

    fn wait_timeout(&mut self, timeout: Duration) -> Option<std::process::ExitStatus> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(status) = self.child.try_wait().expect("wait for Web process") {
                return Some(status);
            }
            if Instant::now() >= deadline {
                return None;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn reserve_loopback_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("reserve loopback port");
    listener.local_addr().expect("reserved address").port()
}

fn wait_for_server(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        match TcpStream::connect(("127.0.0.1", port)) {
            Ok(_) => return,
            Err(_) if Instant::now() < deadline => thread::sleep(Duration::from_millis(50)),
            Err(error) => panic!("foreground Web did not listen on port {port}: {error}"),
        }
    }
}

fn open_workspace_stream(port: u16, workspace: &Path) -> TcpStream {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect workspace SSE");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .expect("set workspace SSE read timeout");
    write!(
        stream,
        "GET /api/v1/workspace/watch?rootPath={} HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Accept: text/event-stream\r\n\
         Connection: keep-alive\r\n\r\n",
        workspace.display()
    )
    .expect("request workspace SSE");
    stream.flush().expect("flush workspace SSE request");

    let mut response = Vec::new();
    let mut buffer = [0_u8; 1024];
    while !response
        .windows(b"workspace_watch_ready".len())
        .any(|window| window == b"workspace_watch_ready")
    {
        let read = stream
            .read(&mut buffer)
            .expect("read workspace SSE response");
        assert!(read > 0, "workspace SSE closed before its ready event");
        response.extend_from_slice(&buffer[..read]);
    }
    stream
}

fn test_config() -> &'static str {
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
"#
}
