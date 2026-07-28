# CLI-Owned Session LLM Client Design

## Goal

Remove the CLI's dependency on the unpublished `AgentSession::llm_client()`
accessor while keeping manual and automatic compact operations on the exact
LLM client used by the active TUI or Web session.

The change is limited to the CLI repository. It does not modify or require a
new release of `a3s-code-core` or `a3s-tui`.

## Ownership

The CLI resolves one `Arc<dyn LlmClient>` when it creates or rebuilds an
interactive session. It passes a clone of that `Arc` to
`SessionOptions::with_llm_client()` and retains another clone in CLI-owned
session state. This creates one client object and one `AgentSession`; cloning
the `Arc` only adds a shared reference.

TUI state stores the client alongside the active `AgentSession`. Web state
stores the client by session ID alongside the existing session map. Manual and
automatic compact operations clone the retained reference and call the
existing `compact_timeline()` implementation.

## Client Resolution

The resolver supports every interactive model source already supported by the
CLI:

- `config.acl` providers use `CodeConfig::llm_config()` and
  `a3s_code_core::llm::create_client_with_config()`.
- Claude account models use `ClaudeClient::from_claude_login()`.
- Codex account models use `CodexClient::from_codex_login()`.
- OS Gateway models use the existing authenticated OpenAI-compatible client.

For config-backed clients, the CLI applies the same effective session inputs
that core would otherwise apply before constructing the client: selected or
default model, session ID, temperature, thinking budget, API timeout, and
supported log-probability overrides. Model/provider credentials, base URL,
headers, retry policy, limits, and session header configuration continue to
come from `CodeConfig::llm_config()`.

Passing the resolved client through `with_llm_client()` makes the agent and
compact pipeline share the same provider, model, credentials, transport,
session identity, and request policy.

## TUI Lifecycle

Initial launch and resume resolve the client before building the session and
store both in `App`. Model/account/effort rebuilds produce a replacement
session and its matching client as one logical result. A failed rebuild keeps
the previous session and client. A successful rebuild replaces both before
later turns or compact requests can use them.

An in-flight compact command owns its own `Arc` clone. Switching models does
not invalidate that request; the old client is released after the request
finishes or times out and all other old references are gone.

## Web Lifecycle

Web session creation, resume, and rebuild resolve and register a matching
client under the session ID. Manual and automatic compact look up that client
and return a clear internal error if the session/client invariant is broken.
Session deletion removes both entries. Login/config-driven session rebuilds
replace both entries together, so compact never reconstructs a client from
possibly changed configuration.

## Compact Behavior

The compact algorithm remains unchanged and CLI-owned:

- Read the complete timeline projection.
- Retain only the latest compact summary.
- Build the compact request window.
- Call `LlmClient::complete()` without entering the agent/tool loop.
- Write the new summary and replace the CLI-maintained model context.
- Preserve the existing automatic compact generation latch.

The three calls to `AgentSession::llm_client()` in TUI, Web manual compact,
and Web automatic compact are removed.

## Error Handling

Client resolution fails session creation or rebuild with the existing
user-visible model/config error rather than falling back to a different model.
No compact-time client reconstruction or silent default is allowed. Web state
invariant failures identify the affected session ID.

## Verification

Focused tests verify config-backed client resolution, shared-client retention,
TUI rebuild pairing, and Web registration/removal behavior. Existing compact
tests remain the source of truth for timeline projection and summary behavior.

Final verification runs formatting, clippy, tests, and release build. A clean
export then runs `.github/scripts/use-published-a3s-crates.sh` and repeats the
CI commands against crates.io `a3s-code-core 4.3.2`, proving that no unpublished
core API remains.
