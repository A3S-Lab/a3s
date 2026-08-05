---
version: 1
slug: "route-conversation-sessionid"
primary_target: "route:/conversation/:sessionId"
related_targets: ["src/features/work/components/work-conversation.tsx","src/styles/work-conversation.css"]
---

# Work conversation surface

- Scope: the canonical task conversation reached from Work Home, task history, browser history, or a copied deep link.
- Mode: Operate. This is a focused execution surface inside the established A3S Web visual world.
- Audience: local-first knowledge workers and developers who need to understand what the agent is doing, intervene safely, and continue a durable task.
- Primary task: read the task thread, inspect execution state, send or queue the next instruction, and open resulting workspace files.
- Chosen direction: a calm command cockpit with a compact task/status ribbon, one centered execution thread, an inspectable runtime slate, and a docked composer.
- Product boundary: the Work Copilot remains contextual to file, Office, and code scenes. It is absent from Work Home and this canonical conversation surface.
- Navigation contract: `#conversation/<opaque-session-key>` is canonical; Home-to-conversation adds history; refresh and browser Back restore the route; missing sessions show a truthful recovery state instead of another task.
- Constraints: inherit `DESIGN.md` tokens, typography, Activity Bar, task library, primitives, and responsive breakpoints. Do not restore the retired Result Workspace or add a competing visual system.
- Memorable moment: after Home submission, the durable task route appears as the conversation opens and the status ribbon moves from preparation to live execution without a side-panel detour.
- Accessibility: labeled icon controls, visible focus, text plus color for status, polite live status, keyboard-safe composer, reduced-motion support, and a mobile header that keeps Home and task identity reachable.
- Required states: route loading, missing/deleted task, empty thread, message loading/error, submission, running, queued follow-ups, completion, service interruption, and responsive sidebar behavior.
- Unresolved decisions: none that block implementation; session titles and status remain service-authoritative with local persisted titles as the existing fallback.
