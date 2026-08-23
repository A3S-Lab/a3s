---
name: A3S Documentation Site
description: A cool-white operational canvas for reading, composing, validating, and tracing real A3S workflows.
colors:
  surface: "#ffffff"
  canvas: "#f7f9fc"
  surface-muted: "#f1f4f8"
  line: "#d8e0eb"
  line-strong: "#b9c5d5"
  ink: "#101827"
  text-muted: "#56657b"
  text-faint: "#68778c"
  action-cobalt: "#1264ff"
  action-cobalt-deep: "#084ed0"
  action-cobalt-soft: "#eaf2ff"
  runtime-green: "#087a5a"
  runtime-green-soft: "#e6f6f0"
  runtime-line: "#a7d7c5"
  decision-amber: "#9a6100"
  decision-amber-soft: "#fff4d6"
  capability-violet: "#6750b8"
  capability-violet-soft: "#f1edff"
  integration-cyan: "#087f9f"
  integration-cyan-soft: "#e8f7fb"
  danger-red: "#b42335"
  danger-red-soft: "#fff0f1"
  danger-line: "#e4b5bc"
  topology: "#9cabc0"
  topology-strong: "#8999ad"
  canvas-dot: "#c5cfdd"
  variable-ink: "#194a98"
  variable-line: "#c8d8f0"
  code-ink: "#dce7f7"
typography:
  display:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: "clamp(50px, 4.25vw, 68px)"
    fontWeight: 760
    lineHeight: 0.99
    letterSpacing: "-0.055em"
  headline:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: "clamp(40px, 4.3vw, 64px)"
    fontWeight: 740
    lineHeight: 1.02
    letterSpacing: "-0.052em"
  title:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, system-ui, sans-serif"
    fontSize: "15px"
    fontWeight: 690
    lineHeight: 1.3
    letterSpacing: "-0.015em"
  body:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif"
    fontSize: "15px"
    fontWeight: 400
    lineHeight: 1.75
  control:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, system-ui, sans-serif"
    fontSize: "12px"
    fontWeight: 650
  label:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, system-ui, sans-serif"
    fontSize: "10px"
    fontWeight: 620
  field:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, system-ui, sans-serif"
    fontSize: "11px"
    fontWeight: 400
  node-title:
    fontFamily: "'Geist Variable', 'PingFang SC', 'Microsoft YaHei', ui-sans-serif, system-ui, sans-serif"
    fontSize: "13px"
    fontWeight: 680
    lineHeight: 1.25
  code:
    fontFamily: "'Geist Mono Variable', 'SFMono-Regular', 'Cascadia Code', Consolas, monospace"
    fontSize: "9px"
    fontWeight: 400
    lineHeight: 1.55
rounded:
  chip: "6px"
  compact: "7px"
  control: "8px"
  floating: "9px"
  rail: "11px"
  card: "12px"
  pill: "999px"
spacing:
  compact: "6px"
  field-x: "10px"
  action-x: "12px"
  node-inset: "13px"
  chrome-inset: "14px"
  panel-inset: "16px"
  section-y: "18px"
components:
  button-primary:
    backgroundColor: "{colors.action-cobalt}"
    textColor: "{colors.surface}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "36px"
  button-primary-hover:
    backgroundColor: "{colors.action-cobalt-deep}"
    textColor: "{colors.surface}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "36px"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-muted}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "36px"
  button-secondary-hover:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.action-cobalt-deep}"
    typography: "{typography.control}"
    rounded: "{rounded.control}"
    padding: "0 12px"
    height: "36px"
  icon-button:
    backgroundColor: "transparent"
    textColor: "{colors.text-muted}"
    rounded: "{rounded.control}"
    size: "34px"
  icon-button-hover:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.action-cobalt-deep}"
    rounded: "{rounded.control}"
    size: "34px"
  field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.field}"
    rounded: "{rounded.control}"
    padding: "0 10px"
    height: "38px"
  variable-chip:
    backgroundColor: "{colors.action-cobalt-soft}"
    textColor: "{colors.variable-ink}"
    typography: "{typography.code}"
    rounded: "{rounded.chip}"
    padding: "5px 7px"
  workflow-node:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.node-title}"
    rounded: "{rounded.card}"
    padding: "13px"
    width: "228px"
  inspector-panel:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.ink}"
    typography: "{typography.title}"
    width: "390px"
  debug-tab:
    backgroundColor: "transparent"
    textColor: "{colors.text-muted}"
    typography: "{typography.label}"
    rounded: "{rounded.compact}"
    padding: "0 10px"
    height: "32px"
  debug-tab-active:
    backgroundColor: "{colors.surface-muted}"
    textColor: "{colors.action-cobalt-deep}"
    typography: "{typography.label}"
    rounded: "{rounded.compact}"
    padding: "0 10px"
    height: "32px"
---

# Design System: A3S Documentation Site

## Overview

**Creative North Star: "The Operational Canvas"**

The Operational Canvas treats A3S documentation as a working control surface: a cool-white field, thin gray-blue seams, cobalt decisions, and green runtime proof. It is precise, compact, and visibly repository-grounded; the workflow editor feels like a real IDE inside the docs rather than a passive product demo.

Generous display typography opens reading surfaces, then yields to dense bordered chrome when the visitor begins composing, configuring, validating, and tracing. Geist keeps the bilingual interface calm and contemporary, Geist Mono marks contracts and runtime data, and restrained shadows distinguish only the elements that float above the canvas.

The system rejects decorative dashboard theater inside operational surfaces. Color, depth, and motion must explain state, hierarchy, or direct manipulation.

**Key Characteristics:**

- Cool-white surfaces divided by fine gray-blue borders.
- Cobalt interaction states paired with green runtime evidence.
- Geist for interface language and Geist Mono for machine-facing values.
- Dense panels, compact controls, and shallow functional elevation.
- Responsive tool geometry that preserves the graph as the primary workspace.

## Colors

The palette is cool, high-legibility, and state-led; most of every screen remains neutral so operational color reads as evidence.

### Primary

- **Operational Cobalt:** Primary actions, selection, connections, focus, and active navigation.
- **Deep Cobalt:** Hovered actions and compact text accents that need stronger contrast.
- **Cobalt Wash:** Selected rows, icon tiles, variable chips, and low-emphasis interactive backgrounds.
- **Variable Blue:** The darker ink and fine stroke reserved for reusable variable tokens.

### Secondary

- **Runtime Green:** Successful validation, active execution, completed traces, and run-history proof.
- **Runtime Mint:** The quiet surface behind valid or successful states.
- **Decision Amber:** Queued work and human-decision semantics.
- **Failure Red:** Failed execution, stop controls, destructive affordances, and validation errors.

### Tertiary

- **Capability Violet:** Agent, model, and memory node families.
- **Integration Cyan:** MCP, tool, service, and execution node families.
- **Soft Violet / Soft Cyan / Soft Amber:** Low-chroma icon grounds paired with their semantic node accents.

### Neutral

- **Operational Surface:** The white plane used by panels, cards, controls, and nodes.
- **Cool Canvas:** The pale field beneath the graph and other recessed work areas.
- **Muted Surface:** Hover fills, search wells, and secondary panels.
- **Operational Ink:** Primary labels and titles.
- **Slate Text / Faint Slate:** Supporting copy, metadata, placeholders, and timestamps.
- **Gray-Blue Line / Strong Gray-Blue Line:** Dividers, field strokes, panel seams, and selected-card structure.
- **Topology Gray / Canvas Dot:** Workflow connections and the dotted pannable field.

### Named Rules

**The State Has a Color Rule.** Cobalt means interaction, green means runtime proof, amber means waiting or human decision, red means failure or destructive intent, and violet or cyan classify node kinds.

**The Cool-White Majority Rule.** Neutral surfaces and fine gray-blue structure occupy most of the interface; semantic color remains sparse enough to stay meaningful.

## Typography

**Display Font:** Geist Variable (with Chinese and system sans fallbacks)
**Body Font:** Geist Variable (with Chinese and system sans fallbacks)
**Label/Mono Font:** Geist Mono Variable (with SFMono, Cascadia Code, Consolas, and monospace fallbacks)

**Character:** The pairing is modern and matter-of-fact. Geist supplies crisp editorial scale and compact controls; Geist Mono makes templates, capability references, variables, trace timings, and JSON feel executable.

### Hierarchy

- **Display** (760, `clamp(50px, 4.25vw, 68px)`, 0.99): Homepage hero statements only.
- **Headline** (740, `clamp(40px, 4.3vw, 64px)`, 1.02): Major documentation sections and calls to action.
- **Title** (690, 15px, 1.3): Inspector, library, and drawer headings.
- **Body** (400, 15px, 1.75): Explanatory documentation copy.
- **Control** (650, 12px): Header actions and prominent editor buttons.
- **Label** (620, 10px): Tabs, field labels, metadata, and dense navigation.
- **Node Title** (680, 13px, 1.25): Workflow node names.
- **Code** (400, 9px, 1.55): Templates, variables, capability references, timestamps, and node summaries.

### Named Rules

**The Two Registers Rule.** Use Geist for human instructions and Geist Mono for values the workflow can execute, inspect, or serialize.

## Layout

Reading surfaces sit inside a centered frame up to 1440px with responsive horizontal gutters and fine vertical borders. The workflow editor is a separate operate-density expression: it fills `100dvh`, keeps a 64px product header, and gives the remaining stage to the graph.

On wide screens the inspector is 390px and the debug console is 304px, with the canvas reserving their space when open. Compact history controls sit at the lower left, cached variables remain directly accessible at the lower center, and the minimap and zoom controls occupy the lower right. At 1180px the inspector tightens to 360px. At 980px it becomes a right-side overlay so the graph no longer shrinks. At 760px the vertical rail becomes a 58px bottom toolbar, primary touch targets grow to 44px, the minimap and keyboard hint recede, and the debug console caps itself against the viewport. At 520px the inspector is full width, library items collapse to one column, trace steps scroll horizontally, and variable rows stack.

Within the workflow editor only, the pannable canvas uses a 22px dotted rhythm and fine topology lines. Panels reveal contextually: selection opens configuration at the right edge, while validation, variables, trace, and history occupy dedicated overlays or the bottom console.

**The Canvas Gives Way Rule.** Contextual panels may reserve or overlay space, but the graph remains the dominant working field and regains the full stage when they close.

## Elevation & Depth

The system is bordered first and lightly elevated second. Flat seams establish permanent structure; low shadows identify sticky chrome, nodes, floating controls, and contextual panels without turning the cool-white canvas cloudy.

### Shadow Vocabulary

- **Chrome Low** (`0 5px 18px rgba(35, 55, 84, 0.08)`): Product header separation.
- **Node Rest** (`0 8px 20px rgba(35, 55, 84, 0.1)`): Workflow nodes at rest.
- **Floating Control** (`0 8px 24px rgba(35, 55, 84, 0.12)`): Canvas controls and similar compact floating utilities.
- **Selected Node** (`0 10px 26px rgba(26, 76, 154, 0.18)`): The currently selected graph node.
- **Panel Float** (`0 18px 46px rgba(35, 55, 84, 0.16)`): Node library and other contextual side surfaces.
- **Console Rise** (`0 -12px 34px rgba(35, 55, 84, 0.1)`): The bottom debug console.

### Named Rules

**The Border-First Rule.** Use a one-pixel line to explain structure; add a shadow only when a surface physically floats, overlays, or responds to selection.

## Shapes

The form language is softly squared and compact. Six- and seven-pixel corners belong to chips, tabs, and toolbar controls; eight-pixel corners are the default for buttons and fields; nine- to twelve-pixel corners identify floating utilities, rails, panels, and workflow nodes. Full pills are reserved for small edge labels or taxonomy-like badges, while circles are reserved for statuses, handles, and icon medallions.

**The Functional Radius Rule.** Corner size follows component scale and role; it is not a decorative way to make every surface look soft.

## Components

### Buttons

- **Shape:** Compact controls use gently squared corners (7-8px); primary header actions are 36px tall and editor footer actions are 40px tall.
- **Primary:** Operational Cobalt with white text, a fine matching border, compact horizontal padding, and a 650-680 weight label.
- **Hover / Focus:** Hover deepens the cobalt or lifts a neutral control by one pixel; active compresses to 0.98 scale. Focus uses a three-pixel translucent cobalt outline with a small offset.
- **Secondary / Ghost / Danger:** Secondary actions are white with a gray-blue border, ghost icon controls use transparent rests, and danger appears as red ink on a pale red surface only at the destructive point of action.

### Chips

- **Style:** Variable chips use a six-pixel corner, pale cobalt fill, fine blue border, dark blue mono text, and tight `5px 7px` padding.
- **State:** Pills describe variables or edge labels; they do not replace buttons or broad status banners.

### Cards / Containers

- **Corner Style:** Workflow nodes and the node library use 12px corners; inspectors and debug panels align flush to the viewport edge.
- **Background:** Operational Surface over Cool Canvas, with Muted Surface for recessed or interactive subregions.
- **Shadow Strategy:** Nodes use low rest elevation; selected nodes and contextual panels use the stronger vocabulary defined above.
- **Border:** Permanent structure uses one-pixel gray-blue strokes.
- **Internal Padding:** Dense components cluster around 13-18px insets.

### Inputs / Fields

- **Style:** White surface, one-pixel gray-blue stroke, eight-pixel corners, 38px single-line height, and compact 10px horizontal padding.
- **Focus:** Cobalt border plus a three-pixel translucent cobalt halo.
- **Code Fields:** Templates and capability references switch to Geist Mono on a slightly cooler field surface.
- **Error / Disabled:** Errors use the pale red/red pair; disabled controls retain their shape and drop to 45% opacity.

### Navigation

- **Style:** The full-screen editor uses a 64px product header, right-aligned validate/run actions, a floating compact tool rail, lower-left history controls, and lower-center cached-variable access.
- **States:** Neutral controls gain a muted fill and deep cobalt ink on hover; the add and run actions remain solid cobalt.
- **Mobile:** The rail moves to the bottom, canvas utilities rise above it, secondary header copy compresses, and tap targets grow to 44px.

### Workflow Node

The signature node is a 228px white card with a strong gray-blue border, 12px corner, compact semantic icon tile, concise title/type pair, mono configuration summary, side handles, and an eight-pixel runtime status dot. Selection changes the border and shadow; runtime changes the status dot and, while running, adds a restrained pulse when motion is allowed.

### Debug Console

The console rises from the bottom with a bordered tab strip, trace list, detail region, variables, and history. Active rows use Cobalt Wash, successful evidence uses Runtime Green, and structured payloads switch to a dark code surface with light mono text.

**The Proof at the Point of Action Rule.** Validation, runtime status, trace output, and history must appear beside the control or graph context that produced them.

## Do's and Don'ts

### Do:

- **Do** keep the canvas and surfaces cool white so topology and runtime state remain legible.
- **Do** use cobalt for direct manipulation and green only when execution or validation has produced evidence.
- **Do** use fine borders for permanent structure and reserve shadows for floating or selected surfaces.
- **Do** keep operational labels compact, with machine-facing values in Geist Mono.
- **Do** preserve visible focus, 44px mobile targets, and the reduced-motion fallback.

### Don't:

- **Don't** spread violet, cyan, amber, green, or red as decoration outside their observed semantic roles.
- **Don't** replace panel borders with heavy shadow-only cards or ornamental glass effects.
- **Don't** hide simulation, validation, or run state away from the graph action that caused it.
- **Don't** promote the shipped example graph's node positions, counts, branch arrangement, or first-view composition into a global layout rule.
- **Don't** animate runtime pulses or transitions when the user requests reduced motion.
