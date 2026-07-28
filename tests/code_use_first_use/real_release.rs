use std::path::{Path, PathBuf};
use std::process::Command;

use super::support::{portable_release_target, FakeReleaseServer, TempWorkspace};

pub(super) struct RealUseRelease {
    pub(super) server: FakeReleaseServer,
    pub(super) version: String,
}

pub(super) fn start(workspace: &TempWorkspace) -> RealUseRelease {
    let binary = required_path("A3S_USE_E2E_BIN");
    let use_source_root = required_path("A3S_USE_E2E_SOURCE_ROOT");
    let browser_source_root = required_path("A3S_USE_E2E_BROWSER_SOURCE_ROOT");
    let ocr_source_root = required_path("A3S_USE_E2E_OCR_SOURCE_ROOT");
    let browser_driver = binary
        .parent()
        .expect("real Use binary must have a parent directory")
        .join(executable_name("a3s-use-browser-driver"));
    assert!(
        browser_driver.is_file(),
        "real Browser driver is missing at {}",
        browser_driver.display()
    );
    let version = use_version(&binary);
    let target = portable_release_target().expect("test host must support a portable Use release");
    let package_name = format!("a3s-use-{version}-{target}");
    let release_root = workspace.path("real-release");
    let package_root = release_root.join(&package_name);
    std::fs::create_dir_all(&package_root).expect("create real Use package root");

    copy_executable(&binary, &package_root.join(executable_name("a3s-use")));
    copy_executable(
        &browser_driver,
        &package_root.join(executable_name("a3s-use-browser-driver")),
    );
    package_ocr_models(&binary, &package_root);
    for (source, destination) in [
        ("crates/browser-driver/skills", "skills"),
        ("crates/browser-driver/skill-data", "skill-data"),
        ("crates/browser-driver/dashboard/out", "dashboard"),
    ] {
        copy_tree(
            &browser_source_root.join(source),
            &package_root.join(destination),
        );
    }
    copy_tree(
        &ocr_source_root.join("skills"),
        &package_root.join("ocr-skills"),
    );
    for source in ["LICENSE", "README.md", "THIRD_PARTY_NOTICES.md"] {
        std::fs::copy(use_source_root.join(source), package_root.join(source))
            .unwrap_or_else(|error| panic!("failed to package {source}: {error}"));
    }
    for source in ["LICENSE-APACHE-2.0", "UPSTREAM.md"] {
        std::fs::copy(
            browser_source_root
                .join("crates/browser-driver")
                .join(source),
            package_root.join(source),
        )
        .unwrap_or_else(|error| panic!("failed to package {source}: {error}"));
    }

    let archive_name = if cfg!(windows) {
        format!("{package_name}.zip")
    } else {
        format!("{package_name}.tar.gz")
    };
    let archive_path = workspace.path(&archive_name);
    let archive = create_archive(&package_root, &archive_path);
    RealUseRelease {
        server: FakeReleaseServer::start("Use", &version, &archive_name, archive),
        version,
    }
}

fn package_ocr_models(binary: &Path, package_root: &Path) {
    let model_root = package_root.join("ocr-models");
    let output = Command::new(binary)
        .args(["component", "install", "ocr", "--force", "--json"])
        .env("A3S_USE_OCR_HOME", &model_root)
        .env_remove("A3S_OCR_MODEL_DIR")
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to package PP-OCRv6 models with {}: {error}",
                binary.display()
            )
        });
    assert!(
        output.status.success(),
        "failed to package PP-OCRv6 models: stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let status: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("decode PP-OCRv6 install response");
    assert_eq!(status["ok"], true);
    assert_eq!(status["data"]["runtime"]["model"], "PP-OCRv6_small");
    let lock = model_root.join(".install.lock");
    if lock.exists() {
        std::fs::remove_file(&lock)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", lock.display()));
    }
    for path in [
        "PP-OCRv6_small/det/inference.onnx",
        "PP-OCRv6_small/det/inference.yml",
        "PP-OCRv6_small/rec/inference.onnx",
        "PP-OCRv6_small/rec/inference.yml",
    ] {
        assert!(
            model_root.join(path).is_file(),
            "packaged PP-OCRv6 model is missing {path}"
        );
    }
}

fn required_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must point to the real Use checkout artifact"))
}

fn executable_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

fn use_version(binary: &Path) -> String {
    let output = Command::new(binary)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| panic!("failed to run {}: {error}", binary.display()));
    assert!(
        output.status.success(),
        "{} --version failed: {}",
        binary.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("Use version output must be UTF-8");
    stdout
        .split_whitespace()
        .last()
        .filter(|version| !version.is_empty())
        .map(str::to_string)
        .expect("Use version output must end with a version")
}

#[cfg(not(windows))]
fn create_archive(package_root: &Path, archive_path: &Path) -> Vec<u8> {
    let status = Command::new("tar")
        .arg("czf")
        .arg(archive_path)
        .arg("-C")
        .arg(package_root)
        .arg(".")
        .status()
        .expect("create real Use release archive");
    assert!(
        status.success(),
        "failed to create real Use release archive"
    );
    std::fs::read(archive_path).expect("read real Use release archive")
}

#[cfg(windows)]
fn create_archive(package_root: &Path, archive_path: &Path) -> Vec<u8> {
    use std::io::{Cursor, Write};

    fn append_directory(
        writer: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
        root: &Path,
        directory: &Path,
    ) {
        let mut entries = std::fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()))
            .map(|entry| entry.expect("read release archive entry"))
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .expect("release archive entry must be inside its package root");
            let archive_name = relative.to_string_lossy().replace('\\', "/");
            let file_type = entry.file_type().expect("inspect release archive entry");
            if file_type.is_dir() {
                writer
                    .add_directory(
                        format!("{archive_name}/"),
                        zip::write::SimpleFileOptions::default(),
                    )
                    .expect("add release archive directory");
                append_directory(writer, root, &path);
            } else if file_type.is_file() {
                writer
                    .start_file(
                        archive_name,
                        zip::write::SimpleFileOptions::default()
                            .compression_method(zip::CompressionMethod::Deflated),
                    )
                    .expect("add release archive file");
                let bytes = std::fs::read(&path)
                    .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
                writer
                    .write_all(&bytes)
                    .expect("write release archive file");
            } else {
                panic!(
                    "release source contains an unsupported entry: {}",
                    path.display()
                );
            }
        }
    }

    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    append_directory(&mut writer, package_root, package_root);
    let archive = writer
        .finish()
        .expect("finish real Use release archive")
        .into_inner();
    std::fs::write(archive_path, &archive).expect("write real Use release archive");
    archive
}

fn copy_executable(source: &Path, destination: &Path) {
    std::fs::copy(source, destination).unwrap_or_else(|error| {
        panic!(
            "failed to copy executable {} to {}: {error}",
            source.display(),
            destination.display()
        )
    });
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|error| panic!("failed to chmod {}: {error}", destination.display()));
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination)
        .unwrap_or_else(|error| panic!("failed to create {}: {error}", destination.display()));
    for entry in std::fs::read_dir(source)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", source.display()))
    {
        let entry = entry.expect("read release source entry");
        let file_type = entry.file_type().expect("inspect release source entry");
        let target = destination.join(entry.file_name());
        assert!(
            !file_type.is_symlink(),
            "release source must not contain symlinks: {}",
            entry.path().display()
        );
        if file_type.is_dir() {
            copy_tree(&entry.path(), &target);
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target).unwrap_or_else(|error| {
                panic!(
                    "failed to copy release file {} to {}: {error}",
                    entry.path().display(),
                    target.display()
                )
            });
        } else {
            panic!(
                "release source contains an unsupported entry: {}",
                entry.path().display()
            );
        }
    }
}
