//! REAL-LLM test of context tracking + auto-compaction — the machinery behind
//! the TUI's ctx% indicator, fill warnings, and mid-turn auto-compact.
//!
//! Proves, against the actually-configured LLM (`~/.a3s/config.acl`):
//!
//!   1. streaming turns REPORT real usage (`TurnEnd.usage.prompt_tokens > 0`)
//!      — this feeds the TUI's live ctx% + warnings; if a gateway stopped
//!      sending the usage chunk this fails first,
//!   2. auto-compaction TRIGGERS once the prompt crosses the threshold
//!      (`ContextCompacted` with fewer messages after than before),
//!   3. the next round's prompt actually SHRINKS after compaction.
//!
//! The threshold is deliberately low and the session starts from a seeded
//! history of more than 30 messages so the core performs real summarization on
//! the first streamed turn instead of burning many real network round trips.
//!
//! Ignored by default — it hits the network + a real model. Run with:
//!   cargo test --test ctx_compact_real_llm -- --ignored --nocapture

use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::hitl::{ConfirmationPolicy, TimeoutAction};
use a3s_code_core::store::{
    ContextUsage, MemorySessionStore, SessionConfig, SessionData, SessionState, SessionStore,
};
use a3s_code_core::{AgentEvent, AgentSession, Message, SessionOptions, TokenUsage};

const TURN_TIMEOUT: Duration = Duration::from_secs(300);
/// Low on purpose: 0.01 × the configured 200k window = trigger at 2k tokens.
const TEST_THRESHOLD: f32 = 0.01;
const BASELINE_SESSION_ID: &str = "ctx-compact-baseline";
const COMPACT_SESSION_ID: &str = "ctx-compact";

/// One streamed turn; returns (last TurnEnd prompt_tokens, compactions seen
/// as (before, after) message counts).
async fn turn(sess: &AgentSession, prompt: &str) -> (usize, Vec<(usize, usize)>) {
    let fut = async {
        let (mut rx, join) = sess.stream(prompt, None).await.expect("stream start");
        let mut prompt_tokens = 0usize;
        let mut compactions = Vec::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                AgentEvent::TurnEnd { usage, .. } => {
                    if usage.prompt_tokens > 0 {
                        prompt_tokens = usage.prompt_tokens;
                    }
                }
                AgentEvent::ContextCompacted {
                    before_messages,
                    after_messages,
                    percent_before,
                    ..
                } => {
                    eprintln!(
                        "[compact] {before_messages} -> {after_messages} messages at {:.0}%",
                        percent_before * 100.0
                    );
                    compactions.push((before_messages, after_messages));
                }
                AgentEvent::End { .. } => break,
                AgentEvent::Error { message } => panic!("turn errored: {message}"),
                _ => {}
            }
        }
        drop(rx);
        join.await.expect("stream join");
        (prompt_tokens, compactions)
    };
    tokio::time::timeout(TURN_TIMEOUT, fut)
        .await
        .expect("turn timed out")
}

fn seeded_history() -> Vec<Message> {
    (0..40)
        .map(|i| {
            let text = format!(
                "Seeded compaction fixture message {i}. \
                 This is inert historical context only; do not act on it. {}",
                format!("Ledger row {i} reconciles to invoice batch {i} and archived note {i}. ")
                    .repeat(6)
            );
            if i % 2 == 0 {
                Message::user(&text)
            } else {
                Message::assistant(&text)
            }
        })
        .collect()
}

fn seeded_session_data(
    session_id: &str,
    workspace: &str,
    messages: Vec<Message>,
    auto_compact: bool,
) -> SessionData {
    SessionData {
        id: session_id.to_string(),
        config: SessionConfig {
            workspace: workspace.to_string(),
            auto_compact,
            auto_compact_threshold: TEST_THRESHOLD,
            max_context_length: 200_000,
            ..Default::default()
        },
        state: SessionState::Active,
        messages,
        context_usage: ContextUsage::default(),
        total_usage: TokenUsage::default(),
        total_cost: 0.0,
        model_name: None,
        cost_records: Vec::new(),
        tool_names: Vec::new(),
        thinking_enabled: false,
        thinking_budget: None,
        created_at: 0,
        updated_at: 0,
        llm_config: None,
        tasks: Vec::new(),
        parent_id: None,
        tenant_id: None,
        principal: None,
        agent_template_id: None,
        correlation_id: None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "hits the real configured LLM over the network"]
async fn context_usage_reports_and_auto_compaction_triggers() {
    let home = std::env::var("HOME").expect("HOME");
    let config = format!("{home}/.a3s/config.acl");
    assert!(
        std::path::Path::new(&config).exists(),
        "no ~/.a3s/config.acl — configure a model first"
    );

    let tmp = std::env::temp_dir().join(format!("a3s-ctx-realllm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let store: Arc<dyn SessionStore> = Arc::new(MemorySessionStore::new());
    let cwd = tmp.to_string_lossy().to_string();
    store
        .save(&seeded_session_data(
            BASELINE_SESSION_ID,
            &cwd,
            seeded_history(),
            false,
        ))
        .await
        .expect("seed baseline session");
    store
        .save(&seeded_session_data(
            COMPACT_SESSION_ID,
            &cwd,
            seeded_history(),
            true,
        ))
        .await
        .expect("seed compacting session");

    let agent = a3s_code_core::Agent::new(config)
        .await
        .expect("build agent from config.acl");
    let first_prompt = "Do not use any tools. Reply with only: OK";

    // Measure the provider-reported prompt usage for the original history.
    // Auto-compaction runs before a request, so the compacting session's first
    // TurnEnd cannot serve as a pre-compaction baseline by itself.
    let baseline_sess = agent
        .resume_session_async(
            BASELINE_SESSION_ID,
            SessionOptions::new()
                .with_session_store(store.clone())
                .with_auto_save(false)
                .with_auto_compact(false)
                .with_llm_api_timeout(120_000)
                .with_continuation(false)
                .with_max_tool_rounds(1)
                .with_temperature(0.0)
                .with_confirmation_policy(
                    ConfirmationPolicy::enabled().with_timeout(500, TimeoutAction::Reject),
                ),
        )
        .await
        .expect("resume baseline session");
    let (baseline_prompt, baseline_compactions) = turn(&baseline_sess, first_prompt).await;
    eprintln!("[baseline] prompt_tokens={baseline_prompt}");
    assert!(baseline_compactions.is_empty());
    assert!(baseline_prompt > 0, "baseline usage was not reported");
    baseline_sess.close().await;

    let sess = agent
        .resume_session_async(
            COMPACT_SESSION_ID,
            SessionOptions::new()
                .with_session_store(store.clone())
                .with_auto_save(true)
                .with_auto_compact(true)
                .with_auto_compact_threshold(TEST_THRESHOLD)
                .with_llm_api_timeout(120_000)
                .with_continuation(false)
                .with_max_tool_rounds(1)
                .with_temperature(0.0)
                .with_confirmation_policy(
                    ConfirmationPolicy::enabled().with_timeout(500, TimeoutAction::Reject),
                ),
        )
        .await
        .expect("resume seeded session");

    let (compacted_prompt, compactions) = turn(&sess, first_prompt).await;
    eprintln!("[compacted turn 1] prompt_tokens={compacted_prompt}");

    assert!(
        compacted_prompt > 0,
        "TurnEnd did not report prompt_tokens > 0 — the provider/gateway is \
         not sending streaming usage, so ctx% and auto-compact are blind"
    );
    let (before, after) = compactions
        .iter()
        .copied()
        .find(|(b, a)| a < b)
        .expect("auto-compaction did not shrink the seeded history on the first turn");
    assert!(after < before, "compaction must reduce messages");
    assert!(
        compacted_prompt < baseline_prompt,
        "compacted prompt ({compacted_prompt}) should be smaller than the \
         same request over the original history ({baseline_prompt})"
    );

    let saved = store
        .load(COMPACT_SESSION_ID)
        .await
        .expect("load compacted session")
        .expect("compacted session saved");
    assert!(
        (after..=after + 1).contains(&saved.messages.len()),
        "auto-saved history should contain the compacted messages plus at most the main response"
    );

    sess.close().await;

    let post_compact_sess = agent
        .resume_session_async(
            COMPACT_SESSION_ID,
            SessionOptions::new()
                .with_session_store(store.clone())
                .with_auto_save(false)
                .with_auto_compact(false)
                .with_llm_api_timeout(120_000)
                .with_continuation(false)
                .with_max_tool_rounds(1)
                .with_temperature(0.0)
                .with_confirmation_policy(
                    ConfirmationPolicy::enabled().with_timeout(500, TimeoutAction::Reject),
                ),
        )
        .await
        .expect("resume compacted session");

    let (post, _) = turn(&post_compact_sess, first_prompt).await;
    eprintln!("[compacted turn 2] prompt_tokens={post}");
    assert!(
        post < baseline_prompt,
        "persisted post-compaction prompt ({post}) should remain smaller than the \
         original-history baseline ({baseline_prompt})"
    );

    eprintln!(
        "\n✅ context tracking + auto-compaction verified against the real LLM:\n   \
         - original-history baseline reported ({baseline_prompt} prompt tokens)\n   \
         - auto-compact fired: {before} -> {after} messages\n   \
         - compacted request shrank to {compacted_prompt} prompt tokens\n   \
         - persisted next request remained at {post} prompt tokens\n"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
