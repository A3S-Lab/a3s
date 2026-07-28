//! Clipboard and durable Markdown sharing for the current TUI session.

use std::io::Write;
use std::path::{Component, Path, PathBuf};

use chrono::{DateTime, SecondsFormat, Utc};

use super::*;

const MAX_EXPORT_PATH_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CopyTarget {
    LatestAssistant,
    Transcript,
}

fn parse_copy_target(rest: &str) -> Result<CopyTarget, &'static str> {
    match rest.trim() {
        "" => Ok(CopyTarget::LatestAssistant),
        "transcript" => Ok(CopyTarget::Transcript),
        _ => Err("usage: /copy [transcript]"),
    }
}

impl App {
    pub(super) fn submit_copy_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let target = match parse_copy_target(rest) {
            Ok(target) => target,
            Err(usage) => {
                self.push_notice(NoticeKind::Warning, usage);
                return None;
            }
        };

        let (subject, content) = match target {
            CopyTarget::LatestAssistant => {
                let content = self
                    .messages
                    .latest_assistant_markdown(Some(self.streaming.raw_content()));
                ("latest assistant response", content)
            }
            CopyTarget::Transcript => (
                "session transcript",
                self.session_markdown_document(Utc::now()),
            ),
        };
        let Some(content) = content else {
            self.push_notice(
                NoticeKind::Info,
                match target {
                    CopyTarget::LatestAssistant => "No assistant response to copy yet",
                    CopyTarget::Transcript => "No semantic transcript to copy yet",
                },
            );
            return None;
        };

        let character_count = content.chars().count();
        let outcome = copy_to_clipboard(&content);
        let (kind, feedback) = clipboard_feedback(subject, character_count, outcome);
        self.push_notice(kind, feedback);
        None
    }

    pub(super) fn submit_export_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let now = Utc::now();
        let Some(document) = self.session_markdown_document(now) else {
            self.push_notice(NoticeKind::Info, "No semantic transcript to export yet");
            return None;
        };
        let relative = match export_relative_path(rest, &self.session_id, now) {
            Ok(path) => path,
            Err(error) => {
                self.push_notice(NoticeKind::Warning, error);
                return None;
            }
        };
        let path = match resolve_export_target(Path::new(&self.cwd), &relative) {
            Ok(path) => path,
            Err(error) => {
                self.push_notice(NoticeKind::Warning, error);
                return None;
            }
        };
        let status_entry = self.push_tracked_line(
            &Style::new()
                .fg(TN_GRAY)
                .render(&format!("  exporting session → {}", relative.display())),
        );
        Some(cmd::cmd(move || async move {
            let result = tokio::task::spawn_blocking(move || {
                write_session_export(&path, document).map(|bytes| (path, bytes))
            })
            .await
            .map_err(|error| format!("session export task failed: {error}"))
            .and_then(|result| result);
            Msg::SessionExported {
                status_entry,
                result,
            }
        }))
    }

    fn session_markdown_document(&self, exported_at: DateTime<Utc>) -> Option<String> {
        let body = self
            .messages
            .semantic_markdown(Some(self.streaming.raw_content()))?;
        let workspace = Path::new(&self.cwd)
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("workspace");
        Some(format_session_document(
            &self.session_id,
            workspace,
            exported_at,
            &body,
        ))
    }
}

fn clipboard_feedback(
    subject: &str,
    character_count: usize,
    outcome: ClipboardCopyOutcome,
) -> (NoticeKind, String) {
    if outcome.native_delivered {
        return (
            NoticeKind::Success,
            format!("Copied {subject} to the native clipboard · {character_count} characters"),
        );
    }
    if outcome.terminal_requested && outcome.terminal_truncated {
        return (
            NoticeKind::Warning,
            format!(
                "Requested terminal clipboard copy for the first {} of {character_count} \
                 characters ({OSC52_PAYLOAD_BYTE_LIMIT}-byte OSC 52 limit) · use /export for \
                 the complete transcript",
                outcome.terminal_character_count
            ),
        );
    }
    if outcome.terminal_requested {
        return (
            NoticeKind::Info,
            format!(
                "Requested terminal clipboard copy for {subject} · {character_count} characters · \
                 delivery depends on OSC 52 support"
            ),
        );
    }
    (
        NoticeKind::Error,
        format!("Could not deliver {subject} to a native or terminal clipboard"),
    )
}

fn export_relative_path(
    rest: &str,
    session_id: &str,
    exported_at: DateTime<Utc>,
) -> Result<PathBuf, String> {
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok(PathBuf::from(default_export_filename(
            session_id,
            exported_at,
        )));
    }
    if rest.len() > MAX_EXPORT_PATH_BYTES {
        return Err(format!("export path exceeds {MAX_EXPORT_PATH_BYTES} bytes"));
    }
    let unquoted = unquote_path(rest)?;
    if unquoted.is_empty() {
        return Err("usage: /export [workspace-relative-path]".to_string());
    }
    Ok(PathBuf::from(unquoted))
}

fn unquote_path(path: &str) -> Result<&str, String> {
    let first = path.chars().next();
    let last = path.chars().next_back();
    match (first, last) {
        (Some('"'), Some('"')) | (Some('\''), Some('\'')) if path.len() >= 2 => {
            Ok(&path[1..path.len() - 1])
        }
        (Some('"' | '\''), _) | (_, Some('"' | '\'')) => {
            Err("export path has an unmatched quote".to_string())
        }
        _ => Ok(path),
    }
}

fn default_export_filename(session_id: &str, exported_at: DateTime<Utc>) -> String {
    let mut slug = session_id
        .chars()
        .take(48)
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = slug.trim_matches('-');
    let slug = if slug.is_empty() { "session" } else { slug };
    let timestamp = exported_at.format("%Y%m%dT%H%M%S%3fZ");
    format!("a3s-session-{slug}-{timestamp}.md")
}

fn resolve_export_target(workspace: &Path, relative: &Path) -> Result<PathBuf, String> {
    if relative.is_absolute() {
        return Err("export path must be relative to the current workspace".to_string());
    }
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                return Err("export path cannot contain `..`".to_string());
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("export path must be relative to the current workspace".to_string());
            }
        }
    }
    let file_name = normalized
        .file_name()
        .ok_or_else(|| "export path must name a file".to_string())?
        .to_os_string();
    let workspace = workspace.canonicalize().map_err(|error| {
        format!(
            "could not resolve workspace {}: {error}",
            workspace.display()
        )
    })?;
    if !workspace.is_dir() {
        return Err(format!(
            "workspace is not a directory: {}",
            workspace.display()
        ));
    }
    let lexical_target = workspace.join(&normalized);
    let parent = lexical_target
        .parent()
        .ok_or_else(|| "export path has no parent directory".to_string())?;
    let parent = parent.canonicalize().map_err(|error| {
        format!(
            "export parent must already exist inside the workspace ({}): {error}",
            parent.display()
        )
    })?;
    if !parent.is_dir() {
        return Err(format!(
            "export parent is not a directory: {}",
            parent.display()
        ));
    }
    if !parent.starts_with(&workspace) {
        return Err("export path resolves outside the current workspace".to_string());
    }
    let target = parent.join(file_name);
    match std::fs::symlink_metadata(&target) {
        Ok(_) => Err(format!(
            "export target already exists; choose a new path: {}",
            target.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(target),
        Err(error) => Err(format!(
            "could not inspect export target {}: {error}",
            target.display()
        )),
    }
}

fn write_session_export(path: &Path, document: String) -> Result<u64, String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("export path has no parent: {}", path.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "could not create a temporary export in {}: {error}",
            parent.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|error| {
                format!(
                    "could not protect temporary session export in {}: {error}",
                    parent.display()
                )
            })?;
    }
    temporary
        .write_all(document.as_bytes())
        .map_err(|error| format!("could not write session export {}: {error}", path.display()))?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|error| format!("could not sync session export {}: {error}", path.display()))?;
    let bytes = document.len() as u64;
    temporary.persist_noclobber(path).map_err(|error| {
        if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            format!(
                "export target already exists; choose a new path: {}",
                path.display()
            )
        } else {
            format!(
                "could not atomically create session export {}: {}",
                path.display(),
                error.error
            )
        }
    })?;
    #[cfg(unix)]
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "session export was written but its directory could not be synced ({}): {error}",
                parent.display()
            )
        })?;
    Ok(bytes)
}

fn format_session_document(
    session_id: &str,
    workspace: &str,
    exported_at: DateTime<Utc>,
    body: &str,
) -> String {
    let session_id = metadata_code_span(session_id);
    let workspace = metadata_code_span(workspace);
    let exported_at = metadata_code_span(&exported_at.to_rfc3339_opts(SecondsFormat::Millis, true));
    format!(
        "# A3S Code Session\n\n\
         - Session: {session_id}\n\
         - Workspace: {workspace}\n\
         - Exported: {exported_at}\n\n\
         {}",
        body.trim_start_matches('\n')
    )
}

fn metadata_code_span(source: &str) -> String {
    let source = source
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let longest = source
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.saturating_add(1).max(1));
    let padding = if source.starts_with('`')
        || source.starts_with(' ')
        || source.ends_with('`')
        || source.ends_with(' ')
    {
        " "
    } else {
        ""
    };
    format!("{fence}{padding}{source}{padding}{fence}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instant() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-07-19T12:34:56.789Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    #[test]
    fn copy_target_accepts_only_bounded_public_forms() {
        assert_eq!(parse_copy_target(""), Ok(CopyTarget::LatestAssistant));
        assert_eq!(
            parse_copy_target(" transcript "),
            Ok(CopyTarget::Transcript)
        );
        assert_eq!(
            parse_copy_target("reasoning"),
            Err("usage: /copy [transcript]")
        );
    }

    #[test]
    fn default_export_path_is_unique_shaped_and_filename_safe() {
        let path = export_relative_path("", "../unsafe/session", instant()).unwrap();
        assert_eq!(
            path,
            PathBuf::from("a3s-session-unsafe-session-20260719T123456789Z.md")
        );
        assert_eq!(
            export_relative_path(" \"notes/session copy.md\" ", "ignored", instant()).unwrap(),
            PathBuf::from("notes/session copy.md")
        );
        assert!(export_relative_path("\"unterminated", "ignored", instant()).is_err());
    }

    #[test]
    fn export_target_stays_inside_workspace_and_never_overwrites() {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir(workspace.path().join("notes")).unwrap();
        let target =
            resolve_export_target(workspace.path(), Path::new("notes/session.md")).unwrap();
        assert_eq!(
            target,
            workspace
                .path()
                .canonicalize()
                .unwrap()
                .join("notes/session.md")
        );
        assert!(resolve_export_target(workspace.path(), Path::new("../escape.md")).is_err());
        assert!(resolve_export_target(workspace.path(), Path::new("/tmp/escape.md")).is_err());
        assert!(resolve_export_target(workspace.path(), Path::new("missing/session.md")).is_err());

        std::fs::write(&target, "existing").unwrap();
        assert!(resolve_export_target(workspace.path(), Path::new("notes/session.md")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn export_target_rejects_a_parent_symlink_that_escapes_workspace() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), workspace.path().join("escape")).unwrap();

        let error = resolve_export_target(workspace.path(), Path::new("escape/session.md"))
            .expect_err("outside symlink must fail");
        assert!(error.contains("outside"), "{error}");
    }

    #[test]
    fn session_export_is_atomic_private_and_no_clobber() {
        let workspace = tempfile::tempdir().unwrap();
        let target = workspace.path().join("session.md");
        let bytes = write_session_export(&target, "# Session\n".to_string()).unwrap();
        assert_eq!(bytes, 10);
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Session\n");
        assert!(write_session_export(&target, "replacement".to_string()).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "# Session\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn session_document_has_shareable_metadata_and_one_trailing_newline() {
        let document =
            format_session_document("session`id", "project", instant(), "## User\n\nHello\n");
        assert!(document.starts_with("# A3S Code Session\n"));
        assert!(document.contains("- Session: ``session`id``"));
        assert!(document.contains("- Workspace: `project`"));
        assert!(document.contains("- Exported: `2026-07-19T12:34:56.789Z`"));
        assert!(document.ends_with("## User\n\nHello\n"));
    }

    #[test]
    fn clipboard_feedback_never_claims_unverified_terminal_delivery() {
        let terminal = clipboard_feedback(
            "response",
            10,
            ClipboardCopyOutcome {
                terminal_requested: true,
                native_delivered: false,
                terminal_truncated: false,
                terminal_character_count: 10,
            },
        );
        assert_eq!(terminal.0, NoticeKind::Info);
        assert!(terminal.1.starts_with("Requested terminal clipboard copy"));

        let truncated = clipboard_feedback(
            "transcript",
            OSC52_PAYLOAD_BYTE_LIMIT + 1,
            ClipboardCopyOutcome {
                terminal_requested: true,
                native_delivered: false,
                terminal_truncated: true,
                terminal_character_count: OSC52_PAYLOAD_BYTE_LIMIT,
            },
        );
        assert_eq!(truncated.0, NoticeKind::Warning);
        assert!(truncated.1.contains("/export"));

        let native = clipboard_feedback(
            "response",
            10,
            ClipboardCopyOutcome {
                terminal_requested: false,
                native_delivered: true,
                terminal_truncated: false,
                terminal_character_count: 10,
            },
        );
        assert_eq!(native.0, NoticeKind::Success);
        assert!(native.1.starts_with("Copied"));
    }
}
