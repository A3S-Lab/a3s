# A3S Web Core User Journeys

## 1. Start work from Home

**Intent:** begin with an outcome instead of choosing an application type.

1. Open A3S Web at `#home`.
2. Review the active workspace and conversation sidebar.
3. Describe the outcome in the full composer.
4. Optionally attach `@` files, `/` Skills, model, effort, and execution mode.
5. Submit.
6. The shared AI Assistant opens and shows execution in the new or active
   conversation.

Success: Home and the assistant show the same draft and session; no product
switch occurs.

## 2. Resume a conversation

**Intent:** continue previous work with its own context.

1. Search or select a row in the left conversation list.
2. The current draft and workspace snapshot are saved.
3. The selected conversation, draft, queue, and workspace are restored.
4. The AI Assistant opens on that conversation.

Success: historical sessions from earlier releases remain visible, and a
running status never leaks onto another row.

## 3. Browse and organize local files

**Intent:** manage real folders with familiar desktop behavior.

1. Enter the local file scene from Home.
2. Choose or switch the root folder.
3. Navigate with history, breadcrumbs, grid/list, filter, sort, or recursive
   filename search.
4. Select with click, marquee, checkbox, Shift range, or keyboard.
5. Use the selection shelf or right-click menu for copy, cut, paste, duplicate,
   favorite, reveal, Quick Look, AI context, or knowledge-base creation.
6. Create and rename directly inline.

Success: empty-space right click opens folder commands and clears stale item
selection. Only permanent deletion interrupts with confirmation.

## 4. Open a code, text, or Markdown file

**Intent:** edit source without leaving Work.

1. Open a non-binary file from the file manager, quick open, AI artifact, or
   semantic navigation.
2. Work enters the code scene and keeps the conversation sidebar and active AI
   session.
3. Edit in Monaco; Markdown also shows live preview.
4. Save with Cmd/Ctrl+S.
5. If disk content changed externally, choose reload or explicit overwrite.
6. Return to the file manager using the scene back action.

Success: no Code route or second session is created.

## 5. Open and edit an Office file

**Intent:** work on a local document, spreadsheet, presentation, or supported
PDF while retaining source identity.

1. Open the file from files or quick open.
2. Work imports or reuses its managed compatibility artifact and records the
   local path fingerprint.
3. Edit in the format-specific Work editor.
4. Use selected text, ranges, slides, or elements to prepare an AI request.
5. Review any structured proposal and select changes.
6. Apply only live-matching targets.
7. Save back or Save As after compatibility and external-change review.

Success: autosave never implies that the original local file was overwritten.

## 6. Open a task-produced file

**Intent:** inspect an AI result in the same product.

1. A completed tool call or artifact card exposes a file path.
2. Select the file entry.
3. Work routes it to the code/text or Office handler.
4. The current conversation remains selected in the sidebar and assistant.

Success: no control navigates to the retired Result Workspace.

## 7. Review delivery evidence

**Intent:** understand what completed and what remains risky.

1. Read required, passed, pending, failed, and residual-risk counts in the
   delivery summary.
2. For incomplete validation, choose Continue correction to prepare a bounded
   follow-up.
3. Choose Prepare review to add an evidence-based review request to the same
   conversation.
4. Edit and submit the prepared instruction.

Success: review remains visible and editable rather than switching to a hidden
task page.

## 8. Create a knowledge base from files

**Intent:** package one or many files/folders without forcing compilation.

1. Select files, folders, or a mixed set in the file manager.
2. Choose Create knowledge base.
3. Review parent/child de-duplication, effective roots, file count, and size.
4. Name and create the base.
5. Choose whether to compile now, later, or enable guarded automatic
   compilation for this base.

Success: base creation succeeds independently of compiler availability.

## 9. Compile a knowledge base

**Intent:** generate a searchable OKF wiki from a stable source package.

1. Open the base at `#knowledge`.
2. Start compilation explicitly, or let its enabled policy observe changes.
3. Automatic policy waits for file stability, a quiet window, and its minimum
   interval; suspicious bulk change pauses the run.
4. Monitor queued, running, failed, or promoted status.
5. On success, the staged generation is validated and promoted.

Success: failure retains the last promoted searchable generation.

## 10. Explore Memory

**Intent:** inspect long-term local memory without changing active work.

1. Open Memory from the Activity Bar, command palette, or `#memory`.
2. Search and combine filters over the complete local store.
3. Switch between bounded graph and timeline projections.
4. Select a memory or entity for detail.
5. Return to Work.

Success: the Work conversation, draft, and workspace are unchanged.

## 11. Use a plugin activity

**Intent:** use a verified vertical capability and hand its result to Work.

1. Open an enabled Activity Bar contribution.
2. Complete activity-specific exploration inside the isolated host.
3. Review the proposed context summary and optional package Skill.
4. Accept the handoff.
5. Return to `#home` with context appended to the current editable draft.

Success: the plugin cannot submit, mutate files, or attach a Skill without the
reviewed handoff.

## 12. Recover from disconnection

**Intent:** retain work when the local service becomes unavailable.

1. The persistent service banner reports the interruption.
2. Unsaved browser drafts and editor content remain visible.
3. Retry reconnect.
4. The app reloads authoritative service state.
5. Context guards discard responses for conversations or workspaces that are no
   longer active.

Success: reconnect does not claim success from a health-only response and does
not overwrite newer local edits.
