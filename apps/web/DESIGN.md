# A3S Web Design System

## Purpose

This document defines the visual and interaction language for A3S Web. It
adapts mature desktop workspace patterns, the calm task-first hierarchy visible
in products such as Skywork and WorkBuddy, and the A3S brand into one coherent
local AI workspace.

The product architecture is authoritative: Work is the only task workbench,
coding is a Work scene, and the left Work sidebar is always the conversation
list.

Related contracts:

- [Product blueprint](docs/PRODUCT_BLUEPRINT.md)
- [Component specification](docs/COMPONENT_SPEC.md)
- [Domain and state model](docs/DOMAIN_MODEL.md)
- [Core user journeys](docs/USER_JOURNEYS.md)

## Design character

```text
Calm and warm · clear and trustworthy · lightly professional
guided · efficient without pressure
```

The interface should feel capable without looking like an IDE control wall or
a promotional landing page.

## Experience principles

1. **The next action is visible.** A user should understand what to do without
   opening Help.
2. **One concept has one visual treatment.** Conversation rows, workspace
   controls, file actions, and status language do not change style by scene.
3. **Content carries the hierarchy.** Borders, spacing, and typography do more
   work than cards and shadows.
4. **Accent color communicates state.** A3S blue marks focus, selection,
   progress, and primary intent; it is not decoration.
5. **Routine actions stay in context.** Inline create and rename preserve
   spatial continuity.
6. **Risk earns interruption.** Dialogs are reserved for destructive,
   conflicting, or compatibility-sensitive decisions.
7. **AI remains accountable.** Selected context, tool scope, proposals, and
   verification evidence are inspectable.

## Foundations

### Color tokens

Use semantic tokens from `src/styles/base.css` rather than literal colors in
feature CSS.

| Purpose | Light | Dark | Token |
| --- | --- | --- | --- |
| Canvas | `#f7f7f8` | `#101118` | `--a3s-bg` |
| Main panel | `#ffffff` | `#171820` | `--a3s-panel` |
| Soft surface | `#f2f3f5` | `#1e2028` | `--a3s-panel-soft` |
| Strong surface | `#e9ebef` | `#282b35` | `--a3s-panel-strong` |
| Primary text | `#17181a` | `#f2f3f5` | `--a3s-ink` |
| Secondary text | `#71757d` | `#a4a8b2` | `--a3s-muted` |
| Border | `#e2e4e8` | `#2a2d35` | `--a3s-line` |
| Focus/accent | `#2864e8` | `#6ca3ff` | `--a3s-blue` |
| Success | `#14a675` | `#3ccf91` | `--a3s-green` |
| Danger | `#d84b4f` | `#f27777` | `--a3s-red` |
| Warning | `#c97816` | `#eda94c` | `--a3s-warning` |

Primary action buttons use the neutral action token. Blue remains a focused
product accent so a page does not become visually saturated.

### File colors

File type is one of the few places where persistent color improves scanning.
Use the shared folder, TypeScript, JavaScript, JSON, document, style, markup,
image, and configuration tokens. The same extension uses the same glyph and
color in grid, list, quick open, artifact entries, and code explorer.

### Typography

The UI uses the system sans stack with Chinese platform fonts. Use 13 px as the
default dense workspace size, 11–12 px for supporting metadata, 14–16 px for
section headings, and larger display type only on the fresh AI Home hero.

Avoid uppercase English labels when a short Chinese label is clearer. Keep
model names, file extensions, code, and standard technical terms unchanged.

### Spacing and shape

- base rhythm: 4 px;
- compact control height: 28–34 px;
- standard control height: 36–40 px;
- sidebar row: 34–36 px;
- small radius: `--a3s-radius-sm` (9 px);
- panel/card radius: `--a3s-radius` (16 px);
- pills are reserved for status, filters, and removable context;
- use one-pixel semantic borders before adding shadow.

### Elevation

Most workspace panels are flat. Use `--a3s-shadow-soft` for menus and transient
popovers, `--a3s-shadow` for dialogs, and `--a3s-shadow-composer` only where the
Home composer needs separation from the canvas.

## Icon system

Lucide is the shared icon family. Use the shell stroke token (1.75) and keep
icons optically aligned on a 16, 18, or 20 px box.

Rules:

- the same action always uses the same icon across pages;
- Activity Bar icons are monochrome and state-driven;
- framed 25 px command glyphs identify high-level sidebar actions;
- file glyphs may use type colors and extension badges;
- do not mix filled emoji, platform icons, and Lucide inside one navigation
  group;
- labels remain available through visible text, tooltip, or accessible name;
- do not use color alone to communicate selected, running, warning, or error
  state.

## Product shell

### Activity Bar

The leftmost 52 px rail is the product switcher.

Upper group:

1. Work;
2. Knowledge;
3. enabled verified package activities.

Lower group:

1. Memory;
2. Plugin Marketplace;
3. Settings.

There is no separate Office or coding icon. The selected item uses an A3S-blue
edge marker, stronger icon treatment, tooltip, and `aria-current="page"`.

### Work sidebar

The sidebar is 232 px and compacts to 208 px. It always displays unified
conversations, regardless of whether the center is Home, files, Office, or
code.

- one compact Work header;
- one 34 px New conversation row with a framed command glyph;
- search expands on demand;
- conversations live in one collapsible group;
- rows are single line with title and concise relative time;
- selection uses a neutral filled row plus stronger type;
- rename and delete appear on hover or keyboard focus and execute inline;
- the running conversation cannot be deleted;
- model IDs and full workspace paths stay out of resting rows.

The file explorer inside file/code scenes is a scene navigator, not a second
product sidebar. It uses the same row height, icon stroke, focus ring, border,
and collapse affordance.

### Center workspace

Work's center renders Home, local files, an Office/PDF editor, or the code/text
editor. Scene headers share:

- a consistent back/collapse action;
- left-aligned identity and current object;
- right-aligned AI Assistant toggle and scene actions;
- 40–44 px height where format-specific ribbon chrome is not required;
- one bottom border and no decorative gradient.

### AI Assistant

The assistant defaults to 460 px, is resizable within safe bounds, and uses a
shared header, execution stream, and composer. At 1120 px and below it becomes
an overlay so the file or editor surface remains usable. The close action must
remain visible.

## AI-native Home

The initial hierarchy is:

1. concise Work identity and outcome promise;
2. complete production composer;
3. implemented capability shortcuts;
4. templates, folders, and recent files.

The hero uses generous whitespace and one restrained gradient or brand accent.
It must not include fake generation types, promotional cards, points, rewards,
or placeholder media tools.

Recent, Favorites, Trash, and managed folders are direct file-management views
and do not repeat the hero.

## Composer

Home and AI Assistant use the same component and control order:

- context chips above the editable instruction;
- mode, workspace, model, effort, and Skills on the lower control rail;
- primary send/stop action at the lower right;
- `@` opens a lazy workspace tree;
- `/` opens enabled Skills and supported control commands;
- dropped files become visible context;
- prompt starters populate the editor but never submit.

The workspace switch/manage control is the same component in both placements.
Do not reproduce it with page-specific buttons.

## Local file manager

### Layout

Use familiar Finder and Explorer anatomy: scene sidebar, toolbar and
breadcrumbs, grid/list content, stable selection shelf, and status bar.

Grid and list show the same selection and action model. A layout switch changes
density, not capability.

### Selection

- click selects one;
- Cmd/Ctrl click toggles;
- Shift click extends a range;
- drag on background creates a marquee;
- checkboxes support explicit multi-selection;
- keyboard supports arrows, Home/End, range extension, and open;
- right-click preserves a multi-selection when the target is already inside
  it, otherwise it selects only the target;
- right-click on whitespace clears stale item selection.

Selected styling combines background, border, and accessible state. Do not
move file positions when the selection shelf appears.

### Context menus

Background menu groups:

1. create and import;
2. paste and select all;
3. refresh, layout, and sort;
4. create knowledge base and AI organization;
5. copy path and reveal.

Item menu groups:

1. open and Quick Look;
2. copy, cut, and paste into folder;
3. AI and knowledge actions;
4. favorite, duplicate, and rename;
5. path/reveal;
6. permanent delete.

Use separators between intent groups. Hide impossible actions only when they
would be misleading; otherwise show a disabled action with a useful tooltip.

### Inline operations

Folder creation, native Office-file creation, rename, and duplicate naming
replace the affected tile or row with a focused input. Enter confirms, Escape
cancels, and a failed operation keeps its value and error. Success does not need
a modal or celebratory toast.

Permanent local deletion uses a concise danger dialog naming exact scope.

### Quick Look and quick open

Quick Look is read-only, modal-light, and navigable with adjacent-item keys.
Quick open is a focused searchable overlay owned by Work. Open code tabs appear
first; selecting a result delegates to the normal Work file handler so Office
and code files receive the correct scene.

## Editors

### Code/text scene

Use one restrained scene header, a lazy file explorer, compact tabs, Monaco,
and a status bar. Markdown shows editable source on the left and semantic live
preview on the right. The AI Assistant toggle matches Office.

Dirty state uses a small dot and accessible label. External-change conflict
appears next to the editor and requires explicit reload or overwrite.

### Office/PDF scenes

Documents, spreadsheets, presentations, and PDF use consistent file menu,
ribbon geometry, scene actions, status, zoom, preview, print, and save language.
Format-specific controls can differ; shared controls cannot.

Ribbon tabs organize commands by workflow. Avoid one long scrolling toolbar.
The editable canvas is visually dominant and retains enough width when the
assistant is open.

### AI proposals

Selected source context is visible before submission. A structured proposal
shows target, before, after, selection checkbox, stale state, and one explicit
Apply action. Advice that cannot be applied safely remains advisory.

## Execution and delivery

Conversation turns use a restrained vertical flow:

- user instruction;
- assistant response and reasoning disclosure;
- tool lifecycle cards;
- permission request at the requesting tool;
- artifacts and reports;
- evidence-backed delivery summary;
- recovery and follow-up composer.

Running, success, warning, denial, cancellation, and failure states use icon,
text, and tone together. Long command output and raw diagnostics are collapsed
behind explicit disclosures with bounded scrolling.

File artifact actions open the current Work handler. Delivery review prepares a
same-conversation instruction; it must not navigate to an unmounted workspace.

## Settings

Settings is a shell-level modal over the current product, approximately
1040 × 720 px within viewport bounds. It uses a quiet 236 px navigation rail and
one scrollable content pane.

Account, Appearance, Model, Agent, Context, Integrations, Channels, About, and
Help share row geometry, labels, units, dirty/saving/error state, and focus
behavior. Category failures stay local. The modal traps focus and restores the
invoker.

## Feedback and interruption

Use the least disruptive surface that can carry the decision:

| Need | Surface |
| --- | --- |
| field validation | inline message |
| row mutation failure | inline row error |
| recoverable service or load issue | inline notice/state view |
| brief non-blocking completion | toast |
| tool permission | inline execution decision |
| external file conflict | focused conflict surface |
| permanent delete or compatibility loss | dialog |

Never use a modal merely to collect a simple name or confirm a reversible
operation.

## Responsive behavior

### Wide desktop: 1200 px and above

- Activity Bar: 52 px;
- Work sidebar: 232 px;
- scene explorer: 220–280 px where applicable;
- AI Assistant: resizable, default 460 px;
- editor and content retain the remaining flexible width.

### Compact desktop: 768–1199 px

- Work sidebar can collapse;
- toolbars tighten before hiding actions;
- AI Assistant overlays at 1120 px and below;
- menus and dialogs remain viewport bounded;
- Office ribbons provide explicit overflow.

### Narrow viewport: below 768 px

Preserve the primary task, file, and close actions. Secondary columns become
overlays or collapse. Do not horizontally shrink text, controls, or Office
canvas into unusable states. The DOM remains at least 320 px wide.

## Accessibility

- every icon-only action has an accessible name and tooltip where discovery is
  needed;
- focus is visible on every interactive element;
- menus, tabs, listboxes, dialogs, and disclosures use correct semantics;
- selection and status are not color-only;
- keyboard and pointer paths expose equivalent core actions;
- focus returns to the invoking control after overlays close;
- reduced motion removes non-essential transitions and spinners retain text;
- graph and canvas experiences provide an accessible alternative browser.

## Motion

Use 100–180 ms transitions for hover, selection, panel entry, and inline naming.
Avoid spring motion in dense workspace chrome. Streaming indicators and
progress motion stop immediately when the underlying state becomes terminal.

## Consistency review checklist

Before shipping a page or scene, verify:

- it uses the canonical route and product identity;
- sidebar width, rows, icons, and collapse behavior match peers;
- the same action uses the same icon and label;
- routine create/rename actions are inline;
- context menus clear or preserve selection correctly;
- AI context is visible and bounded;
- file actions open a mounted Work handler;
- light/dark, keyboard, focus, narrow-width, loading, empty, error, and stale
  states are present;
- no control exposes a retired Code/Office product or Result Workspace.
