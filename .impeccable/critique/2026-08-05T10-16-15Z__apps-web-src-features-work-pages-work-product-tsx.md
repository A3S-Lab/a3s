---
target: "A3S Web Work primary flow: Home to task conversation to runtime and follow-up"
total_score: 20
max_score: 40
na_heuristics:
p0_count: 0
p1_count: 4
timestamp: 2026-08-05T10-16-15Z
slug: apps-web-src-features-work-pages-work-product-tsx
---
Method: dual-agent (A: /root/assessment_a · B: /root/assessment_b)

## Design Health Score

Scores use 0 as failed and 4 as excellent.

| # | Heuristic | Score | Key Issue |
|---|-----------|------:|-----------|
| 1 | Visibility of System Status | 2 | The header badge and floating runtime card report overlapping state, while the mobile runtime card visibly occludes the first user message. |
| 2 | Match System / Real World | 2 | “Workspace” alternately means a task property, a root folder, and a file-browser destination. |
| 3 | User Control and Freedom | 2 | While a task runs, Enter queues a follow-up but the visible primary button stops execution; Files also lacks an explicit return-to-task action. |
| 4 | Consistency and Standards | 2 | Equal-looking Home shortcuts create files, open dialogs, populate prompts, or navigate, and the implemented Activity Bar width differs from the design contract. |
| 5 | Error Prevention | 2 | Permission and destructive-action safeguards are strong, but the running composer makes accidental cancellation plausible. |
| 6 | Recognition Rather Than Recall | 2 | Paths and task history are visible, but users must remember scene semantics and discover queue behavior experimentally. |
| 7 | Flexibility and Efficiency | 3 | Keyboard submission, `@`, `$`, `/`, task history, model/effort controls, and queue editing serve experts well. |
| 8 | Aesthetic and Minimalist Design | 2 | The styling is calm, but Home exposes too many parallel starts and Conversation gives runtime chrome too much prominence. |
| 9 | Error Recovery | 2 | Loading, missing-task, disconnected, retry, and inline error states exist, but route and scene recovery affordances are not consistently direct. |
| 10 | Help and Documentation | 1 | Placeholder syntax and one reassurance line do not explain task/file/workspace ownership or running follow-up semantics. |
| **Total** |  | **20/40** | **Acceptable; substantial product-model work remains.** |

## Design Specificity Verdict

**Authored for A3S, but structurally under-resolved.** Visible local paths, durable tasks, permission modes, execution evidence, and file-aware controls give the flow product-specific character. The interaction model still feels like an AI chat launcher, an Office start page, and a file manager stitched together. The strangeness is semantic rather than cosmetic: one visual tier mixes actions with incompatible consequences, and “workspace” has no stable meaning.

**Deterministic scan:** The required CLI detector returned `[]` for `apps/web/src/features/work/pages/work-product.tsx`. This is a clean scan of the orchestration file, not of its rendered descendants. The injected browser detector found 25 patterns on Home and 12 on Conversation. Conversation findings comprised six undersized functional-text warnings, four low-contrast warnings, one shadow/border warning, and one flat-hierarchy warning. Home included clipped positioned children, undersized/tiny text, and low-contrast warnings. Some are likely context-sensitive false positives: the design system explicitly permits the composer shadow and supporting metadata below the body size, so exact selectors and contrast ratios must be verified before changing tokens.

**Visual overlays:** Injection succeeded in fresh browser pages and static overlay screenshots were captured. The available browser was headless, so no reliable user-visible `[Human]` tab was available. Browser geometry and screenshots were used as the fallback evidence.

## Cognitive Load

Six of eight checks fail: single focus, chunking, one thing at a time, minimal choices, working-memory demand, and progressive disclosure. Grouping and top-level visual hierarchy pass.

Decision points with more than four visible options include the five-destination product rail, seven Home capability shortcuts, six templates, roughly nine file-library controls, and approximately eight Conversation command/status controls on narrow layouts. Home exposed 40 visible interactive elements in the sampled desktop state.

## Emotional Journey

- **Arrival:** Calm and trustworthy; the outcome composer and local-path promise are convincing.
- **Preparation:** Confidence turns into hesitation because users must choose among starting a task, creating a file, opening a file, or entering a workspace.
- **Handoff:** The dedicated conversation is the strongest moment; title, path, transcript, and selected history row establish place and continuity.
- **Execution:** Confidence weakens because status is split between the header and a floating runtime card.
- **Follow-up:** The largest valley: keyboard submission means queue, while the visible primary button means stop.
- **Completion:** Status is clear, but runtime completion is more prominent than produced artifacts, evidence, or a next action tied to the outcome.

## Overall Impression

The implementation is technically coherent and visually restrained, but its product ontology is not. The largest opportunity is to make one object clearly primary: a durable task owns a workspace root and related files; Home starts or resumes that task; Files and editors are subordinate scenes; runtime details support the transcript instead of competing with it.

## What’s Working

- The dedicated conversation successfully replaces the distracting side-panel destination and gives each task a stable identity.
- Local path visibility, permission language, durable history, and text-plus-icon status support A3S’s local-first and accountable positioning.
- The shared composer, route-state components, editable queue, and unified task sidebar are a strong technical foundation for a simpler experience.

## Priority Issues

### P1 — Home mixes incompatible action semantics

**Why it matters:** `新建文档` creates an artifact, `分析数据` only fills the composer, `打开文件` opens a picker, and `浏览工作区` navigates away. Equal visual weight falsely promises equal behavior, while duplicate file actions and templates create a choice wall.

**Fix:** Make the composer the sole primary task-start path. Keep at most three or four prompt starters, all with the same “populate but do not submit” behavior. Move direct create/open/browse actions into one clearly labeled Files section and remove duplicates.

**Suggested command:** `$impeccable clarify`

### P1 — Running follow-up and Stop are contradictory actions

**Why it matters:** The editor stays enabled and Enter calls `sendMessage`, which queues a follow-up, while the visible arrow button becomes Stop and calls cancellation. Pointer and keyboard users receive different meanings from the same composer.

**Fix:** Keep a persistent **Add follow-up** action while running. Move **Stop execution** next to live status as a separate, labeled danger action. Add “Runs after the current instruction” copy and reveal the queued row immediately.

**Suggested command:** `$impeccable harden`

### P1 — Runtime chrome obscures the work it is meant to explain

**Why it matters:** The header badge and floating runtime card duplicate status, the card is detached from the active turn, and at 360 × 800 it visibly overlaps the first user message. After completion, infrastructure progress remains more prominent than delivery evidence.

**Fix:** Keep one concise global status in the header. Attach plan/tool progress to the active assistant turn or a single execution drawer, reserve content space instead of overlaying it, and collapse terminal runtime into the delivery summary with artifact/evidence actions.

**Suggested command:** `$impeccable layout`

### P1 — Mobile and keyboard access are mechanically fragile

**Why it matters:** Narrow-layout primary controls measure 24–34 px rather than the 44 px touch floor. The visually hidden 1 × 1 file input remains in sequential Tab order, creating an invisible focus stop.

**Fix:** Expand narrow-breakpoint hit areas to at least 44 × 44 without enlarging every icon, maintain separation between adjacent actions, remove the hidden input from Tab order or use a correctly associated visible label, and verify focus order plus 200% zoom.

**Suggested command:** `$impeccable adapt`

### P2 — Task, workspace root, and Files lack a stable hierarchy

**Why it matters:** “Workspace” is both a selected root and a destination. `打开工作区` leaves the canonical conversation for Files without a prominent task-return affordance, making users reconstruct context.

**Fix:** Define the task as the parent object, workspace root as its property, and Files as a related scene. Rename the action to **View task files**, use **Change folder** for root selection, retain the task/status ribbon in Files, and provide **Back to conversation**.

**Suggested command:** `$impeccable shape`

## Persona Red Flags

**Lin, local-first researcher:** The visible path and confirmation promise build trust. The seven shortcut semantics do not reveal whether “Analyze data,” “Open file,” and templates create durable tasks or bypass them, so Lin hesitates before the first meaningful action.

**Alex, developer/power user:** Keyboard syntax, queue editing, and run controls are valuable. The Enter-to-queue versus button-to-stop split is a severe trust break, and a route race observed in one independent pass would make Alex distrust copied links until an end-to-end check proves stability.

**Casey, interrupted mobile user:** The 52 px Activity Bar permanently consumes narrow width, the runtime card obscures conversation content, and several 24–34 px controls are hard to target. Moving from Conversation to Files also hides the return path Casey needs after interruption.

## Minor Observations

- Home repeats `打开文件`; Conversation and the sidebar repeat `新建任务`.
- The desktop Activity Bar measured 76 px while `apps/web/DESIGN.md` specifies 52 px; resolve which source is authoritative.
- Repeated “Copy message” accessible names need message/author context in long threads.
- Six templates create a very long narrow-screen block before recent work.
- One independent direct-load pass observed the Conversation DOM with a `#home` URL, while the mechanical click-through pass retained `#conversation/demo-session`. Treat this as a route-race verification item, not a confirmed defect, and add an end-to-end test.
- Detector contrast and clipped-child findings need selector-level manual confirmation; do not blindly restyle all muted metadata.

## Questions to Consider

1. Is Home fundamentally for **starting/resuming outcomes** or for **creating/opening files**? If both remain, which is explicitly secondary?
2. While execution is running, should a submitted instruction **queue by default** or **interrupt**? Where should Stop live so those intents cannot be confused?
3. Is the workspace root a stable **property of a task** or a standalone **destination**? Labels and navigation should commit to one model.
