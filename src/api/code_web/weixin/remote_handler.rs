use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::Mutex;

use super::monitor::{InboundHandlerError, WeixinInboundHandler};
use super::runtime_store::{InboundMessage, RemoteListKind, WeixinRuntimeStore};
use crate::api::code_web::remote::{
    render_help, render_latest_reply, render_progress, render_sessions, render_targets,
    RemoteAgentReadService, RemoteIntent, RemoteIntentError, RemoteIntentRouter, RemoteReadQuery,
    RemoteReadResult, RemoteReadScope, RemoteSnapshot, RemoteTarget, RemoteTargetId,
    REMOTE_LIST_PAGE_SIZE,
};

const DEFAULT_RATE_LIMIT: u32 = 30;
const RATE_WINDOW: Duration = Duration::from_secs(60);

pub(super) struct RemoteReadHandler {
    remote: Arc<RemoteAgentReadService>,
    intent_router: Arc<RemoteIntentRouter>,
    runtime_store: WeixinRuntimeStore,
    scope: RemoteReadScope,
    limiter: Mutex<RateWindow>,
    rate_limit: u32,
}

struct RateWindow {
    started_at: Instant,
    accepted: u32,
}

impl RemoteReadHandler {
    pub(super) fn new(
        remote: Arc<RemoteAgentReadService>,
        runtime_store: WeixinRuntimeStore,
    ) -> Self {
        Self::with_router(
            remote,
            Arc::new(RemoteIntentRouter::deterministic()),
            runtime_store,
        )
    }

    pub(super) fn with_router(
        remote: Arc<RemoteAgentReadService>,
        intent_router: Arc<RemoteIntentRouter>,
        runtime_store: WeixinRuntimeStore,
    ) -> Self {
        Self::with_scope_and_rate(
            remote,
            intent_router,
            runtime_store,
            RemoteReadScope::default(),
            DEFAULT_RATE_LIMIT,
        )
    }

    #[cfg(test)]
    pub(super) fn for_test(
        remote: Arc<RemoteAgentReadService>,
        runtime_store: WeixinRuntimeStore,
        session_content: bool,
        rate_limit: u32,
    ) -> Self {
        Self::with_scope_and_rate(
            remote,
            Arc::new(RemoteIntentRouter::deterministic()),
            runtime_store,
            RemoteReadScope {
                session_content,
                ..RemoteReadScope::default()
            },
            rate_limit,
        )
    }

    fn with_scope_and_rate(
        remote: Arc<RemoteAgentReadService>,
        intent_router: Arc<RemoteIntentRouter>,
        runtime_store: WeixinRuntimeStore,
        scope: RemoteReadScope,
        rate_limit: u32,
    ) -> Self {
        Self {
            remote,
            intent_router,
            runtime_store,
            scope,
            limiter: Mutex::new(RateWindow {
                started_at: Instant::now(),
                accepted: 0,
            }),
            rate_limit: rate_limit.max(1),
        }
    }

    async fn allow_request(&self) -> bool {
        let mut limiter = self.limiter.lock().await;
        if limiter.started_at.elapsed() >= RATE_WINDOW {
            limiter.started_at = Instant::now();
            limiter.accepted = 0;
        }
        if limiter.accepted >= self.rate_limit {
            return false;
        }
        limiter.accepted = limiter.accepted.saturating_add(1);
        true
    }

    async fn dispatch(&self, intent: RemoteIntent) -> Result<String, InboundHandlerError> {
        match intent {
            RemoteIntent::Help => Ok(render_help()),
            RemoteIntent::ListTargets { page } => {
                let snapshot = self.snapshot(RemoteReadQuery::ListTargets).await?;
                let rendered = render_targets(&snapshot, page);
                self.persist_list_context(
                    RemoteListKind::Targets,
                    rendered.page,
                    &rendered.target_ids,
                )
                .await?;
                Ok(rendered.text)
            }
            RemoteIntent::ListSessions { page } => {
                let snapshot = self.snapshot(RemoteReadQuery::ListSessions).await?;
                let rendered = render_sessions(&snapshot, page);
                self.persist_list_context(
                    RemoteListKind::Sessions,
                    rendered.page,
                    &rendered.target_ids,
                )
                .await?;
                Ok(rendered.text)
            }
            RemoteIntent::Select { reference } => self.select(&reference).await,
            RemoteIntent::ClearSelection => {
                self.runtime_store
                    .clear_selection()
                    .await
                    .map_err(|_| InboundHandlerError::Rejected)?;
                Ok("已清除当前选择。发送“智能体”可重新查看目标。".to_string())
            }
            RemoteIntent::Progress => self.selected_progress().await,
            RemoteIntent::LatestReply => self.selected_latest_reply().await,
        }
    }

    async fn persist_list_context(
        &self,
        kind: RemoteListKind,
        page: u16,
        target_ids: &[RemoteTargetId],
    ) -> Result<(), InboundHandlerError> {
        if target_ids.is_empty() {
            return self
                .runtime_store
                .clear_list_context()
                .await
                .map_err(|_| InboundHandlerError::Rejected);
        }
        self.runtime_store
            .set_list_context(
                kind,
                page,
                target_ids
                    .iter()
                    .map(|target_id| target_id.as_str().to_string())
                    .collect(),
            )
            .await
            .map(|_| ())
            .map_err(|_| InboundHandlerError::Rejected)
    }

    async fn snapshot(
        &self,
        query: RemoteReadQuery,
    ) -> Result<RemoteSnapshot, InboundHandlerError> {
        let receipt = self.remote.query(query, self.scope).await;
        match receipt.result {
            RemoteReadResult::Snapshot(snapshot) => Ok(snapshot),
            _ => Err(InboundHandlerError::Rejected),
        }
    }

    async fn select(&self, reference: &str) -> Result<String, InboundHandlerError> {
        if let Ok(index) = reference.parse::<usize>() {
            return self.select_from_list(index).await;
        }

        let snapshot = self.snapshot(RemoteReadQuery::ListTargets).await?;
        if let Some(target_id) = RemoteTargetId::parse(reference) {
            return match find_target(&snapshot, &target_id) {
                Some(target) => self.persist_selection(target).await,
                None => Ok("该目标不再可见。发送“智能体”刷新后重试。".to_string()),
            };
        }

        let short_matches = snapshot
            .items
            .iter()
            .filter(|target| target.id.short_ref().eq_ignore_ascii_case(reference))
            .collect::<Vec<_>>();
        match short_matches.as_slice() {
            [target] => return self.persist_selection(target).await,
            [] => {}
            _ => return self.render_disambiguation(&short_matches, false).await,
        }

        let title_matches = snapshot
            .items
            .iter()
            .filter(|target| target.display_name.eq_ignore_ascii_case(reference))
            .collect::<Vec<_>>();
        match title_matches.as_slice() {
            [target] => self.persist_selection(target).await,
            [] => Ok("无法匹配该目标。发送“智能体”查看当前序号后重试。".to_string()),
            _ => self.render_disambiguation(&title_matches, true).await,
        }
    }

    async fn select_from_list(&self, index: usize) -> Result<String, InboundHandlerError> {
        let checkpoint = self.runtime_store.checkpoint().await;
        let Some(context) = checkpoint.list_context else {
            return Ok("没有可用的列表上下文。请先发送“智能体”或“会话”。".to_string());
        };
        let query = match context.kind {
            RemoteListKind::Sessions => RemoteReadQuery::ListSessions,
            RemoteListKind::Targets | RemoteListKind::Disambiguation => {
                RemoteReadQuery::ListTargets
            }
        };
        let Some(target_id) = index
            .checked_sub(1)
            .and_then(|offset| context.target_ids.get(offset))
            .and_then(|target_id| RemoteTargetId::parse(target_id))
        else {
            return Ok(format!(
                "当前列表没有序号 {index}。请重新发送“智能体”或“会话”查看可选序号。"
            ));
        };

        let snapshot = self.snapshot(query).await?;
        let Some(target) = find_target(&snapshot, &target_id) else {
            self.runtime_store
                .clear_list_context()
                .await
                .map_err(|_| InboundHandlerError::Rejected)?;
            return Ok(
                "该列表中的目标已不可见，列表上下文已清除。请发送“智能体”或“会话”刷新。"
                    .to_string(),
            );
        };
        self.persist_selection(target).await
    }

    async fn persist_selection(
        &self,
        target: &RemoteTarget,
    ) -> Result<String, InboundHandlerError> {
        self.runtime_store
            .set_selection(target.id.as_str())
            .await
            .map_err(|_| InboundHandlerError::Rejected)?;
        Ok(format!("已选择。\n{}", render_progress(target)))
    }

    async fn render_disambiguation(
        &self,
        matches: &[&RemoteTarget],
        duplicate_title: bool,
    ) -> Result<String, InboundHandlerError> {
        let mut visible = matches.to_vec();
        visible.sort_by(|left, right| {
            left.workspace_alias
                .cmp(&right.workspace_alias)
                .then_with(|| left.id.cmp(&right.id))
        });
        visible.truncate(REMOTE_LIST_PAGE_SIZE);
        let target_ids = visible
            .iter()
            .map(|target| target.id.clone())
            .collect::<Vec<_>>();
        self.persist_list_context(RemoteListKind::Disambiguation, 1, &target_ids)
            .await?;

        let heading = if duplicate_title {
            format!("找到 {} 个同名目标，请继续选择：", matches.len())
        } else {
            format!("找到 {} 个匹配目标，请继续选择：", matches.len())
        };
        let mut lines = vec![heading];
        for (index, target) in visible.iter().enumerate() {
            let workspace = target
                .workspace_alias
                .as_deref()
                .map(|alias| format!(" · {alias}"))
                .unwrap_or_default();
            lines.push(format!(
                "{}. {}{} · 引用 {}",
                index + 1,
                target.display_name,
                workspace,
                target.id.short_ref()
            ));
        }
        if matches.len() > visible.len() {
            lines.push(format!(
                "为避免回复过长，仅显示前 {} 个匹配目标。",
                visible.len()
            ));
        }
        lines.push("发送“选择 序号”继续。".to_string());
        Ok(lines.join("\n"))
    }

    async fn selected_target_id(&self) -> Result<Option<RemoteTargetId>, InboundHandlerError> {
        let checkpoint = self.runtime_store.checkpoint().await;
        Ok(checkpoint
            .selection
            .as_ref()
            .and_then(|selection| RemoteTargetId::parse(&selection.target_id)))
    }

    async fn selected_progress(&self) -> Result<String, InboundHandlerError> {
        let Some(target_id) = self.selected_target_id().await? else {
            return Ok("尚未选择目标。请先发送“智能体”，再发送“选择 序号”。".to_string());
        };
        let receipt = self
            .remote
            .query(RemoteReadQuery::Inspect(target_id), self.scope)
            .await;
        match receipt.result {
            RemoteReadResult::Target(Some(target)) => Ok(render_progress(&target)),
            RemoteReadResult::Target(None) => {
                self.runtime_store
                    .clear_selection()
                    .await
                    .map_err(|_| InboundHandlerError::Rejected)?;
                Ok("已选目标不再可见，选择已清除。请发送“智能体”刷新。".to_string())
            }
            _ => Err(InboundHandlerError::Rejected),
        }
    }

    async fn selected_latest_reply(&self) -> Result<String, InboundHandlerError> {
        if !self.scope.session_content {
            return Ok("“最近回复”默认关闭。请先在本机 A3S Web 明确开启会话内容读取。".to_string());
        }
        let Some(target_id) = self.selected_target_id().await? else {
            return Ok("尚未选择会话。请先发送“会话”，再发送“选择 序号”。".to_string());
        };
        let receipt = self
            .remote
            .query(RemoteReadQuery::LatestReply(target_id), self.scope)
            .await;
        match receipt.result {
            RemoteReadResult::LatestReply(Some(reply)) => Ok(render_latest_reply(&reply)),
            RemoteReadResult::LatestReply(None) => {
                Ok("该目标没有可安全转发的最近助手回复。".to_string())
            }
            _ => Err(InboundHandlerError::Rejected),
        }
    }
}

#[async_trait]
impl WeixinInboundHandler for RemoteReadHandler {
    async fn handle(
        &self,
        message: &InboundMessage,
    ) -> Result<Option<String>, InboundHandlerError> {
        if message.group_id.is_some() || message.recipient_id.is_none() {
            return Err(InboundHandlerError::Rejected);
        }
        if !self.allow_request().await {
            return Ok(Some(
                "请求过于频繁。请稍后再试；A3S 未执行任何操作。".to_string(),
            ));
        }
        let intent =
            match self.intent_router.route(message.text.expose()).await {
                Ok(intent) => intent,
                Err(RemoteIntentError::InvalidLength) => {
                    return Ok(Some(
                        "指令过长。发送“帮助”查看受支持的只读指令。".to_string(),
                    ))
                }
                Err(RemoteIntentError::Ambiguous) => return Ok(Some(
                    "我不确定你要查询什么，且未执行任何操作。请改用“智能体”“会话”“进度”或“帮助”。"
                        .to_string(),
                )),
                Err(RemoteIntentError::Empty | RemoteIntentError::Unsupported) => {
                    return Ok(Some(
                        "无法识别该指令，且未执行任何操作。发送“帮助”查看只读指令。".to_string(),
                    ))
                }
            };
        self.dispatch(intent).await.map(Some)
    }
}

fn find_target<'a>(
    snapshot: &'a RemoteSnapshot,
    target_id: &RemoteTargetId,
) -> Option<&'a RemoteTarget> {
    snapshot.items.iter().find(|target| &target.id == target_id)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::api::code_web::kernel::{ManagedQueueEvidence, ManagedSessionEvidence};
    use crate::api::code_web::remote::RemoteAgentReadService;
    use crate::system_agents::SystemAgentSnapshot;
    use a3s_boot::ilink::SecretValue;

    fn message(text: &str) -> InboundMessage {
        InboundMessage {
            key: format!("message-{text}"),
            sender_id: SecretValue::new("owner-id").unwrap(),
            recipient_id: Some(SecretValue::new("bot-id").unwrap()),
            group_id: None,
            context_token: None,
            text: SecretValue::new(text).unwrap(),
            run_id: None,
            created_at_ms: Some(1),
        }
    }

    fn remote_service() -> Arc<RemoteAgentReadService> {
        Arc::new(RemoteAgentReadService::for_test(
            vec![ManagedSessionEvidence {
                source_id: "managed-source-secret".to_string(),
                title: Some("Web tests".to_string()),
                workspace: "/Users/alice/web".to_string(),
                created_at_ms: 1,
                updated_at_ms: 2,
                goal: None,
                queue: ManagedQueueEvidence {
                    pending_turns: 2,
                    active: false,
                    paused: false,
                },
                children: Vec::new(),
            }],
            HashMap::from([(
                "managed-source-secret".to_string(),
                "Tests passed in /Users/alice/private with token=canary".to_string(),
            )]),
            SystemAgentSnapshot::default(),
        ))
    }

    fn paged_remote_service() -> Arc<RemoteAgentReadService> {
        Arc::new(RemoteAgentReadService::for_test(
            (0..25)
                .map(|index| ManagedSessionEvidence {
                    source_id: format!("managed-source-{index:02}"),
                    title: Some(format!("Session {index:02}")),
                    workspace: format!("/Users/alice/workspace-{index:02}"),
                    created_at_ms: 1,
                    updated_at_ms: 2,
                    goal: None,
                    queue: ManagedQueueEvidence {
                        pending_turns: 0,
                        active: false,
                        paused: false,
                    },
                    children: Vec::new(),
                })
                .collect(),
            HashMap::new(),
            SystemAgentSnapshot::default(),
        ))
    }

    fn duplicate_title_remote_service() -> Arc<RemoteAgentReadService> {
        Arc::new(RemoteAgentReadService::for_test(
            ["first", "second"]
                .into_iter()
                .map(|workspace| ManagedSessionEvidence {
                    source_id: format!("managed-{workspace}"),
                    title: Some("Duplicate".to_string()),
                    workspace: format!("/Users/alice/{workspace}"),
                    created_at_ms: 1,
                    updated_at_ms: 2,
                    goal: None,
                    queue: ManagedQueueEvidence {
                        pending_turns: 0,
                        active: false,
                        paused: false,
                    },
                    children: Vec::new(),
                })
                .collect(),
            HashMap::new(),
            SystemAgentSnapshot::default(),
        ))
    }

    async fn runtime_store(temporary: &tempfile::TempDir) -> WeixinRuntimeStore {
        let root = std::fs::canonicalize(temporary.path()).unwrap();
        WeixinRuntimeStore::open(root.join("runtime"))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn remote_handler_persists_selection_and_answers_progress_without_source_ids() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let handler = RemoteReadHandler::for_test(remote_service(), runtime.clone(), false, 20);

        let list = handler.handle(&message("智能体")).await.unwrap().unwrap();
        assert!(list.contains("Web tests"));
        let selected = handler.handle(&message("选择 1")).await.unwrap().unwrap();
        assert!(selected.contains("已选择"));
        let progress = handler.handle(&message("进度")).await.unwrap().unwrap();
        assert!(progress.contains("队列：2 个待处理"));
        assert!(!progress.contains("managed-source-secret"));
        assert!(runtime.checkpoint().await.selection.is_some());
        assert!(runtime.checkpoint().await.list_context.is_some());
        let content = handler.handle(&message("最近回复")).await.unwrap().unwrap();
        assert!(content.contains("默认关闭"));
    }

    #[tokio::test]
    async fn remote_handler_persists_page_local_selection_context_across_restart() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let handler =
            RemoteReadHandler::for_test(paged_remote_service(), runtime.clone(), false, 20);

        let page = handler.handle(&message("会话 2")).await.unwrap().unwrap();
        assert!(page.contains("第 2/3 页"));
        assert!(page.contains("1. [会话] Session 12"));
        let context = runtime
            .checkpoint()
            .await
            .list_context
            .expect("durable list context");
        assert_eq!(context.kind, RemoteListKind::Sessions);
        assert_eq!(context.page, 2);
        assert_eq!(context.target_ids.len(), 12);
        drop(handler);

        let restarted = RemoteReadHandler::for_test(paged_remote_service(), runtime, false, 20);
        let selected = restarted.handle(&message("选择 1")).await.unwrap().unwrap();
        assert!(selected.contains("Session 12"));
        assert!(selected.contains("workspace-12"));
    }

    #[tokio::test]
    async fn remote_handler_disambiguates_duplicate_safe_titles_before_selection() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let handler = RemoteReadHandler::for_test(
            duplicate_title_remote_service(),
            runtime.clone(),
            false,
            20,
        );

        let ambiguous = handler
            .handle(&message("选择 Duplicate"))
            .await
            .unwrap()
            .unwrap();
        assert!(ambiguous.contains("找到 2 个同名目标"));
        let context = runtime
            .checkpoint()
            .await
            .list_context
            .expect("disambiguation context");
        assert_eq!(context.kind, RemoteListKind::Disambiguation);
        assert_eq!(context.target_ids.len(), 2);

        let selected = handler.handle(&message("选择 2")).await.unwrap().unwrap();
        assert!(selected.contains("已选择"));
        assert!(selected.contains("second"));
    }

    #[tokio::test]
    async fn remote_handler_accepts_opaque_short_and_unique_title_references() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let remote = remote_service();
        let snapshot = remote.snapshot(RemoteReadScope::default()).await;
        let target = snapshot.items.first().expect("managed target");
        let full_reference = target.id.as_str().to_string();
        let short_reference = target.id.short_ref().to_string();
        let handler = RemoteReadHandler::for_test(remote, runtime, false, 20);

        let by_title = handler
            .handle(&message("选择 Web tests"))
            .await
            .unwrap()
            .unwrap();
        assert!(by_title.contains("已选择"));
        let by_short = handler
            .handle(&message(&format!("选择 {short_reference}")))
            .await
            .unwrap()
            .unwrap();
        assert!(by_short.contains("已选择"));
        let by_full = handler
            .handle(&message(&format!("选择 {full_reference}")))
            .await
            .unwrap()
            .unwrap();
        assert!(by_full.contains("已选择"));
    }

    #[tokio::test]
    async fn remote_handler_requires_list_context_and_clears_it_when_target_disappears() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let handler = RemoteReadHandler::for_test(remote_service(), runtime.clone(), false, 20);

        let missing_context = handler.handle(&message("选择 1")).await.unwrap().unwrap();
        assert!(missing_context.contains("先发送“智能体”或“会话”"));

        runtime
            .set_list_context(
                RemoteListKind::Targets,
                1,
                vec!["rtm_0123456789abcdef01234567".to_string()],
            )
            .await
            .unwrap();
        let disappeared = handler.handle(&message("选择 1")).await.unwrap().unwrap();
        assert!(disappeared.contains("目标已不可见"));
        assert!(runtime.checkpoint().await.list_context.is_none());
    }

    #[tokio::test]
    async fn remote_handler_forwards_only_bounded_redacted_reply_when_content_scope_is_enabled() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let handler = RemoteReadHandler::for_test(remote_service(), runtime, true, 20);

        handler.handle(&message("会话")).await.unwrap();
        handler.handle(&message("选择 1")).await.unwrap();
        let content = handler.handle(&message("最近回复")).await.unwrap().unwrap();

        assert!(content.contains("Tests passed"));
        assert!(content.contains("[path]"));
        assert!(content.contains("[redacted]"));
        assert!(!content.contains("alice"));
        assert!(!content.contains("canary"));
    }

    #[tokio::test]
    async fn remote_handler_rejects_prompt_injection_and_rate_limits_without_execution() {
        let temporary = tempfile::tempdir().unwrap();
        let runtime = runtime_store(&temporary).await;
        let handler = RemoteReadHandler::for_test(remote_service(), runtime, false, 1);
        let rejected = handler
            .handle(&message("运行 shell rm -rf /"))
            .await
            .unwrap()
            .unwrap();
        assert!(rejected.contains("未执行任何操作"));
        let limited = handler.handle(&message("帮助")).await.unwrap().unwrap();
        assert!(limited.contains("请求过于频繁"));
    }
}
