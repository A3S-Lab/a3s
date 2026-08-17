# A3S Web Roadmap

**Status:** Active product roadmap

**Updated:** 2026-08-17

## Purpose

This document prioritizes the next A3S Web product investments. It incorporates
the most valuable patterns in WorkBuddy without copying its vendor-specific
ecosystem or weakening A3S's local-first, review-first product model.

The roadmap is the sequencing and ownership layer above the current
[product blueprint](docs/PRODUCT_BLUEPRINT.md),
[super-app plan](docs/SUPER_APP.md), and feature-specific plans. Those documents
remain the source of truth for shipped behavior until a roadmap milestone lands
and updates its affected product, architecture, journey, and acceptance
contracts.

Priority order expresses product dependency, not a committed release date:

- **P0 — Foundation:** fix the primary task journey before adding more entry
  points.
- **P1 — Personal leverage:** make one local user substantially more productive.
- **P2 — Reusable capability:** turn runtime power into discoverable product
  objects.
- **P3 — Collaboration:** add local projects first and optional cloud teamwork
  through A3S OS.
- **P4 — Extensions:** add specialist surfaces without expanding the core into
  unrelated applications.

## Product direction

A3S Web should become a conversation-first, local-first work environment in
which a user can start a task, supervise execution, inspect real files and
evidence, automate a reviewed workflow, and reuse that workflow safely.

The ownership boundary is fixed:

| Layer | Owns |
| --- | --- |
| A3S Web and the local A3S service | Personal tasks, conversations, real local files, local projects, approvals, schedules, run history, and offline recovery |
| A3S Use | Signed Skills, plugins, templates, specialist activities, and reviewed capability distribution |
| A3S OS | Optional identity, cross-device sync, public sharing, team membership, RBAC, shared cloud assets, hosted schedules, and always-on assistants |

An A3S OS account must not be required for the core local task, file, Office,
Knowledge, Memory, Skill, or automation journeys.

## Current foundation

The roadmap builds on capabilities that already exist rather than recreating
them:

- durable local conversations, streaming execution, plans, queues, interruption,
  model and effort controls, scoped permission review, and runtime subagents;
- Finder-style access to real local files, Monaco-based code editing, native
  DOCX/XLSX/PPTX/PDF workflows, structured change proposals, and live preview;
- local Knowledge compilation, Memory visualization and evolution review, and a
  signed A3S Use plugin lifecycle;
- generic MCP configuration plumbing; and
- a native WeChat/Weixin channel with truthful read-only remote target state and
  an existing security-gated managed-control plan.

## WorkBuddy capability decisions

The public WorkBuddy product is used as a capability reference, not as an
architecture template.

| WorkBuddy pattern | A3S decision | Priority and owner |
| --- | --- | --- |
| Independent task conversation, result inspection, status/date filters, pinning, archive, workspace grouping, and sharing | Adopt the task-management model. Make the conversation a full page, improve the local task library, and defer public links to A3S OS. | P0 Web; P3 OS for public sharing |
| Scheduled automation with templates, workspace selection, run history, and notifications | Adopt as reviewed local recipes backed by a durable service scheduler. Add hosted execution only through A3S OS. | P1 Web/service; P3 OS |
| Connector catalog, OAuth, custom connectors, mail, calendar, documents, meetings, issue tracking, and cloud drives | Adopt a vendor-neutral Connector Center over MCP and typed adapters. Start with broadly useful protocols instead of hardcoding the Tencent catalog. | P1 Web/service |
| Remote assistants over WeChat, WeCom, QQ, Feishu, and DingTalk | Continue the current WeChat safety plan, then extract a protocol-independent channel adapter before adding channels. | P1 Boot/Web |
| Expert Center, custom experts, and expert teams | Adapt into reusable Agent Profiles and Agent Teams built on the existing agent/subagent runtime. | P2 Web/runtime |
| Skill market, guided Skill creation, enable/disable controls, and an inspiration gallery | Expand A3S Use into a searchable Skill Center plus reviewed recipes. Preserve signing, versioning, provenance, and install-time risk review. | P2 Use/Web |
| Projects with shared instructions, connectors, experts, Skills, knowledge, assets, task handoff, and multi-user work | Adopt a single-user local Project object first. Put membership, RBAC, shared credentials, real-time collaboration, and cloud assets in A3S OS. | P3 Web/OS |
| Agent mailbox | First support user-owned mail accounts through the Connector Center. Consider an A3S-provisioned agent identity only as an opt-in A3S OS service with abuse controls and outbound confirmation. | P3 OS |
| Tencent Docs and cloud knowledge integrations | Integrate through connectors and explicit import/sync contracts. Do not replace A3S's real-file Office editors or local Knowledge product. | P3 Web/OS |
| Ardot design canvas plus image/video creation | Offer creative surfaces as signed A3S Use activities that hand reviewed files back to Work. Do not build a second first-party design suite into core Web. | P4 Use/Web |
| Mobile client and always-on cloud assistant | Provide a bounded companion and hosted execution through A3S OS after remote policy, audit, and recovery contracts are stable. | P4 OS |

## P0 — Conversation-first task experience

**Outcome:** submitting from Home enters an independent conversation page. It
must never make a right- or left-side assistant panel the primary destination.

**Implementation status (2026-08-05):** the dedicated conversation route,
Home/task-list transitions, browser-history restoration, truthful missing-task
state, responsive conversation shell, and contextual-assistant boundary are
implemented. Task-library filters, pinning, archive, and workspace grouping
remain in this milestone.

**Experience foundation update (2026-08-17):** the first WorkBuddy-informed
interaction reset is implemented without copying WorkBuddy visuals or product
architecture:

- [x] Restore the shared 52 px desktop Activity Bar contract and replace the
  permanent narrow rail with a labeled 56 px bottom navigator below 768 px.
- [x] Make Home express one primary intent: describe an outcome in the
  production composer. Keep browse, open, create, analyze, and organize as
  quiet secondary actions.
- [x] Move all six Office creation choices into one accessible disclosure and
  remove the duplicate always-visible template gallery.
- [x] Consolidate the lower Home surface around “continue work,” search,
  folders, managed views, and recent files.
- [x] Make the compact task library a bounded drawer with an outside-click
  target, initial focus on its collapse control, focus restoration to its
  opener, and the same 768 px behavior threshold used by the shell.
- [x] Keep the conversation header on one 50 px row and reserve the center for
  the thread, runtime evidence, and docked follow-up composer.

Browser E2E validation on 2026-08-17 used the repository `.a3s/config.acl`
default, `deepseek/deepseek-v4-pro`. A read-only task created a durable session,
streamed file-read and code-search evidence, and returned the expected first
root `README.md` heading (`## Quick start`, line 39). Desktop and 390 px mobile
flows completed with an accessibility snapshot, no console messages, and no
page errors.

- [ ] Replace Web's remaining local shell/editor mirrors with package-native
  `a3s-ui` and controlled `a3s-office` boundaries after their browser package
  distribution and migration contracts are version-locked. Do not expand the
  duplicated editor implementation meanwhile.
- [ ] Project `a3s-code` retrieval mode, indexing readiness, and evidence into
  Web only after the local service exposes a typed, versioned API. Web must not
  infer semantic-search readiness from generic task events.

### Dedicated conversation surface

- [x] Keep `#home` as the task landing page and new-task composer.
- [x] Add a canonical, reload-safe `#conversation/<opaque-session-key>` route for an
  existing or newly created task.
- [x] After successful durable session creation and submission, navigate to that
  conversation route. If creation or submission fails, remain on Home with the
  draft, selected context, and a recoverable error intact.
- [x] Give the conversation the full Work center: transcript, plan, execution,
  permission decisions, waiting-input states, recovery, artifacts, evidence,
  and the follow-up composer belong to this page.
- [ ] A subordinate result inspector may show artifacts, files, changes, and live
  preview beside the conversation. It must not turn the conversation back into
  a transient side panel.
- [x] Preserve the contextual AI pane only inside file, Office, PDF, and code scenes,
  where the document remains the primary object. Home submission must not open
  that pane as its destination.
- [x] Reuse the unified session catalog, draft, queue, workspace, and runtime state.
  The new route must not create a second task store or hidden conversation.
- [x] Back, reload, deep-link, missing-session, Settings-return, and
  service-recovery flows have explicit destinations and preserve unsent drafts.
  Archive remains part of the Task library work below.

This milestone supersedes the former `#home`-only Work route and side-assistant
submission contract. `SUPER_APP.md`, `PRODUCT_BLUEPRINT.md`,
`PRODUCT_ARCHITECTURE.md`, `WORK_AI_NATIVE_HOME.md`, the domain model, and user
journeys now describe the delivered route boundary.

### Task library

- Retain search, resume, inline rename, truthful running state, and delete.
- Add service-authoritative filters for execution status and date range.
- Add pin/unpin and stable pinned ordering.
- Add recoverable archive/unarchive, visibly distinct from permanent deletion.
- Group tasks by local workspace and, once P3 lands, by Project.
- Show waiting for input, queued, running, completed, failed, interrupted, and
  archived states consistently across the list and conversation header.
- Keep public links and multi-user presence out of the local-only milestone.

### P0 exit gate

- Home submission opens exactly one independent conversation page for the
  created session and never uses the contextual assistant pane as the primary
  result.
- Reload and a copied local deep link restore the same session or show a
  truthful recoverable missing-session state.
- Switching tasks cannot leak messages, drafts, workspace snapshots, tool
  decisions, or results across sessions.
- Archive is reversible; permanent deletion remains separately labeled and
  confirmed.
- Keyboard, 360 px, compact desktop, light/dark theme, route, and service-restart
  regression tests pass.

## P1 — Personal automation, connectors, and remote continuity

These tracks may progress in parallel after the P0 task identity and route are
stable.

### Reviewed automation

- Save a successful conversation as a versioned recipe with an editable prompt,
  workspace, model policy, Agent Profile, Skills, connectors, context policy,
  output destination, and notification policy.
- Support manual test runs, one-time schedules, recurring schedules, pause,
  resume, duplicate, edit, and delete.
- Run schedules in the durable local A3S service, never in a browser timer.
- Record run history, inputs, resolved capability versions, outputs, receipts,
  duration, retry state, missed-run policy, and the conversation created for
  every execution.
- Bound concurrency, duration, retries, and catch-up behavior. Use idempotency
  keys so service restarts cannot silently duplicate consequential work.
- Revalidate workspace access, connector authorization, model availability,
  Skill versions, and required approvals before every run.
- Default recipes with writes, external sends, purchases, publication, or
  destructive actions to a reviewed draft or waiting-for-approval state.
- Add in-product completion, failure, and waiting-input notifications. Reuse the
  approved remote channel policy for optional delivery outside the browser.
- Add webhook and workspace-event triggers only after scheduled execution has a
  reliable audit, throttling, deduplication, and prompt-injection boundary.

Detailed Home integration remains aligned with
[Work AI-Native Home](docs/WORK_AI_NATIVE_HOME.md).

### Connector Center

- Replace raw integration configuration as the primary UX with a searchable
  catalog showing availability, source, trust, capabilities, authorization
  state, requested scopes, data destinations, health, last use, and revoke.
- Use MCP capability discovery where possible and typed A3S adapters where MCP
  cannot express lifecycle or safety requirements.
- Support local/custom MCP servers without requiring an A3S OS account.
- Enable connectors per task, recipe, Agent Profile, and Project; an installed
  connector is not automatically available everywhere.
- Start with generic email, calendar, and cloud document/drive workflows. Add
  meeting and issue-tracker connectors from demonstrated demand.
- Keep credentials out of browser state, logs, task exports, Skills, and shared
  project configuration. Distinguish personal authorization from future
  administrator-managed shared authorization.
- Treat connector content as untrusted data, preserve provenance, and require
  explicit confirmation for consequential external writes.

### Remote companion

- Complete the existing WeChat managed-session milestones for create,
  submit-or-queue, stop/cancel, recoverable archive, and coalesced
  completion/failure/waiting-input notifications.
- Keep mutating scopes local, disabled by default, previewed, expiring,
  idempotent, audited, and bound to configured workspace aliases.
- Evaluate remote `ApproveOnce` and `Deny` only through the separate post-v1
  threat-model gate. Never add remote `ApproveAlways` or arbitrary shell/tool
  execution.
- Extract a channel-neutral capability, identity, receipt, and policy contract
  before adding WeCom, Feishu, QQ, or DingTalk adapters.
- Keep platform credentials and protocol-specific identifiers behind the local
  service boundary.

The implementation source of truth remains the
[WeChat development plan](docs/WEIXIN_REMOTE_CONTROL_DEVELOPMENT_PLAN.md) and
[security and operations contract](docs/WEIXIN_REMOTE_CONTROL_OPERATIONS.md).

### P1 exit gate

- A reviewed recipe can survive a service restart, run once without duplication,
  create a normal conversation, and expose complete run history and recovery.
- Revoking a connector immediately removes it from new executions and yields a
  truthful blocked state for dependent recipes.
- External writes and remote mutations cannot bypass the same local permission,
  confirmation, idempotency, and audit rules used by an interactive task.

## P2 — Skill, Agent Profile, Agent Team, and recipe ecosystem

**Outcome:** users can discover and reuse expertise without understanding raw
package layout or rebuilding prompts for every task.

### Skill Center

- Unify installed Skills, verified A3S Use releases, and explicitly imported
  local Skills in one searchable catalog.
- Show source, publisher, signature, version, update state, permissions,
  dependencies, compatibility, risk scan, and enabled scope.
- Support install, update, rollback, enable/disable, and removal with a visible
  effect on live sessions and scheduled recipes.
- Add guided Skill creation that produces a reviewable local draft, tests it in
  a bounded workspace, and never publishes automatically.
- Preserve `$` as the explicit composer entry for Skills and allow intent-based
  suggestions only as editable, non-auto-installed recommendations.

### Agent Profiles and Teams

- Define a reusable Agent Profile with instructions, preferred model policy,
  allowed tools, Skills, connectors, workspace policy, memory policy, and
  presentation metadata.
- Do not store provider or connector secrets in a profile. Resolve authorization
  at execution time.
- Define an Agent Team as a versioned orchestration recipe over the existing
  runtime subagent model: named roles, task boundaries, dependencies,
  concurrency limits, handoff artifacts, stop conditions, and final owner.
- Show each member's assignment, progress, evidence, failure, and handoff in the
  independent conversation page. The product must not simulate multiple agents
  when the runtime did not create them.
- Support private local profiles and teams first; verified sharing belongs to
  A3S Use, and organization policy belongs to A3S OS.

### Inspiration and templates

- Add a curated gallery of versioned recipes for concrete outcomes such as
  research briefs, spreadsheet cleanup, document review, meeting follow-up, and
  repository maintenance.
- “Use template” must open an editable draft showing required files,
  capabilities, permissions, cost-bearing services, and expected outputs.
- Rank templates by verified compatibility and outcome quality, not only usage.
  Never auto-run a template or install hidden dependencies.

### P2 exit gate

- Every task and automation run records the exact Skill, profile, team, recipe,
  connector, and model-policy versions it resolved.
- Disable, rollback, uninstall, and trust-revocation paths have deterministic
  behavior for active sessions and future schedules.
- Imported or generated Skills cannot become verified or public without the
  signed A3S Use review lifecycle.

## P3 — Local Projects, handoff, and optional team collaboration

### Local Project

- Add a Project as a local container for one or more workspace roots, project
  instructions, Knowledge bases, Agent Profiles, Agent Teams, Skills,
  connectors, recipes, tasks, and reviewed artifacts.
- Keep project context explicit and inspectable before submission; apply bounded
  retrieval rather than injecting every project asset into every prompt.
- Add project templates, task grouping, activity history, and a portable export
  manifest with provenance and secret-free references.
- Add task handoff/export that packages selected messages, progress summary,
  artifacts, evidence, unresolved decisions, and required capabilities. Let the
  recipient review imports before creating a new local session.

### A3S OS collaboration

- Add opt-in account sync, project membership, invitations, RBAC, task sharing
  and transfer, comments, notifications, and shared cloud assets through A3S OS.
- Separate personal and shared connector authorization. Shared credentials need
  administrator policy, least privilege, audit, revocation, and a server-side
  secret store.
- Add public result links only through redacted, immutable, revocable snapshots;
  never expose a live local workspace or conversation database.
- Add hosted schedules and always-on assistants as OS-owned runs with explicit
  compute, data-region, retention, quota, and billing visibility.
- Preserve an offline local mode and deterministic conflict/recovery behavior.
  Real-time co-editing must not become a prerequisite for local Office editing.

### P3 exit gate

- Deleting or leaving a cloud project cannot delete a user's local source files.
- A shared task exposes only explicitly selected content and capabilities, with
  an auditable owner and revocation path.
- Local and cloud project states degrade truthfully during disconnection and do
  not silently merge incompatible instructions, assets, or permissions.

## P4 — Creative activities and cross-device extension

- Define a signed A3S Use activity contract for conversational design canvases,
  image generation, video generation, and future specialist editors.
- Require capability, provider, data-transfer, expected cost, watermark, and
  export-format disclosure before generation.
- Hand generated assets back to Work as ordinary, provenance-carrying files or
  managed artifacts that use the existing preview, version, review, and export
  flows.
- Keep source editing in the specialist activity and consequential file writes
  in Work's reviewed boundaries; a plugin cannot mutate another product
  silently.
- Add a mobile/PWA companion for notifications, bounded replies, approvals that
  pass the remote threat model, and artifact review. Full local file editing is
  not a mobile prerequisite.

## Explicit non-goals

- Hardcoding Tencent identity, QQ Mail, Tencent Docs, Tencent Meetings, TAPD,
  Tencent cloud drive, or any other vendor bundle into the A3S Web core.
- Requiring cloud login for local tasks, files, Knowledge, Memory, Skills, or
  schedules.
- Shipping an unverified Skill marketplace, silently installing dependencies,
  or auto-publishing generated Skills.
- Running schedules in browser timers or allowing unattended automation to
  inherit blanket permissions from an interactive session.
- Exposing arbitrary shell execution, permanent deletion, or permanent approval
  through a remote messaging channel.
- Building a second first-party Office suite, cloud drive, email client, or
  general-purpose design application when connectors or signed activities can
  provide the integration.
- Implementing multi-user collaboration inside the local single-user store.

## Cross-cutting release gates

Every roadmap capability must:

- keep service state authoritative and browser state recoverable;
- expose loading, empty, unavailable, denied, degraded, retry, and recovery
  states without optimistic false success;
- preserve provenance for context, connectors, recipes, Skills, agent members,
  artifacts, external writes, and remote commands;
- use bounded input, output, concurrency, retries, storage, and retention;
- keep secrets out of browser persistence, task exports, logs, telemetry, and
  model context;
- distinguish reversible archive from permanent deletion;
- pass keyboard, accessibility, 360 px, compact desktop, light/dark theme,
  localization, route, service-restart, and stale-response checks; and
- update the relevant product specification, architecture, domain model, user
  journey, README, and tests in the same implementation change.

## Reference baseline

WorkBuddy public product documentation reviewed on 2026-08-05:

- [Overview](https://www.workbuddy.cn/docs/workbuddy/Overview) and
  [changelog](https://www.workbuddy.cn/docs/workbuddy/Changelog)
- [Task management](https://www.workbuddy.cn/docs/workbuddy/Task-Management)
- [Automation](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Automation-Guide)
- [Connectors](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Connector)
- [Remote assistants](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Assistant)
- [Projects](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Project)
- [Experts](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Expert-Center)
  and
  [Skills](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Skills-Market)
- [Inspiration](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Ispiration)
  and
  [design ideas](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Design-Idea)
- [Agent mailbox](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Mailbox)
  and
  [Tencent Docs integration](https://www.workbuddy.cn/docs/workbuddy/From-Beginner-to-Expert-Guide/Function-Description/Knowledge-Base/Tencent-Doc)

Reference links document the analyzed behavior only. They do not create a
runtime dependency, compatibility commitment, or permission to copy WorkBuddy
assets, wording, protocols, or proprietary implementation.
