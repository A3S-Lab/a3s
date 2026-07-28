//! Semantic styling for tool actions and arguments.
//!
//! Keep this separate from cell layout so live, completed, and grouped tool
//! calls can share one visual vocabulary without teaching layout code how to
//! parse shell commands or structured tool details.

use a3s_tui::style::{visible_len, Color, Style};

use super::{TN_CYAN, TN_FG, TN_GRAY, TN_SUBTLE};

// Codex Dark / Catppuccin-inspired command roles. These colors describe
// syntax, never status: success and failure remain confined to the cell marker.
pub(super) const TOOL_ACTION_COLOR: Color = TN_CYAN;
pub(super) const TOOL_PROGRAM_COLOR: Color = Color::Rgb(137, 180, 250);
pub(super) const TOOL_PATH_COLOR: Color = Color::Rgb(185, 187, 191);
pub(super) const TOOL_FLAG_COLOR: Color = Color::Rgb(232, 145, 164);
pub(super) const TOOL_OPERATOR_COLOR: Color = Color::Rgb(132, 202, 195);
pub(super) const TOOL_KEYWORD_COLOR: Color = Color::Rgb(196, 161, 232);
pub(super) const TOOL_STRING_COLOR: Color = Color::Rgb(148, 211, 153);
pub(super) const TOOL_NUMBER_COLOR: Color = Color::Rgb(226, 181, 126);
pub(super) const TOOL_VARIABLE_COLOR: Color = Color::Rgb(215, 168, 207);
pub(super) const TOOL_ARGUMENT_COLOR: Color = Color::Rgb(178, 181, 187);
pub(super) const TOOL_KEY_COLOR: Color = Color::Rgb(137, 180, 250);

/// Tool headings share one near-white hierarchy; color belongs to syntax and
/// to the small state marker, not to the whole action label.
pub(super) fn header_action_color(_action: &str) -> Color {
    TN_FG
}

/// Color the nested row of an `Explored` group without losing its structure.
pub(super) fn highlight_explore_detail(detail: &str) -> String {
    let Some((action, value)) = detail.split_once(' ') else {
        return tool_action_style().render(detail);
    };
    if !matches!(action, "Read" | "Search" | "List") {
        return highlight_tool_detail(detail);
    }

    let mut rendered = tool_action_style().render(action);
    rendered.push(' ');
    if action == "Search" {
        if let Some((pattern, path)) = value.rsplit_once(" in ") {
            rendered.push_str(&style_words(pattern, TOOL_ARGUMENT_COLOR));
            rendered.push(' ');
            rendered.push_str(&Style::new().fg(TN_SUBTLE).render("in"));
            rendered.push(' ');
            rendered.push_str(&style_words(path, TOOL_PATH_COLOR));
        } else {
            rendered.push_str(&style_words(value, TOOL_ARGUMENT_COLOR));
        }
    } else {
        rendered.push_str(&style_words(value, TOOL_PATH_COLOR));
    }
    rendered
}

/// Codex-style shell coloring that preserves the command byte-for-byte.
/// Programs, flags, strings, paths, and shell operators have distinct roles.
pub(super) fn highlight_shell(command: &str) -> String {
    shell_spans(command)
        .into_iter()
        .map(render_shell_span)
        .collect()
}

/// Highlight once, then wrap semantic spans. Parsing each visual continuation
/// as a fresh command would incorrectly color an argument as a new executable.
pub(super) fn highlight_shell_wrapped(
    command: &str,
    first_width: usize,
    continuation_width: usize,
) -> Vec<String> {
    if command.is_empty() {
        return vec![String::new()];
    }
    let first_width = first_width.max(1);
    let continuation_width = continuation_width.max(1);
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut occupied = 0usize;

    for span in shell_spans(command) {
        let mut remaining = span.text;
        while !remaining.is_empty() {
            if remaining.starts_with('\n') {
                rows.push(std::mem::take(&mut row));
                occupied = 0;
                remaining = &remaining[1..];
                continue;
            }

            let before_newline = remaining.find('\n').unwrap_or(remaining.len());
            let segment = &remaining[..before_newline];
            let width = if rows.is_empty() {
                first_width
            } else {
                continuation_width
            };
            if occupied >= width {
                rows.push(std::mem::take(&mut row));
                occupied = 0;
                continue;
            }
            let available = width.saturating_sub(occupied);
            let segment_width = visible_len(segment);
            if span.role != ShellRole::Plain
                && occupied > 0
                && segment_width > available
                && segment_width <= continuation_width
            {
                rows.push(std::mem::take(&mut row));
                occupied = 0;
                continue;
            }
            let split = visible_prefix_end(segment, available);
            if split == 0 {
                if !row.is_empty() {
                    rows.push(std::mem::take(&mut row));
                    occupied = 0;
                    continue;
                }
                let ch = segment.chars().next().unwrap_or_default();
                let end = ch.len_utf8();
                let part = &segment[..end];
                row.push_str(&render_shell_span(ShellSpan {
                    text: part,
                    role: span.role,
                }));
                occupied = visible_len(part);
                remaining = &remaining[end..];
                continue;
            }

            let part = &segment[..split];
            row.push_str(&render_shell_span(ShellSpan {
                text: part,
                role: span.role,
            }));
            occupied += visible_len(part);
            remaining = &remaining[split..];
        }
    }
    if !row.is_empty() || rows.is_empty() || command.ends_with('\n') {
        rows.push(row);
    }
    rows
}

pub(super) fn highlight_tool_detail(detail: &str) -> String {
    let mut first_token = true;
    detail
        .split_inclusive(char::is_whitespace)
        .map(|part| {
            let token = part.trim_end_matches(char::is_whitespace);
            let whitespace = &part[token.len()..];
            let action = first_token && looks_like_tool_action(token);
            if !token.is_empty() {
                first_token = false;
            }
            let token = if action {
                tool_action_style().render(token)
            } else {
                highlight_tool_detail_token(token)
            };
            format!("{token}{whitespace}")
        })
        .collect()
}

/// Highlight a complete JSON document while preserving its visible source.
/// Keys, string values, literals, numbers, and structural punctuation use
/// distinct low-saturation roles so MCP payloads remain scannable without
/// becoming a rainbow-colored code block.
#[cfg(test)]
pub(super) fn highlight_json(value: &str) -> String {
    json_spans(value)
        .into_iter()
        .map(render_json_span)
        .collect()
}

/// Wrap JSON by visible columns without losing the semantic style of a token
/// that crosses a row boundary (for example a long URL or CJK string value).
pub(super) fn highlight_json_wrapped(value: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut occupied = 0usize;

    for span in json_spans(value) {
        let mut remaining = span.text;
        while !remaining.is_empty() {
            if occupied >= width {
                rows.push(std::mem::take(&mut row));
                occupied = 0;
            }
            let available = width.saturating_sub(occupied);
            let split = visible_prefix_end(remaining, available);
            if split == 0 {
                if !row.is_empty() {
                    rows.push(std::mem::take(&mut row));
                    occupied = 0;
                    continue;
                }
                let ch = remaining.chars().next().unwrap_or_default();
                let end = ch.len_utf8();
                row.push_str(&render_json_span(JsonSpan {
                    text: &remaining[..end],
                    color: span.color,
                }));
                occupied = visible_len(&remaining[..end]);
                remaining = &remaining[end..];
                continue;
            }
            let part = &remaining[..split];
            row.push_str(&render_json_span(JsonSpan {
                text: part,
                color: span.color,
            }));
            occupied += visible_len(part);
            remaining = &remaining[split..];
        }
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

#[derive(Clone, Copy)]
struct JsonSpan<'a> {
    text: &'a str,
    color: Option<Color>,
}

fn json_spans(value: &str) -> Vec<JsonSpan<'_>> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    while cursor < value.len() {
        let ch = next_char(value, cursor);
        if ch.is_whitespace() {
            let end = value[cursor..]
                .char_indices()
                .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some(cursor + offset))
                .unwrap_or(value.len());
            spans.push(JsonSpan {
                text: &value[cursor..end],
                color: None,
            });
            cursor = end;
            continue;
        }

        if ch == '"' {
            let end = json_string_end(value, cursor);
            let is_key = value[end..].trim_start().starts_with(':');
            spans.push(JsonSpan {
                text: &value[cursor..end],
                color: Some(if is_key {
                    TOOL_KEY_COLOR
                } else {
                    TOOL_STRING_COLOR
                }),
            });
            cursor = end;
            continue;
        }

        if ch.is_ascii_digit()
            || (ch == '-'
                && value[cursor + ch.len_utf8()..]
                    .chars()
                    .next()
                    .is_some_and(|next| next.is_ascii_digit()))
        {
            let end = json_number_end(value, cursor);
            spans.push(JsonSpan {
                text: &value[cursor..end],
                color: Some(TOOL_NUMBER_COLOR),
            });
            cursor = end;
            continue;
        }

        if ch.is_ascii_alphabetic() {
            let end = value[cursor..]
                .char_indices()
                .find_map(|(offset, ch)| (!ch.is_ascii_alphabetic()).then_some(cursor + offset))
                .unwrap_or(value.len());
            let token = &value[cursor..end];
            let color = if matches!(token, "true" | "false" | "null") {
                TOOL_KEYWORD_COLOR
            } else {
                TN_GRAY
            };
            spans.push(JsonSpan {
                text: token,
                color: Some(color),
            });
            cursor = end;
            continue;
        }

        let color = if matches!(ch, '{' | '}' | '[' | ']' | ',' | ':') {
            TN_SUBTLE
        } else {
            TN_GRAY
        };
        let end = cursor + ch.len_utf8();
        spans.push(JsonSpan {
            text: &value[cursor..end],
            color: Some(color),
        });
        cursor = end;
    }

    spans
}

fn render_json_span(span: JsonSpan<'_>) -> String {
    span.color
        .map(|color| Style::new().fg(color).render(span.text))
        .unwrap_or_else(|| span.text.to_string())
}

fn visible_prefix_end(value: &str, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let mut occupied = 0usize;
    let mut end = 0usize;
    for (offset, ch) in value.char_indices() {
        let char_width = visible_len(&ch.to_string());
        if occupied.saturating_add(char_width) > width {
            break;
        }
        occupied += char_width;
        end = offset + ch.len_utf8();
    }
    end
}

fn json_string_end(value: &str, start: usize) -> usize {
    let mut cursor = start + 1;
    let mut escaped = false;
    while cursor < value.len() {
        let ch = next_char(value, cursor);
        cursor += ch.len_utf8();
        if escaped {
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            break;
        }
    }
    cursor
}

fn json_number_end(value: &str, start: usize) -> usize {
    value[start..]
        .char_indices()
        .find_map(|(offset, ch)| {
            (!ch.is_ascii_digit() && !matches!(ch, '-' | '+' | '.' | 'e' | 'E'))
                .then_some(start + offset)
        })
        .unwrap_or(value.len())
}

fn looks_like_tool_action(token: &str) -> bool {
    let token = token.trim_matches(['(', ')', ',', ':']);
    matches!(
        token,
        "Add"
            | "Added"
            | "Call"
            | "Called"
            | "Create"
            | "Created"
            | "Delete"
            | "Deleted"
            | "Edit"
            | "Edited"
            | "Fetch"
            | "Fetched"
            | "Find"
            | "List"
            | "Read"
            | "Run"
            | "Ran"
            | "Search"
            | "Update"
            | "Updated"
            | "Write"
            | "Wrote"
    ) || ((token.contains('_') || token.contains('-'))
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.')))
}

fn highlight_tool_detail_token(token: &str) -> String {
    let mut rendered = String::new();
    let call = token.split_once('(').filter(|(call, _)| {
        !call.is_empty()
            && call
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
    });
    let token = if let Some((call, rest)) = call {
        rendered.push_str(&tool_action_style().render(call));
        rendered.push_str(&Style::new().fg(TN_SUBTLE).render("("));
        rest
    } else {
        token
    };
    let core = token.trim_end_matches([',', ')']);
    let suffix = &token[core.len()..];

    if let Some((key, value)) = core.split_once('=') {
        rendered.push_str(&Style::new().fg(TOOL_KEY_COLOR).render(key));
        rendered.push_str(&Style::new().fg(TOOL_OPERATOR_COLOR).render("="));
        rendered.push_str(&tool_value_style(value).render(value));
    } else {
        rendered.push_str(&tool_value_style(core).render(core));
    }
    rendered.push_str(&Style::new().fg(TN_SUBTLE).render(suffix));
    rendered
}

fn tool_value_style(value: &str) -> Style {
    if value.starts_with('-') {
        Style::new().fg(TOOL_FLAG_COLOR)
    } else if looks_like_path(value) {
        Style::new().fg(TOOL_PATH_COLOR)
    } else if value.starts_with('$') {
        Style::new().fg(TOOL_VARIABLE_COLOR)
    } else if value.parse::<f64>().is_ok() {
        Style::new().fg(TOOL_NUMBER_COLOR)
    } else if value.starts_with('"') || value.starts_with('\'') {
        Style::new().fg(TOOL_STRING_COLOR)
    } else if matches!(value, "true" | "false" | "null") {
        Style::new().fg(TOOL_KEYWORD_COLOR)
    } else {
        Style::new().fg(TOOL_ARGUMENT_COLOR)
    }
}

fn looks_like_path(value: &str) -> bool {
    let value = value.trim_matches(['"', '\'', ',', ')', ']', '}']);
    matches!(value, "." | "..")
        || value.contains("://")
        || value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with("~/")
        || value.contains('/')
        || value
            .rsplit_once('.')
            .is_some_and(|(stem, extension)| !stem.is_empty() && is_file_extension(extension))
}

fn is_file_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "c" | "cc"
            | "cpp"
            | "css"
            | "go"
            | "h"
            | "hpp"
            | "html"
            | "java"
            | "js"
            | "json"
            | "jsx"
            | "md"
            | "py"
            | "rb"
            | "rs"
            | "sh"
            | "sql"
            | "toml"
            | "ts"
            | "tsx"
            | "txt"
            | "xml"
            | "yaml"
            | "yml"
    )
}

fn style_words(value: &str, color: Color) -> String {
    value
        .split_inclusive(char::is_whitespace)
        .map(|part| {
            let word = part.trim_end_matches(char::is_whitespace);
            let whitespace = &part[word.len()..];
            format!("{}{whitespace}", Style::new().fg(color).render(word))
        })
        .collect()
}

fn tool_action_style() -> Style {
    Style::new().fg(TOOL_ACTION_COLOR)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellRole {
    Plain,
    Program,
    Argument,
    Flag,
    String,
    Path,
    Keyword,
    Operator,
    Redirection,
    Variable,
    Number,
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShellSpan<'a> {
    text: &'a str,
    role: ShellRole,
}

fn render_shell_span(span: ShellSpan<'_>) -> String {
    let color = match span.role {
        ShellRole::Plain => return span.text.to_string(),
        ShellRole::Program => TOOL_PROGRAM_COLOR,
        ShellRole::Argument => TOOL_ARGUMENT_COLOR,
        ShellRole::Flag => TOOL_FLAG_COLOR,
        ShellRole::String => TOOL_STRING_COLOR,
        ShellRole::Path => TOOL_PATH_COLOR,
        ShellRole::Keyword => TOOL_KEYWORD_COLOR,
        ShellRole::Operator => TOOL_OPERATOR_COLOR,
        ShellRole::Redirection | ShellRole::Number => TOOL_NUMBER_COLOR,
        ShellRole::Variable => TOOL_VARIABLE_COLOR,
        ShellRole::Comment => TN_GRAY,
    };
    Style::new().fg(color).render(span.text)
}

fn shell_spans(command: &str) -> Vec<ShellSpan<'_>> {
    let mut spans = Vec::new();
    let mut command_position = true;
    let mut redirection_target = false;

    for token in shell_tokens(command) {
        match token.kind {
            ShellTokenKind::Whitespace => {
                spans.push(ShellSpan {
                    text: token.text,
                    role: ShellRole::Plain,
                });
                if token.text.contains('\n') {
                    command_position = true;
                    redirection_target = false;
                }
            }
            ShellTokenKind::Comment => spans.push(ShellSpan {
                text: token.text,
                role: ShellRole::Comment,
            }),
            ShellTokenKind::Operator => {
                let redirection = is_redirection(token.text);
                spans.push(ShellSpan {
                    text: token.text,
                    role: if redirection {
                        ShellRole::Redirection
                    } else {
                        ShellRole::Operator
                    },
                });
                if redirection {
                    redirection_target = true;
                } else if is_command_separator(token.text) || token.text == "(" {
                    command_position = true;
                }
            }
            ShellTokenKind::Quoted => {
                spans.push(ShellSpan {
                    text: token.text,
                    role: ShellRole::String,
                });
                if redirection_target {
                    redirection_target = false;
                } else {
                    command_position = false;
                }
            }
            ShellTokenKind::Word if redirection_target => {
                push_shell_value_spans(&mut spans, token.text);
                redirection_target = false;
            }
            ShellTokenKind::Word if is_shell_keyword(token.text) => {
                spans.push(ShellSpan {
                    text: token.text,
                    role: ShellRole::Keyword,
                });
                command_position = keyword_expects_command(token.text);
            }
            ShellTokenKind::Word if is_shell_test_punctuation(token.text) => {
                spans.push(ShellSpan {
                    text: token.text,
                    role: ShellRole::Operator,
                });
                if command_position && matches!(token.text, "[" | "[[") {
                    command_position = false;
                }
            }
            ShellTokenKind::Word if command_position && is_assignment(token.text) => {
                push_assignment_spans(&mut spans, token.text);
            }
            ShellTokenKind::Word if command_position => {
                spans.push(ShellSpan {
                    text: token.text,
                    role: if matches!(token.text, "true" | "false") {
                        ShellRole::String
                    } else {
                        ShellRole::Program
                    },
                });
                command_position = false;
            }
            ShellTokenKind::Word => push_shell_value_spans(&mut spans, token.text),
        }
    }

    spans
}

fn push_shell_value_spans<'a>(spans: &mut Vec<ShellSpan<'a>>, token: &'a str) {
    if token.starts_with('-') {
        if let Some((flag, value)) = token.split_once('=') {
            spans.push(ShellSpan {
                text: flag,
                role: ShellRole::Flag,
            });
            spans.push(ShellSpan {
                text: &token[flag.len()..flag.len() + 1],
                role: ShellRole::Operator,
            });
            spans.push(ShellSpan {
                text: value,
                role: shell_value_role(value),
            });
        } else {
            spans.push(ShellSpan {
                text: token,
                role: ShellRole::Flag,
            });
        }
    } else if is_assignment(token) {
        push_assignment_spans(spans, token);
    } else {
        spans.push(ShellSpan {
            text: token,
            role: shell_value_role(token),
        });
    }
}

fn push_assignment_spans<'a>(spans: &mut Vec<ShellSpan<'a>>, token: &'a str) {
    let Some((name, value)) = token.split_once('=') else {
        spans.push(ShellSpan {
            text: token,
            role: ShellRole::Argument,
        });
        return;
    };
    spans.push(ShellSpan {
        text: name,
        role: ShellRole::Variable,
    });
    spans.push(ShellSpan {
        text: &token[name.len()..name.len() + 1],
        role: ShellRole::Operator,
    });
    spans.push(ShellSpan {
        text: value,
        role: shell_value_role(value),
    });
}

fn shell_value_role(value: &str) -> ShellRole {
    if looks_like_path(value) {
        ShellRole::Path
    } else if value.starts_with('$') || value.contains("${") {
        ShellRole::Variable
    } else if value.parse::<f64>().is_ok() {
        ShellRole::Number
    } else if matches!(value, "true" | "false" | "null") {
        ShellRole::Keyword
    } else {
        ShellRole::Argument
    }
}

fn is_shell_keyword(token: &str) -> bool {
    matches!(
        token,
        "!" | "case"
            | "do"
            | "done"
            | "elif"
            | "else"
            | "esac"
            | "fi"
            | "for"
            | "function"
            | "if"
            | "in"
            | "select"
            | "then"
            | "time"
            | "until"
            | "while"
    )
}

fn keyword_expects_command(token: &str) -> bool {
    matches!(
        token,
        "!" | "do" | "elif" | "else" | "if" | "then" | "time" | "until" | "while"
    )
}

fn is_shell_test_punctuation(token: &str) -> bool {
    matches!(token, "[" | "]" | "[[" | "]]" | "{" | "}")
}

fn is_redirection(operator: &str) -> bool {
    operator.contains('<') || operator.contains('>')
}

fn is_assignment(token: &str) -> bool {
    token.split_once('=').is_some_and(|(name, _)| {
        let mut chars = name.chars();
        chars
            .next()
            .is_some_and(|ch| ch == '_' || ch.is_ascii_alphabetic())
            && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    })
}

fn is_command_separator(operator: &str) -> bool {
    matches!(
        operator,
        "|" | "||" | "|&" | "&&" | ";" | ";;" | ";&" | ";;&" | "&"
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellTokenKind {
    Whitespace,
    Word,
    Quoted,
    Operator,
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShellToken<'a> {
    kind: ShellTokenKind,
    text: &'a str,
}

fn shell_tokens(command: &str) -> Vec<ShellToken<'_>> {
    let mut tokens = Vec::new();
    let mut cursor = 0usize;

    while cursor < command.len() {
        let start = cursor;
        let ch = next_char(command, cursor);
        if ch.is_whitespace() {
            cursor += ch.len_utf8();
            while cursor < command.len() && next_char(command, cursor).is_whitespace() {
                cursor += next_char(command, cursor).len_utf8();
            }
            tokens.push(ShellToken {
                kind: ShellTokenKind::Whitespace,
                text: &command[start..cursor],
            });
            continue;
        }

        if ch == '#' {
            cursor += ch.len_utf8();
            while cursor < command.len() && next_char(command, cursor) != '\n' {
                cursor += next_char(command, cursor).len_utf8();
            }
            tokens.push(ShellToken {
                kind: ShellTokenKind::Comment,
                text: &command[start..cursor],
            });
            continue;
        }

        if matches!(ch, '\'' | '"') {
            cursor = quoted_token_end(command, cursor, ch);
            tokens.push(ShellToken {
                kind: ShellTokenKind::Quoted,
                text: &command[start..cursor],
            });
            continue;
        }

        if let Some(length) = operator_length(&command[cursor..]) {
            cursor += length;
            tokens.push(ShellToken {
                kind: ShellTokenKind::Operator,
                text: &command[start..cursor],
            });
            continue;
        }

        while cursor < command.len() {
            let ch = next_char(command, cursor);
            if ch.is_whitespace()
                || matches!(ch, '\'' | '"')
                || operator_length(&command[cursor..]).is_some()
            {
                break;
            }
            if ch == '\\' {
                cursor += ch.len_utf8();
                if cursor < command.len() {
                    cursor += next_char(command, cursor).len_utf8();
                }
            } else {
                cursor += ch.len_utf8();
            }
        }
        tokens.push(ShellToken {
            kind: ShellTokenKind::Word,
            text: &command[start..cursor],
        });
    }

    tokens
}

fn quoted_token_end(command: &str, start: usize, quote: char) -> usize {
    let mut cursor = start + quote.len_utf8();
    while cursor < command.len() {
        let ch = next_char(command, cursor);
        cursor += ch.len_utf8();
        if ch == quote {
            break;
        }
        if ch == '\\' && quote == '"' && cursor < command.len() {
            cursor += next_char(command, cursor).len_utf8();
        }
    }
    cursor
}

fn operator_length(value: &str) -> Option<usize> {
    [
        ";;&", "&&", "||", "|&", ">>", "<<", "<&", ">&", "<>", ">|", ";;", ";&",
    ]
    .into_iter()
    .find(|operator| value.starts_with(operator))
    .map(str::len)
    .or_else(|| {
        value
            .chars()
            .next()
            .filter(|ch| matches!(ch, '|' | '&' | ';' | '<' | '>' | '(' | ')'))
            .map(char::len_utf8)
    })
}

fn next_char(value: &str, cursor: usize) -> char {
    value[cursor..].chars().next().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::style::strip_ansi;

    #[test]
    fn shell_styling_preserves_compact_operators_quotes_and_arguments() {
        let command = "cd '../work tree'&&cargo test -p a3s|rg \"tool call\"";
        let rendered = highlight_shell(command);

        assert_eq!(strip_ansi(&rendered), command);
        assert!(rendered.contains(&Style::new().fg(TOOL_PROGRAM_COLOR).render("cd")));
        assert!(rendered.contains(&Style::new().fg(TOOL_PROGRAM_COLOR).render("cargo")));
        assert!(rendered.contains(&Style::new().fg(TOOL_OPERATOR_COLOR).render("&&")));
        assert!(rendered.contains(&Style::new().fg(TOOL_OPERATOR_COLOR).render("|")));
        assert!(rendered.contains(&Style::new().fg(TOOL_FLAG_COLOR).render("-p")));
        assert!(rendered.contains(&Style::new().fg(TOOL_STRING_COLOR).render("\"tool call\"")));
    }

    #[test]
    fn wrapped_shell_keeps_semantic_roles_across_visual_rows() {
        let command = "cargo test --workspace && git diff --name-only";
        let rows = highlight_shell_wrapped(command, 6, 16);

        assert_eq!(
            rows.iter().map(|row| strip_ansi(row)).collect::<String>(),
            command
        );
        assert!(rows.len() > 2, "{rows:?}");
        assert!(
            rows.iter()
                .any(|row| row.contains(&Style::new().fg(TOOL_ARGUMENT_COLOR).render("test"))),
            "a wrapped argument must not be recolored as a program: {rows:?}"
        );
        assert!(rows
            .iter()
            .any(|row| { row.contains(&Style::new().fg(TOOL_FLAG_COLOR).render("--workspace")) }));
    }

    #[test]
    fn shell_comments_are_muted_without_changing_source() {
        let command = "cargo check # verify the workspace";
        let rendered = highlight_shell(command);

        assert_eq!(strip_ansi(&rendered), command);
        assert!(rendered.contains(&Style::new().fg(TN_GRAY).render("# verify the workspace")));
    }

    #[test]
    fn explored_detail_has_distinct_action_pattern_and_path_roles() {
        let detail = "Search ToolStatusLine in src/tui/ui/render.rs";
        let rendered = highlight_explore_detail(detail);

        assert_eq!(strip_ansi(&rendered), detail);
        assert!(rendered.contains(&tool_action_style().render("Search")));
        assert!(rendered.contains(
            &Style::new()
                .fg(TOOL_ARGUMENT_COLOR)
                .render("ToolStatusLine")
        ));
        assert!(rendered.contains(
            &Style::new()
                .fg(TOOL_PATH_COLOR)
                .render("src/tui/ui/render.rs")
        ));
    }

    #[test]
    fn generic_tool_detail_distinguishes_call_keys_paths_and_values() {
        let detail = "lookup(path=src/main.rs, count=2)";
        let rendered = highlight_tool_detail(detail);

        assert_eq!(strip_ansi(&rendered), detail);
        assert!(rendered.contains(&tool_action_style().render("lookup")));
        assert!(rendered.contains(&Style::new().fg(TOOL_KEY_COLOR).render("path")));
        assert!(rendered.contains(&Style::new().fg(TOOL_PATH_COLOR).render("src/main.rs")));
    }

    #[test]
    fn json_styling_preserves_source_and_separates_semantic_tokens() {
        let json = "{\n  \"name\": \"A3S 编码\",\n  \"count\": 2,\n  \"ready\": true\n}";
        let rendered = highlight_json(json);

        assert_eq!(strip_ansi(&rendered), json);
        assert!(rendered.contains(&Style::new().fg(TOOL_KEY_COLOR).render("\"name\"")));
        assert!(rendered.contains(&Style::new().fg(TOOL_STRING_COLOR).render("\"A3S 编码\"")));
        assert!(rendered.contains(&Style::new().fg(TOOL_NUMBER_COLOR).render("2")));
        assert!(rendered.contains(&Style::new().fg(TOOL_KEYWORD_COLOR).render("true")));
        assert!(rendered.contains(&Style::new().fg(TN_SUBTLE).render("{")));
    }

    #[test]
    fn wrapped_json_keeps_long_unicode_string_styled_across_rows() {
        let json = "  \"title\": \"世界级终端消息设计体验\"";
        let rows = highlight_json_wrapped(json, 12);
        let plain = rows.iter().map(|row| strip_ansi(row)).collect::<String>();

        assert_eq!(plain, json);
        assert!(rows.len() > 1, "{rows:?}");
        for row in rows.iter().skip(1) {
            if strip_ansi(row).contains(|ch: char| !ch.is_whitespace()) {
                assert!(
                    row.contains(&TOOL_STRING_COLOR.fg_ansi()),
                    "continued string rows should retain their token role: {rows:?}"
                );
            }
        }
    }
}
