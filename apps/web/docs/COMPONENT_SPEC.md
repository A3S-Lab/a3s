# A3S Web Component Specification

## Purpose

This document defines the current component boundaries for the unified A3S Web
workbench. It is an ownership and continuity contract, not a catalog of backend
capabilities.

Component rules:

- one component owns one user-visible concern;
- feature components receive typed actions and do not duplicate API clients;
- Home, files, Office, and code are scenes inside Work;
- one conversation list, one canonical conversation page, and one shared task
  state serve every Work scene;
- `WorkCopilot` is contextual to file/editor scenes and is not the destination
  for Home submission or task selection;
- inline interactions own reversible naming and row mutations;
- dialogs are reserved for destructive, conflicting, or compatibility-sensitive
  decisions;
- a capability appears only when its useful state, recovery, and next action
  are implemented.

## Composition

```text
App
├── CodeBootScreen                       internal bootstrap boundary
└── AppShell
    ├── ActivityBar
    ├── ProductWorkspace
    │   ├── WorkProduct                  #home / #conversation/<session>
    │   │   ├── TaskLibrary              unified conversation list
    │   │   ├── WorkHome
    │   │   │   ├── WorkHomeHero
    │   │   │   │   └── TaskComposer
    │   │   │   └── WorkLibraryCards
    │   │   ├── WorkConversation
    │   │   │   ├── ExecutionStream / TaskRuntimeFloatingPanel
    │   │   │   └── TaskComposer
    │   │   ├── WorkFilesWorkspace
    │   │   │   ├── WorkFilesSidebar
    │   │   │   ├── WorkFilesView
    │   │   │   ├── WorkQuickLook
    │   │   │   └── WorkFilesContextMenu
    │   │   ├── WorkEditorShell
    │   │   │   ├── DocumentEditor
    │   │   │   ├── SpreadsheetEditor
    │   │   │   └── PresentationEditor / PDF handler
    │   │   ├── WorkCodeWorkspace
    │   │   │   ├── WorkIdeExplorer
    │   │   │   └── MonacoCodeEditor
    │   │   └── WorkCopilot
    │   │       ├── ExecutionStream
    │   │       └── TaskComposer
    │   ├── KnowledgePage                #knowledge
    │   ├── MemoryPage                   #memory
    │   ├── PluginHostPage               #plugin/<key>
    │   └── PluginMarketplacePage        #plugins
    └── GlobalOverlays
        ├── SettingsDialog
        └── CommandPalette
```

There is no `CodeProduct`, `TasksPage`, `NewTaskPreparation`, `TaskHeader`, or
Work-specific navigation sidebar in the mounted composition. Coding is entered
through `WorkCodeWorkspace`; conversation navigation stays in `TaskLibrary`.

## Shell components

### `CodeBootScreen`

**Role:** bridge the blank document and authoritative A3S workspace with one
bounded loading or recovery card.

**Contract:** the user-visible identity is A3S Web. Failure copy distinguishes
an unavailable local service from a page/service version mismatch, keeps raw
details collapsed, and offers one reconnect action.

### `AppShell`

**Role:** compose product navigation, the active product, Settings, service
recovery, and global overlays.

**Inputs:** task, plugin, knowledge, and channel actions plus the authoritative
application state.

**Contract:** `activeProduct=work` always mounts `WorkProduct`. No task view or
route may mount a retired Code shell. Reconnect reloads authoritative data
without discarding unsaved client state.

### `ActivityBar`

**Role:** open Work, Knowledge, verified plugin contributions, Memory,
Marketplace, and Settings.

**Contract:** Work is first, selected by default, and its Activity Bar action
navigates to `#home`. Durable task links use `#conversation/<session>`.
Knowledge follows Work. Memory, Marketplace, and Settings remain pinned in the
system group. There is no separate coding or Office icon.

### `CommandPalette`

**Role:** provide keyboard access to implemented global and Work actions.

**Contract:** commands use Work terminology and canonical routes. It must not
expose retired task review, activity, Code, or Office product destinations.

## Unified conversation components

### `TaskLibrary`

**Role:** create, search, select, rename, and delete conversations for every
Work scene.

**Visual contract:** one compact sidebar, one New conversation action, an
on-demand search field, and dense single-line rows. Rename and delete operate
inside the affected row, with Enter/Escape behavior and inline failure state.

**Data contract:** render the unified session catalog, including historical
sessions created before the product merge. New conversations use the default
agent. Selecting a session restores its workspace and opens its canonical
conversation route.

### `TaskComposer`

**Role:** collect a goal, context, Skills, execution mode, model, and effort,
then submit or queue it through the durable task controller.

**Contract:** the Home, conversation-page, and contextual AI Assistant instances
read and write the same active draft. `@` file, `$` Skill, and built-in `/`
command suggestions,
drag-and-drop context, workspace
selection, keyboard submission, queue state, and recovery behave consistently.

### `WorkspaceQuickOpen`

**Role:** find files from any Work scene and open them in the unified file
handler.

**Contract:** the overlay is owned by `WorkProduct`, orders its open code tabs
first, and delegates opening back to Work so Office and text files reach the
correct scene. It never opens the retired Result Workspace.

### `ExecutionStream`

**Role:** project the active conversation into readable turns, tool activity,
permissions, interruptions, evidence, and recoveries.

**Contract:** tool decisions remain next to the requesting operation. File
actions open a supported Work file scene; controls that require a retired
workspace are not rendered. Streaming updates stay scoped to their originating
session.

### `WorkConversation`

**Role:** own the full-center, reload-safe task conversation addressed by
`#conversation/<opaque-session-key>`.

**Contract:** render only when the requested route session exists and is active;
otherwise show opening, service-recovery, or missing-task state without leaking
another transcript. Keep task identity/status, `ExecutionStream`,
`TaskRuntimeFloatingPanel`, and the active `TaskComposer` in one responsive
surface. It never mounts `WorkCopilot`.

## Work scene owner

### `WorkProduct`

**Role:** own the Work route family, conversation sidebar, scene selection,
Home-to-conversation handoff, and contextual assistant beside file/editor
workspaces.

**Scene priority:** a conversation route renders `WorkConversation` before any
stale file state. Otherwise an active Office artifact renders `WorkEditorShell`;
open code/text tabs render `WorkCodeWorkspace`; then the persisted library or
local-files surface renders.

**Continuity:** scene changes preserve the same active session and draft. A
local path can open the applicable code, text, Office, PDF, or unsupported-file
state without creating another product identity.

### `WorkHome`

**Role:** provide the AI-native starting point and managed artifact library.

**Contract:** the home view leads with `WorkHomeHero` and the complete composer.
Implemented shortcuts create artifacts, open files, prepare editable drafts,
or enter the local file manager. Reversible folder and artifact naming is
inline; permanent deletion remains confirmed.

### `WorkHomeHero`

**Role:** frame the user's intended outcome and host the production composer.

**Contract:** starters populate an editable draft. Submission opens the active
session route immediately or waits for the newly created durable session key;
failure remains on Home. The hero never opens the contextual assistant and does
not imitate unavailable media or remote-generation features.

## Local file scene

### `WorkFilesWorkspace`

**Role:** compose the local file sidebar, toolbar, file surface, selection
actions, context menus, Quick Look, naming operations, and AI Assistant toggle.

**Contract:** background clicks and background context menus clear item
selection. The sidebar-open control and workspace controls share the same
visual language as the conversation sidebar.

### `WorkFilesView`

**Role:** render filtered and sorted entries in grid or list form and own
pointer/keyboard selection semantics.

**Selection contract:** support marquee, additive, range, checkbox, and
keyboard selection. Right-click preserves an existing multi-selection when the
target belongs to it; otherwise it selects the target. Empty padding owns the
folder-background menu.

**Inline contract:** create, rename, and duplicate naming use an in-place field
in both layouts. Enter confirms, Escape cancels, and failed validation remains
next to the field.

### `WorkFilesContextMenu`

**Role:** expose actions appropriate to the current folder background or item
selection.

**Contract:** commands are selection-aware, grouped by intent, and disabled
when their preconditions are false. Permanent local deletion is the only common
file mutation that escalates to confirmation.

### `WorkQuickLook`

**Role:** preview one visible entry without creating or autosaving an artifact.

**Contract:** folders show metadata; bounded text, images, PDF, and supported
Office files use safe read-only handlers. Arrow keys move through the current
visible ordering. Oversized or unsupported binaries are not read speculatively.

## Editing scenes

### `WorkCodeWorkspace`

**Role:** edit code, text, and Markdown inside Work.

**Contract:** use the selected local root, lazy explorer, multiple Monaco tabs,
diagnostics, semantic navigation, external-change conflict review, safe save,
and AI context actions. Markdown keeps source and live preview visible together.
Back returns to the file manager; it does not navigate to another product.

### `WorkEditorShell`

**Role:** select the correct Office/PDF editor, provide shared file chrome,
manage local binding, and expose the AI Assistant toggle.

**Contract:** save-back validates the source fingerprint and compatibility
report. Save As, external conflict, degraded export, and irreversible discard
use explicit reviewed flows.

### Office editors

`DocumentEditor`, `SpreadsheetEditor`, and `PresentationEditor` own their
format-specific ribbon, canvas, selection, undo/redo, review panels, status,
preview, print, and export behavior. They receive typed agent-request callbacks
and never apply assistant prose directly. Structured proposals are checked
against live targets before application.

### `WorkCopilot`

**Role:** host the shared contextual AI Assistant beside a file, Office, PDF, or
code scene.

**Contract:** bind to the selected workspace, render only the active compatible
session, and use the unified composer and execution stream. New conversation
creates a default-agent session in the same sidebar. At compact desktop widths
the assistant becomes an overlay instead of squeezing the editor. It is absent
from Home and `WorkConversation`.

## Knowledge and Memory

### `KnowledgePage`

**Role:** create, import, search, pin, edit, and compile local knowledge bases.

**Contract:** knowledge-base creation and compilation are separate. Automatic
compilation is opt-in per base and respects stability, quiet-window,
minimum-interval, retry, and bulk-change safeguards.

### `MemoryPage`

**Role:** compose the read-only Memory journey at `#memory`.

**Contract:** entering or leaving Memory does not change the Work session or
draft. Search, filters, timeline, and graph use the complete loaded store;
rendering stays bounded and stale successful data remains usable after refresh
failure.

## Plugins and Settings

### `PluginHostPage`

**Role:** render verified plugin content in its isolated host and mediate
reviewed context handoff.

**Contract:** accepted context appends to the current unified Work draft and
returns to `#home`. Removed or unavailable plugin keys cannot leave a blank
surface.

### `SettingsDialog`

**Role:** provide Account, Appearance, Model, Agent, Context, Integrations,
Channels, About, and Help as a global modal.

**Contract:** category data is lazy and independently retryable. Failed saves
preserve the draft; secrets never return to the browser. Help uses
`#settings/help`; channels use `#settings/channels/<channel>`.

## State ownership

- `appState.activeProduct` owns only top-level Work, Memory, Knowledge, Plugins,
  and plugin-host selection.
- Work scene state is local to `WorkProduct` and does not appear in the route.
- task state owns one session collection, active-session key, draft collection,
  queue state, and workspace snapshot collection.
- file and Office controllers own their format-specific data but receive the
  same task actions.
- legacy storage keys may be read only to recover user data; all new writes use
  the unified keys.

## Verification

Component changes must include focused tests for state ownership, route
normalization, keyboard and pointer behavior, async stale-response guards, and
inline recovery. The Web app must also pass type checking, lint, formatting,
the full test suite, production build, and browser regression of the canonical
routes and primary Work scenes.
