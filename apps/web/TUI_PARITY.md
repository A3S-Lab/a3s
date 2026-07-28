# A3S TUI to Web Product Mapping

The TUI is a capability reference, not the Web information architecture. A Web
capability is complete only when it participates in a coherent Work journey
with honest state, recovery, and browser verification.

## Current integrated journey

| TUI capability | Web intent | Current owner | State |
| --- | --- | --- | --- |
| New/resume session | Create or continue durable work | Unified conversation list | Integrated |
| Streaming transcript | Understand work without raw transport events | AI Assistant execution stream | Integrated |
| Live plan | Understand intended and current steps | Execution stream | Runtime-backed only; no synthetic progress |
| Busy-submit queue | Add follow-up work without interruption | Shared composer | Integrated |
| Stop/interrupt | Stop current execution without losing context | Shared composer | Integrated |
| Tool execution | Understand lifecycle, output, and failure | Tool timeline | Integrated |
| Tool confirmation | Decide a scoped sensitive action | Inline permission block | Integrated |
| `/model` | Select the active model | Composer model control | Integrated |
| `/effort` | Select reasoning effort | Composer effort control | Integrated |
| Shift+Tab / `/auto` | Select permission behavior | Composer mode control | Integrated |
| `/goal` | Preserve explicit intent | Composer command | Integrated |
| `/compact` | Reduce active context safely | Composer context state | Integrated |
| `/help` | Learn Web workflows | Settings / Help | Integrated |
| `@<path>` | Attach explicit file context | Composer context picker | Integrated |
| `/ide` files | Browse and edit local files | Work file and code scenes | Integrated |
| `/ide` search | Find workspace files and text | File search / quick open | Partially integrated; filename and mounted editor paths are complete |
| File operations | Create, rename, copy, cut, paste, move, delete | Work file manager | Integrated with inline naming and permanent-delete review |
| Git status/diff | Inspect repository changes | Local service and reusable workspace APIs | Not exposed as a separate Web product page |
| Stage/unstage/commit | Record reviewed changes | Local service | Deferred until a Work-native mounted review scene exists |
| Memory | Explore durable local memory | Memory at `#memory` | Integrated |
| Configuration | Manage account, model, agent, context, integrations, channels | Settings | Integrated |
| Local Claude Code, Codex, and WorkBuddy accounts | Reuse valid local account models without exposing credentials | Account Settings / runtime model catalog | Integrated |

## Web-only product capabilities

| Capability | Owner | Reason |
| --- | --- | --- |
| AI-native Home | Work | Outcome-first task entry and shortcuts |
| Unified conversation sidebar | Work | One identity across home, files, Office, and code |
| Finder/Explorer file UX | Work | Pointer, marquee, context menu, drag/drop, Quick Look |
| Office/PDF editing | Work | Native browser editing and compatibility workflows |
| Structured editor proposals | Work editors | Reviewable AI changes with live-target validation |
| Knowledge source packaging | Work + Knowledge | Separate base creation from compilation |
| Verified plugin activities | Shell + Plugins | Declarative vertical product contributions |

## Mapping rules

- A TUI command does not automatically become a top-level Web destination.
- Coding capabilities enter through file handlers inside Work.
- A command palette item is not parity unless its destination is mounted and
  recoverable.
- TUI execution and permission semantics remain authoritative.
- Browser-local UI state cannot replace durable service truth.
- A capability without a coherent Work scene remains deferred rather than
  linked to retired UI.

## Deferred capabilities

- a Work-native workspace-wide Git changes and commit scene;
- broad text replacement with a mounted, reviewable result surface;
- remote multi-user collaboration;
- terminal-first controls that do not yet have a safe Web interaction model.

Deferred capabilities may reuse typed APIs and internal components, but they
must not reintroduce a separate coding product, route, or session store.
