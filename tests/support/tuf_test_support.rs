#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use olpc_cjson::CanonicalFormatter;
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde::Serialize;
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

pub(crate) const FUTURE: &str = "2999-01-01T00:00:00Z";
pub(crate) const EXPIRED: &str = "2000-01-01T00:00:00Z";
pub(crate) const PACKAGE_VERSION: &str = "0.1.1";

pub(crate) struct TestRepository {
    pub(crate) routes: HashMap<String, Vec<u8>>,
    pub(crate) root_sha256: String,
    pub(crate) target_name: String,
    pub(crate) target_sha256: String,
}

impl TestRepository {
    pub(crate) fn new(archive: Vec<u8>, metadata_version: u64, expires: &str) -> Self {
        Self::with_package_version(archive, PACKAGE_VERSION, metadata_version, expires)
    }

    pub(crate) fn with_package_version(
        archive: Vec<u8>,
        package_version: &str,
        metadata_version: u64,
        expires: &str,
    ) -> Self {
        let key = Ed25519KeyPair::from_seed_unchecked(&[7_u8; 32]).unwrap();
        let public = hex_lower(key.public_key().as_ref());
        let key_value = json!({
            "keytype": "ed25519",
            "scheme": "ed25519",
            "keyval": {"public": public}
        });
        let key_id = sha256(&canonical(&key_value));
        let role = json!({"keyids": [key_id.clone()], "threshold": 1});
        let mut keys = Map::new();
        keys.insert(key_id.clone(), key_value);
        let root_signed = json!({
            "_type": "root",
            "spec_version": "1.0.0",
            "consistent_snapshot": false,
            "version": 1,
            "expires": FUTURE,
            "keys": keys,
            "roles": {
                "root": role.clone(),
                "snapshot": role.clone(),
                "targets": role.clone(),
                "timestamp": role
            }
        });
        let root = signed_document(&key, &key_id, root_signed);
        let root_sha256 = sha256(&root);

        let target = host_target();
        let archive_name = format!("a3s-use-a3s-science-{package_version}-{target}.tar.gz");
        let target_name =
            format!("extensions/a3s/science/{package_version}/stable/{target}/{archive_name}");
        let target_sha256 = sha256(&archive);
        let mut targets_map = Map::new();
        targets_map.insert(
            target_name.clone(),
            json!({
                "length": archive.len(),
                "hashes": {"sha256": target_sha256},
                "custom": {
                    "a3s": {
                        "schemaVersion": 1,
                        "packageId": "a3s/science",
                        "version": package_version,
                        "channel": "stable",
                        "target": target
                    }
                }
            }),
        );
        let targets_signed = json!({
            "_type": "targets",
            "spec_version": "1.0.0",
            "version": metadata_version,
            "expires": expires,
            "targets": targets_map
        });
        let targets = signed_document(&key, &key_id, targets_signed);
        let snapshot_signed = json!({
            "_type": "snapshot",
            "spec_version": "1.0.0",
            "version": metadata_version,
            "expires": expires,
            "meta": {
                "targets.json": {
                    "version": metadata_version,
                    "length": targets.len(),
                    "hashes": {"sha256": sha256(&targets)}
                }
            }
        });
        let snapshot = signed_document(&key, &key_id, snapshot_signed);
        let timestamp_signed = json!({
            "_type": "timestamp",
            "spec_version": "1.0.0",
            "version": metadata_version,
            "expires": expires,
            "meta": {
                "snapshot.json": {
                    "version": metadata_version,
                    "length": snapshot.len(),
                    "hashes": {"sha256": sha256(&snapshot)}
                }
            }
        });
        let timestamp = signed_document(&key, &key_id, timestamp_signed);

        let routes = HashMap::from([
            ("/metadata/root.json".to_string(), root),
            ("/metadata/timestamp.json".to_string(), timestamp),
            ("/metadata/snapshot.json".to_string(), snapshot),
            ("/metadata/targets.json".to_string(), targets),
            (format!("/targets/{target_name}"), archive),
        ]);
        Self {
            routes,
            root_sha256,
            target_name,
            target_sha256,
        }
    }
}

fn signed_document(key: &Ed25519KeyPair, key_id: &str, signed: Value) -> Vec<u8> {
    let signature = key.sign(&canonical(&signed));
    serde_json::to_vec(&json!({
        "signatures": [{"keyid": key_id, "sig": hex_lower(signature.as_ref())}],
        "signed": signed
    }))
    .unwrap()
}

fn canonical(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut serializer =
        serde_json::Serializer::with_formatter(&mut bytes, CanonicalFormatter::new());
    value.serialize(&mut serializer).unwrap();
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub(crate) fn extension_archive(version: &str) -> Vec<u8> {
    let manifest = format!(
        "extension \"a3s/science\" {{\n  schema_version = 1\n  version = \"{version}\"\n  route = \"science\"\n  actions = [\"read\"]\n\n  cli {{\n    executable = \"bin/a3s-use-a3s-science\"\n    json_output = true\n  }}\n}}\n"
    );
    let mut bytes = Vec::new();
    {
        let encoder = flate2::write::GzEncoder::new(&mut bytes, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        append_tar_file(
            &mut archive,
            "package/a3s-use-extension.acl",
            0o644,
            manifest.as_bytes(),
        );
        append_tar_file(
            &mut archive,
            "package/bin/a3s-use-a3s-science",
            0o755,
            b"#!/bin/sh\nprintf 'slack fixture\\n'\n",
        );
        archive.finish().unwrap();
    }
    bytes
}

fn append_tar_file<W: Write>(archive: &mut tar::Builder<W>, path: &str, mode: u32, body: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_path(path).unwrap();
    header.set_size(body.len() as u64);
    header.set_mode(mode);
    header.set_cksum();
    archive.append(&header, body).unwrap();
}

pub(crate) fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x86_64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "x86_64") => "linux-x86_64",
        ("windows", "x86_64") => "windows-x86_64",
        (os, arch) => panic!("unsupported TUF test target {os}-{arch}"),
    }
}

pub(crate) struct TestServer {
    base_url: String,
    routes: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    pub(crate) fn start(routes: HashMap<String, Vec<u8>>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let base_url = format!("http://{}/", listener.local_addr().unwrap());
        let routes = Arc::new(Mutex::new(routes));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_routes = Arc::clone(&routes);
        let thread_requests = Arc::clone(&requests);
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        let routes = Arc::clone(&thread_routes);
                        let requests = Arc::clone(&thread_requests);
                        std::thread::spawn(move || serve(stream, &routes, &requests));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base_url,
            routes,
            requests,
            stop,
            thread: Some(thread),
        }
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn requests(&self) -> Vec<String> {
        self.requests.lock().unwrap().clone()
    }

    pub(crate) fn clear_requests(&self) {
        self.requests.lock().unwrap().clear();
    }

    pub(crate) fn replace_routes(&self, routes: HashMap<String, Vec<u8>>) {
        *self.routes.lock().unwrap() = routes;
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn serve(
    mut stream: TcpStream,
    routes: &Mutex<HashMap<String, Vec<u8>>>,
    requests: &Mutex<Vec<String>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let mut buffer = [0_u8; 8192];
    let Ok(size) = stream.read(&mut buffer) else {
        return;
    };
    let request = String::from_utf8_lossy(&buffer[..size]);
    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/")
        .to_string();
    requests.lock().unwrap().push(path.clone());
    let body = routes.lock().unwrap().get(&path).cloned();
    let (status, body) = body
        .as_deref()
        .map(|body| ("200 OK", body))
        .unwrap_or(("404 Not Found", b"not found"));
    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    if stream.write_all(header.as_bytes()).is_ok() && stream.write_all(body).is_ok() {
        let _ = stream.flush();
        let _ = stream.shutdown(Shutdown::Write);
    }
}
