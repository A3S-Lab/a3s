//! `/kb`: ingest typed text, a file, or a folder into the local personal
//! knowledge base at `.a3s/kb/sources/`. Shareable OKF knowledge-package assets
//! live under `.a3s/okf` and are managed by `/okf`.

use std::path::{Path, PathBuf};

/// The local personal KB vault root (`<cwd>/.a3s/kb`).
pub(crate) fn kb_dir(cwd: &str) -> PathBuf {
    Path::new(cwd).join(".a3s").join("kb")
}

/// Cap a folder ingest so `/kb .` on a big tree can't run away.
const MAX_DIR_FILES: usize = 300;
/// Skip any single file larger than this (KB stores text notes, not blobs).
const MAX_FILE_BYTES: u64 = 1_048_576; // 1 MiB
const MAX_SEARCH_HITS: usize = 30;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct KbStats {
    pub(crate) sources: usize,
    pub(crate) concepts: usize,
    pub(crate) imports: usize,
    pub(crate) bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImportKind {
    File,
    Folder,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImportPreview {
    pub(crate) arg: String,
    pub(crate) path: PathBuf,
    pub(crate) kind: ImportKind,
    pub(crate) addable: usize,
    pub(crate) skipped: usize,
    pub(crate) capped: bool,
    pub(crate) bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImportOutcome {
    pub(crate) source: PathBuf,
    pub(crate) destination: PathBuf,
    pub(crate) kind: ImportKind,
    pub(crate) added: usize,
    pub(crate) skipped: usize,
    pub(crate) capped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SearchHit {
    pub(crate) path: String,
    pub(crate) line: usize,
    pub(crate) snippet: String,
}

pub(crate) fn kb_stats(cwd: &str) -> KbStats {
    let kb = kb_dir(cwd);
    KbStats {
        sources: count_regular_files(&kb.join("sources"), true),
        concepts: count_md(&kb.join("wiki")),
        imports: count_source_log(&kb.join("sources").join("SOURCES.md")),
        bytes: dir_bytes(&kb),
    }
}

pub(crate) fn recent_sources(cwd: &str, limit: usize) -> Vec<String> {
    let root = kb_dir(cwd).join("sources");
    let mut files = Vec::new();
    collect_files(&root, &mut files);
    files.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .map(std::cmp::Reverse)
    });
    files
        .into_iter()
        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("SOURCES.md"))
        .take(limit)
        .map(|p| show(cwd, &p))
        .collect()
}

pub(crate) fn add_text_to_kb(cwd: &str, text: &str, now: &str) -> String {
    capture_text(cwd, text, now)
        .map(|dest| format!("✔ captured note to KB · {}", show(cwd, &dest)))
        .unwrap_or_else(|e| format!("✗ /kb add failed: {e}"))
}

pub(crate) fn import_to_kb(cwd: &str, arg: &str, now: &str) -> String {
    import_source(cwd, arg, now)
        .map(|outcome| match outcome.kind {
            ImportKind::File => {
                format!("✔ added file to KB · {}", show(cwd, &outcome.destination))
            }
            ImportKind::Folder => {
                let mut note = String::new();
                if outcome.skipped > 0 {
                    note.push_str(&format!(" ({} skipped)", outcome.skipped));
                }
                if outcome.capped {
                    note.push_str(&format!(" (capped at {MAX_DIR_FILES} — folder truncated)"));
                }
                format!(
                    "✔ added {} file(s) from {arg} to KB{note} · {}/",
                    outcome.added,
                    show(cwd, &outcome.destination)
                )
            }
        })
        .unwrap_or_else(|e| format!("✗ /kb import failed: {e}"))
}

pub(crate) fn capture_text(cwd: &str, text: &str, now: &str) -> std::io::Result<PathBuf> {
    let text = text.trim();
    if text.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "text must not be empty",
        ));
    }
    ingest_text(text, &kb_dir(cwd).join("sources"), now)
}

pub(crate) fn import_source(cwd: &str, arg: &str, now: &str) -> std::io::Result<ImportOutcome> {
    let arg = arg.trim();
    let sources = kb_dir(cwd).join("sources");
    let path = resolve_path(cwd, arg);
    if path.is_file() {
        let destination = ingest_file(&path, &sources)?;
        log_source(&sources, now, "file", arg, &destination);
        Ok(ImportOutcome {
            source: path,
            destination,
            kind: ImportKind::File,
            added: 1,
            skipped: 0,
            capped: false,
        })
    } else if path.is_dir() {
        let destination = sources.join(dir_name(&path));
        let (added, skipped, capped) = ingest_dir(&path, &sources)?;
        log_source(&sources, now, "folder", arg, &destination);
        Ok(ImportOutcome {
            source: path,
            destination,
            kind: ImportKind::Folder,
            added,
            skipped,
            capped,
        })
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "path not found",
        ))
    }
}

pub(crate) fn preview_import(cwd: &str, arg: &str) -> Result<ImportPreview, String> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Err("usage: /kb import <file|folder>".to_string());
    }
    let path = resolve_path(cwd, arg);
    if path.is_file() {
        let bytes = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if !is_text_file(&path).map_err(|e| e.to_string())? {
            return Err("not a text file (KB stores text)".to_string());
        }
        return Ok(ImportPreview {
            arg: arg.to_string(),
            path,
            kind: ImportKind::File,
            addable: 1,
            skipped: 0,
            capped: false,
            bytes,
        });
    }
    if path.is_dir() {
        let (addable, skipped, capped, bytes) = preview_dir(&path);
        return Ok(ImportPreview {
            arg: arg.to_string(),
            path,
            kind: ImportKind::Folder,
            addable,
            skipped,
            capped,
            bytes,
        });
    }
    Err("path not found · use `/kb add <text>` to add a text note".to_string())
}

pub(crate) fn search_kb(cwd: &str, query: &str) -> Vec<SearchHit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let mut files = Vec::new();
    collect_files(&kb_dir(cwd), &mut files);
    let mut hits = Vec::new();
    for path in files {
        if hits.len() >= MAX_SEARCH_HITS {
            break;
        }
        if !is_text_file(&path).unwrap_or(false) {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (idx, line) in body.lines().enumerate() {
            if line.to_lowercase().contains(&q) {
                hits.push(SearchHit {
                    path: show(cwd, &path),
                    line: idx + 1,
                    snippet: line.trim().chars().take(180).collect(),
                });
                break;
            }
        }
    }
    hits
}

/// Ingest `arg` into the KB and return a one-line human summary. `now` is an
/// RFC3339 timestamp (injected so the logic is testable).
#[cfg(test)]
pub(crate) fn add_to_kb(cwd: &str, arg: &str, now: &str) -> String {
    let arg = arg.trim();
    if arg.is_empty() {
        let n = kb_stats(cwd).sources;
        return format!(
            "KB at .a3s/kb · {n} source(s). usage: /kb add <text> | /kb import <file|folder>"
        );
    }
    let path = resolve_path(cwd, arg);
    if path.is_file() || path.is_dir() {
        import_to_kb(cwd, arg, now)
    } else {
        add_text_to_kb(cwd, arg, now)
    }
}

/// Capture typed text as a local KB note (frontmatter + body).
fn ingest_text(text: &str, sources: &Path, now: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(sources)?;
    let title = text.lines().next().unwrap_or("note").trim();
    let dest = unique_path(&sources.join(format!("{}.md", slug(title))));
    let body = format!("---\ntype: note\nsource: user\nadded: {now}\n---\n\n{text}\n");
    std::fs::write(&dest, body)?;
    Ok(dest)
}

/// Copy one text file into the vault verbatim.
fn ingest_file(file: &Path, sources: &Path) -> std::io::Result<PathBuf> {
    if !is_text_file(file)? {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "not a text file (KB stores text)",
        ));
    }
    std::fs::create_dir_all(sources)?;
    let name = file
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("source");
    let dest = unique_path(&sources.join(name));
    std::fs::copy(file, &dest)?;
    Ok(dest)
}

/// Copy a folder's text files into `sources/<dirname>/…`, preserving structure.
/// Skips hidden entries + `target`/`node_modules`, binaries, and oversized files.
fn ingest_dir(dir: &Path, sources: &Path) -> std::io::Result<(usize, usize, bool)> {
    let root_dest = sources.join(dir_name(dir));
    let (mut added, mut skipped) = (0usize, 0usize);
    let mut capped = false;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if added >= MAX_DIR_FILES {
            capped = true;
            break;
        }
        let rd = match std::fs::read_dir(&d) {
            Ok(r) => r,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || matches!(name, "target" | "node_modules") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if added >= MAX_DIR_FILES {
                capped = true;
                break;
            }
            if !is_text_file(&p).unwrap_or(false) {
                skipped += 1;
                continue;
            }
            let rel = p.strip_prefix(dir).unwrap_or(&p);
            let dest = root_dest.join(rel);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if std::fs::copy(&p, &dest).is_ok() {
                added += 1;
            } else {
                skipped += 1;
            }
        }
    }
    Ok((added, skipped, capped))
}

/// A file is "text" if it's under the size cap and its first 8 KiB have no NUL.
fn is_text_file(p: &Path) -> std::io::Result<bool> {
    if std::fs::metadata(p)?.len() > MAX_FILE_BYTES {
        return Ok(false);
    }
    use std::io::Read;
    let mut buf = [0u8; 8192];
    let n = std::fs::File::open(p)?.read(&mut buf)?;
    Ok(!buf[..n].contains(&0))
}

fn preview_dir(dir: &Path) -> (usize, usize, bool, u64) {
    let (mut addable, mut skipped, mut bytes) = (0usize, 0usize, 0u64);
    let mut capped = false;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if addable >= MAX_DIR_FILES {
            capped = true;
            break;
        }
        let Ok(rd) = std::fs::read_dir(&d) else {
            skipped += 1;
            continue;
        };
        for entry in rd.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || matches!(name, "target" | "node_modules") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if addable >= MAX_DIR_FILES {
                capped = true;
                break;
            }
            if is_text_file(&p).unwrap_or(false) {
                addable += 1;
                bytes += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            } else {
                skipped += 1;
            }
        }
    }
    (addable, skipped, capped, bytes)
}

/// Append a provenance line to `sources/SOURCES.md` (copied files carry no
/// frontmatter, so this is where their origin is recorded).
fn log_source(sources: &Path, now: &str, kind: &str, origin: &str, dest: &Path) {
    let _ = std::fs::create_dir_all(sources);
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let line = format!("- {now} · {kind} · {origin} → {name}\n");
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(sources.join("SOURCES.md"))
    {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Count `.md` files under the vault (recursive) for the status line.
fn count_md(kb: &Path) -> usize {
    let mut n = 0;
    let mut stack = vec![kb.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().and_then(|x| x.to_str()) == Some("md") {
                    n += 1;
                }
            }
        }
    }
    n
}

fn count_regular_files(root: &Path, exclude_sources_log: bool) -> usize {
    let mut files = Vec::new();
    collect_files(root, &mut files);
    files
        .into_iter()
        .filter(|p| {
            !exclude_sources_log || p.file_name().and_then(|n| n.to_str()) != Some("SOURCES.md")
        })
        .count()
}

fn count_source_log(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|s| {
            s.lines()
                .filter(|l| l.trim_start().starts_with("- "))
                .count()
        })
        .unwrap_or(0)
}

fn dir_bytes(root: &Path) -> u64 {
    let mut files = Vec::new();
    collect_files(root, &mut files);
    files
        .into_iter()
        .filter_map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
        .sum()
}

fn collect_files(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(root) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

/// Filesystem-safe slug from a title (keeps unicode letters/digits, e.g. CJK).
fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.trim_matches('-').chars().take(48).collect();
    if out.is_empty() {
        "note".to_string()
    } else {
        out
    }
}

/// Return `p`, or `p` with a `-2`/`-3`/… suffix if it already exists.
fn unique_path(p: &Path) -> PathBuf {
    if !p.exists() {
        return p.to_path_buf();
    }
    let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = p.extension().and_then(|e| e.to_str());
    let parent = p.parent().unwrap_or_else(|| Path::new("."));
    for i in 2..10_000 {
        let name = match ext {
            Some(e) => format!("{stem}-{i}.{e}"),
            None => format!("{stem}-{i}"),
        };
        let cand = parent.join(name);
        if !cand.exists() {
            return cand;
        }
    }
    p.to_path_buf()
}

fn resolve_path(cwd: &str, arg: &str) -> PathBuf {
    if arg == "~" || arg.starts_with("~/") {
        if let Some(home) = crate::user_paths::user_home_dir() {
            let rest = arg.strip_prefix("~/").unwrap_or("");
            return home.join(rest);
        }
    }
    let p = Path::new(arg);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        Path::new(cwd).join(p)
    }
}

fn dir_name(p: &Path) -> String {
    p.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("folder")
        .to_string()
}

fn show(cwd: &str, p: &Path) -> String {
    p.strip_prefix(cwd).unwrap_or(p).display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        // Unique per call (atomic counter) so parallel tests never share a dir.
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let id = N.fetch_add(1, Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("a3s-kb-{}-{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn captures_typed_text_as_a_note() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let out = add_to_kb(
            cwds,
            "Decision: use ACL over TOML for config",
            "2026-07-01T00:00:00Z",
        );
        assert!(out.contains("captured note"), "{out}");
        // A single .md note landed under sources with frontmatter + body.
        let src = kb_dir(cwds).join("sources");
        let note = std::fs::read_dir(&src)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
            .unwrap();
        let body = std::fs::read_to_string(&note).unwrap();
        assert!(body.contains("type: note") && body.contains("source: user"));
        assert!(body.contains("use ACL over TOML"));
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn ingests_a_file_verbatim() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let f = cwd.join("notes.txt");
        std::fs::write(&f, "hello kb").unwrap();
        let out = add_to_kb(cwds, f.to_str().unwrap(), "2026-07-01T00:00:00Z");
        assert!(out.contains("added file"), "{out}");
        let copied = kb_dir(cwds).join("sources").join("notes.txt");
        assert_eq!(std::fs::read_to_string(&copied).unwrap(), "hello kb"); // verbatim
        assert!(kb_dir(cwds).join("sources/SOURCES.md").exists()); // provenance logged
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn ingests_folder_text_files_and_skips_binary() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let dir = cwd.join("docs");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.md"), "alpha").unwrap();
        std::fs::write(dir.join("sub/b.md"), "beta").unwrap();
        std::fs::write(dir.join("bin.dat"), [0u8, 1, 2, 0]).unwrap(); // binary → skipped
        let out = add_to_kb(cwds, dir.to_str().unwrap(), "2026-07-01T00:00:00Z");
        assert!(out.contains("added 2 file(s)"), "{out}");
        let base = kb_dir(cwds).join("sources/docs");
        assert!(base.join("a.md").exists() && base.join("sub/b.md").exists());
        assert!(!base.join("bin.dat").exists()); // binary excluded
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn folder_cap_is_surfaced_not_silent() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let dir = cwd.join("big");
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..(MAX_DIR_FILES + 5) {
            std::fs::write(dir.join(format!("f{i}.md")), "x").unwrap();
        }
        let out = add_to_kb(cwds, dir.to_str().unwrap(), "2026-07-01T00:00:00Z");
        assert!(out.contains("capped at"), "{out}"); // truncation must be visible
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn empty_arg_reports_status() {
        let cwd = tmp();
        let out = add_to_kb(cwd.to_str().unwrap(), "  ", "2026-07-01T00:00:00Z");
        assert!(out.contains("KB at .a3s/kb") && out.contains("usage:"));
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn explicit_add_text_and_import_path() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let note = add_text_to_kb(cwds, "A reusable API note", "2026-07-01T00:00:00Z");
        assert!(note.contains("captured note"), "{note}");

        let f = cwd.join("source.md");
        std::fs::write(&f, "Runtime evidence").unwrap();
        let imported = import_to_kb(cwds, f.to_str().unwrap(), "2026-07-01T00:00:00Z");
        assert!(imported.contains("added file"), "{imported}");
        assert!(kb_dir(cwds).join("sources/source.md").exists());
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn preview_import_file_and_folder_counts() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let file = cwd.join("one.txt");
        std::fs::write(&file, "one").unwrap();
        let p = preview_import(cwds, file.to_str().unwrap()).unwrap();
        assert_eq!(p.kind, ImportKind::File);
        assert_eq!(p.addable, 1);
        assert_eq!(p.bytes, 3);

        let dir = cwd.join("docs");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.md"), "alpha").unwrap();
        std::fs::write(dir.join("blob.bin"), [0u8, 1, 2]).unwrap();
        let p = preview_import(cwds, dir.to_str().unwrap()).unwrap();
        assert_eq!(p.kind, ImportKind::Folder);
        assert_eq!(p.addable, 1);
        assert_eq!(p.skipped, 1);
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn search_kb_finds_source_lines() {
        let cwd = tmp();
        let cwds = cwd.to_str().unwrap();
        let _ = add_text_to_kb(
            cwds,
            "Alpha\nThe Runtime needs parallel workers\nOmega",
            "2026-07-01T00:00:00Z",
        );
        let hits = search_kb(cwds, "parallel workers");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert_eq!(hits[0].line, 8); // frontmatter + blank + body line 2
        assert!(hits[0].snippet.contains("Runtime needs parallel workers"));
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn missing_import_path_guides_to_add() {
        let cwd = tmp();
        let err = preview_import(cwd.to_str().unwrap(), "missing.md").unwrap_err();
        assert!(err.contains("/kb add <text>"), "{err}");
        let out = import_to_kb(cwd.to_str().unwrap(), "missing.md", "2026-07-01T00:00:00Z");
        assert!(out.contains("/kb import failed"), "{out}");
        let _ = std::fs::remove_dir_all(&cwd);
    }

    #[test]
    fn resolve_path_expands_home_prefix() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(
            resolve_path("/tmp/project", "~/notes.md"),
            Path::new(&home).join("notes.md")
        );
        assert_eq!(resolve_path("/tmp/project", "~"), PathBuf::from(home));
    }

    #[test]
    fn slug_keeps_unicode_and_dedupes() {
        assert_eq!(slug("Hello, World!"), "hello-world");
        assert_eq!(slug("Café ACL Config"), "café-acl-config");
        assert_eq!(slug("   "), "note");
    }
}
