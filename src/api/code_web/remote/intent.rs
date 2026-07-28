use thiserror::Error;

const MAX_REMOTE_COMMAND_CHARS: usize = 512;
pub(super) const MAX_REMOTE_LIST_PAGE: u16 = 100;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) enum RemoteIntent {
    Help,
    ListTargets { page: u16 },
    ListSessions { page: u16 },
    Select { reference: String },
    ClearSelection,
    Progress,
    LatestReply,
}

pub(in crate::api::code_web) fn parse_remote_intent(
    text: &str,
) -> Result<RemoteIntent, RemoteIntentError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(RemoteIntentError::Empty);
    }
    if text.chars().count() > MAX_REMOTE_COMMAND_CHARS || text.contains('\0') {
        return Err(RemoteIntentError::InvalidLength);
    }
    let normalized = text.strip_prefix('/').unwrap_or(text).trim();
    match normalized {
        "帮助" => Ok(RemoteIntent::Help),
        "进度" => Ok(RemoteIntent::Progress),
        "最近回复" => Ok(RemoteIntent::LatestReply),
        "清除选择" | "取消选择" => Ok(RemoteIntent::ClearSelection),
        _ => parse_list(normalized, "智能体", |page| RemoteIntent::ListTargets {
            page,
        })
        .or_else(|| {
            parse_list(normalized, "会话", |page| RemoteIntent::ListSessions {
                page,
            })
        })
        .or_else(|| parse_selection(normalized))
        .ok_or(RemoteIntentError::Unsupported),
    }
}

fn parse_list(
    text: &str,
    command: &str,
    intent: impl FnOnce(u16) -> RemoteIntent,
) -> Option<RemoteIntent> {
    let remainder = text.strip_prefix(command)?;
    let page = if remainder.is_empty() {
        1
    } else {
        let remainder = remainder.trim();
        if remainder.is_empty() {
            1
        } else {
            let page = remainder.parse::<u16>().ok()?;
            (1..=MAX_REMOTE_LIST_PAGE).contains(&page).then_some(page)?
        }
    };
    Some(intent(page))
}

fn parse_selection(text: &str) -> Option<RemoteIntent> {
    let reference = text.strip_prefix("选择")?.trim();
    if reference.is_empty()
        || reference.chars().count() > 32
        || reference.chars().any(char::is_control)
    {
        return None;
    }
    Some(RemoteIntent::Select {
        reference: reference.to_string(),
    })
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(in crate::api::code_web) enum RemoteIntentError {
    #[error("remote command is empty")]
    Empty,
    #[error("remote command length is invalid")]
    InvalidLength,
    #[error("remote command is unsupported")]
    Unsupported,
    #[error("remote command is ambiguous")]
    Ambiguous,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_remote_parser_accepts_only_closed_read_intents() {
        assert_eq!(parse_remote_intent("帮助"), Ok(RemoteIntent::Help));
        assert_eq!(
            parse_remote_intent("智能体"),
            Ok(RemoteIntent::ListTargets { page: 1 })
        );
        assert_eq!(
            parse_remote_intent("智能体 2"),
            Ok(RemoteIntent::ListTargets { page: 2 })
        );
        assert_eq!(
            parse_remote_intent("会话 100"),
            Ok(RemoteIntent::ListSessions { page: 100 })
        );
        assert_eq!(
            parse_remote_intent("选择 2"),
            Ok(RemoteIntent::Select {
                reference: "2".to_string()
            })
        );
        assert_eq!(
            parse_remote_intent("/最近回复"),
            Ok(RemoteIntent::LatestReply)
        );
        assert_eq!(
            parse_remote_intent("运行 shell rm -rf /"),
            Err(RemoteIntentError::Unsupported)
        );
        assert_eq!(
            parse_remote_intent("批准永久权限"),
            Err(RemoteIntentError::Unsupported)
        );
        assert_eq!(
            parse_remote_intent("智能体 0"),
            Err(RemoteIntentError::Unsupported)
        );
        assert_eq!(
            parse_remote_intent("会话 101"),
            Err(RemoteIntentError::Unsupported)
        );
    }
}
