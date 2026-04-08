# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Structure (MANDATORY — DO NOT MODIFY)

**CRITICAL: This is the A3S monorepo. The root directory contains ONLY the following files and directories. DO NOT add Rust source code, Cargo.toml, or any crate-specific files to the root.**

```
a3s/                            ← THIS IS THE MONOREPO ROOT
├── .gitignore                  # Git ignore rules
├── .gitmodules                 # Submodule registry (ALL crates listed here)
├── CLAUDE.md                   # This file
├── LICENSE                     # MIT license
├── README.md                   # Project overview, architecture, project index
├── apps/                       # Applications (desktop, web, CLI)
│   ├── os/                     # A3S platform (NestJS backend + React frontend + CLI)
│   └── safeclaw/               # SafeClaw desktop app (Tauri + React + embedded gateway)
│       ├── src/                # React frontend
│       ├── src-tauri/          # Tauri Rust backend
│       └── crates/safeclaw/    # SafeClaw gateway library + CLI binary
├── crates/                     # Shared Rust crates (submodules or local)
│   ├── box/                    # [submodule] git@github.com:A3S-Lab/Box.git
│   ├── code/                   # [submodule] git@github.com:A3S-Lab/Code.git — AI coding agent framework (library, not service)
│   ├── event/                  # [submodule] git@github.com:A3S-Lab/Event.git
│   ├── flow/                   # [submodule] git@github.com:A3S-Lab/Flow.git — workflow engine (JSON DAG, Dify/n8n style)
│   ├── gateway/                # [submodule] git@github.com:A3S-Lab/Gateway.git
│   ├── lane/                   # [submodule] git@github.com:A3S-Lab/Lane.git
│   ├── power/                  # [submodule] git@github.com:A3S-Lab/Power.git
│   ├── common/                 # Shared types: privacy, tools, transport
│   ├── search/                 # [submodule] git@github.com:A3S-Lab/Search.git
│   └── updater/                # [submodule] git@github.com:A3S-Lab/Updater.git
├── docs/                       # Documentation and architecture diagrams
│   └── architecture/           # LikeC4 architecture diagrams
└── homebrew-tap/               # [submodule] git@github.com:A3S-Lab/homebrew-tap.git
```

### Root Directory Protection Rules

**⛔ NEVER do any of the following in the root directory:**

1. **NEVER add `Cargo.toml` to root** — Each crate has its own `Cargo.toml` inside its submodule
2. **NEVER add `src/`, `tests/`, `benches/` to root** — Source code belongs in `crates/<name>/`
3. **NEVER add `justfile` to root** — Each crate has its own build commands
4. **NEVER run `cargo init` or `cargo new` in root** — This is not a Rust workspace
5. **NEVER change `git remote` of root** — It MUST point to `git@github.com:A3S-Lab/a3s.git`

**The root directory is a pure orchestration layer. It contains ONLY:**
- `.gitignore`, `.gitmodules`, `CLAUDE.md`, `LICENSE`, `README.md`, `justfile`
- `apps/` directory (frontend apps and non-Rust projects)
- `crates/` directory (Rust crates — submodules or local)
- `docs/` directory (documentation and architecture diagrams)
- `homebrew-tap/` submodule

**If you find yourself adding Rust source files to the root, STOP. You are doing it wrong.**

---

## Git Safety Rules (MANDATORY — PREVENT DATA LOSS)

### Core Principle: Never Lose Uncommitted Work

**⛔ CRITICAL: `git stash pop` CAN DESTROY YOUR CHANGES**

When running `git stash pop` after a failed merge/rebase, git may create an "autostash" that replaces your original stash. The original stash content can be LOST. This has caused real data loss.

### Rules

1. **Before `git pull --rebase`: ALWAYS commit or stash first**
   ```bash
   # BAD: pull with uncommitted changes — auto-stashes and can lose work
   git pull --rebase origin main

   # GOOD: commit first, then pull
   git add -A && git commit -m "wip: my changes"
   git pull --rebase origin main

   # GOOD: stash explicitly, then pop carefully
   git stash push -m "my changes"
   git pull --rebase origin main
   git stash pop  # CHECK DIFF AFTER!
   ```

2. **After `git stash pop` with conflicts: VERIFY your changes**
   ```bash
   # Check what changed
   git diff --stat HEAD

   # Verify critical files are correct
   grep "your-feature" src/important-file.tsx

   # If changes are missing, recover from reflog
   git reflog | grep "stash"
   git stash branch recovery-branch stash@{N}  # Create branch from stash
   ```

3. **When stash pop fails: Use `git stash branch` instead of autostash**
   ```bash
   # Create a new branch from the stash to be safe
   git stash branch recovery stash@{0}

   # Then manually merge the branch
   git checkout your-branch
   git merge recovery
   ```

4. **Always use `git stash push -m "message"` with descriptive messages**
   - Unnamed stashes (`stash@{N}`) are hard to track
   - Descriptive messages make recovery easier

### Why This Matters

`git pull --rebase` internally calls `git stash` to save uncommitted changes. If the rebase fails (due to conflicts) and you run `git stash pop`, git may create an "autostash" that overwrites the original stash. Your uncommitted work can be lost permanently.

---

## Adding a New Crate (MANDATORY Procedure)

**CRITICAL: Every new Rust crate MUST follow this exact procedure. No exceptions.**

### Step-by-Step

```bash
# 1. Create the new repo on GitHub first (under A3S-Lab org)
#    Repository name convention: PascalCase (e.g., MyNewCrate)
#    URL: git@github.com:A3S-Lab/<RepoName>.git

# 2. Initialize the crate locally (in a temp directory, NOT in a3s root)
cd /tmp
cargo new my-new-crate --lib   # or --bin
cd my-new-crate
git init && git add -A && git commit -m "feat: initial <crate-name>"
git remote add origin git@github.com:A3S-Lab/<RepoName>.git
git push -u origin main

# 3. Add as submodule in A3S (from the a3s root)
cd /path/to/a3s
git submodule add git@github.com:A3S-Lab/<RepoName>.git crates/<crate-name>

# 4. Verify .gitmodules was updated
cat .gitmodules | grep -A2 <crate-name>

# 5. Commit in A3S
git add .gitmodules crates/<crate-name>
git commit -m "chore: add <crate-name> submodule"
git push origin main

# 6. Update README.md — add to Modules table, Roadmap, Repository Structure, Test Coverage
```

### Naming Conventions

| Item | Convention | Example |
|------|-----------|---------|
| GitHub repo | PascalCase | `A3S-Lab/MyNewCrate` |
| Submodule path | kebab-case | `crates/my-new-crate` |
| Rust crate name | kebab-case with `a3s-` prefix | `a3s-my-new-crate` |
| Rust lib name | snake_case with `a3s_` prefix | `a3s_my_new_crate` |

### Checklist for New Crate

```markdown
New crate checklist (ALL items required):
- [ ] GitHub repo created under A3S-Lab org
- [ ] Crate initialized with Cargo.toml, src/, README.md
- [ ] Added as git submodule at crates/<name> (NOT in root)
- [ ] .gitmodules updated and committed in A3S root
- [ ] NO Cargo.toml, src/, tests/, or other crate files in A3S root
- [ ] A3S README.md updated (Modules table, Roadmap, Repository Structure, Test Coverage)
- [ ] git remote of A3S root still points to git@github.com:A3S-Lab/a3s.git
```

### What NOT To Do

```bash
# ❌ WRONG: Creating a crate directly in the A3S root
cd /path/to/a3s
cargo init --name a3s-event    # This pollutes the root with Cargo.toml + src/

# ❌ WRONG: Adding source files to root
cp -r /tmp/my-crate/src /path/to/a3s/src    # Root must NOT have src/

# ❌ WRONG: Changing root remote to the new crate's repo
git remote set-url origin git@github.com:A3S-Lab/Event.git   # DESTROYS the monorepo

# ✅ CORRECT: Always work inside the submodule
cd /path/to/a3s/crates/event
# ... develop here, commit here, push here
```

---

## Code Style

**Rust:** Follow [Microsoft Rust Guidelines](https://microsoft.github.io/rust-guidelines). `cargo fmt` for formatting, `cargo clippy` for linting (enforced in CI).

**MANDATORY: All Rust code MUST be formatted with `cargo fmt` before committing.** Run `cargo fmt --all` in the crate root before every commit. Unformatted code will not be accepted. This is a blocking pre-commit requirement — no exceptions.

Key guidelines:
- **M-PANIC-IS-STOP**: Panics terminate, don't use for error handling
- **M-CONCISE-NAMES**: Avoid "Service", "Manager", "Factory" in type names
- **M-UNSAFE**: Minimize and document all unsafe blocks

**Code Conventions:**

- **Async-first**: All I/O uses Tokio. No blocking operations in async context.
- **Error handling**: Centralized `BoxError` enum (thiserror) in `core/src/error.rs`. Use `Result<T>` type alias. Always include full context in error messages with `map_err`.
- **Event keys**: Dot-separated lowercase: `<domain>.<subject>.<action>` (e.g., `session.context.warning`, `prompt.tool.called`).
- **State machine**: `BoxState` enum with `RwLock` synchronization: `Created → Ready → Busy → Compacting → Stopped`.
- **Public types** must be `Send + Sync`.
- **No panics** in production code.
- **Naming**: crates are kebab-case, modules are snake_case, types are PascalCase.

**Python SDK:** Async/await for all I/O. Context managers (`async with`) for automatic cleanup. Type hints encouraged.

**Configuration Format:** All crates that require configuration files MUST prefer HCL (HashiCorp Configuration Language) as the primary format. TOML may be supported as a secondary format. Use `.hcl` file extension by default. When adding configuration support to a new or existing crate, implement HCL parsing first (via `hcl-rs`) and auto-detect format by file extension.

---

## Language Policy

**MANDATORY: All code and documentation MUST be written in English.**

### Code Comments (English)

All code comments MUST be written in English, including:

- Module-level documentation (`//!` in Rust)
- Function/struct/field documentation (`///` in Rust)
- Inline comments (`//` in Rust)
- Python docstrings and comments

```rust
// ✅ Correct: English comments
/// Create a new session manager
pub fn new() -> Self { ... }

// ❌ Wrong: Non-English comments
/// 创建新的会话管理器
pub fn new() -> Self { ... }
```

### Documentation Files (English)

All documentation files MUST be written in English:

- README.md
- docs/*.md
- CLAUDE.md (this file)

### Rationale

- **Consistency**: Single language across codebase
- **Accessibility**: English is the standard for open-source projects
- **Tooling**: Better IDE support, documentation generation, and AI assistance

---

## Documentation Maintenance

**MANDATORY: Update documentation EVERY TIME a feature is completed. No exceptions.**

A feature is NOT considered complete until its documentation is updated. This is a blocking requirement — code changes without corresponding documentation updates will be treated as incomplete work.

### Rule: Documentation Update Is Part of Feature Completion

**Every** completed feature (not just "major" ones) MUST trigger the following documentation updates before the feature can be considered done:

1. **Update README.md Features section**:
   - Add new capabilities to the Features list
   - Update descriptions of changed capabilities
   - A feature that isn't listed in Features doesn't exist to users

2. **Update README.md Roadmap**:
   - Mark completed items with ✅ and `[x]`
   - Add a brief description of what was implemented (e.g., `(JSON file default, pluggable SessionStore trait)`)
   - Update phase status emoji (🚧 → ✅) when all items in a phase are done

3. **Update Related Usage Documentation**:
   - Update `docs/*.md` files that reference the changed functionality
   - Update code examples and usage guides to reflect new behavior
   - Update API Reference if public APIs were added or changed
   - Update configuration documentation if new options were introduced

4. **Remove Obsolete Content**:
   - Delete outdated documentation that no longer reflects reality
   - Remove TODO comments for completed work
   - Update or remove examples that use deprecated APIs
   - Clean up roadmap items that are no longer planned

5. **Keep Consistent**:
   - Ensure code comments match actual behavior
   - Ensure README examples are runnable
   - Ensure version numbers and statistics are accurate
   - Update test counts (run `just test` to get count)

### Mandatory Checklist (MUST complete for EVERY feature)

```markdown
Feature completion checklist (ALL items required):
- [ ] README.md Features section updated
- [ ] README.md Roadmap updated (mark ✅, update descriptions)
- [ ] README.md test count updated (run `just test` to get count)
- [ ] Related docs/*.md files updated
- [ ] README.md API Reference updated (if public API changed)
- [ ] Usage examples updated to reflect new behavior
- [ ] Obsolete documentation removed
- [ ] Code examples verified to work
```

### Workflow

```
1. Implement feature  →  2. Tests pass  →  3. Update documentation  →  4. Feature complete ✓
                                              ↑ YOU ARE NOT DONE WITHOUT THIS STEP
```

---

## Mandatory Code Design Rules

**CRITICAL: These rules are MANDATORY for all code contributions.**

### Meta-Principle

**0. DON'T BE YES MAN** — Challenge assumptions, question designs, identify flaws

- **Challenge yourself too, not just the user**
- Before implementing: "Does this already exist?" (search first)
- Before adding logic: "Does this layer need to know this?"
- After writing: "What breaks if I delete this?"

### First-Principles Feature Gate (MANDATORY)

**Every time the user requests a new feature, you MUST perform a first-principles review BEFORE writing any code.** This is non-negotiable.

Ask yourself these questions in order:

1. **What is the project's core mission?** (SafeClaw: privacy-preserving AI assistant runtime with TEE support)
2. **Does this feature directly serve that mission?** If the answer is "not really" or "only tangentially", push back.
3. **Does this feature strengthen or weaken the existing architecture?** A feature that adds complexity without closing a known gap in the threat model is a net negative.
4. **Is the problem real or hypothetical?** Reject features that solve problems no actual user has encountered.
5. **Is there a simpler alternative?** A config flag, a trait bound, or a 10-line change often beats a new module.

**If the feature fails this review, you MUST refuse to implement it and clearly explain why.** Be direct, be specific, cite the architectural conflict. Do not soften the refusal with "but we could do it later" — if it doesn't belong, say so.

Example refusals:
- "This adds a new network dependency to a privacy-focused system without a clear threat model justification. I won't implement it."
- "This duplicates what the L2 Artifact layer already handles. Adding a parallel path will create inconsistency. Let's extend the existing layer instead."
- "This is a nice-to-have that increases the attack surface. The complexity cost outweighs the benefit for SafeClaw's core mission."

**The goal is not to block progress — it's to protect architectural coherence.** Every feature that doesn't belong is technical debt that makes the system harder to reason about and harder to secure.

### Core Principles (Must Know)

**1. Minimal Core + External Extensions (First Principles Architecture)** — MANDATORY for all new modules and features
   - **Core**: Minimal, stable, non-replaceable components (5-10 max)
   - **Extensions**: Everything else is replaceable via traits with default implementations
   - **Rule**: Before adding ANY code, ask "Is this core or extension?"
   - **Example**: A3S Code has 5 core components (Agent, AgentSession, AgentLoop, ToolExecutor, LlmClient) + 14 extension points (Security, Permissions, Context, Memory, Skills, Hooks, etc.)
   - **Test**: Can this be replaced without changing core? If yes, it's an extension. If no, justify why it must be core.
   - **Default**: All extensions MUST have working default implementations — system works out of the box

**2. Single Responsibility** — One function, one job
**3. Boring Code** — Obvious > clever
**4. Search Before Implement** — grep before writing, read before coding
**5. Only What's Used** — No future-proofing, delete dead code immediately
**6. DRY** — Don't Repeat Yourself (single source of truth)
**7. Explicit Errors** — Self-documenting error messages
**8. Periodic Pruning** — After every feature, audit for organic cruft

### Periodic Pruning (MANDATORY — Rule 8)

**Code grows organically. Wrappers accumulate. Abstractions overlap. Nobody goes back to clean up. This rule forces it.**

After completing any feature that touches an existing module, you MUST perform a pruning audit:

1. **Wrapper audit**: Does any struct/module exist solely to wrap another type with zero added logic? If the wrapper adds nothing (no extra fields, no extra methods, no trait impl), inline it and delete the wrapper.
2. **Dead export audit**: `grep` for every `pub` type/function in the changed files. If something is exported but has zero external callers, make it `pub(crate)` or delete it.
3. **Redundant module audit**: If a module re-exports everything from another module and adds < 10 lines of logic, merge it into the caller or the source.
4. **Stale integration layer audit**: When crate A wraps crate B's types just to "adapt" them, check if the adaptation is still needed. If crate B's API evolved to match, delete the glue layer.
5. **Orphaned test audit**: If you deleted or renamed code, search for tests that reference the old names. Delete or update them.

**The rule is simple: every feature commit should leave the module cleaner than you found it.** If you added a new abstraction, check whether an old one became redundant. If you changed an API, check whether a wrapper layer lost its reason to exist.

```bash
# Quick pruning commands
grep -rn "pub struct.*Wrapper\|pub struct.*Enhanced\|pub struct.*Adapter" src/
grep -rn "pub fn\|pub struct\|pub enum\|pub trait" src/changed_module/ | wc -l
# For each pub item: grep -rn "TypeName" src/ — if only 1 hit (the definition), it's dead
```

**Signs of organic cruft to watch for:**
- `EnhancedFoo` wrapping `Foo` with only passthrough methods
- `foo_integration.rs` that just re-exports `foo` crate types with trivial conversions
- Two modules with 80%+ overlapping functionality
- `pub` items with zero external callers
- Config structs with fields that are always set to the same value

### Supporting Principles (Reference When Needed)

**9. Minimal Knowledge** — Components only know interfaces, not internals
**10. No Premature Optimization** — Measure first, optimize later
**11. Explicit Paths** — Calculate from known roots, never assume
**12. Prepare Before Execute** — Setup before irreversible operations
**13. Validate Early** — Check preconditions before expensive work
**14. Thoughtful Naming** — Consider 5+ alternatives, choose the clearest
**15. Structured Code** — Organized hierarchy, clear layers, predictable organization
**16. Idiomatic by Default** — Follow each language's standard patterns and libraries first; only diverge with a clear, documented reason
**17. Typed Extension Options** — Extension/backend choices in API options MUST be typed objects, never raw primitives (strings or booleans). See rule below.

### Typed Extension Options (MANDATORY — Rule 17)

**Extension points exposed in public APIs — especially SDK options structs — MUST use typed objects, not raw string paths or boolean flags.**

#### The Rule

When an option selects or configures an extension (a swappable backend, provider, or strategy), it MUST be expressed as a typed object or class instance, not a raw primitive:

```typescript
// ❌ WRONG: raw string leaks the backend name, can't be swapped
agent.session('.', { memoryDir: './memory' });
agent.session('.', { defaultSecurity: true });
agent.resumeSession('id', './sessions', options);

// ✅ CORRECT: typed objects are explicit, swappable, self-documenting
agent.session('.', { memoryStore: new FileMemoryStore('./memory') });
agent.session('.', { securityProvider: new DefaultSecurityProvider() });
agent.resumeSession('id', { sessionStore: new FileSessionStore('./sessions') });
```

#### Why This Matters

- **Clarity**: `new FileMemoryStore('./memory')` communicates both WHAT backend and HOW it's configured. `memoryDir: './memory'` communicates only the path — the backend is implicit.
- **Extensibility**: Typed objects let callers substitute any implementation (`new RedisMemoryStore(...)`, `new S3SessionStore(...)`) without changing the API surface.
- **No hidden coupling**: A boolean `defaultSecurity: true` silently instantiates `DefaultSecurityProvider` inside the library. The typed form `securityProvider: new DefaultSecurityProvider()` makes that explicit and lets the caller swap it.

#### When to Apply

This rule applies whenever an option field:
- Selects a backend (memory store, session store, security provider, context provider, ...)
- Enables a feature that corresponds to a named extension/implementation
- Accepts a path string that implicitly determines which provider to use

#### Cross-Language SDK Implementation Patterns

The two binding layers require different techniques because napi-rs and PyO3 have different type systems.

**napi-rs (Node.js) — Structural Compatibility**

`#[napi(object)]` structs (plain JS objects) cannot hold `#[napi]` class instances. The solution: define a plain config struct as the field type, then expose a named class with the same shape. TypeScript's structural typing accepts the class instance wherever the plain struct is expected.

```rust
// Plain struct — used as the field type in SessionOptions
#[napi(object)]
pub struct JsMemoryStore { pub backend: String, pub dir: Option<String> }

// Named class — same shape, self-documenting constructor
#[napi]
pub struct FileMemoryStore { pub backend: String, pub dir: String }
#[napi]
impl FileMemoryStore {
    #[napi(constructor)]
    pub fn new(dir: String) -> Self { Self { backend: "file".into(), dir } }
}

// In SessionOptions (#[napi(object)])
pub memory_store: Option<JsMemoryStore>,
// TypeScript accepts `new FileMemoryStore('./memory')` here via structural subtyping
```

**PyO3 (Python) — Runtime Dispatch**

PyO3 allows `Option<PyObject>` fields in `#[pyclass]` structs, which stores any Python object reference. Use `Python::with_gil` + `extract::<PyRef<T>>` to dispatch on the concrete class type at conversion time.

```rust
// Named classes — no plain struct needed; store directly as PyObject
#[pyclass(name = "FileMemoryStore")]
struct PyFileMemoryStore { #[pyo3(get, set)] dir: String }
#[pymethods]
impl PyFileMemoryStore {
    #[new] fn new(dir: String) -> Self { Self { dir } }
}

#[pyclass(name = "MemorySessionStore")]
struct PyMemorySessionStore {}

// In PySessionOptions
memory_store: Option<pyo3::PyObject>,    // accepts FileMemoryStore or any future backend
session_store: Option<pyo3::PyObject>,   // accepts FileSessionStore or MemorySessionStore

// In build_rust_session_options — runtime dispatch
if let Some(ref store) = so.memory_store {
    let dir = Python::with_gil(|py| {
        store.extract::<PyRef<PyFileMemoryStore>>(py).ok().map(|s| s.dir.clone())
    });
    if let Some(dir) = dir { o = o.with_file_memory(dir); }
}
if let Some(ref store) = so.session_store {
    Python::with_gil(|py| {
        if let Ok(s) = store.extract::<PyRef<PyFileSessionStore>>(py) {
            o = o.with_file_session_store(s.dir.clone());
        } else if store.extract::<PyRef<PyMemorySessionStore>>(py).is_ok() {
            o = o.with_session_store(Arc::new(MemorySessionStore::new()));
        }
    });
}
```

**Summary of trade-offs:**

| Binding | Field type | Dispatch | Extensible to 3rd-party types |
|---------|-----------|----------|-------------------------------|
| napi-rs | Plain struct (`JsMemoryStore`) | Compile-time via struct fields | No — must match struct shape |
| PyO3 | `Option<PyObject>` | Runtime via `extract::<PyRef<T>>` | Yes — any Python class accepted |

#### Anti-Patterns to Reject

| Bad | Good |
|-----|------|
| `memoryDir: './memory'` | `memoryStore: new FileMemoryStore('./memory')` |
| `defaultSecurity: true` | `securityProvider: new DefaultSecurityProvider()` |
| `useMemorySessionStore: true` | `sessionStore: new MemorySessionStore()` |
| `resumeSession(id, dir, opts)` | `resumeSession(id, { sessionStore: new FileSessionStore(dir) })` |
| `fileContextRoot: './src'` | `contextProvider: new FileSystemContextProvider('./src')` |
| `mode: string` where only 3 values are valid | `mode: 'internal' \| 'external' \| 'hybrid'` (literal union) |

#### What Is NOT a Violation

Not every boolean or string option violates this rule. The rule applies specifically to **swappable extensions/backends**. These are fine:

- **Feature flags** (`builtinSkills: boolean`, `permissive: boolean`, `planning: boolean`) — toggle a single fixed behavior, not a backend choice.
- **Feature toggles with sub-config** (`enableDlq: boolean` + `dlqMaxSize: number`) — these enable/disable a component within one implementation, not select between implementations. Acceptable, though combining into a config object (`dlq?: { maxSize?: number }`) is a better V2 design.
- **Numeric/string scalars** (`model: string`, `toolTimeoutMs: number`, `maxToolRounds: number`) — configuration parameters for a fixed layer, not backend selectors.

The test: **"Could a user swap this for a different implementation?"** If yes → typed object. If no → primitive is fine.

#### Rust Builder Convenience Methods — Separate Standard

Rust core `SessionOptions` builder methods (e.g. `with_fs_context(path)`, `with_file_memory(path)`) are **not held to the same standard** as SDK options fields. Convenience builder methods that wrap a typed call are idiomatic Rust — the full typed path (`with_context_provider(Arc::new(...))`) must always exist alongside them. The rule is:

- ✅ Rust builder: convenience `with_fs_context(path)` is fine **as long as** `with_context_provider(Arc<dyn ContextProvider>)` also exists.
- ❌ SDK option field: `fsContextDir: './src'` is never acceptable — SDK users must pass `contextProviders: [new FileSystemContextProvider('./src')]`.

If a convenience builder method is added to Rust core, it MUST NOT be mechanically forwarded as a primitive field in the SDK options. The SDK always exposes the typed form only.

#### Internal Shims Are Not Public API

In napi-rs bindings, the `#[napi(object)]` plain structs (`JsMemoryStore`, `JsSessionStore`, etc.) that back the options fields are **implementation details**, not public API. They MUST NOT appear in the exported TypeScript declarations (`index.d.ts`). Only the named classes (`FileMemoryStore`, `FileSessionStore`, etc.) are public.

### First Principles Architecture (MANDATORY — Rule 1)

**Every new module or feature MUST follow the Minimal Core + External Extensions pattern.**

#### The Rule

When designing any new module, system, or feature:

1. **Identify the minimal core** (5-10 components max)
   - What MUST be stable and non-replaceable?
   - What is the absolute minimum needed for the system to function?
   - Core components should be simple, well-tested, and rarely change

2. **Everything else is an extension**
   - Can this be replaced without changing core? → Extension
   - Does this add optional functionality? → Extension
   - Is this a policy/strategy that might vary? → Extension

3. **All extensions MUST be trait-based**
   - Define clear trait interfaces
   - Provide working default implementations
   - System works out of the box, customization is optional

4. **Document the architecture**
   - Clearly list core components (with justification)
   - Clearly list extension points (with trait definitions)
   - Explain why each core component cannot be an extension

#### Example: A3S Code Architecture

**Core Components (5)** — Stable, non-replaceable:
- `Agent` — Configuration and session management
- `AgentSession` — Workspace-bound execution context
- `AgentLoop` — Core execution engine (turn-based LLM interaction)
- `ToolExecutor` — Tool execution coordinator
- `LlmClient` — LLM provider abstraction

**Extension Points (14)** — Replaceable via traits:
- `SecurityProvider` — Input taint, output sanitization
- `PermissionChecker` — Tool access control
- `ConfirmationProvider` — Human-in-the-loop confirmation
- `ContextProvider` — RAG retrieval
- `SessionStore` — Session persistence
- `MemoryStore` — Long-term memory
- `Tool` — Custom tools
- `Planner` — Task decomposition
- `HookHandler` — Event handling
- `HookExecutor` — Event execution
- `McpTransport` — MCP protocol
- `HttpClient` — HTTP requests
- `SessionCommand` — Queue tasks
- `SkillRegistry` — Skill management

**Why this works:**
- Core is stable (rarely changes)
- Extensions can evolve independently
- Users can replace any extension without touching core
- System works with defaults, customization is opt-in

#### Anti-Patterns to Avoid

❌ **Everything is core** — No extensibility, hard to customize
❌ **Everything is an extension** — No stable foundation, too complex
❌ **Extensions without defaults** — System doesn't work out of the box
❌ **Core components that should be extensions** — Unnecessary coupling

#### Design Checklist

Before implementing any new module or feature:

- [ ] Identified minimal core (5-10 components max)
- [ ] Justified why each core component cannot be an extension
- [ ] Defined trait interfaces for all extensions
- [ ] Provided working default implementations for all extensions
- [ ] Verified system works out of the box with defaults
- [ ] Documented core vs extensions in README/docs
- [ ] Can explain the architecture in 2 minutes

### Quick Examples

**Single Responsibility**

```rust
// ❌ One function doing everything
fn setup_and_start_vm(image: &str) -> Result<VM> { /* ... */ }

// ✅ Each function has one job
fn pull_image(image: &str) -> Result<Manifest> { /* ... */ }
fn create_workspace(manifest: &Manifest) -> Result<Workspace> { /* ... */ }
fn start_vm(workspace: &Workspace) -> Result<VM> { /* ... */ }
```

**Boring Code**

```rust
// ❌ Clever, hard to understand
fn metrics(&self) -> RawMetrics {
    self.process.as_ref()
        .and_then(|p| System::new().process(Pid::from(p.id())))
        .map(|proc| RawMetrics { cpu: proc.cpu_usage(), mem: proc.memory() })
        .unwrap_or_default()
}

// ✅ Boring, obvious
fn metrics(&self) -> RawMetrics {
    if let Some(ref process) = self.process {
        let mut sys = System::new();
        sys.refresh_process(pid);
        if let Some(proc_info) = sys.process(pid) {
            return RawMetrics {
                cpu_percent: Some(proc_info.cpu_usage()),
                memory_bytes: Some(proc_info.memory()),
            };
        }
    }
    RawMetrics::default()
}
```

**Search Before Implement**

```bash
# ❌ Writing transformation without searching
# ✅ Search first, find existing code
$ grep -r "transform.*guest" src/
src/runtime/engines/krun/engine.rs:113:fn transform_guest_args(...)
# → Found it! Use existing code, don't duplicate.
```

**DRY (Don't Repeat Yourself)**

```rust
// ❌ Duplicated constants
const VSOCK_PORT: u32 = 4088;  // host
const VSOCK_PORT: u32 = 4088;  // guest

// ✅ Shared in core
use a3s_box_core::VSOCK_GUEST_PORT;
```

**Explicit Error Context**

```rust
// ❌ Generic error
std::fs::create_dir_all(&dir)?;

// ✅ Self-documenting
std::fs::create_dir_all(&socket_dir).map_err(|e| {
    BoxError::Other(format!(
        "Failed to create socket directory {}: {}", socket_dir.display(), e
    ))
})?;
```

### Pre-Submission Checklist

**Pre-Implementation (BEFORE writing code):**

- [ ] Searched for similar functionality (`grep -r "pattern" src/`)
- [ ] Read ALL files that would be affected (completely, not skimmed)
- [ ] Identified correct layer for new logic (ownership analysis)
- [ ] Verified no duplicate logic exists
- [ ] Questioned: "Does this component need to know this?"
- [ ] Applied Rule #0 to OWN design (not just user's request)
- [ ] **First Principles Architecture (Rule 1):**
  - [ ] If new module: Identified minimal core (5-10 components max)
  - [ ] If new module: Justified why each core component cannot be an extension
  - [ ] If new module: Defined trait interfaces for all extensions
  - [ ] If new module: Provided working default implementations
  - [ ] If new feature: Determined if it's core or extension
  - [ ] If new feature: If extension, implemented as trait with default

**Core Principles:**

- [ ] Each function has single responsibility (one job)
- [ ] Code is boring and obvious (not clever)
- [ ] Only code that's actually used exists (no future-proofing, no dead code)
- [ ] No duplicated knowledge (DRY - single source of truth)
- [ ] Every error has full context (self-documenting)
- [ ] Pruning audit done: no dead wrappers, no orphaned exports, no redundant modules
- [ ] **Typed Extension Options (Rule 17):** every extension/backend option uses a typed object, not a raw string or boolean

**Supporting Principles:**

- [ ] Components only know interfaces (minimal knowledge / loose coupling)
- [ ] No optimization without measurement
- [ ] Paths calculated from known roots (never assume)
- [ ] Setup completed before irreversible operations
- [ ] Preconditions validated early
- [ ] Names considered carefully (5+ alternatives evaluated)
- [ ] Code has clear hierarchy and predictable organization

---

## Test-Driven Development (TDD)

**MANDATORY: All feature development MUST follow Test-Driven Development.**

### The TDD Workflow

```
1. Write tests FIRST  →  2. Run tests (should fail)  →  3. Implement feature  →  4. Run tests (should pass)  →  5. Feature complete
```

### Rules

**Rule 1: Tests Before Code** — Write unit tests that define expected behavior before any implementation.

**Rule 2: Tests Define Completion** — A feature is complete ONLY when all related tests pass.

**Rule 3: Code Changes Require Test Updates** — Modified code must have updated tests.

**Rule 4: Deleted Features = Deleted Tests** — No orphaned or `#[ignore]` tests for removed code.

**Rule 5: Integration Tests Are Mandatory for User-Facing Workflows** — Unit tests alone are NOT sufficient for network services, CLI commands, or cross-module workflows.

**Rule 6: Feature-Gated Code Must Be Compiled and Tested** — `cargo build --all-features` must succeed.

**Rule 7: Clean Up Test Artifacts** — Tests MUST NOT leave behind temporary files, directories, sockets, databases, or any other artifacts on the filesystem. Use `tempdir` / `tempfile` for scratch space, and ensure cleanup runs even on test failure (use RAII / `Drop` / `finally` / scope guards). After running tests, verify no leftover artifacts exist. If a test creates files in a known location, it must delete them in a cleanup step or use a directory that is automatically removed.

### Test File Organization

```
src/
  └── module/
      ├── mod.rs           // Implementation
      └── (tests at bottom of mod.rs, or in tests/ directory)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() { ... }
}
```

### Running Tests

```bash
# Run all Rust tests
just test

# Run tests for specific crate
just test-code      # a3s-code
just test-core      # a3s-box-core
just test-runtime   # a3s-box-runtime

# Run specific test by name
cd src && cargo test -p <crate> --lib -- test_name
```

### Pre-Submission Test Checklist

- [ ] Wrote tests BEFORE implementation
- [ ] All new functions have corresponding unit tests
- [ ] All modified functions have updated tests
- [ ] Deleted code has no remaining tests
- [ ] `just test` passes with all green
- [ ] No `#[ignore]` tests added for "later"
- [ ] `cargo build --all-features` succeeds
- [ ] No test artifacts left on filesystem (temp files, dirs, sockets cleaned up)
- [ ] Integration tests exist for new service endpoints
- [ ] Integration tests cover error paths (not found, invalid input)
