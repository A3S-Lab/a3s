# Product

<!-- impeccable:product-schema 1 -->

This record is derived from the explicit product objective and the checked-in
A3S Web product, architecture, design, journey, and roadmap documents. No
commercial claims, customer segments, or deployment promises beyond those
sources are assumed.

## Platform

web

## Users

The primary user is an individual knowledge worker, maker, researcher, or
developer working on a desktop with real local folders and durable AI tasks.
They need to move between conversation, files, Office documents, code, evidence,
and review without losing task identity or surrendering control of consequential
changes.

Team members and mobile users are future audiences served through optional A3S
OS collaboration and companion surfaces; they are not prerequisites for the
local product.

## Product Purpose

A3S Web turns an intended outcome and real local files into reviewed, editable,
and deliverable work. Success means a user can start or resume a task quickly,
understand what the agent is doing, inspect evidence and artifacts, intervene
when necessary, and continue working in the appropriate file scene without
losing conversation context.

The immediate product objective is to make the task journey conversation-first:
submitting from Home opens an independent conversation page instead of revealing
a right- or left-side chat panel.

## Positioning

A3S Web combines a local-first agent runtime with direct access to real files,
native document and code editing, explicit permission and proposal review,
local Knowledge and Memory, and signed capability extension. The browser is a
supervisory and editing workspace over durable local service state, not a thin
generic chat client or a cloud-only Office clone.

## Operating Context

- Work at `#home` is the default entry for a new outcome.
- Each distinct outcome has one durable session, conversation, draft, queue,
  workspace snapshot, and execution history.
- A submitted Home task continues on a dedicated, reload-safe conversation
  route. Files, Office/PDF, and code remain separate Work scenes associated with
  that same task.
- The contextual AI pane is appropriate when a document or file is the primary
  object. It is not the destination for a Home submission.
- Local filesystem paths remain visible user identities and local service state
  remains authoritative across refresh, reconnection, and stale browser work.
- Sensitive tools, destructive file actions, external writes, compatibility
  loss, and structured AI changes require bounded review.

## Capabilities and Constraints

- Current capabilities include durable conversations, streaming execution,
  plans, queues, interruption, permission decisions, runtime subagents, local
  filesystem management, Monaco editing, DOCX/XLSX/PPTX/PDF workflows,
  structured proposals, live preview, Knowledge, Memory, signed A3S Use packages,
  MCP configuration, and a security-gated WeChat channel.
- `#home` remains the landing and new-task route. An independent
  `#conversation/<opaque-session-key>` route is the canonical destination for a
  durable task conversation.
- Home submission must preserve the draft and context on failure, and must
  navigate only after a durable session exists. Reload, deep-link, missing
  session, archive, and service recovery require truthful states.
- Conversation routing must reuse the existing unified session catalog and
  controller. It must not create a second store, hidden conversation, or retired
  Result Workspace.
- A3S Web and the local service own personal work and offline recovery. A3S Use
  owns signed capability distribution. A3S OS owns optional identity, sharing,
  hosted execution, and multi-user collaboration.
- Core local work must remain useful without an A3S OS account.
- Vendor-specific ecosystems are integrated through reviewed connectors or
  signed activities rather than hardcoded into the core shell.
- The existing React, Valtio, typed local API, semantic-token, Lucide, and
  app-local Bun/Vitest/Biome stack remains authoritative.

## Brand Commitments

- The product name is A3S Web and the default workbench is Work.
- Preserve the authoritative A3S logo at `public/logo.png`.
- Product language is calm, direct, trustworthy, and outcome-oriented. It must
  not imitate promotional reward systems or fabricate capabilities.
- The established semantic design tokens, light/dark themes, system UI type,
  Lucide icon language, and restrained A3S blue focus accent remain the visual
  authority for this work.

## Evidence on Hand

- Product direction and current behavior: `README.md`, `ROADMAP.md`,
  `docs/PRODUCT_BLUEPRINT.md`, `docs/SUPER_APP.md`, and
  `docs/WORK_AI_NATIVE_HOME.md`.
- Architecture and state boundaries: `docs/PRODUCT_ARCHITECTURE.md`,
  `docs/DOMAIN_MODEL.md`, and the current `src/state`, `src/features/tasks`, and
  `src/features/work` implementations.
- Experience and visual rules: `DESIGN.md`, `docs/USER_JOURNEYS.md`, and the
  semantic tokens and feature styles under `src/styles`.
- Verification evidence must come from repository tests, builds, and rendered
  desktop/mobile screenshots. No customer testimonials, usage benchmarks, or
  commercial proof assets are currently established and future work must not
  fabricate them.

## Product Principles

1. **Conversation is a place, not a panel.** A task receives a stable page and
   identity; contextual assistant panes remain subordinate to the active file.
2. **One outcome, one durable state.** Routes and scenes project one session,
   draft, queue, workspace, and evidence history rather than forking them.
3. **Real work stays inspectable.** Files, paths, context, plans, tool scope,
   artifacts, proposals, and verification remain visible and actionable.
4. **Review before consequence.** Risk, external effects, destructive actions,
   and stale targets stop for explicit human control.
5. **Local first, extensible by contract.** The core remains useful offline;
   connectors, Skills, specialist activities, and cloud collaboration join
   through typed, bounded ownership layers.

## Accessibility & Inclusion

Core flows must support keyboard and pointer parity, visible focus, semantic
landmarks and labels, non-color status cues, reduced motion, light and dark
themes, Chinese and English content growth, compact desktop layouts, and a
minimum 320 px viewport without horizontal page overflow. Loading, empty,
denied, missing, degraded, retry, and recovery states must be understandable
without relying on animation alone.
