# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

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

**MANDATORY: Update documentation after completing major features.**

### After Completing a Feature Module

When a significant feature or module is completed, you MUST:

1. **Update README.md**:
   - Update the Features section if new capabilities were added
   - Update the Roadmap to mark completed items with ✅
   - Update test counts if they changed
   - Update API Reference if new public APIs were added
   - Update code examples if behavior changed

2. **Remove Obsolete Content**:
   - Delete outdated documentation that no longer reflects reality
   - Remove TODO comments for completed work
   - Update or remove examples that use deprecated APIs
   - Clean up roadmap items that are no longer planned

3. **Keep Consistent**:
   - Ensure code comments match actual behavior
   - Ensure README examples are runnable
   - Ensure version numbers and statistics are accurate

### Checklist

```markdown
After completing a major feature:
- [ ] README.md Features section updated
- [ ] README.md Roadmap updated (mark ✅, update descriptions)
- [ ] README.md test count updated (run `just test` to get count)
- [ ] README.md API Reference updated (if public API changed)
- [ ] Obsolete documentation removed
- [ ] Code examples verified to work
```

### Example

```markdown
// Before: Roadmap shows "Session persistence" as TODO
### Phase 2: Reliability 🚧
- [ ] Session persistence

// After: Feature completed, update roadmap
### Phase 2: Reliability ✅
- [x] Session persistence (JSON file default, pluggable SessionStore trait)
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

**DON'T BE YES MAN**

```
// ❌ Yes Man behavior
User: "Let's add a Supervisor component to track metrics"
AI: "Great idea! I'll implement it right away."
[Creates empty struct with no implementation, violates YAGNI]

// ✅ Critical thinking
User: "Let's add a Supervisor component to track metrics"
AI: "Wait - what data would Supervisor need? The process handle is in VmController.
     Creating a separate component means duplicating PID tracking (violates DRY).
     Supervisor has no access to the data it needs. This design won't work.

     Better approach: VmController already owns the process, it should provide metrics.
     Do you want me to implement metrics() in VmController instead?"
```

**When to push back:**

- Design violates any of the 15 golden rules
- User requests create duplicate state/logic
- Implementation requires blocking operations (sleep, sync I/O in async contexts)
- Code adds complexity without clear benefit
- "Future-proofing" that isn't needed now (YAGNI)

**How to push back:**

1. Identify specific rule violations
2. Explain why current approach won't work
3. Propose concrete alternative
4. Let user decide

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

BEFORE writing ANY code, search for existing implementations:

```bash
# ❌ Writing transformation without searching
# (adds duplicate unix→vsock transformation in runtime/vm.rs)

# ✅ Search first, find existing code
$ grep -r "transform.*guest" src/
src/runtime/engines/krun/engine.rs:113:fn transform_guest_args(...)
# → Found it! Use existing code, don't duplicate.
```

**Search patterns to try:**

- Similar functionality: `grep -r "transform.*args" src/`
- Function names: `grep -r "function_name" src/`
- Constants/config: `grep -r "VSOCK_PORT\|4088" src/`
- Layer ownership: `grep -r "GUEST_AGENT" src/` (shows which modules use it)

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

**Explicit Path Calculation**

```rust
// ❌ Assumes relationship
let box_dir = rootfs_dir.join(box_id);

// ✅ Calculate from known root
let home_dir = rootfs_dir.parent().ok_or(...)?;
let box_dir = home_dir.join(dirs::BOXES_DIR).join(box_id);
```

**Minimal Knowledge**

```rust
// ❌ Component knows about other's internals
mod krun_engine {
    use crate::networking::constants::GUEST_MAC;
    fn configure_network(&self, socket_path: &str) {
        self.ctx.add_net_path(socket_path, GUEST_MAC);
    }
}

// ✅ Component only knows interface
mod krun_engine {
    fn configure_network(&self, socket_path: &str, mac_address: [u8; 6]) {
        self.ctx.add_net_path(socket_path, mac_address);
    }
}
```

**Prepare Before Execute**

```rust
// ❌ Setup mixed with critical operation
fn start_vm() -> Result<()> {
    let ctx = create_ctx()?;
    ctx.start();  // Process takeover - can't recover from errors!
}

// ✅ All setup before point of no return
std::fs::create_dir_all(&socket_dir)?;  // Can fail safely
let ctx = create_ctx()?;                 // Can fail safely
ctx.configure()?;                        // Can fail safely
ctx.start();                             // Point of no return
```

**Structured Code**

```rust
// ❌ Flat, disorganized
mod rootfs {
    pub fn prepare() { ... }
    pub fn extract() { ... }
    pub fn mount() { ... }
    pub struct PreparedRootfs { ... }
    pub struct SimpleRootfs { ... }
}

// ✅ Hierarchical, organized by responsibility
mod rootfs {
    mod operations;  // Low-level primitives
    mod prepared;    // High-level orchestration (uses operations)
    mod simple;      // Alternative implementation

    pub use operations::{extract_layer_tarball, mount_overlayfs_from_layers};
    pub use prepared::PreparedRootfs;
    pub use simple::SimpleRootfs;
}
```

File organization pattern:

```
src/
  ├── lib.rs              // Public API only
  ├── errors.rs           // Shared error types
  ├── feature/
  │   ├── mod.rs          // Public interface + re-exports
  │   ├── operations.rs   // Low-level primitives
  │   ├── types.rs        // Feature-specific types
  │   └── impl.rs         // High-level implementation
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

## How to Use These Rules

**❌ WRONG: Checklist after coding**

1. Write code
2. Check if it follows rules
3. Fix violations

**✅ RIGHT: Active thinking before coding**

1. Search for existing solutions (`grep -r "pattern" src/`)
2. Read affected files completely (don't skim)
3. Analyze ownership/layering ("Who should know this?")
4. Question necessity ("What breaks if I don't add this?")
5. THEN code (following rules)

**The rules are not a QA checklist—they're a design thinking framework.**

---

## Test-Driven Development (TDD)

**MANDATORY: All feature development MUST follow Test-Driven Development.**

### The TDD Workflow

```
1. Write tests FIRST  →  2. Run tests (should fail)  →  3. Implement feature  →  4. Run tests (should pass)  →  5. Feature complete
```

### Rules

**Rule 1: Tests Before Code**

Before writing ANY implementation code, write unit tests that define the expected behavior.

```rust
// ✅ Correct workflow
// Step 1: Write test first
#[test]
fn test_parse_skill_frontmatter() {
    let content = "---\nname: test\n---\n# Content";
    let result = parse_frontmatter(content);
    assert!(result.is_ok());
    assert_eq!(result.unwrap()["name"], "test");
}

// Step 2: Run test → FAILS (function doesn't exist)
// Step 3: Implement parse_frontmatter()
// Step 4: Run test → PASSES
// Step 5: Feature complete ✓

// ❌ Wrong: Writing implementation without tests
fn parse_frontmatter(content: &str) -> Result<Value> {
    // Implementation without tests...
}
```

**Rule 2: Tests Define Completion**

A feature is considered **complete** ONLY when:
- All related unit tests pass
- `just test` shows green for affected crates

```bash
# Feature is NOT complete until:
just test
# ✓ PASSED  262 passed  0 ignored  (4 crates)
```

**Rule 3: Code Changes Require Test Updates**

When modifying existing code:
- Update corresponding tests to reflect new behavior
- Add new tests for new code paths
- All tests MUST pass before considering the change complete

```rust
// If you change this function:
fn calculate_timeout(base: u64, multiplier: f32) -> u64 {
    (base as f32 * multiplier) as u64  // Changed from base * 2
}

// You MUST update its test:
#[test]
fn test_calculate_timeout() {
    assert_eq!(calculate_timeout(100, 1.5), 150);  // Updated assertion
}
```

**Rule 4: Deleted Features = Deleted Tests**

When removing a feature or function:
- Delete ALL corresponding unit tests
- Do NOT leave orphaned tests or `#[ignore]` tests for removed code
- Run `just test` to ensure no test failures from missing code

```rust
// ❌ Wrong: Leaving tests for deleted code
#[test]
#[ignore]  // TODO: removed feature
fn test_old_feature() { ... }

// ✅ Correct: Delete the test entirely when deleting the feature
// (test file should not contain test_old_feature at all)
```

### Test File Organization

```
src/
  └── module/
      ├── mod.rs           // Implementation
      └── (tests at bottom of mod.rs, or in tests/ directory)

// Tests go in the same file, at the bottom:
// --- mod.rs ---
pub fn my_function() -> Result<()> { ... }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_my_function() { ... }
}
```

### Running Tests

```bash
# Run all tests with progress display
just test

# Run tests for specific crate
just test-code      # a3s-code
just test-core      # a3s-box-core
just test-runtime   # a3s-box-runtime

# Run specific test by name
cd src && cargo test -p a3s-code --lib -- test_name

# Run tests with output
just test-v
```

### Pre-Submission Test Checklist

- [ ] Wrote tests BEFORE implementation
- [ ] All new functions have corresponding tests
- [ ] All modified functions have updated tests
- [ ] Deleted code has no remaining tests
- [ ] `just test` passes with all green
- [ ] No `#[ignore]` tests added for "later"
