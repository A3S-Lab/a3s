# A3S Web Domain and State Model

## Work product

Work is the single task product. `#home` is its outcome-first entry and
`#conversation/<opaque-session-key>` addresses a durable conversation.

```text
Work
├── unified conversations
├── canonical conversation page
├── active workspace
├── home and managed artifacts
├── local filesystem
├── Office/PDF handlers
├── code/text handler
└── contextual file AI Assistant
```

Home, the conversation page, files, Office, and code are presentation scenes.
Only the durable conversation has its own Work subroute; no scene owns a second
product identity or session store.

## Workspace

The bounded local project or folder context.

```text
Workspace
├── root path and service health
├── files and directories
├── repository metadata
├── code intelligence
├── configuration diagnostics
└── conversations
```

The local service is authoritative for disk content. Client service states are
`checking`, `connected`, and `disconnected`, with an optional recoverable error.

## Conversation

The durable unit of user intent, adapted from a service session.

```text
Conversation
├── identity and workspace
├── turns and messages
├── run configuration
├── active execution output
├── queued follow-up instructions
├── selected context
├── artifact references
└── verification delivery
```

All historical sessions belong to the same catalog. New sessions use the
default agent. Presentation states include draft, idle, running, waiting for a
decision, stopped, failed, and completed.

## Draft

A conversation-scoped or new-conversation input snapshot.

```text
Draft
├── content
├── workspace-relative context paths
├── Skills
├── model and effort
└── execution mode
```

The Home composer, conversation-page composer, and contextual file assistant
edit the same active draft. Switching Work scenes does not snapshot or replace
it; switching conversations does.

## Turn

One user instruction and its resulting assistant work. A turn can contain text,
reasoning, a plan, executions, permission requests, proposals, artifacts, and
verification evidence.

## Execution

One semantic lifecycle for a tool or runtime operation.

```text
preparing → waiting for permission → running
          → succeeded | failed | denied | timed out | cancelled
```

Persisted and live events with the same identity collapse into one projection.
Denial and timeout remain terminal decision outcomes even if later transport
events contain generic failure metadata.

## Permission request

Contains execution identity, operation, reason, affected scope, timeout, and
allowed decisions. It cannot be applied to another execution.

## Context reference

A bounded reference attached to the next instruction. Sources include local
paths, selected text, spreadsheet ranges, slides, elements, plugin evidence,
and knowledge excerpts. A reference is visible and reviewable; it does not
silently copy or mutate the source.

## Artifact reference

An addressable result in a conversation turn.

```text
ArtifactReference
├── stable identity
├── owning conversation and turn
├── kind
├── label and summary
└── domain locator
```

File artifacts open through Work's file handler. The reference never becomes a
second copy of disk truth.

## Verification delivery

Service evidence containing required, passed, pending, failed, and residual-risk
data. It is the source of delivery status. Preparing further review appends a
request to the same conversation.

## Local filesystem state

```text
LocalFilesState
├── root and current path
├── navigation history
├── entries and bounded search result
├── grid/list, sort, and filter preferences
├── selected paths and focus anchor
├── favorites
├── copy/cut clipboard
├── inline operation
└── active mutation paths
```

Selection identity is the normalized absolute local path. Background actions
clear item selection. Search results retain real path identity and parent
location.

## Code/text editor state

```text
WorkCodeState
├── open tabs
│   ├── path
│   ├── loaded disk content
│   ├── current draft
│   ├── loading and save state
│   └── optional navigation location
├── active path
├── external-change conflict
└── guarded close request
```

Disk and draft content remain separate. The controller refuses silent overwrite
after an external change and keeps dirty tabs until the user resolves them.

## Managed artifact

```text
WorkArtifact
├── stable ID, kind, title, folder, and timestamps
├── favorite and trash state
├── revision and compatibility state
├── optional local path binding and source fingerprint
└── format-specific content
```

Managed artifacts support autosave, recovery, versioning, and Office
interoperability. A bound local path remains the user-facing file identity.

## Structured proposal

```text
Proposal
├── originating conversation response
├── target artifact and bounded source selection
├── typed changes with trusted before/after values
├── per-change selection
└── applied, stale, skipped, or failed result
```

The owning editor revalidates each selected target against live content before
applying it.

## Knowledge base

```text
KnowledgeBase
├── identity, title, and workspace
├── source package and manifest
├── compilation policy
├── staged and promoted generations
├── searchable wiki
└── compilation history and errors
```

Creation and compilation are separate state transitions. Failed compilation
cannot replace the last successful promoted generation.

## Memory state

```text
MemoryState
├── load and refresh phase
├── last successful complete snapshot
├── query and combinable filters
├── graph/timeline view and bounded projection
├── selected memory or entity
└── evolution candidates and history
```

Only the latest active request can replace the snapshot. Refresh failure keeps
stale successful data available.

## Top-level navigation state

```text
ProductId = work | memory | knowledge | plugins | plugin
WorkRoute = home | conversation
conversationSessionId = opaque session key | null
```

Work scene selection is not part of `ProductId`. Settings is an overlay with a
canonical tab route and closes back to the underlying product route, including
an addressed conversation.

## State ownership

- The service owns sessions, messages, execution controls, output, disk files,
  repository truth, durable Knowledge, and durable Memory.
- shared client state owns the active product, Work route, addressed and active
  conversation selection, drafts, queues, workspace snapshots, overlays, and
  request guards;
- `WorkProduct` owns current Work scene, Home-to-conversation transition, local
  code tabs, and contextual AI Assistant presentation;
- file and editor controllers own their bounded operation state;
- local storage is best effort for theme, active conversation, drafts, safe
  workspace continuity, and Work presentation preferences;
- legacy keys can be read to recover data but are not new write targets;
- mutation failure retains the previous authoritative value;
- browser state cannot reconstruct durable service truth after a failed refresh.
