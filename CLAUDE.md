# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository Structure

```
a3s/                            ← MONOREPO ROOT (NOT a Rust workspace)
├── apps/                       # Applications (desktop, web, CLI)
│   ├── os/                     # A3S platform (NestJS + React)
│   └── safeclaw/               # SafeClaw desktop app (Tauri + React)
├── crates/                     # Rust crates (submodules)
│   ├── box/ code/ event/ flow/ gateway/ lane/ power/ search/ updater/
│   └── common/                 # Shared types
├── docs/                       # Documentation
└── homebrew-tap/              # Homebrew tap
```

### Root Rules (NEVER do in root)

- No `Cargo.toml`, `src/`, `justfile` in root — root is NOT a Rust workspace
- No `cargo init` or `cargo new` in root
- Git remote MUST point to `git@github.com:A3S-Lab/a3s.git`

---

## Git Safety

**`git stash pop` CAN destroy your changes** after failed merge/rebase (autostash issue).

1. Before `git pull --rebase`: commit or stash first
2. After `git stash pop`: verify changes with `git diff --stat HEAD`
3. If stash pop fails: use `git stash branch recovery` instead
4. Always use `git stash push -m "message"` with descriptive messages

---

## Adding a New Crate

1. Create repo on GitHub (PascalCase, under A3S-Lab org)
2. Init in `/tmp`, push, then `git submodule add` into `crates/<name>`
3. Commit `.gitmodules` change in a3s root
4. Update README.md (Modules table, Roadmap, Repository Structure)
5. Never `cargo init` directly in a3s root

| Item | Convention |
|------|-----------|
| GitHub repo | PascalCase (`MyNewCrate`) |
| Submodule path | kebab-case (`crates/my-new-crate`) |
| Rust crate name | `a3s-my-new-crate` |

---

## Code Style

**Rust:** Follow [Microsoft Rust Guidelines](https://microsoft.github.io/rust-guidelines). `cargo fmt` before every commit, `cargo clippy` for linting (enforced in CI).

Key rules:
- Async-first: all I/O uses Tokio, no blocking in async context
- Error handling: `BoxError` in `core/src/error.rs`, always include context in errors
- Event keys: dot-separated lowercase (`<domain>.<subject>.<action>`)
- Public types must be `Send + Sync`, no panics in production

**Python SDK:** Async/await, `async with` for cleanup, type hints.

**Config:** Prefer HCL over TOML. Use `.hcl` extension by default.

---

## Language Policy

All code and documentation MUST be in English. This includes:
- Rust doc comments (`//!`, `///`, `//`)
- Python docstrings
- README.md, docs/*.md, CLAUDE.md

---

## Documentation

**Update docs EVERY time a feature is completed.** Code is not done until docs reflect it.

For every feature:
1. Update README.md Features section
2. Update README.md Roadmap (mark ✅, add implementation notes)
3. Update related `docs/*.md` files
4. Remove obsolete content (outdated docs, deprecated examples, completed TODOs)
5. Verify code examples still work

---

## Mandatory Code Design Rules

### Rule 0: DON'T BE YES MAN

Challenge assumptions. Before implementing: "Does this already exist?", "Does this layer need to know this?". After writing: "What breaks if I delete this?".

### Rule 1: First-Principles Feature Gate

**Every new feature request requires first-principles review BEFORE code:**

1. What is the project's core mission?
2. Does this feature directly serve that mission? If not, refuse.
3. Does it strengthen or weaken the architecture?
4. Is the problem real or hypothetical?
5. Is there a simpler alternative?

If it fails this review, refuse clearly with architectural justification. Do not soften with "we could do it later".

### Rule 2: Minimal Core + External Extensions

Every module system:
- **Core** (5-10 components): Minimal, stable, non-replaceable
- **Extensions**: Everything else, trait-based with default implementations

Before adding code, ask: "Is this core or extension?" If it can be replaced without breaking core, it's an extension.

### Core Principles

3. Single Responsibility — one function, one job
4. Boring Code — obvious > clever
5. Search Before Implement — grep before writing
6. Only What's Used — no future-proofing
7. DRY — single source of truth
8. Explicit Errors — self-documenting error messages
9. Periodic Pruning — after every feature, audit for dead wrappers/orphaned exports

### Rule 3: Reasonable File and Folder Splitting

**Keep files focused, directories logically organized. Fight the "god object" and "dump everything in one folder" tendencies.**

File splitting guidelines:
- **File size**: If a file exceeds ~500 lines, consider splitting it. If it exceeds ~1000 lines, it MUST be split.
- **Single responsibility per file**: One struct/trait + its closely related helpers. Don't mix unrelated concerns.
- **Protocol handlers**: Each protocol (HTTP, WebSocket, gRPC, TCP, UDP) in its own file, not one massive `entrypoint.rs`.
- **Middleware**: Each middleware type in its own file, not one `middleware/mod.rs` with 15 match arms.
- **Managers**: Extract lifecycle managers (discovery, autoscaler, ACME, providers) into separate files, not all in `gateway.rs`.

Directory structure guidelines:
- **Group by concern, not by type**: `proxy/http.rs`, `proxy/websocket.rs` not `http_proxy.rs`, `ws_proxy.rs` scattered
- **No "catch-all" directories**: `utils/`, `helpers/` with unrelated code are smells
- **Nested depth**: Maximum 3 levels deep for source code (`src/a/b/c.rs`), 2 for tests

Warning signs (refactor immediately):
- `mod.rs` files that re-export 20+ items
- `src/gateway.rs` with 1000+ lines
- `src/entrypoint.rs` handling HTTP, TCP, UDP, TLS, ACME all in one file
- Any file you need to scroll past 3 screens to understand

When in doubt: err on the side of more files with clearer names over fewer files with clever organization.

### Typed Extension Options (SDK/API only)

Extension/backend choices in SDK options MUST use typed objects, not raw primitives:

```typescript
// ❌ BAD: leaks backend name, can't be swapped
agent.session('.', { memoryDir: './memory', defaultSecurity: true });

// ✅ GOOD: typed, explicit, swappable
agent.session('.', { memoryStore: new FileMemoryStore('./memory') });
agent.session('.', { securityProvider: new DefaultSecurityProvider() });
```

This applies to: memory stores, session stores, security providers, context providers.

This does NOT apply to: feature flags (`builtinSkills: boolean`), numeric/string scalars (`model: string`, `toolTimeoutMs: number`).

### Pre-Submission Checklist

- [ ] Searched for existing functionality before implementing
- [ ] Read all affected files before modifying
- [ ] Applied Rule 0-2 to own design (not just user's request)
- [ ] Single responsibility, boring code, no future-proofing
- [ ] Pruning audit: no dead wrappers, orphaned exports, redundant modules
- [ ] Typed extension options use objects (SDK only)
- [ ] `cargo fmt --all` and `cargo clippy` pass

---

## Test-Driven Development (TDD)

1. Write tests FIRST → 2. Run (should fail) → 3. Implement → 4. Run (should pass) → 5. Feature complete

Rules:
- Tests define completion — feature is done only when tests pass
- Modified code must have updated tests
- Deleted features = deleted tests (no orphaned `#[ignore]` tests)
- Integration tests required for CLI/network/cross-module workflows
- Tests must NOT leave temp files/sockets behind
- `cargo build --all-features` must succeed

Run tests: `just test` (all) or `cargo test -p <crate>` (specific)

---

## Problem Solving Memory

After solving a problem, store the lesson:

**Location:** `docs/lessons-learned/<category>-<short-description>.md`

**When to record:**
- Bug fix (root cause + solution)
- CSS/styling issue (what tried, failed, worked)
- Configuration problem
- Tool version mismatch
- Architecture decision

```markdown
---
title: <short description>
date: <YYYY-MM-DD>
category: <css|rust|typescript|git|docker|...>
tags: [<relevant-tags>]
---

## Problem
## Root Cause
## Attempted Solutions
## Final Solution
## Prevention
```

**Before starting work on known problem categories:** `grep -r "pattern" docs/lessons-learned/`
