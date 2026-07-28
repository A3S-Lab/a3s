use super::*;
use a3s_code_core::workspace::{WorkspaceFileSystem, WorkspacePath};

fn saved_line(text: &str, line: usize) -> Option<&str> {
    text.split('\n')
        .nth(line)
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
}

/// Convert the editor's expanded-tab, Unicode-scalar cursor column into the
/// public zero-based UTF-16 position using the saved file as the authority.
pub(super) fn editor_position_to_saved_utf16(
    saved_text: &str,
    line: usize,
    expanded_col: usize,
) -> Result<CodePosition, String> {
    let saved_line = saved_line(saved_text, line)
        .ok_or_else(|| "the cursor line does not exist in the saved version".to_owned())?;
    let mut editor_col = 0usize;
    let mut utf16_col = 0usize;
    for ch in saved_line.chars() {
        if editor_col >= expanded_col {
            break;
        }
        let editor_width = if ch == '\t' { 4 } else { 1 };
        if editor_col + editor_width > expanded_col {
            // The editor renders a tab as four spaces. Any cursor cell inside
            // those spaces still denotes the position before the saved tab.
            break;
        }
        editor_col += editor_width;
        utf16_col += ch.len_utf16();
    }
    let total_editor_cols = saved_line
        .chars()
        .map(|ch| if ch == '\t' { 4 } else { 1 })
        .sum::<usize>();
    if expanded_col > total_editor_cols {
        return Err(
            "the cursor column does not exist in the saved version; save changes and retry"
                .to_owned(),
        );
    }
    Ok(CodePosition::new(
        u32::try_from(line).map_err(|_| "cursor line is too large".to_owned())?,
        u32::try_from(utf16_col).map_err(|_| "cursor column is too large".to_owned())?,
    ))
}

/// Convert a saved-file UTF-16 column into the editor's expanded-tab column.
pub(super) fn saved_utf16_to_editor_column(line: &str, utf16_col: u32) -> Result<usize, String> {
    let target = utf16_col as usize;
    let mut current_utf16 = 0usize;
    let mut editor_col = 0usize;
    for ch in line.chars() {
        if current_utf16 == target {
            return Ok(editor_col);
        }
        let next_utf16 = current_utf16 + ch.len_utf16();
        if target < next_utf16 {
            return Err(
                "Code Intelligence returned a position inside a UTF-16 character".to_owned(),
            );
        }
        current_utf16 = next_utf16;
        editor_col += if ch == '\t' { 4 } else { 1 };
    }
    if current_utf16 == target {
        Ok(editor_col)
    } else {
        Err("Code Intelligence returned a position past the saved line".to_owned())
    }
}

/// Read a semantic jump target through the shared workspace filesystem. The
/// backend performs canonical containment checks, including symlink escapes.
pub(super) async fn read_ide_intelligence_jump(
    file_system: Arc<dyn WorkspaceFileSystem>,
    workspace_path: WorkspacePath,
    display_path: PathBuf,
    position: CodePosition,
    cancellation: CancellationToken,
) -> Result<IdeIntelligenceJump, String> {
    let text = tokio::select! {
        _ = cancellation.cancelled() => {
            return Err("Code Intelligence jump cancelled".to_owned());
        }
        result = file_system.read_text(&workspace_path) => result.map_err(|error| {
            format!("failed to read saved file {}: {error}", workspace_path.as_str())
        })?,
    };
    let row = position.line as usize;
    let line = saved_line(&text, row)
        .ok_or_else(|| "Code Intelligence returned a line past the saved file".to_owned())?;
    let col = saved_utf16_to_editor_column(line, position.character)?;
    let normalized = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ");
    let mut lines = normalized.lines().map(str::to_owned).collect::<Vec<_>>();
    while lines.len() <= row {
        lines.push(String::new());
    }
    Ok(IdeIntelligenceJump {
        path: display_path,
        lines,
        row,
        col,
    })
}
