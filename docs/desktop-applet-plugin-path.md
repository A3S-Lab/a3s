# Desktop Applet Plugin Path

Status: planning baseline (re-verified against working tree, 2026-09-08)

## Objective

Define—from first principles—the work and mechanisms each A3S subproject must
own so **a3s-desktop** can deliver an **Applet**-like plugin experience
(comparable to WeChat’s applet model): install from a store, show package
entries in the shell, open sandboxed pages, and call frozen backend
capabilities—without collapsing ownership or inventing a second platform.

Related: [Capability Expansion Implementation Path](./capability-expansion-implementation-path.md)
(Host republish of Tool/MCP/Skill). That loop is a **prerequisite** for Applet
UX (install must become live session capabilities). This document focuses on
**UiHost** and shell composition once supply + freeze exist.

## Terminology

| Term | Meaning |
| --- | --- |
| **Applet** | Host-rendered, Use-installed UI package page (English product term for 小程序) |
| **Plugins page** | Desktop package management UI (catalog / plan / apply), not the Applet runtime |
| **UiHost** | Desktop role that projects Use UI into Code `UiBinding` and renders it |

---

## First principles (derive the system, then assign owners)

A WeChat-like plugin page is not “HTML in a WebView.” It is six coupled
invariants. If any invariant is owned by the wrong layer, the product either
forks a second platform or cannot fail closed.

### I1 — Signed supply with a single mutation path

Packages are discovered, planned, granted, installed, upgraded, and removed
through one reviewed service. Hosts must not ship a second installer.

**Implication:** Use `PluginManagerService` is the only mutation path. Desktop
Plugins UI is a client of that service, not a package manager.

### I2 — Content-addressed UI, not host filesystem paths

An openable page is bytes (HTML/CSS/JS + digests) admitted with the same
generation as Tool/MCP/Skill/Flow. Incomplete dependency evidence must not
publish UI.

**Implication:** Use owns UI surface supply and publish gates. Code freezes
path-free `UiBinding` values. Renderers load digest-matched bytes only.

### I3 — Atomic capability generations (N → N+1)

Install/upgrade republishes an entire snapshot. Open pages pin the generation
they opened with; upgrades require reopen. Sessions must not hot-mutate under
a live page.

**Implication:** Code `SessionCapabilityBatch` + Use leases. Hosts must refresh
both agent sessions **and** the shell UI catalog after apply / Use watch.

### I4 — Demand freeze before invoke

Agents and Applet bridges may call only capabilities frozen into the current
session generation. Missing capability fails closed; HighRisk still confirms.

**Implication:** Code owns freeze contracts (`HOST-UI1` and Tool/MCP/…). Desktop
bridge is a policy client of that freeze, not a free IPC pipe to the OS.

### I5 — Shell composition is host product surface

Store install must produce a visible entry; opening it must render the frozen
document. Management UI ≠ runtime.

**Implication:** Only Desktop closes the product loop for Applet UX. CLI may be
the reference projection host; `a3s-webview` may supply sandbox/bridge patterns
or a TUI companion window—neither replaces Desktop UiHost.

### I6 — Strong isolation backends ≠ page shell

If an Applet’s Tool/Service needs OCI/microVM isolation, that is a **backend**
binding. The page itself stays in a Desktop sandbox WebView.

**Implication:** Box/Runtime execute workloads. Package HTML never shells to
`a3s-box` / `a3s-use box` as a page runtime.

### Ownership collapse rules (refuse these)

| Collapse | Why it fails I1–I6 |
| --- | --- |
| Desktop ad-hoc HTML outside Use generation | Breaks I2/I3 |
| Code Core installer or WebView | Breaks I1/I5; host policy leak |
| Box as Applet page process | Breaks I5/I6 |
| Cloud-rendered package UI as truth | Breaks I3/I5 (assignment ≠ executable surface) |
| Treat Plugins page as Applet-complete | Breaks I5 |

---

## Product mapping (WeChat Applet → A3S)

| WeChat-like experience | A3S mechanism | Primary owner |
| --- | --- | --- |
| Store discover / install / update | Plugin Manager + Desktop Plugins page | Use + Desktop |
| App icon / entry in the client | `activity_bar` → projected UI catalog → shell destination | Use supply → Desktop shell |
| Independent page | `UiBinding` bytes + sandboxed renderer | Code freeze → Desktop render |
| Call backend APIs | Bridge → same-generation Tool / MCP / Flow | Desktop bridge + Code freeze |
| Permission / isolation | Grants + CSP/bridge policy + confirmation | Use + Desktop |
| Instant after install | Host republish: apply → batch → shell + Code N+1 | Desktop (W1 + P0) |
| Old sessions not hot-mutated | Catalog CAS + retained Use lease on N | Code + Use |
| Optional strong backend | OCI Tool/Service via Runtime/Box | Use Runtime bindings + Box |

---

## Current state (verified 2026-09-08)

| Subproject | Role | State | Evidence |
| --- | --- | --- | --- |
| `crates/use` | Ui surface supply | **P1 exit delivered** | `docs/applet-ui-supply.md`; six-surface via `PluginManagerService` (`plugin_manager_six_surface`); Executable Tool `Prepared` from file inspection |
| `crates/code` | `UiBinding` freeze | Delivered | `HOST-UI1` in `crates/code/ROADMAP.md`; `docs/applet-ui-freeze.md` |
| `crates/cli` | Reference UI projection | Delivered (integrity + Executable Tools) | UI batch + `executable_tools`; FP package bytes + atomic `bind_tool=echo`; live Use E2E admissions + **committed** `registry/` (`real_use_installs_committed_applet_demo_*`) |
| `apps/desktop` | Plugins page + UiHost | **Split — P0 open** | Plugins management delivered; `activity-destinations.ts` hard-codes `projects`/`plugins` only; **no** `activity_bar` / `UiBinding` under `src/` or `src-tauri/` |
| `crates/box` / Runtime | Isolated backends | **P4 boundary freeze delivered** | `docs/applet-backend-boundary.md` + `applet_boundary` contract tests |
| `crates/webview` | Native WebView companion | **P4b boundary freeze delivered** | `docs/applet-webview-substrate.md` + `applet_boundary_tests`; optional substrate — not product UiHost |
| `use-registry` / `packages/*` | Signed supply | **P5 package source + committed registry delivered** | `packages/applet-demo` + admission; Track A/J6/S0/S1; signed archive; monorepo `just up/test/down::registry` require the applet-demo target file (fail closed on stale gitlink) |
| `apps/cloud` | Assignment | Later (P6) | `U0.4` Planned; must not become a second UiHost |

### Desktop distinction (do not conflate)

| Delivered | Missing for Applet UX |
| --- | --- |
| Plugins catalog / plan / apply | Package `activity_bar` → dynamic shell entries |
| ActivityBar destination `plugins` (management) | Sandboxed page runtime for `UiBinding` |
| Use apply → Skill/Tool/MCP republish path (W1 MCP staged) | Controlled Applet bridge to frozen Tool/MCP/Flow + UI catalog republish |

---

## Workstreams by subproject

### P0 — `apps/desktop` (UiHost; highest product gap)

**Owner:** Desktop  
**Why:** Supply (Use P1) and freeze (Code `HOST-UI1`) exist; without UiHost
wiring, installing a UI package never becomes an openable Applet.

**Mechanisms to implement:**

1. **UI projection.** Map Use snapshot `activity_bar` → load entry/style/script
   bytes (integrity-bound) → `UiBinding` → stage into the same
   `SessionCapabilityBatch` path used for Skill/Tool (align with CLI).
2. **Shell catalog.** ActivityBar / panel list consumes the **projected UI
   catalog**, not only hard-coded `projects` / `plugins` destinations.
3. **Sandboxed renderer.** Dedicated webview/iframe: content-addressed
   `UiBinding` only; CSP; no arbitrary `file://`; no Node. May reuse
   `a3s-webview` Workspace Host patterns (generation, typed bridge, origin
   allowlists) or an embedded equivalent—product ownership stays Desktop.
4. **Controlled bridge.** Page JS may invoke only Tool/MCP/Flow frozen in the
   current generation; reuse Desktop confirmation / HighRisk policy.
5. **Republish loop.** After Plugin Manager apply and on Use watch: refresh live
   Code sessions **and** the shell UI catalog; require
   `ScopedCapabilityRuntimeEvidence` (catalog digest + Use cursor). Depends on
   capability-expansion **W1** for Skill/Tool/MCP republish completeness.
6. **Generation UX.** Open pages pin open-time generation; upgrade prompts
   reopen for N+1.

**Exit evidence:**

- Install a signed package with `ui` / `activity_bar` in Desktop Plugins.
- A new shell entry appears after N+1 republish.
- Opening it renders the frozen `UiBinding` document.
- Bridge call reaches a same-generation Tool/MCP; missing capability fails closed.
- Focused tests: apply → batch includes `CapabilityKind::Ui` → shell lists entry
  → renderer loads digest-matched bytes (no Host-internal overfitting mocks).

**Non-goals:** Reimplement Plugin Manager; use `a3s-box` as the page process;
bypass Use for ad-hoc HTML; make Desktop depend on the CLI binary.

### P1 — `crates/use` (supply hardening)

**Owner:** Use  
**Status:** Exit proof delivered (maintain under regression).

**Mechanisms (keep true):**

1. Ui surface contract stable: manifest UI, asset digests,
   `ActivityBarContribution`, dependency evidence completeness.
2. Same snapshot generation and lease for UI and Tool/MCP/Skill/Flow.
3. Grant / enable / disable / uninstall clears receipt-owned UI projections.
4. Observation progress (phase · checkpoint) for Desktop Plugins streaming.
5. OCI URIs remain **Runtime/Tool/MCP artifact** references, not an Applet
   container platform.
6. Package-local Executable Tool / stdio MCP mark Prepared from file inspection
   so Skill/UI graphs can publish without false RuntimeBinding requirements.

**Exit:** Six-surface packages (including UI) install/enable/upgrade/disable/
uninstall through `PluginManagerService`; incomplete dependency evidence stays
unpublished — covered by `plugin_manager_six_surface` (+ related host matrix).

### P2 — `crates/code` (freeze maintenance)

**Owner:** Code  

**Mechanisms:**

1. Preserve `HOST-UI1` (`UiBinding`, host handle, N/N+1 isolation).
2. Dependency edges limited to Tool / Skill / MCP / Flow; fail closed otherwise.
3. Session close cancels active UI host use and releases leases.

**Non-goals:** WebView, navigation chrome, payment, camera, or install APIs.

### P3 — `crates/cli` (reference host)

**Owner:** CLI  

**Mechanisms:**

1. Keep UI staging in `SessionCapabilityBatch` as the reference projection.
2. Optionally document Desktop-facing projection parity checklist.
3. Do not regress static-integrity loading (`load_managed_ui_asset`).

Desktop must **copy contracts**, not depend on the CLI binary.

### P4 — `crates/box` / Runtime / OCI (backend isolation only)

**Owner:** Box / Runtime / Use Runtime bindings  
**Status:** Boundary freeze delivered (maintain under regression).

**Mechanisms:**

1. When an Applet page needs a strong-isolation backend, Use binds OCI
   Tool/Service generations; Box/OCI executes them.
2. Desktop bridge talks to **projected** Tool/MCP identities, never to raw
   `a3s-box` CLI from package HTML.

**Non-goals:** One microVM per open page; Box as package manager; Box as
WebView replacement.

### P4b — `crates/webview` (optional substrate / companion)

**Owner:** WebView crate (consumed by CLI/TUI today; optionally by Desktop)  
**Status:** Boundary freeze delivered (maintain under regression).

**Mechanisms it can contribute (not required as the product host):**

1. Native multi-platform WebView window and Workspace Host (shell vs content
   views, generation-aware open/bounds/close, typed JS bridge, origin and
   `file://` allowlists).
2. Pattern and/or library reuse for Desktop’s sandboxed Applet renderer and
   generation-pinned bridge.
3. CLI/TUI companion window to preview Use-installed UI when no Desktop shell
   is present.
4. Contract: Applet loads stay content-addressed `UiBinding` bytes — never
   arbitrary package `file://` (`applet_boundary_tests`).

**Non-goals:** Own Plugins mutation; become a second Desktop UiHost; load
unsigned or path-escaping package HTML.

### P5 — `use-registry` / `packages/*` (supply)

**Owner:** Registry + package authors  
**Status:** Package source + admission + committed `registry/` republish delivered.

**Mechanisms:**

1. Publish at least one signed example package with `ui` / activity-bar
   metadata, icons, order, and declared Tool/MCP dependencies —
   `packages/applet-demo` (`a3s/applet-demo`).
2. Document authoring rules (assets, digests, dependencies).
3. No projection or renderer code in the registry.
4. First-principles Track A gate in `scripts/test_just_registry.sh`.

### P6 — `apps/cloud` (later)

**Owner:** Cloud  

**Mechanisms:** U0.4 may assign UI-bearing packages; Node/Desktop Host still
holds Use lease and performs projection/render. Cloud must not become a second
UiHost or Code Core.

---

## Sequencing

```text
Now ──────────────────────────────────────────────────────────►
 Use P1 supply exit          ✅ delivered (maintain)
 Box P4 boundary freeze      ✅ delivered (maintain)
 Code HOST-UI1               ✅ delivered (maintain)
 CLI UI batch reference      ✅ delivered (maintain)
 WebView P4b boundary freeze ✅ delivered (maintain)
 Capability W1 Desktop republish (Skill/Tool/MCP)   ← still open; Applet prerequisite
      └─ P0 Desktop UiHost (projection → entry → sandbox → bridge)  ← critical gap
           └─ P3 CLI parity checklist as Desktop deltas appear
      └─ P5 example UI package           ✅ delivered (committed registry + Track A)
 P2 Code maintenance (continuous)
 P4 Box backends only when a UI package needs OCI
 P6 Cloud U0.4 after host UiHost exists
```

**Priority rule:** Close Desktop UiHost (and W1 republish) before inventing new
package surfaces or Box-based “Applet VMs.”

---

## Explicit refusals

| Anti-pattern | Why |
| --- | --- |
| Treat Plugins page as Applet complete | Management ≠ UiHost runtime |
| `a3s-box run` as page runtime | Wrong isolation boundary; latency; no shell integration |
| Code Core installer or WebView as product UiHost | Second mutation path; host policy leak |
| Desktop ad-hoc HTML outside Use generation | Breaks lease and N/N+1 |
| Cloud-rendered package UI as source of truth | Assignment ≠ executable surface |
| Depend on CLI binary from Desktop | Wrong coupling; copy contracts instead |

---

## Per-project persistence

Canonical plan stays in this file. Each owning tree also keeps a local stub so
submodule work does not lose the Applet slice:

| Workstream | Local roadmap |
| --- | --- |
| P0 Desktop UiHost | [`apps/desktop/docs/applet-uihost-roadmap.md`](../apps/desktop/docs/applet-uihost-roadmap.md) |
| P1 Use supply | [`crates/use/docs/applet-ui-supply.md`](../crates/use/docs/applet-ui-supply.md) + ROADMAP UI item |
| P2 Code freeze | [`crates/code/docs/applet-ui-freeze.md`](../crates/code/docs/applet-ui-freeze.md) + `HOST-UI1` |
| P3 CLI reference | [`crates/cli/docs/applet-ui-projection-parity.md`](../crates/cli/docs/applet-ui-projection-parity.md) |
| P4 Box backends | [`crates/box/docs/applet-backend-boundary.md`](../crates/box/docs/applet-backend-boundary.md) |
| P4b WebView substrate | [`crates/webview/docs/applet-webview-substrate.md`](../crates/webview/docs/applet-webview-substrate.md) |
| P5 Registry supply | [`use-registry/docs/applet-ui-package-supply.md`](../use-registry/docs/applet-ui-package-supply.md) |
| P6 Cloud assignment | [`apps/cloud/docs/applet-ui-assignment.md`](../apps/cloud/docs/applet-ui-assignment.md) + ROADMAP `U0.4` |

---

## Cross-links

- Capability expansion (Tool/MCP/Skill loop): `docs/capability-expansion-implementation-path.md`
- Code UI contract: `crates/code/manual/SCOPED_CAPABILITY_ARCHITECTURE.md` (`HOST-UI1`)
- Code verification: `crates/code/manual/CAPABILITY_VERIFICATION.md`
- CLI projection: `crates/cli/src/use_registry/capability_batch.rs`
- Use UI ownership: `crates/use/README.md` (UI surface row); `crates/use/ROADMAP.md`
- Desktop Plugins UI: `apps/desktop/src/features/plugins/management.tsx`
- Desktop shell destinations: `apps/desktop/src/shell/activity-destinations.ts`

## Completion criteria for this plan document

This document is the **analysis / planning** deliverable. Implementation tracks
P0–P6 (and W1) as separate execution goals. Do not mark “Desktop Applet
plugins” product-done until P0 exit evidence is green.
