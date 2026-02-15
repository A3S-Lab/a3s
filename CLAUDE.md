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
├── README.md                   # Project overview, module list, roadmap
├── apps/                       # Frontend apps and non-Rust projects
│   ├── a3s-deep/               # [submodule] git@github.com:A3S-Lab/Deep.git
│   ├── os/                     # A3S platform (NestJS backend + React frontend + CLI)
│   └── safeclaw-ui/            # [submodule] git@github.com:A3S-Lab/SafeClawUI.git
├── crates/                     # ALL Rust crates live here (submodules or local)
│   ├── box/                    # [submodule] git@github.com:A3S-Lab/Box.git
│   ├── code/                   # [submodule] git@github.com:A3S-Lab/Code.git
│   ├── cron/                   # [submodule] git@github.com:A3S-Lab/Cron.git
│   ├── event/                  # [submodule] git@github.com:A3S-Lab/Event.git
│   ├── gateway/                # [submodule] git@github.com:A3S-Lab/Gateway.git
│   ├── lane/                   # [submodule] git@github.com:A3S-Lab/Lane.git
│   ├── power/                  # [submodule] git@github.com:A3S-Lab/Power.git
│   ├── privacy/                # Shared PII classification types
│   ├── safeclaw/               # [submodule] git@github.com:A3S-Lab/SafeClaw.git
│   ├── search/                 # [submodule] git@github.com:A3S-Lab/Search.git
│   ├── tools-core/             # Core types for tools
│   ├── transport/              # Shared vsock transport protocol
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

**1. Single Responsibility** — One function, one job
**2. Boring Code** — Obvious > clever
**3. Search Before Implement** — grep before writing, read before coding
**4. Only What's Used** — No future-proofing, delete dead code immediately
**5. DRY** — Don't Repeat Yourself (single source of truth)
**6. Explicit Errors** — Self-documenting error messages

### Supporting Principles (Reference When Needed)

**7. Minimal Knowledge** — Components only know interfaces, not internals
**8. No Premature Optimization** — Measure first, optimize later
**9. Explicit Paths** — Calculate from known roots, never assume
**10. Prepare Before Execute** — Setup before irreversible operations
**11. Validate Early** — Check preconditions before expensive work
**12. Thoughtful Naming** — Consider 5+ alternatives, choose the clearest
**13. Structured Code** — Organized hierarchy, clear layers, predictable organization
**14. Idiomatic by Default** — Follow each language's standard patterns and libraries first; only diverge with a clear, documented reason

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

**Core Principles:**

- [ ] Each function has single responsibility (one job)
- [ ] Code is boring and obvious (not clever)
- [ ] Only code that's actually used exists (no future-proofing, no dead code)
- [ ] No duplicated knowledge (DRY - single source of truth)
- [ ] Every error has full context (self-documenting)

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
- [ ] Integration tests exist for new service endpoints
- [ ] Integration tests cover error paths (not found, invalid input)
