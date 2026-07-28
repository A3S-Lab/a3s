use super::model::{
    truncate_chars, RemoteAttention, RemoteSnapshot, RemoteTarget, RemoteTargetKind,
    RemoteTargetState, SafeReplyExcerpt,
};

const MAX_REMOTE_REPLY_CHARS: usize = 3_500;
pub(in crate::api::code_web) const REMOTE_LIST_PAGE_SIZE: usize = 12;

pub(in crate::api::code_web) struct RenderedRemoteList {
    pub(in crate::api::code_web) text: String,
    pub(in crate::api::code_web) page: u16,
    pub(in crate::api::code_web) target_ids: Vec<super::model::RemoteTargetId>,
}

pub(in crate::api::code_web) fn render_help() -> String {
    "A3S 微信只读指令\n\
帮助：查看指令\n\
智能体 [页码]：分页列出远程可见目标\n\
选择 2：选择列表中的目标\n\
进度：查看已选目标状态\n\
会话 [页码]：分页列出 A3S 管理会话\n\
最近回复：读取最近回复（默认关闭）\n\
清除选择：清除当前选择\n\n\
当前阶段不接受 Shell、PID、进程信号、文件操作或永久授权。"
        .to_string()
}

pub(in crate::api::code_web) fn render_targets(
    snapshot: &RemoteSnapshot,
    page: u16,
) -> RenderedRemoteList {
    render_list(snapshot, None, "远程可见智能体", "智能体", page)
}

pub(in crate::api::code_web) fn render_sessions(
    snapshot: &RemoteSnapshot,
    page: u16,
) -> RenderedRemoteList {
    render_list(
        snapshot,
        Some(RemoteTargetKind::ManagedSession),
        "A3S 管理会话",
        "会话",
        page,
    )
}

fn render_list(
    snapshot: &RemoteSnapshot,
    filter: Option<RemoteTargetKind>,
    title: &str,
    command: &str,
    page: u16,
) -> RenderedRemoteList {
    let matching = snapshot
        .items
        .iter()
        .filter(|target| filter.is_none_or(|kind| target.kind == kind))
        .collect::<Vec<_>>();
    if matching.is_empty() {
        return RenderedRemoteList {
            text: format!("{title}\n当前没有可见目标。请稍后重试。"),
            page: 1,
            target_ids: Vec::new(),
        };
    }

    let total_pages_usize = matching.len().div_ceil(REMOTE_LIST_PAGE_SIZE);
    let total_pages = u16::try_from(total_pages_usize).unwrap_or(u16::MAX);
    if page == 0 || page > total_pages {
        return RenderedRemoteList {
            text: bound_reply(format!(
                "{title}（共 {} 个，共 {total_pages} 页）\n没有第 {page} 页。发送“{command} 1”从第一页开始。",
                matching.len()
            )),
            page,
            target_ids: Vec::new(),
        };
    }

    let offset = usize::from(page.saturating_sub(1)).saturating_mul(REMOTE_LIST_PAGE_SIZE);
    let visible = matching
        .iter()
        .skip(offset)
        .take(REMOTE_LIST_PAGE_SIZE)
        .copied()
        .collect::<Vec<_>>();
    let mut lines = vec![format!(
        "{title}（第 {page}/{total_pages} 页，共 {} 个）",
        matching.len()
    )];
    for (index, target) in visible.iter().enumerate() {
        let workspace = target
            .workspace_alias
            .as_deref()
            .map(|alias| format!(" · {alias}"))
            .unwrap_or_default();
        let observed = if target.kind == RemoteTargetKind::ObservedProcess {
            " · 只读，执行状态未知"
        } else {
            ""
        };
        lines.push(format!(
            "{}. [{}] {} · {}{}{}",
            index + 1,
            kind_label(target.kind),
            target.display_name,
            state_label(target.state),
            workspace,
            observed
        ));
    }
    let mut navigation = Vec::new();
    if page > 1 {
        navigation.push(format!("发送“{command} {}”查看上一页", page - 1));
    }
    if page < total_pages {
        navigation.push(format!("发送“{command} {}”查看下一页", page + 1));
    }
    if !navigation.is_empty() {
        lines.push(format!("翻页：{}。", navigation.join("；")));
    }
    lines.push("发送“选择 序号”后可查询进度。".to_string());
    RenderedRemoteList {
        text: bound_reply(lines.join("\n")),
        page,
        target_ids: visible.iter().map(|target| target.id.clone()).collect(),
    }
}

pub(in crate::api::code_web) fn render_progress(target: &RemoteTarget) -> String {
    let mut lines = vec![
        format!("{} [{}]", target.display_name, kind_label(target.kind)),
        format!("状态：{}", state_label(target.state)),
        format!("证据：{}", confidence_label(target)),
    ];
    if let Some(workspace) = &target.workspace_alias {
        lines.push(format!("工作区：{workspace}"));
    }
    if let Some(progress) = &target.progress {
        if let Some(goal) = &progress.goal_summary {
            lines.push(format!("任务：{goal}"));
        }
        if let Some(percent) = progress.percent {
            let steps = if progress.total_steps > 0 {
                format!(
                    "（{}/{} 步）",
                    progress.completed_steps, progress.total_steps
                )
            } else {
                String::new()
            };
            lines.push(format!("进度：{percent}%{steps}"));
        }
        lines.push(format!(
            "队列：{} 个待处理{}",
            progress.pending_turns,
            if progress.active_turn {
                "，当前有任务执行中"
            } else {
                ""
            }
        ));
    }
    if target.attention == RemoteAttention::ActionRequired {
        lines.push("需要处理：本机存在等待输入或已暂停的工作。".to_string());
    } else if target.attention == RemoteAttention::Error {
        lines.push("需要处理：目标报告失败或错误，请在本机查看详情。".to_string());
    }
    if target.kind == RemoteTargetKind::ObservedProcess {
        lines.push("限制：仅检测到进程，无法确认任务内容，也不能远程控制。".to_string());
    }
    lines.push(format!("引用：{}", target.id.short_ref()));
    bound_reply(lines.join("\n"))
}

pub(in crate::api::code_web) fn render_latest_reply(reply: &SafeReplyExcerpt) -> String {
    let suffix = if reply.truncated {
        "\n（内容已截断）"
    } else {
        ""
    };
    bound_reply(format!("最近回复\n{}{}", reply.text, suffix))
}

fn kind_label(kind: RemoteTargetKind) -> &'static str {
    match kind {
        RemoteTargetKind::ManagedSession => "会话",
        RemoteTargetKind::CooperativeAgent => "协作",
        RemoteTargetKind::ObservedProcess => "进程",
    }
}

fn state_label(state: RemoteTargetState) -> &'static str {
    match state {
        RemoteTargetState::Planning => "规划中",
        RemoteTargetState::Working => "执行中",
        RemoteTargetState::WaitingApproval => "等待批准",
        RemoteTargetState::WaitingInput => "等待输入",
        RemoteTargetState::Queued => "排队中",
        RemoteTargetState::Paused => "已暂停",
        RemoteTargetState::Idle => "空闲",
        RemoteTargetState::Completed => "已完成",
        RemoteTargetState::Failed => "失败",
        RemoteTargetState::Cancelled => "已取消",
        RemoteTargetState::Detected => "已检测",
        RemoteTargetState::Unknown => "未知",
    }
}

fn confidence_label(target: &RemoteTarget) -> &'static str {
    use super::model::RemoteEvidenceConfidence;
    match target.confidence {
        RemoteEvidenceConfidence::Authoritative => "A3S 管理状态",
        RemoteEvidenceConfidence::Exact => "A3S 精确心跳",
        RemoteEvidenceConfidence::Process => "进程推断",
    }
}

fn bound_reply(value: String) -> String {
    truncate_chars(&value, MAX_REMOTE_REPLY_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::code_web::remote::model::{
        RemoteCapability, RemoteEvidenceConfidence, RemoteProgress, RemoteSnapshot, RemoteTargetId,
    };

    #[test]
    fn renderer_labels_observed_targets_as_unknown_and_read_only() {
        let snapshot = RemoteSnapshot::new(
            10,
            vec![RemoteTarget::observed(
                "process:42",
                "codex".to_string(),
                Some("project".to_string()),
                10,
            )],
            Vec::new(),
        );
        let rendered = render_targets(&snapshot, 1);
        assert!(rendered.text.contains("只读，执行状态未知"));
        assert!(!rendered.text.contains("42"));
        assert_eq!(rendered.target_ids.len(), 1);
    }

    #[test]
    fn renderer_paginates_inventory_deterministically_with_page_local_references() {
        let snapshot = RemoteSnapshot::new(
            10,
            (0..25)
                .map(|index| {
                    RemoteTarget::observed(
                        &format!("process-{index}"),
                        format!("agent-{index:02}"),
                        None,
                        10,
                    )
                })
                .collect(),
            Vec::new(),
        );

        let second = render_targets(&snapshot, 2);
        assert!(second.text.contains("第 2/3 页"));
        assert!(second.text.contains("1. [进程] agent-12"));
        assert!(second.text.contains("12. [进程] agent-23"));
        assert!(!second.text.contains("agent-11"));
        assert!(!second.text.contains("agent-24"));
        assert!(second.text.contains("智能体 1"));
        assert!(second.text.contains("智能体 3"));
        assert_eq!(second.page, 2);
        assert_eq!(second.target_ids.len(), 12);

        let last = render_targets(&snapshot, 3);
        assert_eq!(last.target_ids.len(), 1);
        assert!(last.text.contains("1. [进程] agent-24"));

        let missing = render_targets(&snapshot, 4);
        assert!(missing.target_ids.is_empty());
        assert!(missing.text.contains("没有第 4 页"));
        assert!(missing.text.contains("共 3 页"));
    }

    #[test]
    fn progress_renderer_never_needs_raw_source_identifiers() {
        let target = RemoteTarget {
            id: RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, "secret-session"),
            kind: RemoteTargetKind::ManagedSession,
            display_name: "Build panel".to_string(),
            workspace_alias: Some("web".to_string()),
            state: RemoteTargetState::Working,
            state_detail: "Working".to_string(),
            confidence: RemoteEvidenceConfidence::Authoritative,
            attention: RemoteAttention::None,
            evidence_at_ms: 10,
            parent_id: None,
            capabilities: vec![RemoteCapability::ReadStatus],
            progress: Some(RemoteProgress {
                goal_summary: Some("Finish tests".to_string()),
                percent: Some(50),
                completed_steps: 1,
                total_steps: 2,
                pending_turns: 0,
                active_turn: true,
            }),
        };
        let rendered = render_progress(&target);
        assert!(rendered.contains("进度：50%（1/2 步）"));
        assert!(!rendered.contains("secret-session"));
    }
}
