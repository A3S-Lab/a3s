# A3S Agent Evaluation Design System

A3S Agent Evaluation adapts [A3S Code Web's design system](../web/DESIGN.md) to a tactical
simulation interface. The shared principles remain authoritative: direct Chinese
copy, one obvious next action, progressive disclosure, restrained elevation,
honest states, keyboard access, and A3S brand blue for selection and focus.

## Product character

```text
战术沉浸 · 可信可读 · 工业克制 · 游戏化但不失真
Tactical immersion · trustworthy · industrial restraint · game-like without fiction
```

The interface should feel like a premium aircraft strategy game's operations
room, not an administration dashboard. Game language organizes real Bench
capabilities; it must never invent progress, scores, provenance, or runtime
state. Instrument detail is useful only when it communicates real visualization
state, Bench state, or provenance.

## Color roles

- A3S blue (`#6ca3ff` in the dark surface) owns navigation selection, keyboard
  focus, links, and active Bench progress.
- Flow cyan owns only physical visualization: streamlines, Mach telemetry,
  reticles, and force-field annotations.
- Green means ready or completed.
- Amber/orange means caution, quarantined evidence, preview, or unstable flow.
- Red means blocked or failed.
- Primary actions use a high-contrast neutral fill, following `apps/web`.

The A3S blue–indigo–purple gradient appears only in the small brand signature.
Persistent panels use one-pixel dividers and surface contrast, not large
shadows. Shadows are reserved for transient notices.

## Information architecture

The top-level navigation has four destinations:

1. **作战地图** — Task-as-map selection, weather, flight profile, deployment,
   run lifecycle, and current score.
2. **机库编队** — 3D airframe inspection and aircraft, pilot/Agent, model, and
   effort/loadout composition.
3. **战报** — named, latest, and current-run result inspection.
4. **工程舱** — Doctor, Task validation, Task Lock, and Candidate Lock.

These are destinations, not command tabs. Each has one local primary action per
decision area. Advanced commands and identity detail remain behind disclosures
until needed.

## Evaluation composition

At supported desktop widths, the evaluation arena is a two-surface map
workspace. The live battlefield is the leftmost and dominant surface:

```text
┌────────────── Three.js battlefield ───────────────────┬── Mission board ──┐
│ formation · exterior theater · weather · hover HUD    │ maps · briefing   │
│ direct inspection · collapsible telemetry HUD         │ deploy            │
└───────────────────────────────────────────────────────┴───────────────────┘
```

The Three.js viewport retains dominant visual weight and appears before support
panels in both DOM and visual order. The mission board presents Tasks as a
compact, game-style map route rather than a permanent configuration form.
Search, blocked maps, exact Task identity, Candidate configuration, and manual
flight controls stay behind disclosures. The collapsed telemetry summary sits
inside the battlefield as a lightweight HUD; expanding it opens a transient
popover instead of introducing a third persistent surface.

Selecting a Task means selecting a map. One shared, deterministic theater
resolver keeps its map card, briefing, and Three.js exterior aligned. The
runtime provides eight procedural exterior families: training range, littoral
front, mountain pass, desert frontier, industrial city, arctic highland,
forest valley, and offshore platforms. Task-derived weather is layered over
that exterior and aircraft pose, so changing maps changes recognizable terrain,
landmarks, palette, lighting, and atmosphere—not only a weather label. The map
scene has no indoor chamber walls or ventilation fans.

Direct body hover opens a transient HUD with model-selected airframe,
Agent-specific pilot attire, effort-selected loadout, Task, state, and
telemetry. Selection uses a real spotlight, while material emission, floor
markers, and force arrows are not used as selection feedback. The map briefing
owns progressively disclosed Candidate configuration and the only filled
deployment action. There is no persistent bottom run panel.

The hangar is a character-selection flow. A large real-time Three.js aircraft
stage is paired with airframe tiles, Agent pilot cards, effort/loadout slots,
and a five-position squadron. Candidate paths, model identifiers, and callsigns
remain in an advanced disclosure. Selecting a squadron member activates the
same combination in the evaluation arena; the UI must not maintain a second
configuration source.

## Typography and controls

- Chinese UI uses the same platform-first font stack as `apps/web`.
- Technical IDs and telemetry use the platform monospace stack.
- Chinese headings never receive artificial letter spacing.
- Standard controls are 34–38 px high with 7–9 px radii.
- Small 7–9 px labels are restricted to redundant instrument metadata that has
  a readable adjacent name or accessible label.
- Selected rows use a blue edge marker and stronger type; color alone is not
  sufficient.

## Motion

CSS interaction motion stays within 120–180 ms. WebGL motion represents
aircraft attitude, airflow, weather, clouds, water, and exterior landmarks.
`prefers-reduced-motion` slows the field and nonessential scenery motion
substantially; the explicit Pause control stops simulation time without hiding
current state.

## Content and state

- Use direct Chinese outcome labels such as “进入地图”, “部署评测”, “读取战报”,
  and “生成 Task Lock”.
- Keep exact CLI commands as secondary technical evidence, not primary actions.
- Preview always says “预览”; it never claims to be an official result.
- A failure states what failed and preserves the user's configuration.
- Ordinary completion reports the score or written lock without celebratory
  language.
