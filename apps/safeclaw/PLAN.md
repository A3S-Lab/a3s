# SafeClaw UI — First-Principles Feature Plan

## Core Mission

SafeClaw is a **privacy-preserving AI assistant with TEE support**. The UI must let users:
1. **Use AI agents across channels** — chat, configure, monitor sessions
2. **Configure everything** — providers, models, channels, per-channel policies, workflows
3. **Observe what's happening** — costs, tool calls, security events, taint tracking in real time
4. **Trust the system** — security posture, TEE status, audit trail always visible
5. **Control their data** — memory browser, explicit forget, per-user isolation

---

## What to Remove

### Radar page — delete
RSS/news feeds have nothing to do with privacy-preserving AI. Tangential feature creep.

### A2A routing graph — descope
SafeClaw is a single-user security proxy, not a multi-agent orchestration platform.
Keep basic @mention input in chat. No visual routing graph needed.

---

## Pages & Modules

### Existing pages (keep + fix gaps)

#### `/` — Agent (main chat)
Current state: session list + chat ✅

Gaps to fix:
- **Tool progress ticker** — `activeToolProgress` in agentModel is unused; show tool name + elapsed time in SessionStatusBar
- **Per-turn cost breakdown** — expand SessionStatusBar: input/output/cache tokens + USD per turn
- **Session health badges** — colored dot per session in sidebar (green=idle, yellow=running, red=error)
- **Per-session config drawer** — gear icon → drawer to edit model/temperature/system prompt mid-session (currently frozen after creation)
- **Streaming status indicator** — for channel messages (sending → editing → done), needed after Phase 8 streaming lands

#### `/sessions` — Sessions Overview
Current state: implemented but not in router ⚠️

Gaps to fix:
- Add to router
- Quick actions: open, stop, fork, archive
- Session health at a glance

#### `/security` — Security & Privacy
Current state: UI exists but many endpoints return empty data ⚠️

Gaps to fix:
- **Channels panel**: wire to real `/api/v1/channels` data; add WhatsApp, Teams, Google Chat, Signal config forms (Phase 16)
- **Per-channel agent config**: expand each channel row → model selector, permission mode, tool whitelist (Phase 17)
- **Credential health**: show `credentials` field from `/health` — green/red per provider and channel token (Phase 8)
- **Taint tracking**: wire `GET /api/v1/taint/entries` to real data
- **Audit events**: wire filters (severity, session, vector, time range) to `POST /api/v1/audit/query`

#### `/settings` — Settings
Current state: provider/model config ✅

Gaps to fix:
- **Provider connection test** — "Test" button per provider → latency + validity confirmation (Phase 8)
- **Global agent defaults** — max turns, default temperature, default cwd, auto-archive timeout
- **Per-persona defaults** — default model/system prompt per persona

### New pages

#### `/workflows` — Workflow Orchestration (Phase 18)
Multi-step workflow editor and execution history.

- **Workflow list**: name, trigger type (manual/schedule/webhook), last run status + time
- **Step editor**: add/reorder/delete steps; each step has prompt + output variable binding + privacy check toggle
- **Trigger config**: manual button, cron expression, or webhook event selector
- **Execution history**: per-workflow run log with step-by-step output and privacy gate results
- **HITL integration**: pending confirmation steps shown inline with approve/reject

API: `GET/POST/PATCH/DELETE /api/v1/workflows`, `POST /api/v1/workflows/:id/trigger`

#### `/memory` — Memory Browser (Phase 19)
Cross-session knowledge store: Resources → Artifacts → Insights.

- **Layer tabs**: Resources (L1) / Artifacts (L2) / Insights (L3)
- **Search**: keyword search across all layers
- **Taint labels**: show taint badges on each entry; entries with active taint highlighted
- **Decay indicator**: entries approaching `decay_days` shown with warning
- **Forget action**: delete button with confirmation → `DELETE /api/v1/memory/artifacts/:id` (secure erase)
- **Cross-session retrieval toggle**: enable/disable per-session context injection from past sessions

API: `GET /api/v1/memory/resources`, `GET /api/v1/memory/artifacts`, `GET /api/v1/memory/insights`, `DELETE /api/v1/memory/artifacts/:id`, `DELETE /api/v1/memory/insights/:id`

#### `/users` — User Management (Phase 20, admin only)
Multi-user support for enterprise channels (Teams, Slack).

- **User list**: display name, role (admin/user), channel associations, last active
- **Role assignment**: promote/demote admin
- **Per-user config**: override privacy rules, cumulative risk threshold, memory settings
- **Remove user**: delete + wipe session data (zeroize confirmation dialog)
- **Access control**: entire page hidden for non-admin users; 403 redirect

API: `GET/POST/PATCH/DELETE /api/v1/users`

#### `/archive` — Session Archive (P3)
Archived sessions browsable and replayable.

- **Search + filter**: by persona, date range, cost, keyword in messages
- **Read-only chat replay**: full message history, tool calls, cost breakdown
- **Restore**: unarchive a session

---

## Implementation Order

```
Sprint 1 — Observability gaps (no new backend needed)
  - Tool progress ticker in SessionStatusBar
  - Per-turn token/cost breakdown
  - Session health badges in sidebar
  - Add /sessions to router

Sprint 2 — Config completeness
  - Per-session config drawer (model/temperature/system prompt mid-session)
  - Provider connection test button
  - Global agent defaults in Settings
  - Per-persona defaults in agent-detail

Sprint 3 — Security page wiring (Phase 8 + 16 + 17)
  - Credential health indicators in /security and /settings
  - Channels panel: wire real data, add WhatsApp/Teams/Google Chat/Signal forms
  - Per-channel agent config: model, permission mode, tool whitelist

Sprint 4 — New pages: Workflows + Memory (Phase 18 + 19)
  - /workflows: list, step editor, execution history, HITL
  - /memory: layer browser, search, taint badges, forget action

Sprint 5 — Multi-user + Archive (Phase 20 + P3)
  - /users: user list, role assignment, per-user config (admin only)
  - /archive: search, read-only replay, restore
```

---

## What NOT to Build

- **Radar / RSS feeds**: tangential to core mission — removed
- **A2A routing graph**: SafeClaw is a proxy, not an orchestration platform
- **Billing dashboard**: self-hosted, cost tracking is informational only
- **Notification center**: toasts + activity feed cover this adequately
- **Manual theme toggle**: system preference is sufficient
- **Drag-and-drop pipeline editor**: workflow steps are sequential, a simple ordered list is enough
