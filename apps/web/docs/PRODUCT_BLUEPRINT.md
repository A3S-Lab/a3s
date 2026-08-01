# A3S Web Product Blueprint

## Product promise

A3S Web is a desktop, local-first AI workspace that lets a user start with an
outcome, work directly with real files, supervise execution, and retain control
of every meaningful change.

The product is not a generic chat client and does not make Office and coding
compete as separate destinations. Work at `#home` is the single task surface;
file type and user intent determine the scene.

## Product principles

1. **Outcome first.** The home composer asks what the user wants to accomplish.
2. **One work identity.** Home, files, Office, code, and AI use one conversation
   list and one active draft.
3. **Real files remain visible.** Local paths and source fingerprints are
   user-facing identities, not implementation details hidden behind uploads.
4. **Review before consequence.** Sensitive tool use, destructive filesystem
   actions, compatibility loss, and structured AI changes require explicit
   review.
5. **Inline for routine work.** Naming, rename, and reversible row actions stay
   in context; dialogs are reserved for real risk.
6. **Evidence over confidence.** Delivery separates passed, pending, failed,
   and residual-risk checks.
7. **Local by default.** The app remains useful without an A3S OS account and
   keeps local state recoverable during disconnection.

## Primary journey

```text
Open #home
→ describe an outcome, resume a conversation, or open a file
→ inspect and select context
→ submit through the shared composer
→ supervise execution and decisions in the AI Assistant
→ open resulting files in the applicable Work scene
→ edit, review, save, and validate
→ optionally package selected sources into Knowledge
```

## Information architecture

### Activity Bar

- Work;
- Knowledge;
- enabled A3S Use activities;
- Memory;
- Plugin Marketplace;
- Settings.

There is no separate Code or Office entry.

### Work sidebar

The left sidebar is the unified conversation list:

- new conversation;
- search;
- selected conversation;
- inline rename and delete;
- truthful running state.

Historical sessions remain visible regardless of which earlier product created
them. New sessions use the default agent.

### Work center

The center renders one scene at a time:

- AI-native home and managed artifact library;
- local file manager;
- Office/PDF editor;
- code/text/Markdown editor.

The active scene is local UI state. It does not change the canonical Work route
or conversation identity.

### AI Assistant

The resizable left pane displays the active conversation, execution, tool
decisions, recovery, evidence, artifacts, and the same composer used by Home;
the files, Office, or code workspace remains on its right. On compact desktop
widths it becomes an overlay.

## Home

The home page follows a task-first hierarchy:

1. concise Work identity and outcome promise;
2. production task composer;
3. implemented capability shortcuts;
4. templates, folders, and recent files.

The assistant starts open beside the workspace; an explicit close choice is
persisted. Prompt starters populate an editable draft and never auto-send.

## File management

The file manager follows familiar Finder and Windows Explorer behavior:

- real folder selection and persisted workspace;
- back, forward, up, breadcrumbs, grid/list, filtering, sorting, and search;
- marquee, checkbox, additive, range, and keyboard selection;
- copy, cut, collision-safe paste, drag and drop, favorites, reveal, and Quick
  Look;
- background and item context menus;
- inline create, rename, and duplicate naming;
- a stable selection action shelf;
- permanent deletion confirmation.

The empty-space context menu is a first-class folder command surface. It must
not act on a stale item selection.

## Editing

### Code, text, and Markdown

Opening a non-binary text file enters the Work code scene with a lazy explorer,
multiple Monaco tabs, language services, diagnostics, semantic navigation,
safe saving, and external-change conflict review. Markdown shows source and
live preview together.

### Office and PDF

DOCX, spreadsheet, presentation, and PDF handlers use consistent Work chrome,
format-specific editing, explicit save-back, compatibility review, preview,
print, and export flows. Heavy dependencies load only when needed.

### AI proposals

Editor context actions add bounded selected content to an editable request.
Actionable AI output uses a structured proposal with trusted before/after
values and per-target approval. A stale target is skipped rather than
overwritten.

## Delivery and review

Execution stays inside the active conversation. Tool calls show requested
scope, permission state, progress, output, and recovery. Completed file entries
open through Work's current file handlers.

The delivery summary reports verification evidence. “Prepare review” adds an
evidence-based review request to the same conversation; it never navigates to a
deleted task workspace.

## Knowledge handoff

One or many selected files and folders can create a managed knowledge base.
Creation records and copies the source package but does not compile it.

Compilation is a second action:

- explicit user request; or
- opt-in automatic policy after stability, quiet-window, minimum-interval,
  retry, and bulk-change safeguards pass.

Only a successfully promoted OKF wiki generation becomes searchable.

## Memory

Memory is a standalone system surface at `#memory`. It loads the complete local
store for search, filters, timeline, and bounded graph projection. Entering it
preserves the current Work conversation and draft.

## Plugins

Verified A3S Use packages can contribute Activity Bar views. A contribution is
isolated and can propose reviewed context and an optional Skill back to the
current Work draft. Unknown or disabled keys recover safely.

## Settings and Help

Settings is a global modal with Account, Appearance, Model, Agent, Context,
Integrations, Channels, About, and Help. Canonical channel routes are nested
under `#settings/channels/`.

## Success signals

- time from opening `#home` to a meaningful action;
- successful conversation continuation across scenes;
- file operation completion and recovery rate;
- rate of AI context requests that remain within selected scope;
- proposal approval, stale-target, and rollback outcomes;
- knowledge creation-to-compile conversion;
- route, keyboard, narrow-width, and dark-theme regressions.

## Release boundary

The release excludes:

- separate Code or Office products;
- legacy product-prefixed Work routes;
- silent destructive organization;
- automatic publication of memory or failed knowledge generations;
- controls whose destination or recovery path is not mounted.
