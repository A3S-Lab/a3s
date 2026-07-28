# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.11.0] - 2026-07-29

### Changed

- Moved the `a3s` CLI package, tests, skills, and release automation from the
  former `A3S-Lab/CLI` repository into the `A3S-Lab/a3s` repository root.
- Moved GitHub releases, standalone installers, self-update downloads, and the
  Homebrew formula source to `A3S-Lab/a3s`.

## [0.10.14] - 2026-07-29

### Fixed

- Kept PDF artifacts at the full Live Preview panel height by propagating the
  definite document height through the EmbedPDF renderer chain instead of
  collapsing the page viewport to its toolbar.

## [0.10.13] - 2026-07-28

### Changed

- Updated the shared Code runtime used by the CLI, TUI, and Web adapter to
  `a3s-code-core` 6.5.2, including strict built-in tool contracts, root JSON
  Schema value support, governed generation timeouts, and typed transport
  failures.

## [0.10.12] - 2026-07-28

### Added

- Added a persistent, resizable A3S Work live-preview panel for static sites,
  loopback development servers, text, images, PDFs, and supported Office files.
  Static sites reload after debounced workspace changes and provide responsive
  device, zoom, refresh, pause, and external-window controls.
- Added `/preview <path|localhost-url>`, `/preview status`, and `/preview stop`
  to the Code TUI. The workspace IDE also opens the selected item with `p`, and
  `:preview` saves a dirty editor buffer before opening it.

### Security

- Confined file previews to canonical active-workspace paths, blocked traversal,
  hidden and sensitive files, escaping symbolic links, and oversized assets,
  and limited URL previews to HTTP(S) loopback hosts. Static HTML runs in an
  opaque-origin CSP sandbox, and preview descriptors and content now expire at
  their declared deadline.
- Kept native preview-window ownership separate from the shared Web process.
  Preview replacement, stop, and TUI exit close only the exact tracked window,
  and the window receives no A3S OS access or refresh token.

### Fixed

- Kept structured generation compatible with Kimi thinking-only models by
  advertising prompt-and-schema fallback instead of an unsupported forced tool
  choice; non-thinking Kimi routes retain native structured-output support.

## [0.10.11] - 2026-07-28

### Changed

- Updated the shared Code runtime used by both the TUI and Web adapter to
  `a3s-code-core` 6.5.1.
- Local Bash now uses the active workspace's host command runner behind one
  shared Rust guardrail. Default silently admits only commands proven read-only
  and asks for unproven host work; Plan denies Bash; Auto admits the proven
  read-only subset and denies everything else without entering HITL.

### Removed

- Removed the Anthropic managed SRT bootstrap, capability probe, packaged
  support tree, native sandbox prerequisites, and release/update plumbing.
- Standalone installations from 0.9.9 through 0.10.10 must install the first
  SRT-free release once from its GitHub archive because those legacy updaters
  require the deleted support payload. Homebrew upgrades are unaffected.

## [0.10.10] - 2026-07-28

### Changed

- Made Work the single AI-native browser workspace. Coding, Office editing,
  files, and AI conversations now share one session list, composer, workspace
  selector, model controls, queue, and artifact flow instead of separate Code
  and Office products.
- Rebuilt the Work home around a task-first entry point with document,
  spreadsheet, presentation, file, and analysis shortcuts plus recent files
  and reusable creation templates.
- Standardized browser navigation on `#home` and `#memory`; retired Code,
  Work-prefixed, Help, and Weixin route aliases now return to the canonical
  Work home rather than mounting legacy interfaces.

### Fixed

- Reconnected quick open and generated artifact actions to the unified Work
  editors, including code/text and Office/PDF targets.
- Removed an unintended horizontal scrollbar caused by the Work home hero's
  decorative background at desktop widths.

## [0.10.9] - 2026-07-26

### Changed

- Updated the shared CLI, TUI, and Web adapter to `a3s-deep-research` 0.1.3.
  Reports are pinned to the user's language, comprehensive publication now
  requires a multi-source analytical chain in every resolved dimension, and a
  fully bounded run may qualify one deeply analyzed partial conclusion only
  while retaining an explicit typed gap.
- Added normalized direct retrieval for up to three explicit HTTP(S) references
  in a Web-capable query, a shared synthesis track for named comparisons, and
  plan-wide validation of required question roles.
- Unified headless CLI, TUI, and Code Web on one typed runner with isolated
  read-only sessions, run-scoped artifacts, journal-backed refresh, settled
  cancellation, and report delivery by validated run ID instead of filesystem
  path.
- Standardized single-file reports on the A3S Web design system with a left
  action menu, centered editable report, right table of contents, save, print,
  and responsive controls. Natural narrative planning produces continuous
  prose while preserving closed evidence and traceability.
- Removed the bundled `report-master` Skill and pruned its retired managed
  directory from the CLI-owned Skill root.

### Fixed

- Bounded long Codex provider session and prompt-cache identifiers while
  sharing a proven HTTPS fallback across isolated research child sessions.

## [0.10.8] - 2026-07-26

### Added

- Added complete blank-area and item context menus to the Work file manager,
  including keyboard invocation, copy, cut, collision-safe paste, view and sort
  controls, path actions, OS reveal, and selection-aware AI actions.
- Added mixed file-and-folder source packages for personal knowledge bases.
  Creation now materializes only `sources/`; compilation is a separate manual
  action or an opt-in smart-automatic policy.
- Added a durable independent-compiler protocol with manual-priority claiming,
  one global running job, cancellation, bounded retries, staged OKF output, and
  atomic `wiki/` promotion.

### Changed

- Made knowledge creation and import non-modal, exposed compilation state and
  policy controls on the Knowledge page, and limited search publication to
  successfully compiled `wiki/**/*.md` content.
- Debounced smart-automatic compilation until files are stable for 5 seconds
  and sources are quiet for 30 seconds, with a 10-minute minimum interval and
  safety pauses for suspicious bulk changes.
- Updated the bundled A3S Web source to the responsive file and knowledge
  workflow release.

### Fixed

- Kept mixed-selection context-menu wording aligned with the complete selected
  set and made the inline knowledge composer reflow around an open Work AI
  panel.
- Prevented policy changes from manufacturing an unchanged source generation,
  and tracked compiler identity separately from the compilation protocol
  version.

## [0.10.7] - 2026-07-26

### Added

- Added Finder- and Explorer-style mouse marquee, Command/Ctrl additive
  selection, Shift range selection, checkboxes, Select All, sortable list
  headers, and a non-displacing selection action shelf to the Work file
  manager.
- Documented the AI-native local full-text retrieval architecture for compiled
  OKF knowledge, with Tantivy indexing and explicit separation from Office,
  PDF, OCR, and other knowledge-compilation concerns.

### Changed

- Unified the Code, Work, and Knowledge sidebars around the same dimensions,
  navigation rows, framed icon grammar, stroke weight, and responsive behavior.
- Kept folder creation, Office-file naming, rename, and duplication inline in
  both grid and list views; only irreversible permanent deletion opens a
  confirmation dialog.
- Made the Work assistant cover the full Work pane at 1120 px and below, and
  excluded generated dependency and build trees from workspace search while
  retaining compiled `.a3s/kb` knowledge.

## [0.10.6] - 2026-07-25

### Fixed

- Cleared the legacy anonymous Code draft once when upgrading A3S Web so text
  previously copied from the Work assistant no longer reappears in the Code
  composer.
- Kept Code Auto mode direct when structured pre-analysis is unavailable
  instead of presenting locally fabricated task steps.
- Preserved the reader's paused transcript position while Markdown output is
  streaming, including brief layout shrinkage that places the viewport at the
  current bottom.

## [0.10.5] - 2026-07-25

### Added

- Render successful A3S Web file edits as inline unified diffs with added and
  deleted counts, old and new line numbers, and full red/green row backgrounds.

### Changed

- Updated the TUI research adapter to `a3s-deep-research` 0.1.2. Malformed
  bootstrap and planned envelopes now degrade independently through closed
  stage and packet-version state; raw bootstrap bytes remain audit-only, and
  the resulting publication receipt and journal projection stay consistent.
- Simplified A3S Web settings around one larger dialog and one Provider
  list/detail flow, removed the connector-specific editor, and removed the
  redundant framed container from channel details.
- Added a short fade only to newly streamed Markdown words and changed response
  following to an interruptible frame-based scroll that stops when the reader
  moves away and respects reduced-motion preferences.

### Fixed

- Recover an exact run-scoped DeepResearch publication immediately when the
  publication port committed its artifacts and receipt before returning an
  error. CLI, TUI, smoke, convergence, and terminal settlement now share the
  same receipt-backed resolver, and source-backed quality metrics survive
  journal replay without inspecting result prose. Receipt quality uses an
  independent publication event stream, so TUI pre-settlement evidence cannot
  collide with terminal projection.
- Aligned A3S Web execution modes with the Code TUI: Default uses managed SRT
  for ordinary shell work, Plan remains read-only, and Auto denies unavailable
  sandbox or host-boundary execution instead of opening a permission prompt.
- Kept the Web task-progress surface hidden until a real plan or subagent
  lifecycle exists, repaired compact streamed Markdown tables, stopped
  inserting the old starter instruction into new Code tasks, and left-aligned
  Office and Knowledge quick-create menus.
- Kept the Code TUI transcript following model output smoothly without
  suppressing a reader's intentional scroll position.

## [0.10.4] - 2026-07-23

### Changed

- Updated the managed local sandbox to sandbox-runtime 0.0.67 and A3S Code
  Core 6.4.2.

### Fixed

- Passed generated macOS Seatbelt profiles through private mode-0600 files
  instead of the process argument vector, preventing `spawn E2BIG` when a
  workspace requires many credential or hard-link rules.
- Kept managed-runtime temporary files inside the command's private scratch
  directory so profile-file cleanup follows the command lifecycle.

## [0.10.3] - 2026-07-23

### Fixed

- Trusted the official A3S tap before release smoke installation so current
  Homebrew versions can resolve the tap-owned `a3s-webview` dependency.
- Updated Core so the managed SRT capability handshake tolerates files and
  directories removed concurrently by other workspace processes.
- Included the complete causal error chain in local-sandbox startup warnings
  instead of hiding the actionable workspace or platform failure.

## [0.10.2] - 2026-07-23

### Fixed

- Restored Homebrew-rewritten Node shebangs after formula cleanup, verified the
  complete managed SRT tree in `brew test`, and added release smoke coverage
  for both a fresh install and a same-version reinstall.

## [0.10.1] - 2026-07-23

### Added

- Added a one-shot DeepResearch mode to the A3S Web Code composer. Queued turns
  retain the selected mode, execute through the shared host-managed research
  runtime, support cancellation and progress streaming, and expose admitted
  Markdown and sandboxed HTML report artifacts through the session workspace.

### Changed

- Changed the default `web_search` engines to DuckDuckGo and Wikipedia.
  AnySearch is now opt-in through `config.acl`, while an explicit request or
  ACL engine selection remains authoritative.

### Fixed

- Applied one provider-neutral fallback policy to search failures and empty
  results. AnySearch quota exhaustion now appears as a structured
  `provider_quota` diagnostic before search automatically tries eligible
  fallback engines.
- Strengthened DeepResearch outcome extraction, accountable-source admission,
  citation gates, and report publication so time-sensitive result questions
  produce a direct source-backed answer or an honest degraded artifact.
- Removed the legacy A3S Web Code draft that preselected `CLAUDE.md` or
  `CLAUD.md` and inserted the Chinese “inspect the current code file” prompt,
  without deleting ordinary saved user drafts.

## [0.10.0] - 2026-07-22

### Fixed

- Resolved user-scoped configuration and component paths through native Windows
  profile variables when `HOME` is absent, so commands such as
  `a3s install webview` work in PowerShell and clean Windows CI environments.
- Pinned managed sandbox release verification to the x64 macOS 15 runner that
  still provides Seatbelt, explicitly provisioned its ripgrep dependency, and
  added pull-request coverage for both packaged and first-use behavior. The
  first-use compatibility patcher now also recognizes macOS's symlinked
  temporary-directory paths and ships inside the crates.io package.
- Restored executable permissions for the managed sandbox CLI and native
  seccomp helpers after GitHub artifact transport, and now reject release
  archives that contain non-executable Unix sandbox components.
- Ordered mandatory Linux sandbox child mounts before A3S's stricter parent
  denies and collapsed mounts that converge on the same missing ancestor,
  preventing bubblewrap startup failures in both bundled and first-use managed
  sandbox installations. The runtime's immutable seccomp helper also remains
  readable when the surrounding user home is hidden, without weakening either
  policy boundary.
- Made local Claude Code, Codex, Kimi Code, and WorkBuddy account discovery
  fall back to the native Windows user profile when `HOME` is unset. WorkBuddy
  now locates its bundled CodeBuddy CLI through standard Windows installation
  directories and registered uninstall metadata, including custom install
  locations.
- Made official installers accept both legacy archives and complete 0.10
  bundles. Web assets, the WebView companion, and managed sandbox support now
  activate as one rollback-safe installation, while transient Windows download
  failures receive bounded retries.

## [0.9.9] - 2026-07-22

### Added

- Bundled the unified A3S Web Work workspace with Finder-style local file
  management, independent Chinese AI-assistant sessions, full WebIDE editing,
  split Markdown preview, and aligned document, spreadsheet, presentation, and
  PDF editing surfaces backed by PDFium.
- Integrated the convergent DeepResearch runtime, managed Web lifecycle, and
  projected A3S Use activity and plugin capabilities prepared on the release
  branch.

### Fixed

- Preserved streamed file response bodies in the Web API gateway instead of
  interpreting PDF and other ordinary binary streams as server-sent events.
- Bundled A3S WebView 0.1.5 so Agent Island keeps notch-safe placement, native
  dragging, and the user's chosen position through expand and collapse.
- Made release archives resolve their standalone dependency graph directly
  from published A3S crates instead of reading a missing transient manifest.

## [0.9.8] - 2026-07-21

### Changed

- Updated the bundled Code runtime to A3S Code Core 6.1.0 and A3S Search 2.0.0.
  `web_search` now uses AnySearch by default when neither the request nor ACL
  configuration selects engines, while explicit AnySearch, Tavily, and
  conventional engine selections remain available.

### Fixed

- Kept raw A3S Use MCP tools hidden from the primary Code model while allowing
  the dedicated Use worker to execute its exact built-in Browser, Office, and
  OCR surface. Provider installers and newly projected extension tools now
  retain `Ask` policy and settle through the parent TUI confirmation flow.
- Added a real Windows Code TUI-to-Use regression covering the verified Use ZIP
  layout, all 31 Browser core tools with Microsoft Edge, every native Office
  operation and view, confirmed OfficeCLI installation, and confirmed local
  PP-OCRv6 installation and extraction.
- Registered `a3s-webview` as a verified first-use component and made
  `a3s code` install its platform release before terminal takeover when policy
  permits. The managed path is passed directly to RemoteUI and Agent Island,
  Windows assets use their Rust target ZIP names and protocol-aware health
  probe, and a release-page/checksum fallback survives GitHub API rate limits.
- Increased the Windows main-thread stack reserve for the `a3s` executable so
  `a3s code` can complete TUI session startup without a stack overflow, with a
  Windows-only startup smoke test covering the regression.

## [0.9.7] - 2026-07-19

### Added

- Added local Memory Evolution for LLM-authored durable-memory signals. Code
  accepts only the validated `a3s.evolution.signal.v1` metadata emitted by
  Core memory extraction; it does not infer reusable preferences, Skills, or
  OKF packages from keyword overlap. Candidates retain evidence and maturity,
  materialize as workspace-local versioned assets, preserve recovery copies
  during rollback, and never publish automatically. Conflict-free candidates
  that reach the stricter recurrence, session, confidence, importance, and
  explicit-signal thresholds materialize locally without another prompt.
  `/evolution` provides TUI review, while `/api/v1/evolution` exposes the same
  scan, review, materialize, reject, reopen, and rollback lifecycle to Code
  Web. Baseline rollback removes the active asset while preserving immutable
  versions and a recovery copy. Active Preference instructions enter bounded
  TUI and Web prompt context, active Skills enter the session registry, and an
  activation barrier remains pending until every affected live session
  refreshes successfully.
- Added the Core local workspace credential boundary to every Code TUI
  manifest backend. Built-in reads, range reads, writes, edits, patches, and
  grep now enforce the same sensitive-file and source-hardlink rules as
  sandboxed Bash without blocking ordinary package-store hardlinks. Read-only
  Git diff filters through the same path policy, option-like revisions cannot
  become Git flags, and remote display removes embedded URL credentials and
  query tokens.
- Added a managed local command sandbox lifecycle for Code TUI startup.
  Official CLI archives and Homebrew now carry the exact support tree, whose
  package graph, file types, size bounds, and complete normalized digest are
  verified before use. Standalone self-update replaces that tree
  transactionally with the CLI. Offline mode and `A3S_NO_AUTO_INSTALL=1` can
  use the release payload without mutation. Source and Cargo installs retain a
  fixed, official-registry npm bootstrap with lifecycle scripts disabled.
  Setup failure is non-fatal, Default can request one exact host command, and
  Auto denies Bash without opening HITL.
- Added `/checkup` as a host-owned, read-only context-hygiene review. It scans
  at most 128 persisted local sessions for actual `Skill` tool invocations and
  reports bounded invocation/session counts and context bytes. Cleanup
  suggestions require at least three sampled sessions, twelve completed turns,
  and a Skill older than 14 days. Recently changed, disabled, managed,
  duplicate-name, and unknown-age Skills are excluded. Instruction and MCP
  counts remain footprint signals rather than invented usage telemetry. The
  composer stays locked during collection, raw session contents and MCP errors
  do not enter the prompt, and a host-enforced Plan turn offers each eligible
  Skill as a separate reversible `/plugin` disable choice. No file is deleted
  and no state changes before the existing Approve / Revise / Abandon boundary
  and normal HITL.
- Added `/permissions` as a searchable exact-grant inspector. Session and
  project grants remain visibly distinct, Enter opens their canonical
  arguments, and `X` or Delete requires a second matching confirmation before
  revocation. Both scopes stop authorizing new calls immediately; project
  removal then atomically rewrites `.a3s/permissions.acl` through `a3s-acl` and
  restores the in-memory grant if persistence fails. Revocation affects future
  checks and never claims to cancel an already-running tool.
- Added reliable TUI session sharing. `/copy` copies the latest assistant
  source Markdown, `/copy transcript` requests the complete semantic session
  through the native clipboard or bounded OSC 52 path, and `/export [path]`
  atomically creates a private no-clobber Markdown snapshot inside the current
  workspace. Exports preserve visible messages, tools, and delegated results
  while excluding private reasoning, transient terminal chrome, and hidden
  duplicate cells.
- Added `/history` and `Ctrl+R` prompt-history search. The non-blocking modal
  preserves the current draft, ranks fuzzy matches by relevance and recency,
  retains duplicate prompts as distinct historical positions, caps rendering
  at 100 results, supports keyboard and mouse navigation, cycles matches with
  repeated Ctrl+R, and restores a selected prompt with Enter or Tab.
- Added `/tasks` and `Ctrl+B` as a live delegated-work control panel. It reads
  authoritative Core task snapshots without interrupting the parent turn,
  retains running tasks plus bounded recent history, preserves semantic
  selection across one-second refreshes, searches progress and output, opens
  full task details, and requires a second matching action before invoking real
  subagent cancellation.
- Added a `/queue` modal for pending TUI follow-ups. It preserves Lane
  priority/FIFO metadata while selecting by stable sequence, shows each
  submission-time execution mode, sends the exact selected row now without
  losing attachments or Plan state, removes individual rows, and requires a
  second explicit decision before clearing all pending work. Enter on an empty
  composer now sends the current queue head immediately during a live turn.
- Added policy-aware A3S Use preparation to Code TUI startup. A missing Use
  component is installed from its verified release before terminal takeover
  when networking and automatic setup are allowed; offline mode and
  `A3S_NO_AUTO_INSTALL=1` remain strict zero-network, zero-receipt boundaries.
  Browser, native Office, built-in OCR, and verified external MCP/Skill
  surfaces are then projected into the dedicated restricted `use` worker.
- Added conflict-safe TUI recovery workflows. `/fork worktree` creates an
  isolated sibling branch/worktree, transfers tracked and untracked workspace
  content without changing the real Git index, and copies the complete session
  plus TUI sidecar. Ordinary prompts retain bounded pre/post Git tree
  checkpoints, and `/rewind` forks the pre-turn conversation before reversing
  a whole-turn patch only when conflict checks pass.
- Added pinned-root TUF registries for external Use packages, including full
  metadata refresh, review/apply-bound install plans, signed provenance
  receipts, and source-preserving upgrades. Registry upgrades query only the
  recorded registry and channel, reject identity drift and version downgrades,
  and converge without downloading an already installed target.
- Added durable journals for multi-component install, upgrade, and uninstall
  batches. A global cross-process lock protects the active journal, successful
  checkpoints resume only while their receipt and health state still match,
  interrupted records remain inspectable, and recovered JSON operations are
  marked explicitly without changing partial-success semantics.
- Added a minimal host-managed DeepResearch pipeline: one LLM-authored semantic
  plan commits stable research obligations and observable stop conditions, an
  initial bounded retrieval pass gathers evidence, closed semantic reduction
  returns chunk IDs plus typed source-to-obligation coverage, and one optional
  supplemental pass fetches at most two remaining candidates beyond the eight
  initial web-fetch slots when typed criterion/source-role coverage or an
  operational fetch/source-selection gap remains. Provider queries are never
  regenerated or rewritten. Questions linked to the same research obligation
  share one durable closed-evidence review, and one Host-derived typed contract
  assessment gates the report.
  Active execution has no scout, perspective, maker/checker, query-generating
  follow-up wave, or adaptive-route branch.
- DeepResearch now inherits the active `web_search` engine selection from
  `config.acl`. The built-in default is DuckDuckGo plus Wikipedia; AnySearch
  and Tavily are opt-in. Structured provider failures and empty-result cases
  use the same generic fallback policy, while quota and other degradation
  notices remain visible in partial research metadata.
- Added automatic per-run Loop Engineering contracts to DeepResearch. The
  transient durable input declares `quota.mode = bounded`, an immutable
  `evidence-first-deep-research` stage graph, and fixed cardinality for outline,
  retrieval, extraction, reduction, report generation, and deterministic
  publication. Rust Inquiry validates it and remains terminal authority. It
  does not create `.a3s/loops/` assets or restore maker/checker loops, while
  per-call deadlines, output limits, concurrency ceilings, and closed-catalog
  bounds remain independent safety fuses.
- Added typed DeepResearch contract assessment and report-stage Inquiry
  events. Obligations, questions, accepted evidence, outline sections, drafts,
  audit results, and their graph relations are strictly replayable through the
  A3S Code state-graph runtime.
- Added signed-in Kimi account models to `/model`, `a3s model`, and
  `a3s code models`. A3S prefers Kimi Desktop's local Daimon account and falls
  back to Kimi Code OAuth, discovers account-enabled model/context metadata,
  and keeps credentials in Kimi-owned state instead of A3S configuration,
  output, or logs. OAuth refresh uses a file lock and atomic credential
  rotation, while model responses preserve native A3S host-tool execution.
- Added a native system-agent island to `a3s code`. Fresh per-user
  heartbeats provide exact A3S parent/subagent lifecycle from cooperating
  `a3s code` TUI processes; the shared `a3s top` process collector supplies
  explicitly inferred fallbacks for Claude Code, Codex, Cursor, Gemini, and
  WorkBuddy. Heartbeats persist only a sanitized workspace basename and redact
  parent and child task descriptions unless
  `A3S_AGENT_STATUS_SHARE_TASKS=1` explicitly enables local sharing. The CLI
  atomically exports the same bounded, sanitized evidence to a private snapshot
  and best-effort launches `a3s-webview --agent-island` after either a fresh
  exact non-idle lifecycle or a recognized coding-agent process appears.
  Process-only observations count as running evidence, keep the surface alive,
  and enable its multicolor breathing border while remaining explicitly
  inferred and control-free. Every TUI shares the same per-user snapshot and
  singleton lock, so concurrent `a3s code` instances render only one island.
  The helper owns the independent desktop window, requested at screen top
  center, and reserves transparent glow bleed around its rounded surface. On
  macOS it reads the display safe areas, joins a centered notch at the physical
  top edge, and places compact content in the two safe side wings. A dedicated
  handle starts native window dragging; successful manual placement disables
  periodic recentering, and expand/collapse preserves the moved top-center.
  Exact HITL rows now display the bounded approval reason, use larger action
  buttons, and accept a direct bounded reply through the existing private,
  one-shot control queue. An approval-time reply queues a normal follow-up
  without implicitly approving or denying the tool. The island never overlays
  or intercepts input inside the terminal.

### Fixed

- Prevented weak or stale web evidence from being presented as a successful
  DeepResearch answer. Web acquisition now keeps the exact query and adds only
  one deterministic, current-date outcome-and-news companion query, using
  localized Han terms and an English fallback otherwise. A valid non-empty
  semantic selection is now supplemented, within the existing fetch cap, by
  distinct-host verified institutions and accountable publishers so a narrow
  model selection cannot starve the evidence set. The
  source-admission model gets one 60-second attempt inside a 150-second
  discovery-and-fetch stage. A failed or timed-out admission now degrades only
  acquisition to a six-source, cross-query and distinct-host fallback. That
  fallback ranks verified institutions and accountable publishers before
  unknown, social, or protected-publisher lookalike hosts, records its
  provenance, prefers outcome-bearing retrieval opportunities within the same
  trust tier, and requires deterministic query, publisher-accountability, and
  publication gates; it no longer claims a post-fetch semantic selector that
  the active evidence-first path does not run. Protected publisher names on an
  unrelated registrable domain cannot support conclusions even if semantic
  admission succeeded.
  Escaped hydration payloads, serialized application state, dense JavaScript,
  template expressions, image syntax, and inline transport URLs are removed
  before chunking and again at publication while visible link labels survive,
  and
  self-published or otherwise ineligible sources are visibly marked as unusable
  for conclusions.
  Self-publishing disclaimers, community and streaming hosts, and earlier-stage
  snapshots cannot qualify a current core claim. A schedule, format, or
  participant list can no longer qualify as the direct answer to a requested
  competition result. Summary and Findings blocks
  are atomic: every cited source must contain every date and number, and a core
  block requires one complete verified institutional record or one explicitly
  accountable publisher that establishes the whole claim. Independent
  corroboration remains preferred when available, but unrelated citations are
  never added merely to increase source count. Reports that
  cannot meet these gates remain explicitly degraded instead of claiming
  traceability. Degraded reports show at most two readability-ranked excerpts
  per eligible source and one per ineligible source, and report-model packets
  exclude ineligible excerpts entirely.
  Explicit result, winner, champion, score, and standings queries now get a
  strict deterministic acquisition and publication fast path. When discovery
  contains at least two cross-host accountable candidates with explicit result
  retrieval opportunities, the Host fetches at most four of them without
  spending a 60-second model call on URL admission. Extractive publication uses
  only exact atomic spans from a verified institution or accountable publisher,
  rejects questions, schedules, generic indexes, navigation piles, headline
  lists, betting odds, predictions, historical roundups, and time-only widgets,
  and ranks the direct outcome separately from concrete Findings. Findings stay
  source-local unless an independent source has one identical score and at
  least two matching non-generic event-identity features; an equal score alone
  cannot join unrelated events across publishers. Pure outcome-restatement
  headlines are removed instead of padding the Findings count. A successful
  extractive result passes the normal Host admission gates, records
  `deterministic_outcome_extract`, and skips report-model generation; otherwise
  the closed proposal and honest degraded fallback remain unchanged. Final
  reports number only sources that are actually cited, in first-citation order,
  render inline references as visible `[n]` links, and avoid duplicated
  ordered-list/source identifiers such as `2. [2]`.
- Made foreground `a3s web` Ctrl+C shutdown cancel in-flight requests and
  long-lived HTTP streams before draining connections, with bounded server and
  application cleanup plus a second-interrupt emergency exit. Raw terminals
  with signal generation disabled remain supported while preserving and
  restoring the caller's original terminal mode.
- Made DeepResearch generation capacity session-scoped across independent
  host-direct Flow calls, preventing parallel obligation reviews from bypassing
  a provider's typed single-flight contract. Replaced the ineffective use of a
  per-Program timeout as the retrieval clock with a durable 45-minute Inquiry
  policy: one real 25-minute whole-retrieval deadline, a protected 15-minute
  closed-review stage, and a two-minute finalization reserve. Review groups keep
  completed durable siblings, bound unfinished units at the stage deadline,
  and use at most two identical 300-second active attempts after admission;
  this recovers a stuck provider generation while the 15-minute review-stage
  deadline remains authoritative. The planner now
  has a 360-second active-generation fuse; the shared Inquiry deadline still
  shortens retrieval when planning consumes that long-tail allowance.
- Added a durable initial-evidence checkpoint before optional supplemental
  retrieval. If the shared 25-minute retrieval deadline interrupts supplemental
  admission, fetch, or semantic selection, the Host now validates and restores
  the same run/query checkpoint and continues closed review instead of losing
  completed material evidence and publishing an empty Recovery report.
- Prevented DeepResearch semantic-selector fan-out from consuming active model
  deadlines while an OpenAI-compatible or account provider is still waiting
  for generation capacity. Unknown capacity is now conservatively
  single-flight, admission is cancellation-safe, and durable
  `generate_object` metadata separates queue wait from the active timeout.
  Flow now acquires capacity before starting the selector's Program VM and
  passes a one-shot identity-checked permit into the nested generation, so
  neither deadline includes queue wait and concurrent nested calls cannot
  reuse a reservation. Pre-fetch web candidate admission now has one 60-second
  Flow attempt plus an acquisition-only deterministic failure fallback;
  fetched-text catalog selection retains its 300-second active
  fuse, while one-shot source-local selectors receive 270 seconds after
  admission.
- Replaced the semantic selector's contradictory source-role contract with a
  complete typed role object. Its schema requires `supporting=true` and
  explicit `primary` and `independent` booleans; the Host validates every flag
  before canonicalizing the edge. A schema-valid primary source can no longer
  be rejected merely because an independently required support label was
  omitted.
- Added exact source/obligation relevance edges beside full criterion-coverage
  edges. Obligation reviews now receive only relevance-linked evidence plus
  explicitly unscoped legacy evidence, reducing prompt size without lexical
  filtering. Provider schemas now use short Host-owned evidence references such
  as `E1` instead of requiring models to reproduce long evidence hashes; the
  Host maps each reference through the exact closed catalog per question, so a
  malformed citation bounds only that question instead of discarding its valid
  sibling. The Host now also isolates wire decoding per expected question and
  safely demotes `answered` entries with an explicit limitation to `partial`,
  preventing one schema-valid status contradiction from erasing every answer in
  a shared obligation review.
  Supplemental recovery now records each initial candidate as retained,
  fetch-failed, or selection-empty and avoids repeating a failed transport
  surface when a distinct candidate remains.
- Made every committed section evidence binding an explicit citation
  requirement. Initial section writers must cite one exact source from every
  binding, and the single targeted revision receives the precise uncited
  binding/source alternatives instead of an opaque claim ID, preventing an
  otherwise unfixable repeat of the same cross-evidence audit failure.
- Made citation repair monotonic across a complete replacement. Revision input
  now repeats the exact accepted source alternatives for every binding, not only
  the binding missing from the prior draft, and forbids constructing deeper
  links from claim text. The Host canonicalizes only a strict same-origin path
  descendant to the longest committed parent source; lexical siblings and
  root-domain widening remain rejected.
- Added bounded claim excerpts to closed section and revision packets and a
  deterministic date-grounding audit. ISO, English, and Chinese full dates are
  normalized before publication; a transcribed date absent from the section's
  committed claims is rejected and routed through the single targeted revision.
  Section prompts also preserve useful partial answers and forbid mentioning
  outside knowledge even as a disclaimer.
- Tightened the closed-evidence reasoning boundary across question review,
  section generation and revision, and editorial framing. Models must preserve
  source metadata semantics, cannot calculate unstated intervals, turn a
  dependency into incompatibility, turn discontinuation into a promise of no
  future fixes, assign unsupported properties to a recommended replacement, or
  generalize a few reviewed examples into an ecosystem-wide claim. Internal web
  discovery/review notes remain available to assessment but no longer appear in
  the reader-facing source ledger. Reader-facing review, section, revision, and
  frame text must use the query language, and a question-local evidence gap
  cannot be widened into a report-wide absence. The editorial frame now receives
  a 270-second active-generation allowance and obligation review receives 300
  seconds per attempt, so source-complete reports are not degraded by their
  former 180- and 270-second tail fuses.
- Kept provider titles, snippets, ranking, engine names, and dates on the
  discovery side of the DeepResearch evidence boundary. In particular, an
  index, crawl, or documentation-build date can no longer become a claimed
  publication date unless the fetched source text establishes it. Semantic
  source admission now fills the fixed eight-slot initial web-fetch allowance
  with coverage-resilient alternatives when enough candidates exist, and typed
  criterion coverage is emitted only when the
  selected fetched text resolves every material element; partial relevance
  remains retained evidence but stays a typed gap that can trigger the single
  supplemental pass. The planner now restricts every completion criterion to
  one independently sourced target, preventing comparison-wide criteria that no
  single source can close. GitHub release
  catalogs are fetched through their official Atom feeds, and a child result
  hidden by the aggregate `batch` output cap is detected and refetched alone;
  truncated navigation can no longer be silently promoted as source text.
- Made A3S Web startup idempotent and ownership-safe. Repeated and concurrent
  managed starts now converge on one healthy workspace instance, while
  foreground and legacy A3S listeners can be discovered and reused without
  granting stop authority. Listener reservation happens before assets,
  configuration, and session restoration; foreign port owners are never
  signaled; `--replace` performs only authenticated graceful replacement of a
  managed instance. Packaged Web assets are again discovered beside the
  executable and under `share/a3s/web`, and unavailable saved-session warnings
  are summarized without deleting their data.
- Replaced tool-category and shell-string approval routing in Code TUI with an
  enforced local process boundary. When the verified managed runtime is ready,
  Default mode runs workspace file mutations, ordinary shell commands, and
  governed nested orchestration without repeated HITL prompts; only explicit
  host escalation, missing-sandbox shell execution, protected
  Git/configuration changes, and annotated external side effects remain
  interactive. Plan denies mutations, while Auto never opens HITL and fails
  closed for sandbox escape or a missing process sandbox. The sandbox denies
  network egress and local binding, limits writes to the workspace and per-run
  scratch directory, protects control metadata, scrubs ambient secrets,
  preserves streamed bounded output and timeouts, and is inherited by
  delegated and Skill child runs without an unsandboxed fallback.
- Froze permission and confirmation routing at run admission. Foreground,
  queued, delegated, parallel, Skill, and background work now retain the exact
  submitted mode even after the composer advances, so an Auto child cannot
  start requesting HITL under a later Default turn. The managed sandbox also
  denies existing nested `.env*` files and pre-existing multi-link source
  aliases.
- Restored consistent transcript vertical rhythm in both the main history and
  the `Ctrl+T` view. One compositor-owned blank row now separates every
  top-level message, notice, reasoning block, tool call, delegated-task result,
  and live or finalized assistant cell; adjacent tool calls use the same rule
  instead of forming a dense exception.
- Wait for the initial A3S Use MCP projection within a bounded startup budget,
  so the first model turn receives ready Use routes through `task`. Slow or
  broken surfaces remain non-fatal and continue converging in the background.
- Made TUI Auto mode genuinely non-interactive for every operation that
  survives explicit policy and workspace hard denials, including confirmation
  escalation requested by a tool implementation. Late confirmation events are
  resolved before any approval projection is created, so hard denials fail
  without opening or briefly flashing an approval prompt. Running and queued turns keep their
  submission-time mode, while Plan mode remains strictly read-only.
- Removed local lexical matching from DeepResearch source admission and
  evidence selection. Planner-authored queries now reach search providers
  unchanged. Safe candidate URLs are admitted by a closed semantic ID decision.
  Fetched pages and PDFs plus exact Host-reread workspace ranges form one
  complete multilingual chunk catalog. Catalogs of at most 10 chunks use one
  complete selector; larger catalogs use exactly one complete source-local
  selector per canonical source, without positional chunking or a lossy source
  reducer. The complete-catalog ceiling is 384 chunks so an ordinary successful
  eight-source portfolio is not discarded before source-local selection. Each
  source retains at most four excerpts (2,800 characters), and a
  source-local call gets one 270-second active attempt. A failed source unit
  drops only that canonical source, preserves independently validated sibling
  sources, and records an explicit operational gap that can trigger the one
  supplemental replacement pass. Every fetched chunk for that source enters
  the same selector call, and a completed sibling effect is reused after
  interruption.
  Duplicate, over-limit, oversized, and out-of-catalog IDs fail closed.
  Active questions no longer receive provider queries by array position, and
  query-term coverage counters and gates no longer affect completion.
- Restricted the one transient fetch retry to structured `network`, `timeout`,
  or `transport` error kinds. Transport-looking prose alone no longer schedules
  another fetch.
- Removed DeepResearch runtime selection and its disabled OS/adaptive-route
  state. Interactive, smoke, and CLI runs now enter the same host-managed
  pipeline; callers choose only the explicit web or local-only evidence scope.
- Checkpointed DeepResearch Inquiry prefixes after logical transactions and
  made the same sectioned-report transaction resume from a committed outline,
  existing drafts, or an audit boundary. Completed Flow effects retain stable
  identities across restart, while active report generation shares one global
  targeted-section-revision allowance followed by one deterministic re-audit.
  One idempotent `research.report.started` event anchors the complete report
  budget across process restart; callers cannot extend it, and wall-clock
  regression fails closed. Resume does not reset that deadline or allowance or
  create another completed-report pipeline.
- Replaced DeepResearch's report-writing compliance assumptions with Host
  invariants. The Host now derives the outline from the typed
  obligation/question/evidence graph, reviews all questions linked to one
  obligation in one durable unit, runs at most three obligation or section calls
  in flight, and owns the only contract assessment. Retrieval materializes
  exactly one accepted evidence item per canonical source and keeps only that
  source's claims and coverage on the item; the global key-evidence truncation
  was removed. Section source IDs are derived from exact inline Markdown
  anchors: every committed claim requires one cited source from the same
  source-local accepted evidence binding, but unused alternative sources no
  longer trigger a model rewrite or enter the published source ledger. Nested
  H1/H2 syntax is deterministically demoted, and qualified cautions use typed
  question limitations, obligation metadata, and evidence diagnostics instead
  of leaking internal assessment rationale. An exact accepted URL emitted as
  citation-shaped `[https://…]` text is canonicalized to a CommonMark autolink
  outside code before citation extraction, without lexical relevance
  inference.
- Required host-managed DeepResearch output to carry exactly one committed
  research contract and one contract assessment before reporting, preventing
  missing event/state pairs from falling back to legacy completion authority.
  Rust now stamps that authority while attaching every current Inquiry
  projection, so retrieval-adapter or historical checker fields cannot regain
  terminal control.
- Deferred default-model validation from agent bootstrap to session creation so
  hosts can start account-only sessions with an injected Claude, Codex, Kimi,
  or WorkBuddy client while sessions lacking both sources still fail with an
  actionable configuration error.
- Installed the shared risk-aware permission policy and a real HITL manager for
  `a3s code exec`. Auto mode now executes bounded workspace edits, unresolved
  approvals terminate immediately with a nonzero `approval.required` result,
  and a stream cannot report success without a terminal completion event.
- Restored the Code TUI `/relay` picker for native A3S Code sessions and
  Claude Code or Codex task handoff, added WorkBuddy project transcripts as a
  fourth source, and preserved the selected native session's model, effort,
  execution mode, theme, and paused-goal state during an in-app resume. The
  panel now pins the current session and surfaces saved state, model, age,
  unfinished runs, and live background-agent counts. It keeps stable semantic
  selection across manual or 15-second refreshes, searches a bounded 64-row
  catalog per source, remembers each source tab independently, supports wheel
  navigation, and provides a compact task peek before continuation.

## [0.9.1] - 2026-07-16

### Added

- Projected observed `mcp__use_<route>__*` progress into capability-oriented
  TUI lifecycle labels such as `Using Browser` and `Used Browser`. Routes stay
  ordered and deduplicated, restored tracker snapshots replay the same
  identity, and ordinary workers or unrelated MCP tools are never reclassified.
- Added typed `list`, `info`, `install`, `upgrade`, `uninstall`, and `doctor`
  component lifecycle commands for Code, Box, Bench, Search, Use, and delegated
  Use capabilities.
- Added native Code Intelligence shared by agent tools, the TUI `/ide` editor,
  and A3S Web. The first release provides saved-file symbols, semantic
  navigation, and diagnostics for Rust and TypeScript/JavaScript while reusing
  the existing workspace manifest, file tools, path policy, and editor file
  selection.
- Added the canonical `a3s compose ...` Box route and concise
  `a3s up`, `a3s down`, `a3s ps`, and `a3s logs` application shortcuts. All
  routes preserve raw child arguments, working-directory context, streams, and
  the Box process exit status while retaining verified first-use installation.
  Box now discovers canonical `compose.acl` project files through the same
  transparent route while explicit Compose YAML remains available.
- Added `a3s use box ...` as a component-backed route. The root remains the
  sole Box installer and receipt owner, injects one canonical executable into
  Use, preserves native argv and status, and never auto-installs Box for
  unrelated Use commands. External Use domains now expose generation-based
  enable, disable, snapshot, and watch operations with graceful route draining.
- Added unified A3S Use capability hot-plug for Code TUI and Web sessions,
  including one shared Web watcher, generation-driven MCP/Skill projection,
  session-rebuild replay, bounded startup discovery, background recovery, and
  a permission-isolated `use` worker. Capabilities converge across install,
  upgrade, disable, and re-enable without restarting A3S Code.

### Changed

- Reorganized the umbrella command surface around typed Clap trees, immutable
  invocation context, explicit application services, and registered component
  proxies. Unknown top-level commands no longer discover and execute arbitrary
  `a3s-*` binaries from `PATH`.
- Made macOS and Linux the current component/runtime support targets; Windows
  remains a compile/package preview until its managed lifecycle and persistent
  Browser conformance gates pass.
- Promoted the dedicated A3S Use worker into the live `task` and
  `parallel_task` catalog with current capability IDs, MCP routes, and Skill
  guidance. It returns observable application evidence and never falls back to
  shell, workspace, unrelated MCP, or recursive delegation.
- Updated the A3S Code Core baseline to 5.3.2 so packaged CLI builds include
  the live worker-definition contract used by the dedicated Use worker and
  position-aware HITL classification for shell commands.
- Pinned standalone releases to ACL 0.2.2, Boot 0.1.2, Lane 0.5.1, and Updater
  0.3.0 so packaged builds retain bounded nested-block parsing, typed HTTP
  failures, deterministic queue lifecycle, and checksum-verified ZIP updates
  without depending on monorepo paths.

### Fixed

- Kept `--version` inside native product proxies so `a3s use --version` and
  sibling proxy commands report the delegated component version instead of the
  umbrella CLI version.
- Completed the Parent → Use worker → live `mcp__use_*` path for hot-plugged
  capabilities. Office `use.office.outcome_unknown` mutations are surfaced as
  potentially applied and are never retried automatically.
- Capped provider-facing child concurrency at eight for every interactive
  effort profile while retaining larger reasoning, tool-round, and continuation
  budgets. Ultracode and `/goal` now schedule larger workloads in bounded waves
  instead of bursting one signed-in provider account.
- Made `/goal` execute maker and verifier as dependency-ordered phases, with
  parallelism limited to independent read-only work inside each phase. Goal
  state writes are now checkpointed at five-percent progress boundaries and
  unchanged runtime sections are not rewritten.
- Rendered per-branch `parallel_task` output excerpts, retry attempts, and
  recovered branch counts without repeating the complete batch output in every
  plan step or DeepResearch synthesis prompt.
- Replaced the TUI-local pending-turn heap with `a3s-lane`'s stable typed
  priority queue. Explicit user messages now outrank automatic continuations,
  equal-priority turns remain FIFO, failed stream admission restores the exact
  queue item without losing image attachments, and Esc settles the real Core
  worker before consuming one queued successor. The bottom queue strip now
  projects pending turns only, so a claimed message disappears as soon as its
  execution begins.
- Stopped workspace discovery before the Code TUI and Web host tear down their
  Tokio runtimes. In-progress directory scans and Git file enumeration now
  observe cancellation, so quitting from a large workspace or home directory
  no longer hangs after the session-saved message. TUI session close and
  language-service cleanup also have host-level deadlines, and both startup
  and interactive `/update` checks now terminate their child process when the
  TUI closes, so unresponsive background work cannot block exit indefinitely.

## [0.8.2] - 2026-07-15

### Fixed

- Kept locally signed-in Codex accounts and their picker-visible models
  available when the shorter-lived identity token has expired but reusable
  Codex access state remains. Account refresh and entitlement validation stay
  delegated to the installed Codex CLI instead of treating ID-token expiry as
  a logout.
- Parsed WorkBuddy `hy3` tagged tool-call envelopes and withheld protocol
  markup across split streaming deltas. Tool calls now become native A3S tool
  events instead of exposing `<tool_call:...>` or closing XML tags in TUI
  messages.

## [0.8.1] - 2026-07-14

### Fixed

- Matched Codex Markdown section spacing by keeping exactly one blank terminal
  row before and after headings. A new section no longer touches the final line
  of the preceding paragraph, while existing code/table/heading separators are
  not doubled.

## [0.8.0] - 2026-07-14

### Added

- Added signed-in WorkBuddy account models to `/model`, `a3s model`, and
  `a3s code models`. The integration reuses WorkBuddy's bundled CodeBuddy CLI
  and local account state without reading or copying private tokens, discovers
  the models enabled for the account, streams responses, and preserves native
  A3S tool execution.

### Changed

- Consolidated Claude Code, Codex, and WorkBuddy integrations under the
  `account_providers` boundary. Claude and WorkBuddy share one cancellable
  account-CLI stream/tool bridge, while all three share account detection,
  client construction, model switching, persistence, and session restore.
- Unified TUI `/compact` with the direct, tool-free compactor used by Code Web.
  Repeated manual compaction now includes the previous summary without creating
  a temporary tool-capable agent session, while Core's rolling auto-compaction
  remains re-armed for long-running conversations.
- Rebuilt `/goal` as a durable Ultracode goal loop. Setting a goal now creates
  a complete `.a3s/loops/goal-*` Loop Engineering workspace, forces planning
  and goal tracking, runs separate maker/verifier guidance, and continues
  across normal ends and retryable errors until a matching `GoalAchieved`
  arrives. Esc and `/goal clear` invalidate delayed retries immediately, and
  normal Ultracode message-gated planning is restored when the goal closes.
- Aligned fenced Markdown code with Codex-style terminal highlighting: known
  languages retain distinct token colors, unknown languages stay plain, CRLF
  is normalized, and 512 KiB / 10,000-line guardrails avoid render stalls.
- Replaced width-unstable colorful emoji in the transcript, task queue,
  thinking indicator, and `/ide` tree with monochrome terminal-safe marks and
  consistent hair-space padding.
- Made the footer the single owner of live context usage; the composer status
  chip keeps effort and mode information without duplicating context fill in
  the input border.
- Enabled Core's model-aware rolling compaction for TUI and Code Web sessions.
  Each selected model supplies its actual context window, requests compact
  before overflow, and can compact repeatedly throughout a long-running task.
  A3S Code Core 5.2.4 budgets the retained suffix by estimated message tokens,
  bounds oversized summaries, and refuses replacements that would not reduce
  context. Core summaries are written back to each host's durable timeline so
  later turns continue from the latest generation instead of compacting it
  again.
- Moved model, effort, Ultracode, goal, auth, reload, fork, and clear session
  changes onto an async atomic replacement path. The UI no longer blocks the
  Tokio runtime, failed reconfiguration keeps the old session usable, and
  `/goal` can reliably enter forced-planning Ultracode before its first turn.
- Routed A3S Code 5.2.2 native structured output through TUI launch, configured
  model selection, effort rebuilds, and headless DeepResearch. Codex Responses
  and Responses Lite now force the schema function through `tool_choice`, while
  providers with verified JSON Schema support retain that path and unknown
  custom OpenAI-compatible endpoints keep the safe prompt fallback.
- Clarified that web-and-workspace scope describes available evidence tools,
  not mandatory tracks. The semantic planner now reserves workspace collection
  for queries that explicitly depend on a repository or local artifacts.
- Replaced the normal public-investigation `direct_then_maker` route with
  `direct_then_review`. Multi-query retrieval now flows into one structured
  synthesis-and-coverage review, removing a redundant slow model turn while
  preserving `direct_then_maker` replay compatibility for older journals.
- Expanded the LLM-selectable public evidence envelope to four searches and
  eight parallel fetches. Query-specific candidates are fetched before seed
  URLs, page excerpts are ranked against their owning evidence question, and a
  zero-result unconfigured search receives one bounded Brave fallback.
- Gave the independent checker a 180-second clock within the unchanged
  300-second workflow fuse and carry observed checker latency into subsequent
  scheduling decisions. Public-source gaps route to direct retrieval; makers
  remain reserved for evidence production or required local/non-web work.

### Fixed

- Corrected the real-LLM compaction integration test to compare matched
  compressed and uncompressed histories. It now proves provider-reported
  prompt reduction on the compacted request and again after session restore,
  instead of comparing two already-compacted turns.
- Preserved traceable structured evidence when the independent DeepResearch
  checker times out. The workflow now completes with an explicit degraded
  verification state and publishes a provisional evidence-derived report;
  only runs without reportable evidence fall back to a Recovery artifact.
- Kept explicit checker URLs out of finding prose and bound each one to its
  matching source card, preventing Chinese terminal punctuation from becoming
  part of an auto-linked URL and avoiding unrelated citations on a finding.
- Made long report headings prefer a complete semantic clause, bounded the
  caveat section to eight reader-relevant items, and added a mobile horizontal
  scroll cue to wide evidence matrices.
- Prevented structured no-tool makers from being used for checker-requested
  evidence collection. Existing-evidence synthesis is now an explicit legacy
  optimization, while new gaps retain tool-capable collection semantics.
- Tightened checker output so findings state supported facts instead of merely
  announcing that sources or comparisons exist, and requested recommendations
  must give a conditional answer or remain an explicit evidence gap.

## [0.7.9] - 2026-07-14

### Added

- Added durable Code Web sessions backed by Core `FileSessionStore` snapshots
  under `~/.a3s/code-web`. `a3s code serve` now restores conversation and run
  events after restart, keeps stable titles/timestamps, retains bounded Web-only
  `/help`, shell, fork, and structured-event display history, and deletes both
  live and persisted state through the Kernel session API.
- Added a unified account and model-routing surface: `a3s account
  list|status|login|logout` and `a3s model list|current|use|reset`. Configured
  providers, Claude Code, Codex, and A3S OS Gateway models now have explicit
  route identities while product OAuth credentials remain in their owning
  stores. Model selection is persisted separately from `config.acl`.
- Added `a3s login [token]` and `a3s logout` as main CLI commands for A3S OS
  browser OAuth, bearer-token login, and local session removal. The existing
  `a3s code login` and `a3s code logout` forms remain compatibility aliases.
- Added `a3s search` management commands for engine discovery, configuration
  diagnostics, and explicit Chrome/Lightpanda list, install, update, and repair
  operations. `a3s list` now includes the managed search runtimes.
- Added one component lifecycle under the `a3s` command:
  `a3s install code|box|bench`, `a3s list`, and
  `a3s update [code|box|bench]`. Code remains bundled by default, while Box and
  the private Bench control component install only on explicit install or first
  real use.
- Added validated, version-isolated Bench control-component installation under
  `~/.a3s/components/bench/`, including release and payload digests, manifest
  and protocol checks, atomic activation, stale-lock recovery, and local
  health reporting without adding a second public executable to `PATH`.
  The component plans and evaluates benchmark runs but delegates Candidate and
  Judge Agent Asset execution exclusively to A3S OS Runtime.
- Refreshed the signed-in Codex account model catalog asynchronously for the
  TUI and `a3s code models`, exposing every picker-visible account model,
  including GPT-5.6 Sol, Terra, and Luna when entitled.
- Added native Codex reasoning-effort support to `/effort`. The `low` through
  `max` profiles request their same-named `reasoning.effort`, while `ultracode`
  keeps A3S orchestration and requests the maximum Responses wire effort (`max`
  for GPT-5.6 account models and `xhigh` for older GPT models). Unsupported
  requests clamp downward without disabling the selected profile's host budgets.

### Changed

- Refined the `/ide` and `/config` editor with a terminal-safe file icon
  system, semantic type colors, directory disclosure rows, icon-bearing panel
  titles and metadata, and a quieter ruled line-number gutter. The shared `@`
  file picker now uses the same icon source of truth.
- Decoupled native Codex reasoning effort from A3S automatic delegation. The
  `low` through `max` levels keep their native reasoning settings without
  runtime-driven fan-out; `ultracode` remains the automatic orchestration mode,
  and synthesis-only continuations cannot recursively delegate.

- DeepResearch report turns now return Markdown without model-side file tools.
  The host validates the content against accepted evidence, atomically publishes
  the Markdown/HTML pair, and appends the trusted view marker only after both
  artifacts pass validation. This removes long `write` calls from the report
  critical path while A3S Code 5.2.2 keeps offset-checked, idempotent append mode
  available for ordinary large workspace files.
- Made the LLM planner choose an explicit `direct_only`, `direct_then_maker`,
  or `maker_first` execution route together with the report title, phases,
  tracks, and budgets. A substantial public investigation can now run A3S Code
  5.2.2 capability-governed batch search/fetch, immediately fan those sources
  into planned maker tracks, and invoke its first checker only after cumulative
  direct and maker evidence exists. Narrow lookups still use direct retrieval
  plus a checker, while evidence-production and workspace work can begin with
  makers. No topic, query-length, answer-shape, or track-count rule overrides
  the LLM route. Each maker may use its second evidence round to close a
  consequential gap instead of being forced to stop after one seed fetch.
- Carried the checker's reader-facing summary, verified findings, unresolved
  gaps, contradictions, and the planned title into both normal synthesis and
  deterministic timeout materialization. Generated reports no longer turn
  collection-status prose into findings, duplicate punctuation, or claim that
  no gaps exist after the checker recorded explicit limitations.
- Reduced model-command latency by validating `a3s model use` against only the
  selected credential source, running Codex and A3S OS catalog discovery in
  parallel, and parsing `config.acl` only once during `a3s model list`.
- Fixed TUI model and effort switching for live persisted sessions. Session
  rebuilds now save and close the current session before resuming the same
  durable ID, and restore the previous configuration if the requested rebuild
  fails, avoiding the `session is already live or being built` error.
- Improved JSON in streamed and completed assistant messages. Complete bare
  JSON values and complete `json` fences are now pretty-printed through the
  shared Markdown renderer, while partial streaming fragments remain untouched
  until they form valid JSON.
- Removed duplicate `Updated Plan` transcript cells; planning events now update
  only the pinned checklist above the input. The pinned parallel-subagent
  tracker also disappears immediately once every child reaches a terminal
  state, and completed turns clear the pinned plan instead of retaining stale
  task tracking UI.
- Collapsed persistent TUI state into one Codex-style footer row. Mode and
  context now have one owner, transient work stays in the activity row, and
  typed parallel-child outcomes distinguish succeeded, failed, cancelled, and
  tracking-lost tasks without duplicating agent counts in the footer.
- Restored conventional diff colors in the TUI: additions and `+N` header
  counts use green, while deletions and `-N` counts use red, with matching dark
  backgrounds for changed lines.
- Made interactive TUI DeepResearch synthesis and repair deadlines
  activity-based and independent from report tool execution, so an in-flight
  report write is not cancelled merely because the model synthesis timer
  reached its deadline.
- Closed the DeepResearch evidence-to-report boundary. Synthesis, recovery,
  repair, and verification now use only the bounded evidence package; retrieval,
  batch, shell, Git, delegation, program, runtime, and workflow definitions are
  removed from report-model requests and remain denied by the execution gate.
  A failed/degraded collection immediately publishes an explicit recovery
  report instead of starting another retrieval or 180-second synthesis cycle.
- Reserved final-response capacity in every delegated evidence task: children
  receive at most two high-signal evidence-tool rounds plus one provider turn
  for A3S Code v5.2.2 structured-output validation and repair.
- Removed topic-specific DeepResearch shortcuts, classifiers, query templates,
  source allowlists, report headings, and visual themes. TUI and headless runs
  now share the same LLM-planned generic collection loop for every subject.
  The checker routes a narrow externally retrievable gap to one bounded direct
  follow-up and reserves maker delegation for multi-step analysis or local
  evidence work. Follow-ups use unique replay-safe step IDs and cumulative
  evidence; non-actionable continuations degrade instead of looping.
- Aligned streamed tool cards more closely with Codex-style semantic coloring:
  executable names and paths are cyan, flags and parameter names are yellow,
  and parameter values are green while preserving the original command text.
- Replaced repeated JavaScript-wrapper previews in `program` cards with a
  source-free semantic summary derived from structured inputs. DeepResearch
  calls now show the query, evidence scope, bounded parallel plan, and current
  workflow phase; completed nested calls are aggregated into one bounded result
  row. Workflow handoff stores the same intent summary instead of copying the
  full PTC source back into synthesis context.
- Restored DeepResearch workflow executability after its embedded script grew
  beyond the runtime's 64 KiB source limit, and added a source-size regression
  gate. Delegated tasks now count as successful evidence only after schema and
  source-anchor verification; rejected or fabricated evidence contributes to
  failed/partial status instead of sending an empty package down the successful
  report path.
- Expanded offline `a3s bench --help` to show the canonical
  `list`/`info`/`run`/`result` interface and local Task path rule before the
  private control component is installed.
- Made local DeepResearch a first-class TUI and headless CLI path with explicit
  `web + workspace` versus `offline/local-only` evidence labels, a visible
  evidence-to-report handoff, and distinct completed and low-confidence
  recovery outcomes.
- Made DeepResearch evidence scope an explicit typed setting. CLI callers can
  use `--local-only`/`--offline` or `--web`, while the TUI accepts the same
  scope after `?`; natural-language intent remains only a compatibility
  fallback, and the selected scope drives both workflow inputs and network
  permission enforcement. Research-mode input now displays the available scope
  switches, and delegated/runtime branches receive scope-specific tool and
  prompt contracts instead of reinterpreting query wording. New workflow runs
  emit only `evidence_scope`; the legacy `direct_web_enabled` field is accepted
  only when replaying older inputs.
- Expanded the sanitized DeepResearch TUI/tool-card summary with direct-web
  source, host, fetch, topic, fetched-text, date, and bounded-query coverage.
  Hybrid runs now surface their web seed alongside delegated task progress, so
  users can see why direct completion was accepted or declined without opening
  raw workflow diagnostics.
- Rebuilt TUI tool-call rendering around stable call IDs and source-backed
  transcript entries. Preparing, approval, running, denied, interrupted, and
  completed calls now update in place; parallel completion order cannot reorder
  calls. Exec, Explore, Web, MCP, file-change, task, program, and dynamic
  workflow cells use Codex-style `•`/`└`/`│` grammar and reflow after resize.
- Made streamed Markdown newline-gated and lossless, with stable and mutable
  table regions, adaptive stable-line commit/catch-up pacing, cached mutable-tail
  viewport replacement, source-backed resize, grapheme-safe wrapping,
  ANSI-contained rows, and a key/value fallback for narrow tables instead of
  truncated cells.

### Removed

- Removed the TUI-only `/btw` background side conversation, including its
  command, asynchronous session path, message/state handling, overlay, input
  tint, tests, and documentation. The primary agent conversation is unchanged.
- Removed the TUI `/output` and `/top` commands and their dedicated panels;
  `Ctrl+T` remains the complete semantic transcript, and local process
  inspection remains available through the standalone `a3s top` command.

### Fixed

- Waited for the previous model stream's lifecycle handle before constructing
  synthesis, loop, DeepResearch, or queued continuations. Terminal events can
  precede persistence cleanup and release of the session's single-flight lease;
  stale stream starts are now cancelled on their originating session and joined
  instead of detaching the public handle, preventing `already has an active
  operation` failures between turns.
- Settled every child task observed during a DeepResearch run before opening
  its terminal report view or restoring the previous mode. Live children are
  cancelled through the core tracker, missing cancellers become explicit
  tracking-lost outcomes, and late watcher messages or stale tracker snapshots
  cannot recreate the footer. Esc interruption uses the same scoped settlement
  path without opening an incomplete report.
- Kept direct retrieval and maker child timeouts independent. A slow or
  partially successful maker pass no longer consumes the direct-retrieval clock
  before the checker can request one bounded recovery. The next checker receives
  all initial and follow-up direct evidence together with the retained maker
  evidence, so successful recovery can finalize instead of degrading from a
  stale package.
- Forwarded planner-provided seed URLs into maker prompts, removed the implicit
  raw-success-count early cancellation that could discard later valid evidence,
  and routed a fully failed maker-first pass through one direct evidence recovery
  before the independent checker. Failed Flow step IDs are never rescheduled.
- Bounded each maker to a compact evidence card: at most four sources, five
  decision-relevant facts, and short summaries, contradictions, confidence, and
  gaps. Research depth now comes from independent planner tracks instead of
  making every child spend its deadline writing a miniature report.
- Excluded navigation chrome, CSS, link-target-only keyword matches, and common
  repository-page boilerplate from direct-web evidence snippets and titles.
  Relevant visible text now carries adjacent context so fetched README and
  documentation facts are retained instead of menu rows.
- Made `/exit` and confirmed Ctrl+C close the active session and settle or abort
  its stream before quitting, preventing detached work and stale single-flight
  leases from surviving process shutdown.
- Reserved the final ten seconds of Smoke DeepResearch's six-minute absolute
  deadline for cancellation and recovery artifact publication. Workflow,
  synthesis, and repair phases can no longer consume the time required to write
  an explicit degraded report.
- Excluded leading freshness phrases and date-only tokens from headless
  DeepResearch entity extraction, preventing queries beginning with
  `As of <date>` from being misread as a comparison of crates named `as` and
  `of`.
- Closed and reaped execution-scoped Chrome and Lightpanda processes on search
  success, error, timeout, and cancellation. Browser tabs now remain guarded
  until explicit close, pool shutdown owns child cleanup in a detached task,
  and code-core no longer keeps a session-wide headless pool that can survive a
  cancelled `web_search` invocation.

- Bound DeepResearch report-phase writes to the current query's exact
  `.a3s/research/<slug>/report.md` and `index.html` pair, rejected unchanged
  same-query artifacts from earlier runs, and preflighted/staged both files
  with rollback before publishing a replacement generation.
- Filtered delegated DeepResearch sources against successful child tool
  provenance before follow-up planning or synthesis. Fabricated URLs and paths
  are now omitted, while runtime-observed URLs are stripped of credentials,
  fragments, and query parameters before reaching reports.
- Hardened DeepResearch provenance across live and recovered evidence: failed
  runtime branches, nested evidence-shaped text or objects, and case-changed
  resource paths cannot become structured evidence. Current-run `Completed`
  validation now requires a recognized citation in the Markdown source/reference
  section and at least one recognized HTML citation, and rejects the report when
  any recognized HTTP(S), link-target, or path citation does not canonically
  match an observed anchor. Canonicalization is comparison-only and does not
  rewrite report files. The headless CLI reuses recent same-query durable
  evidence only when its scope and completed-evidence contract still match;
  recovery reports remain explicitly low-confidence and are rejected by
  `Completed` artifact validation.
- Treated user-supplied DeepResearch queries as escaped single-line Markdown in
  host-generated completed, evidence-rebuilt, and recovery artifacts, preventing
  query text from injecting report sections while keeping HTML titles readable.
  Exact query-title URLs no longer count as report citations, while the same URL
  remains provenance-checked everywhere outside that title. The exemption also
  requires every title link target to exactly match the safe projection of a
  complete HTTP(S) URL token in the query, and only exact `href`/`src` attributes
  are recognized as links. Matching canonicalizes Unicode/percent-encoded forms
  after stripping credentials, query parameters, and fragments, preventing
  renderer differences or query secrets from leaking into durable artifacts.
  HTML rebuilt from Markdown without an H1 now applies the same safe projection
  to its fallback `<title>`.
- Preserved balanced parentheses in bare HTTP(S) citations and Markdown link
  targets, while continuing to trim unmatched closing punctuation, so valid
  resources such as `spec_(v2)` remain exactly traceable. Host-generated query
  Markdown also keeps complete URL tokens intact instead of escaping URL
  punctuation into a different rendered `href`.
- Propagated recovered workflow metadata into low-confidence recovery reports,
  so successful source anchors survive when a timeout leaves no final workflow
  output; the evidence-status section now reports those anchors consistently for
  empty output, timeout/error text, and withheld internal logs, distinguishing
  each case from a run with no captured sources.
- Made the 12-source recovery view explicit when additional verified sources are
  bounded out. The sanitized evidence digest carries only an omitted count, so
  reports can disclose "at least N more" without rehydrating truncated raw
  source objects. Omitted entries are now summed across independently bounded
  evidence items and combined with retained-but-hidden anchors, while duplicate
  output/metadata projections are counted only once. The recovery report also
  discloses evidence items omitted by the separate 18-item digest boundary.
- Filtered and safely projected evidence sources before applying the 12-source
  digest boundary, so invalid schemes cannot crowd out later traceable sources
  and URL credentials, query parameters, or fragments cannot enter synthesis
  prompts. Evidence deduplication now uses the first traceable source rather
  than the first raw source entry, preserving distinct evidence whose leading
  entry is invalid.
- Tightened direct-web relevance gating so generic release/version language or
  an authoritative-looking documentation URL cannot make an off-topic result
  count as evidence. At least one substantive query term must now match, and
  non-JSON URL fallback parsing no longer copies shared output text onto every
  candidate URL. ASCII terms use token boundaries, preventing queries such as
  `rust` from matching unrelated `trust`, `trusted`, or `rustic` text. Meaningful
  two-character entities such as `Go` and `AI` are retained rather than
  degrading the query into an unscoped search.
- Required successful `web_fetch` page text to match a substantive query term
  before counting it as verified evidence or enabling the direct-web fast path.
  Off-topic redirects/pages now fall back to the search snippet with an
  explicit bounded warning and lower confidence.
- Required the narrow direct-web fast path to cover every substantive query
  term across its retained search and fetched evidence. Two independent hosts
  can no longer end a comparison early when both only cover the same entity;
  coverage counts are preserved in bounded workflow and hybrid seed digests.
- Distinguished search-result coverage from successfully fetched page coverage.
  The fast path now requires every substantive query term to appear across
  verified page text, so a comparison cannot complete when one entity exists
  only in an unverified search snippet; both coverage ratios remain diagnostic.
- Excluded HTTP(S) URL tokens from fetched-page relevance and entity coverage
  matching. A fetch tool or page that echoes the requested URL can no longer
  verify an entity found only in that URL path while its actual text is silent.
- Added direct-web end-to-end coverage for URLs embedded inside fetched evidence
  text: credentials, query parameters, and fragments are removed from source
  quotes, key evidence, workflow output, and the bounded synthesis digest.
- Preserved `published_date` (plus compatible `publication_date`/`date` aliases)
  from direct web search results as sanitized evidence `date` fields, allowing
  synthesis to compare recency for current-version and news research.
- Merged complementary metadata across canonical duplicate search results.
  A later query or engine can now supply a missing publication date and engine
  provenance without inflating source counts; direct-web dates are bounded to
  compact evidence fields.
- Added explicit freshness intent and dated-source coverage to direct-web
  metadata. Queries asking for the latest/current/release state now require at
  least one dated source before using the fast path; otherwise they continue to
  delegated research even when topic and fetched-page coverage are complete.
- Excluded placeholder publication dates such as `unknown`, `N/A`, `undated`,
  and localized unknown-date markers from evidence and freshness coverage, so
  nonempty engine placeholders cannot satisfy the dated-source fast-path gate.
- Normalized common ASCII compound-name separators during direct-web relevance
  matching, so `a3s-code`, `a3s_code`, and `A3S Code` evidence agree without
  weakening token boundaries. Chinese query extraction now removes research
  instruction phrases before forming bigrams, avoiding synthetic terms that
  span words such as “全面调研” or “最新”.
- Bounded direct-web relevance analysis to 48 substantive query terms and an
  8,192-character input window. Truncation is explicit in workflow and synthesis
  metadata and disables the fast path, so adversarial high-cardinality queries
  cannot grow matching work without limit or complete from partial coverage.
- Safely projected search-result URLs before invoking `web_fetch` or composing
  fetch diagnostics, preventing credentials, query tokens, and fragments from
  entering tool arguments or workflow logs.
- Safely projected HTTP(S) URLs embedded in the user's research query before
  constructing `web_search` requests. Search engines receive the useful base
  URL while credentials, query parameters, and fragments stay out of tool
  arguments and bounded failure diagnostics.
- Deduplicated search results by the same safe canonical URL used for evidence
  and fetch calls, so credential/query/fragment variants of one resource cannot
  inflate `source_count` or satisfy the multi-source fast path. Canonical keys
  also normalize trailing slashes to match final citation validation, while
  preserving case-sensitive resource paths and normalizing only scheme/host
  casing. Search-result scheme validation is case-insensitive, so valid
  `HTTPS://` results reach canonicalization instead of being discarded. Explicit
  HTTP `:80` and HTTPS `:443` default ports are removed before deduplication,
  fetch, evidence, and host coverage accounting.
- Added distinct `host_count` coverage to direct-web metadata and require at
  least two hosts for the narrow-query fast path, preventing multiple pages
  from one site from being mistaken for independent corroboration. Host coverage
  ignores ports (including non-default ports) while canonical source URLs retain
  meaningful non-default port distinctions.
- Prioritized one result per host when selecting direct-web fetch candidates and
  added `fetched_host_count`. Fast-path completion now requires verified page
  text from at least two hosts, rather than allowing the second host to exist
  only as an unfetched search snippet; the TUI summary exposes both host counts.
- Added an end-to-end positive fast-path contract: a narrow run with two hosts,
  at least one relevant fetched page, and no partial failure completes directly
  without scheduling delegated local research.
- Updated TUI, CLI, and Code Web dynamic workflow/tool registration call sites
  for the fallible registration API, eliminating unused `Result` warnings
  introduced by the core registry hardening.
- Preserved direct-web search, source, host, fetch, and verified-fetch counts in
  the bounded synthesis/diagnostics digest, so coverage and confidence decisions
  remain visible after raw workflow metadata is compacted.
- Preserved balanced parentheses in URLs extracted from non-JSON search output
  while trimming unmatched closing punctuation, aligning direct-web fallback
  parsing with final report citation handling.
- Kept DeepResearch workflow complexity, host timeout budgets, and local-only
  wording in sync; `no web` and `no-web` now disable both direct and delegated
  web collection as documented.
- Expanded explicit offline/local-only intent recognition across common English
  and Chinese phrases such as `local-only`, `without web`, `stay offline`,
  `仅本地`, `离线调研`, and `不联网`. Product discussions about an “offline
  mode” still use web evidence when the query explicitly requests current web
  documentation.
- Disambiguated double-negative network language: phrases such as “cannot
  research without web”, “requires web access”, and `需要联网` retain web
  evidence, while an explicit `do not use web`/`不联网` instruction still wins
  when the query discusses those phrases as a topic.
- Waited briefly for timed-out DeepResearch workflows to quiesce before reading
  durable Flow recovery state, and added random nonces to workflow run IDs to
  avoid concurrent same-process collisions.
- Stored all project-local dynamic workflow history exclusively under
  `.a3s/workflow`; DeepResearch no longer reads or references the retired
  sibling workflow-state directory.
- Reported signed-in Codex `usage_limit_reached` responses with the plan and
  local reset time, while skipping duplicate streaming fallbacks and circuit
  retries that cannot succeed before the account quota resets.
- Normalized the Codex catalog's product-only `ultra` tier to the Responses wire
  value `max`, preventing `reasoning.effort=ultra` HTTP 400 failures.
- Prevented inactivity review from treating UI status notices as conversation,
  re-running after navigation keys, or displaying stale background results
  after a new turn or session change.
- Added the Responses Lite request contract and catalog-provided context
  windows for GPT-5.6 account models, so newly discovered models can execute
  tools and compact at the correct context limit instead of only appearing in
  the picker.
- Honored `$CODEX_HOME` when resolving Codex auth and model-cache files.
- Added Codex-style `Calling`/`Called` fallback cards for dynamically registered
  tools and dedicated structured-output and skill-search verbs. `Ctrl+T` now
  opens the complete live semantic session transcript with user and assistant
  messages, plans, every tool lifecycle and full tool output, subagent state,
  and the current streaming tail.
## [0.7.8] - 2026-07-09

### Fixed

- Hardened DeepResearch against child-task evidence packaging failures: when a
  delegated research track returns source-backed notes but misses the expected
  metadata shape, the workflow now preserves the cited evidence, normalizes it
  into the recursive summary, and continues to synthesis instead of discarding
  useful sources.
- Prevented failed DeepResearch collection from producing false-success reports:
  if no source-backed evidence was collected, the CLI now materializes a
  transparent fallback draft without asking the model to recover current facts
  from memory.
- Cleaned DeepResearch partial-success reporting so final Markdown/HTML reports
  cite original sources and do not expose internal workflow labels, tool logs,
  metadata-normalization details, or stale fallback evidence.

## [0.7.7] - 2026-07-09

### Changed

- Reworked DeepResearch evidence gathering into a bounded, complexity-driven
  recursive parallel retrieval-summary workflow. Local DynamicWorkflowRuntime
  rounds now derive follow-up searches from prior gaps and contradictions,
  stop early when no useful follow-up remains, and keep OS Runtime tool-call
  fan-out disabled until Function-as-a-Service support is available.
- Scaled DeepResearch workflow budgets by query complexity so narrow questions
  get bounded child steps, tool-call/output limits, and host timeouts, while
  broad research still keeps the long-running recursive budget.

### Fixed

- Sanitized DeepResearch partial `parallel_task` failures before synthesis so
  successful structured evidence is preserved while failed child tasks are
  surfaced as compact caveats instead of raw failure blocks.
- Hardened DeepResearch report boundaries so workflow JSON, tool-card
  transcripts, workflow diagnostic paths, truncated tool-output artifact notices, and
  raw failure diagnostics are withheld from prompts, fallback drafts, final
  report artifacts, and RemoteUI auto-open validation.
- Removed internal runtime/workflow labels from DeepResearch synthesis and
  repair prompts so reports cite original evidence sources instead of host
  implementation details.
- Added a DeepResearch report-phase tool gate: once evidence collection ends,
  synthesis and repair turns can only write or edit `.a3s/research/**` report
  artifacts, and verified reports can be host-cleaned into a final answer when
  the model's text contains artifact-operation narration.
- Added DeepResearch source-trace validation: when gathered workflow evidence
  contains source URLs or local paths, completed Markdown/HTML reports must cite
  at least one of those sources before RemoteUI can open them as final reports.
- Added host-side DeepResearch report completion: if synthesis produces a clean,
  source-traceable `report.md` but stalls before writing `index.html`, the host
  can materialize the HTML view and still validate it before RemoteUI opens.

## [0.7.6] - 2026-07-08

### Changed

- Made the A3S Code TUI default HITL policy risk-aware: read-only inspection,
  web research, safe git reads, and read-only batch calls can proceed without
  prompting, while writes, state-changing commands, delegated work, and unknown
  tools still require confirmation.

### Fixed

- Denied catastrophic shell patterns such as privilege escalation, destructive
  root/home removals, device writes, and `curl|sh` installer pipelines before
  they reach the approval prompt.
- Stabilized the DeepResearch CLI local workflow e2e test fixture so child
  evidence prompts no longer consume scripted report-synthesis responses.

## [0.7.5] - 2026-07-07

### Fixed

- Updated A3S Code Core to `4.3.1` so DynamicWorkflowRuntime scripts can use
  legacy `ctx.tools.<name>(args)` tool calls without bypassing allow-list,
  call-count, or output-size limits.
- Made `?` DeepResearch choose OS Runtime fan-out adaptively: broad,
  multi-source research can use the signed-in runtime, while concise or
  explicitly local tasks stay on the local dynamic workflow path and no longer
  require RemoteUI evidence.

## [0.7.4] - 2026-07-07

### Fixed

- Fixed A3S Code TUI transcript wrapping so the welcome banner, user bubbles,
  streaming Markdown, thinking text, pasted images, and tool cards render for
  the scrollbar-adjusted viewport width instead of wrapping a second time.
- Restored reliable transcript wheel scrolling and drag-to-copy behavior by
  keeping mouse capture enabled while copying the app-managed selection on mouse
  release with clamped viewport coordinates.
- Ignored terminal key-release events in `a3s-tui` so Windows terminals do not
  replay a single key press as duplicate input.

## [0.7.3] - 2026-07-07

### Fixed

- Published the matching `a3s-tui` crate API used by the CLI so crates.io
  verification can build the released package without relying on local
  workspace patches.
- Made the release workflow fail when crates.io or Homebrew publishing
  credentials are missing instead of reporting a misleading successful release.
- Added a crates.io User-Agent and retry policy to release dependency checks so
  GitHub Actions does not misclassify already published A3S crates as missing.

## [0.7.1] - 2026-07-07

### Fixed

- Changed Agent lifecycle commands to treat an Agent asset as a package
  directory, with `agent.md`/`agent.yaml`/`agent.yml` as the entrypoint only.
- Published Agent assets now upload the whole local package source plus
  generated manifest, config, and runtime-binding metadata instead of only the
  entry definition file.
- Updated non-interactive `a3s code agent ...` examples and resolution logic to
  prefer package paths while retaining entry-file compatibility.

## [0.7.0] - 2026-07-06

### Added

- Added DynamicWorkflowRuntime wiring for deep research and ultracode turns,
  including host-side workflow execution visibility in the TUI.
- Added richer A3S Code asset surfaces for agents, MCP servers, skills, OKF
  packages, workflows, asset resources, lifecycle stages, review, publishing,
  deployment, and runtime activity.
- Added expanded context, memory, and knowledge surfaces, including memory
  lifecycle rendering, knowledge-base panels, OKF management, sleep/context
  consolidation views, and durable loop engineering panels.
- Added mouse support across the slash menu, approvals, model picker, theme
  picker, file picker, plugin panel, effort slider, asset pickers, and top
  process table.
- Added shared TUI chrome for transcript gutters, input, footer status,
  reasoning, tool output, diffs, plans, task queues, subagents, help, panels,
  alerts, dividers, progress, banners, and RemoteUI links.

### Changed

- Reworked the Code TUI around shared `a3s-tui` components and AgentChrome so
  interaction, wrapping, status, and panel rendering are consistent across the
  app.
- Expanded the A3S Code TUI README into a capability guide covering everyday
  workflows, safety, OS/Runtime/RemoteUI behavior, asset development, memory,
  knowledge, and dynamic workflows.
- Hardened runtime tool projection and RemoteUI progressive response handling.
- Improved self-update behavior so Homebrew issues can fall back to direct
  release downloads.

### Fixed

- Scoped confirmation cleanup by tool id and kept confirmation streams draining,
  including resume behavior after pending confirmations.
- Confirmed knowledge-base deletion through the shared TUI confirmation flow.
- Refined footer context meter behavior and top process trend rendering.
