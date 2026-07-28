use super::maintenance::{
    build_fork_context, compact_visible_messages_after_success, fork_messages,
};
use super::messages::visible_message_json;
use super::persistence::{persist_code_web_compact_summary, save_code_web_timeline_message};
use super::sessions::apply_settings_patch;
use super::shell_output::{session_output_json, shell_output_record, ShellOutputRecordInput};
use super::streaming::{send_code_web_event, CodeWebStreamAccumulator};
use super::*;
use a3s_code_core::hitl::{ConfirmationManager, ConfirmationPolicy};
use a3s_code_core::SessionOptions;

#[test]
fn queued_turn_mode_defaults_to_standard_and_validates_deep_research() {
    assert_eq!(
        super::turn_queue::queued_turn_mode(&json!({})).expect("default turn mode"),
        CodeWebQueuedTurnMode::Standard
    );
    assert_eq!(
        super::turn_queue::queued_turn_mode(&json!({ "mode": "deepResearch" }))
            .expect("DeepResearch turn mode"),
        CodeWebQueuedTurnMode::DeepResearch
    );
    assert!(super::turn_queue::queued_turn_mode(&json!({ "mode": "research" })).is_err());
}

#[test]
fn stream_accumulator_uses_last_turn_prompt_tokens_and_final_usage() {
    let mut accumulator = CodeWebStreamAccumulator::default();
    accumulator.observe(AgentEvent::TurnEnd {
        turn: 0,
        usage: TokenUsage {
            prompt_tokens: 100,
            completion_tokens: 10,
            total_tokens: 110,
            cache_read_tokens: None,
            cache_write_tokens: None,
        },
    });
    accumulator.observe(AgentEvent::ToolStart {
        id: "tool-1".to_string(),
        name: "read".to_string(),
    });
    accumulator.observe(AgentEvent::TurnEnd {
        turn: 1,
        usage: TokenUsage {
            prompt_tokens: 170,
            completion_tokens: 20,
            total_tokens: 190,
            cache_read_tokens: None,
            cache_write_tokens: None,
        },
    });
    accumulator.observe(AgentEvent::TextDelta {
        text: "streamed".to_string(),
    });
    accumulator.observe(AgentEvent::ContextCompacted {
        session_id: "session-1".to_string(),
        before_messages: 30,
        after_messages: 12,
        percent_before: 0.85,
        summary: Some("latest durable summary".to_string()),
    });
    accumulator.observe(AgentEvent::End {
        text: "final answer".to_string(),
        usage: TokenUsage {
            prompt_tokens: 270,
            completion_tokens: 30,
            total_tokens: 300,
            cache_read_tokens: None,
            cache_write_tokens: None,
        },
        verification_summary: Box::new(
            a3s_code_core::verification::VerificationSummary::from_reports(&[]),
        ),
        meta: None,
    });

    let result = accumulator.finish().expect("completed stream");
    assert_eq!(result.last_prompt_tokens, 170);
    assert_eq!(result.usage.prompt_tokens, 270);
    assert_eq!(result.text, "final answer");
    assert_eq!(result.tool_calls_count, 1);
    assert_eq!(
        result.compact_summary.as_deref(),
        Some("latest durable summary")
    );
}

#[tokio::test]
async fn task_tracking_updates_are_encoded_as_sse_agent_events() {
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
    let event = AgentEvent::TaskUpdated {
        session_id: "session-1".to_string(),
        tasks: vec![a3s_code_core::planning::Task::new(
            "step-1",
            "Build the panel",
        )],
    };

    send_code_web_event(&sender, &event).await;

    let encoded = receiver
        .recv()
        .await
        .expect("SSE item")
        .expect("serializable event")
        .encode();
    let encoded = String::from_utf8(encoded).expect("UTF-8 SSE frame");
    assert!(encoded.contains("\"type\":\"task_updated\""));
    assert!(encoded.contains("\"content\":\"Build the panel\""));
}

#[tokio::test]
async fn hitl_service_resolves_approval_rejection_and_stale_ids() {
    let root = temp_code_web_store_dir("hitl-service");
    let workspace = root.join("workspace");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    let code_config = a3s_code_core::CodeConfig::from_acl(
        r#"
            default_model = "openai/test-model"
            providers "openai" {
              apiKey = "sk-test"
              baseUrl = "https://example.com/v1"
              models "test-model" {}
            }
        "#,
    )
    .expect("test config");
    let agent = Arc::new(
        a3s_code_core::Agent::from_config(code_config.clone())
            .await
            .expect("test agent"),
    );
    let repository = Arc::new(
        crate::api::code_web::session_store::CodeWebSessionRepository::open(root.join("state"))
            .await
            .expect("session repository"),
    );
    let state = Arc::new(CodeWebState::new(
        Arc::clone(&agent),
        root.join("config.acl"),
        workspace.clone(),
        code_config,
        repository,
    ));
    let (event_tx, _) = tokio::sync::broadcast::channel(8);
    let manager = Arc::new(ConfirmationManager::new(
        ConfirmationPolicy::enabled(),
        event_tx,
    ));
    let session = Arc::new(
        agent
            .session_async(
                workspace.display().to_string(),
                Some(SessionOptions::new().with_confirmation_manager(manager.clone())),
            )
            .await
            .expect("session"),
    );
    let session_id = session.session_id().to_string();
    state
        .sessions
        .lock()
        .await
        .insert(session_id.clone(), session);
    let service = KernelService::new(Arc::clone(&state));

    let approved = manager
        .request_confirmation(
            "tool-approved",
            "write",
            &json!({ "file_path": "README.md" }),
        )
        .await;
    let response = service
        .confirm_tool_use(
            &session_id,
            "tool-approved",
            ConfirmToolUseRequest {
                approved: true,
                reason: None,
            },
        )
        .await
        .expect("approve pending tool");
    assert_eq!(response["confirmed"], true);
    assert_eq!(response["approved"], true);
    assert!(approved.await.expect("approval response").approved);

    let rejected = manager
        .request_confirmation("tool-rejected", "bash", &json!({ "command": "cargo test" }))
        .await;
    service
        .confirm_tool_use(
            &session_id,
            "tool-rejected",
            ConfirmToolUseRequest {
                approved: false,
                reason: Some("Rejected in Web".to_string()),
            },
        )
        .await
        .expect("reject pending tool");
    let rejection = rejected.await.expect("rejection response");
    assert!(!rejection.approved);
    assert_eq!(rejection.reason.as_deref(), Some("Rejected in Web"));

    let error = service
        .confirm_tool_use(
            &session_id,
            "tool-rejected",
            ConfirmToolUseRequest {
                approved: true,
                reason: None,
            },
        )
        .await
        .expect_err("resolved confirmation must not be accepted twice");
    assert!(matches!(error, BootError::BadRequest(_)));

    state.close().await;
    std::fs::remove_dir_all(root).expect("remove test data");
}

#[test]
fn task_tracking_events_are_kept_in_visible_assistant_messages() {
    let events = serde_json::to_value([AgentEvent::TaskUpdated {
        session_id: "session-1".to_string(),
        tasks: vec![a3s_code_core::planning::Task::new(
            "step-1",
            "Build the panel",
        )],
    }])
    .expect("serializable task event");

    let message = visible_message_json(
        "session-1",
        "assistant",
        "Done",
        Some("provider/model".to_string()),
        Some(events),
    );

    assert_eq!(message["events"][0]["type"], "task_updated");
    assert_eq!(
        message["events"][0]["tasks"][0]["content"],
        "Build the panel"
    );
    assert_eq!(message["model"], "provider/model");
}

fn temp_code_web_store_dir(name: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "a3s-code-web-{name}-{}-{nanos}",
        std::process::id()
    ))
}

#[test]
fn fork_messages_rekeys_copied_messages_and_marks_the_branch() {
    let source_messages = vec![json!({
        "id": "old",
        "sessionId": "source",
        "role": "user",
        "content": "continue the UI parity work",
    })];
    let messages = fork_messages("source", "target", &source_messages, "sleep UI");
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["sessionId"], "target");
    assert_ne!(messages[0]["id"], "old");
    assert_eq!(messages[1]["role"], "system");
    assert!(messages[1]["content"]
        .as_str()
        .unwrap()
        .contains("Focus: sleep UI"));
}

#[test]
fn fork_context_prefers_visible_messages_and_keeps_focus() {
    let source_messages = vec![json!({
        "role": "assistant",
        "content": "The toolbar now uses GUI actions for sleep.",
    })];
    let history = vec![Message::user("history fallback")];
    let context = build_fork_context(
        "source",
        "finish fork",
        Some("Earlier compact summary"),
        &source_messages,
        &history,
    )
    .expect("fork context");
    assert!(context.contains("Fork focus: finish fork"));
    assert!(context.contains("Earlier compact summary"));
    assert!(context.contains("The toolbar now uses GUI actions for sleep."));
    assert!(!context.contains("history fallback"));
}

#[test]
fn compact_visible_messages_preserve_history_and_hide_summary_body() {
    let existing = vec![
        json!({
            "id": "user-1",
            "sessionId": "session",
            "role": "user",
            "content": "keep this visible",
        }),
        json!({
            "id": "assistant-1",
            "sessionId": "session",
            "role": "assistant",
            "content": "keep this response",
        }),
    ];
    let summary = "private compact summary that must not be visible";

    let updated = compact_visible_messages_after_success("session", existing.clone(), summary);

    assert_eq!(updated.len(), 3);
    assert_eq!(updated[0], existing[0]);
    assert_eq!(updated[1], existing[1]);
    assert_eq!(updated[2]["sessionId"], "session");
    assert_eq!(updated[2]["role"], "system");
    let marker = updated[2]["content"].as_str().expect("marker content");
    assert!(marker.contains("Context compacted"));
    assert!(!marker.contains(summary));
}

#[test]
fn code_web_timeline_message_persists_timeline_and_context() {
    let store_dir = temp_code_web_store_dir("timeline-context");
    let message = Message::user("hello from code web");

    save_code_web_timeline_message(&store_dir, "session", &message, 128_000, 0.85)
        .expect("save timeline message");
    save_code_web_timeline_message(
        &store_dir,
        "session",
        &Message::assistant("incremental reply"),
        128_000,
        0.85,
    )
    .expect("append timeline message incrementally");

    let timeline_store = crate::timeline::TimelineJsonlStore::for_session(&store_dir, "session");
    let events = timeline_store.load_all().expect("load timeline");
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].message.as_ref().unwrap().text(),
        "hello from code web"
    );

    let context_store = crate::compact::ContextJsonStore::for_session(&store_dir, "session");
    let context = context_store
        .load()
        .expect("load context")
        .expect("context");
    assert_eq!(context.source_message_count, 2);
    assert_eq!(context.source_event_count, 2);
    assert_eq!(
        context.source_file_bytes,
        timeline_store.file_len().unwrap()
    );
    assert_eq!(context.messages.len(), 2);
    assert_eq!(context.messages[0].text(), "hello from code web");
    assert_eq!(context.messages[1].text(), "incremental reply");
}

#[test]
fn code_web_compact_summary_persists_hidden_summary_and_context_marker() {
    let store_dir = temp_code_web_store_dir("compact-summary");

    persist_code_web_compact_summary(&store_dir, "session", "compact summary", 128_000, 0.85)
        .expect("persist compact summary");

    let timeline_store = crate::timeline::TimelineJsonlStore::for_session(&store_dir, "session");
    let events = timeline_store.load_all().expect("load timeline");
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].event_kind,
        crate::timeline::TranscriptEventKind::ContextSummary
    );
    assert!(!events[0].display.visible);
    assert_eq!(
        events[1].event_kind,
        crate::timeline::TranscriptEventKind::CompactMarker
    );
    assert!(events[1].display.visible);

    let context_store = crate::compact::ContextJsonStore::for_session(&store_dir, "session");
    let context = context_store
        .load()
        .expect("load context")
        .expect("context");
    assert_eq!(context.compact_generation, 1);
    assert_eq!(context.messages.len(), 1);
    assert_eq!(context.messages[0].role, "user");
    assert_eq!(context.messages[0].text(), "compact summary");
    assert!(context
        .messages
        .iter()
        .all(|message| message.role != crate::compact::A3S_COMPACT_ROLE));
}

#[test]
fn code_web_core_summary_keeps_the_current_assistant_turn_after_it() {
    let store_dir = temp_code_web_store_dir("compact-then-assistant");

    persist_code_web_compact_summary(&store_dir, "session", "compact summary", 128_000, 0.85)
        .expect("persist compact summary");
    save_code_web_timeline_message(
        &store_dir,
        "session",
        &Message::assistant("current turn complete"),
        128_000,
        0.85,
    )
    .expect("persist assistant after compact summary");

    let context_store = crate::compact::ContextJsonStore::for_session(&store_dir, "session");
    let context = context_store
        .load()
        .expect("load context")
        .expect("context");
    assert_eq!(context.messages.len(), 2);
    assert_eq!(context.messages[0].text(), "compact summary");
    assert_eq!(context.messages[1].role, "assistant");
    assert_eq!(context.messages[1].text(), "current turn complete");
}

#[test]
fn settings_patch_pins_model_and_execution_mode() {
    let mut settings = CodeWebSessionSettings::default();
    let changed = apply_settings_patch(
        &mut settings,
        &json!({
            "model": "openai/gpt-5.5",
            "followDefaultModel": false,
            "permissionMode": "plan",
            "planningMode": "enabled",
            "goalTracking": true,
        }),
        Some("openai/default".to_string()),
    )
    .expect("settings patch");
    assert!(changed);
    assert_eq!(settings.model.as_deref(), Some("openai/gpt-5.5"));
    assert!(!settings.follow_default_model);
    assert_eq!(settings.permission_mode, "plan");
    assert_eq!(settings.planning_mode.as_deref(), Some("enabled"));
    assert_eq!(settings.goal_tracking, Some(true));
}

#[test]
fn settings_patch_returns_to_default_model() {
    let mut settings = CodeWebSessionSettings {
        model: Some("openai/gpt-5.5".to_string()),
        follow_default_model: false,
        ..CodeWebSessionSettings::default()
    };
    let changed = apply_settings_patch(
        &mut settings,
        &json!({ "followDefaultModel": true }),
        Some("openai/default".to_string()),
    )
    .expect("settings patch");
    assert!(changed);
    assert!(settings.model.is_none());
    assert!(settings.follow_default_model);
}

#[test]
fn settings_patch_rejects_unknown_execution_mode() {
    let mut settings = CodeWebSessionSettings::default();
    let error = apply_settings_patch(&mut settings, &json!({ "permissionMode": "danger" }), None)
        .expect_err("unsupported permission mode should fail");
    assert!(error.to_string().contains("unsupported permissionMode"));
}

#[test]
fn session_output_pairs_tool_use_and_result_blocks() {
    let messages = vec![json!({
        "id": "assistant-1",
        "role": "assistant",
        "createdAt": "2026-07-07T00:00:00Z",
        "contentBlocks": [
            {
                "type": "tool_use",
                "id": "call-1",
                "name": "shell_command",
                "input": { "command": "just web" }
            },
            {
                "type": "tool_result",
                "toolUseId": "call-1",
                "content": "server started",
                "isError": false,
                "exitCode": 0,
                "durationMs": 1200
            }
        ]
    })];

    let output = session_output_json("session-1", &messages);
    assert_eq!(output["sessionId"], "session-1");
    assert_eq!(output["total"], 1);
    assert_eq!(output["items"][0]["toolUseId"], "call-1");
    assert_eq!(output["items"][0]["toolName"], "shell_command");
    assert_eq!(
        output["items"][0]["input"],
        "{\n  \"command\": \"just web\"\n}"
    );
    assert_eq!(output["items"][0]["output"], "server started");
    assert_eq!(output["items"][0]["exitCode"], 0);
    assert_eq!(output["items"][0]["durationMs"], 1200);
    assert_eq!(output["items"][0]["sourceMessageId"], "assistant-1");
}

#[test]
fn session_output_reads_legacy_content_array_aliases() {
    let messages = vec![json!({
        "id": "assistant-2",
        "role": "assistant",
        "content": [
            {
                "type": "tool_use",
                "tool_use_id": "call-2",
                "tool_name": "read_file",
                "tool_input": { "path": "README.md" }
            },
            {
                "type": "tool_result",
                "tool_use_id": "call-2",
                "tool_output": [
                    { "type": "text", "text": "# A3S" }
                ],
                "is_error": "true",
                "file_path": "README.md"
            }
        ]
    })];

    let output = session_output_json("session-2", &messages);
    assert_eq!(output["total"], 1);
    assert_eq!(output["items"][0]["toolName"], "read_file");
    assert_eq!(output["items"][0]["output"], "# A3S");
    assert_eq!(output["items"][0]["isError"], true);
    assert_eq!(output["items"][0]["filePath"], "README.md");
}

#[test]
fn session_output_keeps_result_without_matching_use_visible() {
    let messages = vec![json!({
        "id": "assistant-3",
        "role": "assistant",
        "content_blocks": [
            {
                "type": "tool_result",
                "toolCallId": "orphan-result",
                "name": "result",
                "result": { "ok": true }
            }
        ]
    })];

    let output = session_output_json("session-3", &messages);
    assert_eq!(output["total"], 1);
    assert_eq!(output["items"][0]["toolUseId"], "orphan-result");
    assert_eq!(output["items"][0]["toolName"], "result");
    assert_eq!(output["items"][0]["output"], "{\n  \"ok\": true\n}");
}

#[test]
fn shell_output_record_is_visible_to_output_page() {
    let record = shell_output_record(ShellOutputRecordInput {
        session_id: "session-shell",
        command: "printf hello",
        cwd: "/workspace",
        stdout: "hello",
        stderr: "",
        output: "hello",
        exit_code: Some(0),
        is_error: false,
        timed_out: false,
        duration_ms: 25,
        started_at: "2026-07-07T00:00:00Z",
        completed_at: "2026-07-07T00:00:00Z",
    });
    let messages = vec![json!({
        "id": "shell-message",
        "role": "assistant",
        "contentBlocks": [
            {
                "type": "tool_use",
                "id": record["toolUseId"],
                "name": "shell_command",
                "input": {
                    "command": record["input"],
                    "cwd": record["cwd"],
                }
            },
            {
                "type": "tool_result",
                "toolUseId": record["toolUseId"],
                "content": record["output"],
                "isError": record["isError"],
                "exitCode": record["exitCode"],
                "durationMs": record["durationMs"],
            }
        ]
    })];

    let output = session_output_json("session-shell", &messages);
    assert_eq!(output["total"], 1);
    assert_eq!(output["items"][0]["toolName"], "shell_command");
    assert_eq!(
        output["items"][0]["input"],
        "{\n  \"command\": \"printf hello\",\n  \"cwd\": \"/workspace\"\n}"
    );
    assert_eq!(output["items"][0]["output"], "hello");
    assert_eq!(output["items"][0]["exitCode"], 0);
}
