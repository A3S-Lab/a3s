# Capability Expansion Implementation Path

Status: planning baseline (verified against working tree, 2026-09-08)

## Objective

Plan how A3S subprojects jointly let **a3s-code expand capabilities via a3s-use**
efficiently—without collapsing ownership, inventing a Core package-manager tool,
or leaving product hosts half-wired.

## First principles

1. **Use owns supply.** Discovery, verification, Grants, install/apply, capability
   generations, snapshot leases, and recovery stay in A3S Use.
2. **Code owns demand projection.** Session/Run scopes, governance ceilings,
   model-visible tools, and effect supervision stay in A3S Code. Code never
   installs or resolves packages.
3. **Host closes the loop.** Only a product host may: watch Use → build
   `SessionCapabilityBatch` → `apply_capability_batch` → admit Runs on generation N.
4. **Atomic generations beat hot mutation.** Expand by publishing N→N+1. Old Runs
   retain N and its Use lease; new Runs see N+1. No mid-turn tool-table mutation.
5. **One mutation path.** Package mutation goes through `PluginManagerService`
   (CLI / TUI `/packages` / manager MCP / Desktop Plugins). No parallel agent
   builtin `use` installer in Code Core.
6. **Evidence over narrative.** A host claims expansion only with canonical Code
   catalog identity + Use capability snapshot cursor for the admitted Run.

## Target loop

```text
use-registry (signed feed)
        ↓
a3s-use plan + apply  →  capability snapshot generation N  (+ lease)
        ↓
Host reconcile  →  SessionCapabilityBatch  →  apply_capability_batch
        ↓
Code Run admits N (frozen Tool / MCP / Skill / Flow / UI / Knowledge…)
        ↓
need more capability  →  Use apply again  →  Host publishes N+1
```

## Current state (verified)

| Subproject | Role in loop | State | Evidence |
| --- | --- | --- | --- |
| `use-registry` | Signed catalog input only | Delivered | Static TUF tree; no lease/batch |
| `crates/use` | Install, snapshot, lease, progress | Delivered (+ Executable Tool projection) | `PluginManagerService`, `CapabilityRegistry` lease; package-local Executable Tools emit `executable_tools` (file evidence, no Runtime BindingStore) — FP: `monorepo_applet_demo_executable_echo_*` |
| `crates/code` | Atomic batch + Run freeze | Delivered | `SessionCapabilityBatch`, HOST-* gates; Core does not watch Use |
| `crates/cli` | Official host that closes the loop | Delivered (+ package-local Executable Tools) | `reconcile_atomic_projection` → `apply_capability_batch`; `executable_tools` reinspect+spawn; FP: `applet_demo_*executable*`, `applet_demo_executable_echo_satisfies_ui_bind_tool_*`; live E2E admissions + committed-tree `real_use_installs_committed_applet_demo_*` asserts UI `bind_tool=echo` |
| `apps/desktop` | Install UI + embedded Code | In progress (W1) | Plugins apply → `UseCapabilityRepublisher` → `SessionCapabilityBatch` / `apply_capability_batch` (Skill + Runtime Tool + managed stdio MCP); FP MCP reinspect against `use-registry/packages/applet-demo` |
| `apps/cloud` | Assignment / Fleet → Use | Partial | U0.1–U0.3 foundation; **U0.4** executable surfaces Planned; not a Code projector yet |

CLI closes: install → N+1 → next surfaces. Desktop now republishes Use snapshots into
live Code sessions after Plugin Manager apply (and on cold session create), including
Skill values and Core `UseRuntimeTaskProjectionAdapter` Runtime Tools. MCP / Flow / UI
adapters and Plugins UI progress streaming remain follow-ons.

## What must not be built

| Anti-pattern | Why |
| --- | --- |
| Code Core builtin that shells out to `a3s-use` | Second mutation path; bypasses confirmation; breaks generation atomicity |
| Per-Turn package resolve inside Code | Slow; fights Use lease model; violates `CAP-DEP1` |
| Desktop ad-hoc MCP/skill bypass as the long-term path | Shadows atomic batch (e.g. Office filesystem shortcuts) |
| Cloud auto-projecting surfaces from assignment alone | Assignment ≠ executable surface; needs U0.4 adapters + host evidence |
| Fake install % progress | Progress is phase · checkpoint steps from observation only |

## Workstreams by subproject

### W0 — Keep Use + Code Core stable (maintenance)

**Owner:** Use / Code  
**Do:** Preserve `PluginManagerService`, snapshot lease, observation progress,
`SessionCapabilityBatch` contracts. Prefer CLI as reference host.  
**Exit:** Existing Use/Code/CLI capability tests remain green; no Core installer.

### W1 — Desktop closes Host republish (highest product gap)

**Owner:** Desktop (+ reuse CLI projection patterns)  
**Why first principles:** Desktop already mutates Use state; without republish,
install does not expand agent capability—the loop is open.

**Path:**

1. After successful Plugin Manager apply (and on Use capability watch), build the
   same atomic projection inputs CLI uses (`DesiredCapabilities` /
   snapshot cursor).
2. Call Code `apply_capability_batch` on the active `CodeTaskRuntime` session(s).
3. Emit/require `ScopedCapabilityRuntimeEvidence` (types already in
   `apps/desktop/shared/protocol.ts`); reject success without catalog + Use cursor.
4. Stream Use operation progress into Plugins UI (reuse observation contract;
   no fake %).
5. Retire or quarantine ad-hoc Office MCP/skill bypasses that skip the batch.

**Exit evidence:**

- Install/enable package in Desktop → next Code Run lists new frozen Tool/MCP/Skill.
- N Runs retain N across N+1 cutover (same invariant as CLI).
- Focused Desktop + Code integration test proves apply → batch → Run tools.
- Grep/runtime asserts Desktop uses `SessionCapabilityBatch` / `apply_capability_batch`.

**Non-goals:** Reimplementing Plugin Manager inside Code; inventing a Desktop-only
capability schema.

### W2 — Align Runtime Tool projection on Core adapter ✅ (CLI)

**Owner:** CLI (then Desktop)  
**Why:** Core ships `UseRuntimeTaskProjectionAdapter`; CLI previously staged a
local `UseRuntimeTaskTool`. Convergence reduces dual surfaces.

**Done (CLI):**

1. Stage Core `UseRuntimeTaskProjectionAdapter` in CLI
   `SessionCapabilityBatch` via `runtime_tasks::projection_adapter`.
2. CLI-local Runtime Tool deleted; host bridge is
   `RuntimeTaskInvokerDispatcher` (`UseRuntimeTaskDispatcher` → Plugin Manager
   `RuntimeTaskInvoker`).
3. Desktop still adopts the same adapter when W1 lands (out of this CLI slice).

**Exit:** CLI capability matrix green (`use_registry::runtime_tasks`,
`use_registry::tests` Runtime Tool paths). One Runtime Tool projection adapter
on CLI; Desktop remains pending W1.

### W3 — Cloud U0.4 executable surfaces (control-plane expansion)

**Owner:** Cloud (+ Use Node Agent, Runtime/Box/Gateway as named deps)  
**Why:** Cloud today assigns and observes Use generations; agents still need
host-local projection, not Cloud-as-Code.

**Path (per Cloud ROADMAP):**

1. Finish U0.3 assignment durability/ops as required prerequisites.
2. U0.4: permission-bearing Tool Task, private Tool Service, MCP, Secret-ref, UI,
   OKF adapters—**no** Cloud-local surface lifecycle duplicate.
3. Node Agent / Edge host remains the party that holds Use lease and (where
   Code runs) publishes `SessionCapabilityBatch`.
4. U0.5: multi-host ops after U0.4.

**Exit:** Assignment + confirmed apply → observed Use `capability_generation` →
host publishes executable surfaces with gate evidence; Cloud does not become a
second Code Core.

### W4 — Registry / science packages as supply only

**Owner:** `use-registry`, `packages/science`  
**Path:** Keep signed admissions high quality; document which packages matter for
Desktop/CLI smoke. No projection logic in the registry.  
**Exit:** Install-from-registry E2E on CLI (and later Desktop W1) remains the proof.

**CLI exit evidence (2026-09-08):** Track A A1–A4
(`use-registry/scripts/test_just_registry.sh`) plus CLI
`real_use_installs_applet_demo_and_cli_projects_panel_ui` prove admissions
assemble → real Use plan/apply → Host projects package `ui/panel` HTML bytes.
Committed `use-registry/registry/` includes `a3s/applet-demo` with
`bind_tool=["echo"]` (Track J6 catalog + apply `activity_bar` deps match
package source; Track S1 fails closed if source ACL ≠ archive ACL; Track S0
requires git HEAD to advertise the archive; monorepo `just */registry`
requires the applet-demo target file so stale submodule tips fail closed;
bootstrap root pin refreshed on custody republish). Desktop W1
remains out of this non-Desktop slice.

### W5 — Operator / agent-governed package proposals (optional, later)

**Owner:** Product host  
**Path:** If agents must *propose* installs, expose **manager-v5 MCP** under
existing confirmation/`--yes` policy—not a Code builtin.  
**Exit:** Mutations still hit `PluginManagerService`; Host republish (W1/CLI)
remains mandatory after apply.

## Sequencing

```text
Now ──────────────────────────────────────────────────────────►
 W0 maintenance (continuous)
      └─ W1 Desktop republish          ← critical product gap
           └─ W2 Runtime Tool adapter convergence
           └─ Desktop UiHost (Applet)  ← see desktop-applet-plugin-path.md
      └─ W3 Cloud U0.4 (after U0.3 + Use M5/M6 deps)
 W4 registry supply (parallel, low coupling)
 W5 governed manager MCP for agents (only after W1)
```

**Priority rule:** Close open Host loops before adding new mutation surfaces.

## Efficiency rules (once the loop is closed)

1. Prefetch/install common packages before Session start; avoid install-during-chat.
2. Profile-filter model-visible tools (`CAP-PROFILE1`); do not dump full catalogs.
3. Prefer Use watch + reconcile over full rediscovery every Run.
4. Short-lived Exec hosts: bounded MCP/Skill/UI cut; stop discovery before admit
   (CLI scoped path already models this).

## Cross-links

- Desktop Applet UiHost roadmap: `docs/desktop-applet-plugin-path.md`
  (per-project stubs listed under “Per-project persistence” in that file)
- Code: `crates/code/manual/SCOPED_CAPABILITY_ARCHITECTURE.md`
- CLI reference host: `crates/cli/src/use_registry.rs` (`reconcile_atomic_projection`)
- CLI review: `crates/cli/docs/capability-first-principles-review.md`
- Use Manager / progress: `crates/use/README.md`
- Cloud U0: `apps/cloud/ROADMAP.md` (`U0.4` Planned)

## Completion criteria for this plan

This document is the planning deliverable. Implementation tracks W1–W5 as separate
execution goals. Do not mark product capability-expansion “done” until W1 exit
evidence is green for Desktop (CLI already satisfies the loop).
