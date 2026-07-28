# CLI-Owned Session LLM Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make TUI and Web sessions share one CLI-owned `Arc<dyn LlmClient>` with compact, removing all dependence on the unpublished `AgentSession::llm_client()` accessor.

**Architecture:** A small CLI module resolves config-backed clients from the same effective `SessionOptions` used to build an agent session. Account-backed overrides remain CLI-created clients. TUI stores the resolved client beside `App.session`; Web stores session/client pairs in `CodeWebState` and updates their lifecycle together.

**Tech Stack:** Rust, `a3s-code-core 4.3.2` public `CodeConfig`/`LlmConfig`/`LlmClient` APIs, Tokio, existing in-file Rust tests.

---

### Task 1: Config-Backed Client Resolution

**Files:**
- Create: `src/session_llm.rs`
- Modify: `src/main.rs`
- Test: `src/session_llm.rs`

- [ ] **Step 1: Write failing resolver tests**

Add tests that build an in-memory ACL config and assert a public preparation
function resolves the selected/default model and copies session-scoped inputs
into `LlmConfig`: session ID, temperature, thinking budget, API timeout,
logprobs, and top-logprobs. Add an error test for an invalid model reference.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test session_llm::tests -- --nocapture
```

Expected: compilation fails because `session_llm` and its preparation API do
not exist.

- [ ] **Step 3: Implement the minimal resolver**

Implement a focused API equivalent to:

```rust
pub(crate) fn resolve_config_llm_client(
    code_config: &CodeConfig,
    options: &SessionOptions,
    session_id: &str,
) -> Result<Arc<dyn LlmClient>, String>
```

Resolve `options.model` or `code_config.default_model`, split
`provider/model`, call `CodeConfig::llm_config()`, apply the public session
overrides, attach `session_id`, and pass the result to
`a3s_code_core::llm::create_client_with_config()`. Preserve the existing env
fallbacks for logprob flags so injection does not change agent behavior.

- [ ] **Step 4: Run the focused tests and verify GREEN**

Run:

```bash
cargo test session_llm::tests -- --nocapture
```

Expected: all resolver tests pass.

- [ ] **Step 5: Commit the resolver**

```bash
git add src/main.rs src/session_llm.rs
git commit -m "fix(cli): resolve session LLM clients in CLI"
```

### Task 2: TUI Session and Client Pairing

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/tui/panels/system/model.rs`
- Test: `src/tui/mod.rs`

- [ ] **Step 1: Write a failing shared-client test**

Add a test-only recording `LlmClient` assertion around a small session/client
pair helper. Verify the same `Arc` supplied to session options is retained for
compact and that replacing a pair changes both values together.

- [ ] **Step 2: Run the focused test and verify RED**

Run the new test by exact name. Expected: compilation fails because the pair
helper and retained `App.llm_client` do not exist.

- [ ] **Step 3: Pair initial TUI session creation with its client**

For config models, call the Task 1 resolver before `resume_session()` or
`session()` and inject the returned client into both attempts. For Claude,
Codex, and OS Gateway, retain the existing override client. Add
`llm_client: Arc<dyn LlmClient>` to `App` and initialize it with the exact
client injected into the initial session.

- [ ] **Step 4: Pair TUI rebuilds and switches**

Change rebuild results to carry `(AgentSession, Arc<dyn LlmClient>, bool)`.
Construct config clients from the effective options; reuse account overrides.
Update `replace_session`, model switches, effort changes, and auth refreshes so
session and client are replaced together. Preserve the current rollback on a
failed model/account switch.

- [ ] **Step 5: Route compact through the retained client**

Replace:

```rust
self.session.llm_client()
```

with:

```rust
Arc::clone(&self.llm_client)
```

- [ ] **Step 6: Run focused TUI tests and verify GREEN**

Run the new pairing test plus existing compact/model switch tests. Expected:
all pass with no warnings.

- [ ] **Step 7: Commit TUI lifecycle changes**

```bash
git add src/tui/mod.rs src/tui/panels/system/model.rs
git commit -m "fix(tui): retain active session LLM client"
```

### Task 3: Web Session and Client Pairing

**Files:**
- Modify: `src/api/code_web/state.rs`
- Modify: `src/api/code_web/session_runtime.rs`
- Modify: `src/api/code_web/kernel/service.rs`
- Test: nearest existing `#[cfg(test)]` modules in these files

- [ ] **Step 1: Write failing Web lifecycle tests**

Add tests for the Web session-client registry: registering a client makes it
available by session ID, replacement returns the current client, and removal
clears it. Use `Arc::ptr_eq` with a recording client to verify identity rather
than only checking map length.

- [ ] **Step 2: Run the focused tests and verify RED**

Run the new tests by exact name. Expected: compilation fails because Web state
does not yet retain LLM clients.

- [ ] **Step 3: Resolve clients with Web session options**

Make `code_web_session_options()` produce options with a CLI-resolved client
and return that client with the runtime data. Use the explicit or default model
and the final session ID. All `Agent::session()` calls receive
`with_llm_client()` options.

- [ ] **Step 4: Maintain Web client lifecycle**

Add a `session_llm_clients` map to `CodeWebState`. Register it whenever a
session is inserted or rebuilt, remove it in `delete_session`, and keep login
or configuration rebuilds synchronized with the session replacement.

- [ ] **Step 5: Route Web compact through the registry**

Change manual and automatic compact to retrieve the matching client by session
ID. Missing client state produces an error naming the affected session. Do not
re-read configuration during compact.

- [ ] **Step 6: Run focused Web tests and verify GREEN**

Run all `api::code_web` tests. Expected: all pass with no warnings.

- [ ] **Step 7: Commit Web lifecycle changes**

```bash
git add src/api/code_web/state.rs src/api/code_web/session_runtime.rs src/api/code_web/kernel/service.rs
git commit -m "fix(web): retain session LLM clients for compact"
```

### Task 4: Published-Crate Regression and Final Verification

**Files:**
- Modify only files required by formatting or a directly observed compile error
- Verify: complete CLI crate

- [ ] **Step 1: Prove no unpublished accessor remains**

Run:

```bash
rg -n 'session\.llm_client\(|\.llm_client\(\)' src
```

Expected: no production call sites.

- [ ] **Step 2: Run local dependency checks**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo build --release
```

Expected: every command exits successfully.

- [ ] **Step 3: Reproduce CI against published crates**

Export `HEAD` to a temporary directory, run
`.github/scripts/use-published-a3s-crates.sh`, then execute the same fmt,
clippy, test, and release commands there. Confirm Cargo selects
`a3s-code-core 4.3.2` and compilation succeeds without `llm_client()`.

- [ ] **Step 4: Review the final diff and worktree**

Confirm every changed production line belongs to client resolution/lifecycle,
all user-owned untracked files remain untouched, and the pre-existing local
`Cargo.lock` modification was not accidentally staged.

- [ ] **Step 5: Commit any final task-scoped corrections**

```bash
git add <only task-scoped files>
git commit -m "test(cli): cover shared session LLM lifecycle"
```

Skip this commit when verification required no additional changes.
