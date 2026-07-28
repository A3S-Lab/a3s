use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

pub(super) struct FixtureSite {
    pub(super) url: String,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FixtureSite {
    pub(super) fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind Browser fixture server");
        listener
            .set_nonblocking(true)
            .expect("configure Browser fixture listener");
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let mut request = [0_u8; 8 * 1024];
                        let _ = stream.read(&mut request);
                        let body = browser_fixture_html().as_bytes();
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(body);
                        let _ = stream.flush();
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            url,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for FixtureSite {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn browser_fixture_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>A3S Windows Browser E2E</title>
</head>
<body style="min-height: 1600px">
  <main>
    <h1>A3S Use Browser fixture</h1>
    <label for="name">Name</label>
    <input id="name" name="name" placeholder="Enter a name">
    <label><input id="enabled" type="checkbox"> Enabled</label>
    <label for="choice">Choice</label>
    <select id="choice">
      <option value="one">One</option>
      <option value="two">Two</option>
    </select>
    <button id="run" type="button"
      onclick="document.querySelector('#status').textContent='clicked'">Run</button>
    <p id="status">ready</p>
  </main>
</body>
</html>
"#
}

pub(super) fn prepare_use_install(binary: &Path, source_root: &Path, install_root: &Path) {
    std::fs::create_dir_all(install_root).expect("create Use E2E install root");
    let browser_driver = binary
        .parent()
        .expect("Use binary must have a parent")
        .join("a3s-use-browser-driver.exe");
    assert!(
        browser_driver.is_file(),
        "missing Browser driver at {}",
        browser_driver.display()
    );
    copy_file(binary, &install_root.join("a3s-use.exe"));
    copy_file(
        &browser_driver,
        &install_root.join("a3s-use-browser-driver.exe"),
    );
    for (source, destination) in [
        ("crates/browser-driver/skills", "skills"),
        ("crates/browser-driver/skill-data", "skill-data"),
        ("crates/office/skills", "office-skills"),
        ("crates/ocr/skills", "ocr-skills"),
        ("crates/browser-driver/dashboard/out", "dashboard"),
    ] {
        copy_tree(&source_root.join(source), &install_root.join(destination));
    }
}

fn copy_file(source: &Path, destination: &Path) {
    std::fs::copy(source, destination).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} to {}: {error}",
            source.display(),
            destination.display()
        )
    });
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", destination.display()));
    for entry in std::fs::read_dir(source)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()))
    {
        let entry = entry.expect("read Use package entry");
        let file_type = entry.file_type().expect("inspect Use package entry");
        let destination = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else if file_type.is_file() {
            copy_file(&entry.path(), &destination);
        } else {
            panic!(
                "Use package contains an unsupported entry: {}",
                entry.path().display()
            );
        }
    }
}

pub(super) fn prepare_webview_probe(install_root: &Path) {
    std::fs::create_dir_all(install_root).expect("create WebView E2E install root");
    let mut binary = vec![0_u8; 0x80];
    binary[..2].copy_from_slice(b"MZ");
    binary[0x3c..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
    binary[0x40..0x44].copy_from_slice(b"PE\0\0");
    binary[0x44..0x46].copy_from_slice(&0x8664_u16.to_le_bytes());
    binary.extend_from_slice(
        b"usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>",
    );
    binary.extend_from_slice(b"a3s.system_agent_snapshot.v1");
    std::fs::write(install_root.join("a3s-webview.exe"), binary).expect("write WebView E2E probe");
}

pub(super) fn create_ocr_fixture(path: &Path) {
    use image::{Rgb, RgbImage};

    let mut image = RgbImage::from_pixel(360, 112, Rgb([255, 255, 255]));
    let glyphs = [
        (
            'A',
            [
                "01110", "10001", "10001", "11111", "10001", "10001", "10001",
            ],
        ),
        (
            '3',
            [
                "11110", "00001", "00001", "01110", "00001", "00001", "11110",
            ],
        ),
        (
            'S',
            [
                "01111", "10000", "10000", "01110", "00001", "00001", "11110",
            ],
        ),
        (
            '4',
            [
                "00110", "01010", "10010", "11111", "00010", "00010", "00010",
            ],
        ),
        (
            '2',
            [
                "11110", "00001", "00001", "01110", "10000", "10000", "11111",
            ],
        ),
    ];
    let scale = 10_u32;
    let mut origin_x = 18_u32;
    for character in "A3S 42".chars() {
        if character == ' ' {
            origin_x += scale * 3;
            continue;
        }
        let rows = glyphs
            .iter()
            .find_map(|(candidate, rows)| (*candidate == character).then_some(rows))
            .expect("OCR fixture glyph");
        for (row, pattern) in rows.iter().enumerate() {
            for (column, pixel) in pattern.bytes().enumerate() {
                if pixel != b'1' {
                    continue;
                }
                for y in 0..scale {
                    for x in 0..scale {
                        image.put_pixel(
                            origin_x + u32::try_from(column).unwrap() * scale + x,
                            18 + u32::try_from(row).unwrap() * scale + y,
                            Rgb([0, 0, 0]),
                        );
                    }
                }
            }
        }
        origin_x += scale * 6;
    }
    image.save(path).expect("write OCR fixture PNG");
}

pub(super) fn required_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must point to a real Use checkout artifact"))
}
