# A3S Work AI-Native Home

## Product intent

A3S Web opens its canonical `#home` route with the user's intended outcome,
not with a file grid or a product chooser.
The home surface therefore makes the durable A3S task composer the primary
entry point while retaining templates, folders, and recent files as supporting
content below it.

The interaction model is informed primarily by the current Codex desktop app:
the app acts as a command center, local folders provide project context,
distinct outcomes stay in distinct task threads, and real outputs remain close
to the conversation that created them. A3S keeps its own local-workspace,
Office editing, and safety contracts rather than copying Codex visuals.

Reference baseline:

- [ChatGPT desktop app](https://learn.chatgpt.com/docs/app)
- [Projects and chats](https://learn.chatgpt.com/docs/projects)
- [Code review](https://learn.chatgpt.com/docs/code-review)
- [Codex environments](https://learn.chatgpt.com/docs/environments/modes)

## Information architecture

The Work home is ordered as follows:

1. A3S identity and one outcome-oriented question.
2. A compact current-task strip when a task is selected. It offers explicit
   resume and new-task actions but never replaces the composer.
3. The task composer and its real queue submission path. The selected local
   workspace stays visible with both folder name and path. Execution mode stays
   at hand; research mode, effort, model, and context controls move into an
   on-demand Run settings panel.
4. One-click capabilities that map only to implemented actions: create text,
   spreadsheet, or presentation artifacts; open a file; prepare data-analysis
   or file-organization tasks; and open the local-files workspace.
5. Managed templates, folders, and recent files for direct manipulation.

Recent, Favorites, Trash, and managed-folder views remain file-management
surfaces. They do not repeat the home hero.

## Interaction contract

- A fresh A3S Web installation opens the AI home. An explicit previous choice to
  use the local-files surface is still restored.
- Home does not mount the contextual AI Assistant. After an existing session is
  submitted, or a new session receives its durable key, Work enters
  `#conversation/<opaque-session-key>`.
- The Home and conversation composers use the same task draft and durable
  runtime. Keyboard submission and the send button follow the same path. A
  creation failure remains on Home with the draft and selected context intact.
- Selecting an existing task keeps the composer available for a follow-up.
  Resume opens that task's dedicated conversation page. Starting a different
  outcome is always an explicit New task action, so transcripts do not blend.
- Workspace identity is persistent context, not an advanced setting. The full
  path remains visible before a new task is submitted.
- Defaults carry the common path. Model choice, inference effort, deep research,
  and context maintenance remain available without competing with Send.
- Home, files, Office editing, and code editing are scenes in one Work product.
  The conversation page and contextual file assistant share the same
  conversation list, active session, draft, context, queue, and default agent.
  Scene changes never create or restore a second hidden session.
- Asynchronous session creation is correlated with the draft that initiated it,
  so a late response cannot replace a conversation the user selected while the
  request was in flight.
- Prompt starters populate an editable draft; they never auto-send.
- Creating an Office artifact, opening a file, or entering the local workspace
  stays a direct operation rather than becoming an artificial chat step.
- Potentially destructive file organization remains plan-first and must use the
  existing reviewed file-operation boundaries.

## Delivery plan

### Phase 1 — Task-first home

Implemented in the first release:

- AI-native hero and full task composer.
- Persistent composer with separate resume-current and new-task actions.
- Visible workspace identity and progressive Run settings.
- Readable top-level navigation labels with a compact mobile rail.
- User-facing Knowledge and Settings terminology that describes outcomes before
  implementation details.
- Real capability shortcuts with no placeholder media features.
- Task-submit-to-independent-conversation transition.
- Reload-safe conversation route, browser-history restoration, and truthful
  missing-session recovery.
- Contextual assistant limited to file, Office, PDF, and code scenes.
- Existing templates and file library preserved below the task entry.
- Responsive layouts for a narrow Work pane and a 360 px viewport, plus dark
  theme support and keyboard-accessible controls.

### Phase 2 — Personalized starting points

- Rank task starters from recent Work outcomes without exposing file content.
- Add resumable recent tasks and generated-output previews beside recent
  files.
- Provide a searchable Skill catalog launched with `$`, while reserving `/` for
  built-in commands.
- Add format-aware starters when the current workspace contains spreadsheets,
  presentations, or document collections.

### Phase 3 — Proactive Work orchestration

- Allow reviewed task recipes to be saved and scheduled.
- Surface workspace-change suggestions without auto-running destructive work.
- Hand completed Work outputs to Knowledge as explicit source packages, keeping
  knowledge-base creation and compilation as separate operations.
- Add outcome and recovery telemetry that is local by default and opt-in for
  aggregate product analytics.

## Success measures

- Median time from opening Work to the first submitted task.
- Share of home sessions that start a task, open an existing file, or create an
  artifact without navigation detours.
- Task submission failure and recovery rate.
- Successful Home-to-conversation transition and deep-link recovery rate.
- Keyboard completion and narrow-pane overflow regressions.

## Release acceptance

- The home composer submits through the production unified task controller.
- `@` file references, `$` Skill mentions, built-in `/` commands, execution
  mode, model, and effort remain
  available; advanced controls may be progressively disclosed.
- An active task never replaces or hides the composer, and New task is available
  beside the current-task strip.
- The selected workspace name and path are visible before task submission.
- Starting a task enters exactly one independent conversation page without
  opening the contextual assistant or creating a second conversation.
- Switching among home, files, Office, and code scenes preserves the same
  session ID and draft, including when session creation completes in flight.
- `#home` is the Work entry and `#conversation/<opaque-session-key>` is the only
  durable Work subroute; removed or malformed product-prefixed routes are not
  retained.
- Non-home library views retain direct file-management behavior.
- The page has no horizontal overflow at 360 px and remains legible in light and
  dark themes.
- Focused component tests, type checking, lint, formatting, production build,
  and browser regression all pass.
