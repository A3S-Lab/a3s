//! Deep, REAL-LLM test of the `/fork` capability.
//!
//! `/fork` copies the current session's `SessionData` under a new id and resumes
//! it. This test proves the three properties that make that useful — against the
//! actually-configured LLM (`~/.a3s/config.acl`), exercising the exact core
//! primitives the TUI's `/fork` handler calls (`SessionStore::{load,save}` +
//! `resume_session` + a live turn):
//!
//!   1. the fork REMEMBERS the pre-fork conversation (context carried),
//!   2. the fork can DIVERGE (new facts land only in the fork),
//!   3. the ORIGINAL session is untouched and still resumable.
//!
//! Ignored by default — it hits the network + a real model. Run with:
//!   cargo test --test fork_real_llm -- --ignored --nocapture

use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::hitl::{ConfirmationPolicy, TimeoutAction};
use a3s_code_core::store::{FileSessionStore, SessionStore};
use a3s_code_core::{AgentEvent, AgentSession, SessionOptions};

const TURN_TIMEOUT: Duration = Duration::from_secs(180);

/// Run one turn and return the model's final text (accumulated deltas, or the
/// `End` text). Times out so a wedged turn fails loudly instead of hanging.
async fn turn(sess: &AgentSession, prompt: &str) -> String {
    let fut = async {
        // `None` = let the session use + accumulate its OWN persistent history
        // (the CLI's main turn does this). Passing `Some(history)` overrides the
        // context ephemerally and the session won't accumulate — which is what a
        // fork needs to carry.
        let (mut rx, _join) = sess.stream(prompt, None).await.expect("stream start");
        let mut acc = String::new();
        while let Some(ev) = rx.recv().await {
            match ev {
                AgentEvent::TextDelta { text } => acc.push_str(&text),
                AgentEvent::End { text, .. } => {
                    if acc.trim().is_empty() {
                        acc = text;
                    }
                    break;
                }
                _ => {}
            }
        }
        acc
    };
    tokio::time::timeout(TURN_TIMEOUT, fut)
        .await
        .expect("turn timed out")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "hits the real configured LLM over the network"]
async fn fork_carries_context_diverges_and_leaves_original_intact() {
    let home = std::env::var("HOME").expect("HOME");
    let config = format!("{home}/.a3s/config.acl");
    assert!(
        std::path::Path::new(&config).exists(),
        "no ~/.a3s/config.acl — configure a model first"
    );

    let tmp = std::env::temp_dir().join(format!("a3s-fork-realllm-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let store: Arc<dyn SessionStore> = Arc::new(FileSessionStore::new(&tmp).await.expect("store"));

    let agent = a3s_code_core::Agent::new(config)
        .await
        .expect("build agent from config.acl");
    let cwd = tmp.to_string_lossy().to_string();
    // Minimal opts + auto-reject any tool prompt so a plain Q&A turn can't wedge.
    let opts = |id: &str| {
        SessionOptions::new()
            .with_session_store(store.clone())
            .with_session_id(id)
            .with_auto_save(true)
            .with_confirmation_policy(
                ConfirmationPolicy::enabled().with_timeout(500, TimeoutAction::Reject),
            )
    };

    // 1. Original session A: plant a secret the model must recall later.
    let a = agent
        .session_async(cwd.clone(), Some(opts("fork-A")))
        .await
        .expect("session A");
    let a1 = turn(
        &a,
        "Remember this secret code exactly, I'll ask later: BANANA-42. Reply with only: OK",
    )
    .await;
    eprintln!("[A1 plant]   {a1:?}");
    // Persist A — this is exactly what /fork copies from (the TUI relies on the
    // session being auto-saved at idle; we save deterministically).
    a.save().await.expect("persist original A before forking");

    // Load A's persisted data — that's what /fork copies.
    let mut data = store
        .load("fork-A")
        .await
        .expect("store load")
        .expect("session A persisted to the store");
    let pre_fork_len = data.messages.len();
    eprintln!("[diag] pre-fork history: {pre_fork_len} messages");

    // 2. THE FORK (exactly what the TUI /fork handler does): copy A -> new id.
    data.id = "fork-B".to_string();
    store.save(&data).await.expect("save fork B");

    // 3. Resume the FORK and ask for the secret -> proves context carried over.
    let b = agent
        .resume_session_async("fork-B", opts("fork-B"))
        .await
        .expect("resume fork B");
    let b1 = turn(
        &b,
        "What was the secret code I told you earlier? Reply with ONLY the code.",
    )
    .await;
    eprintln!("[B1 recall]  {b1:?}");
    assert!(
        b1.to_uppercase().contains("BANANA-42") || b1.contains("42"),
        "FORK must remember the pre-fork secret (BANANA-42); got {b1:?}"
    );

    // 4. Diverge the fork: change the secret ONLY in B, then confirm B took it.
    let b2 = turn(
        &b,
        "Forget that. The secret code is now CHERRY-99. Reply with only: OK",
    )
    .await;
    eprintln!("[B2 change]  {b2:?}");
    let b3 = turn(&b, "What is the secret code now? Reply with ONLY the code.").await;
    eprintln!("[B3 recheck] {b3:?}");
    assert!(
        b3.to_uppercase().contains("CHERRY-99") || b3.contains("99"),
        "FORK must reflect its own later change (CHERRY-99); got {b3:?}"
    );

    // 5. Resume the ORIGINAL A -> proves it's untouched and diverged from B.
    let a_again = agent
        .resume_session_async("fork-A", opts("fork-A"))
        .await
        .expect("resume original A");
    let a2 = turn(
        &a_again,
        "What is the secret code? Reply with ONLY the code.",
    )
    .await;
    eprintln!("[A2 intact]  {a2:?}");
    assert!(
        a2.to_uppercase().contains("BANANA-42") || a2.contains("42"),
        "ORIGINAL must still hold the pre-fork secret (BANANA-42); got {a2:?}"
    );
    assert!(
        !a2.to_uppercase().contains("CHERRY") && !a2.contains("99"),
        "ORIGINAL must NOT see the fork's later change (CHERRY-99); got {a2:?}"
    );

    eprintln!(
        "\n✅ /fork verified against the real LLM:\n   - fork recalled the pre-fork secret (context carried, history len={pre_fork_len})\n   - fork diverged (its later change stayed in the fork)\n   - original untouched (still BANANA-42, not CHERRY-99)\n"
    );
    let _ = std::fs::remove_dir_all(&tmp);
}
