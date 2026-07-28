# a3s code TUI — Knowledge Compilation ("LLM Wiki")

> Status: design + shipped capability (the `okf` skill, invoked as `/okf`). Audience: a3s maintainers.
> Companion to [knowledge-base-design.md](./knowledge-base-design.md). The KB is the *store*; this is how it gets *populated* from the codebase.

## What it is

An **LLM-driven wiki compiler**, in the spirit of DeepWiki and conforming to
Google's **[Open Knowledge Format (OKF v0.1)](https://cloud.google.com/blog/products/data-analytics/how-the-open-knowledge-format-can-improve-data-sharing)** —
which formalizes exactly this LLM-wiki pattern as "a directory of markdown files
with YAML frontmatter." The coding agent reads the project's code (and any
existing `.a3s/kb/` notes) and **compiles** an **OKF bundle** under
`.a3s/kb/wiki/`: one markdown *concept* file per module / decision / abstraction
(its file path is the concept's identity), each with a required `type` frontmatter
field, standard markdown links forming the concept graph, `[code](src/foo.rs#L42)`
links to the real source, and a per-directory `index.md` — every claim grounded in
a file the agent actually read.

It is a **compile**, not a one-shot dump: each concept records the source files
(and a digest of them) it was built from, so a re-run only regenerates concepts
whose sources changed — a dependency-tracked, incremental rebuild.

### Why OKF

OKF is the natural fit because it *is* the standardized form of what this feature
already produced: vendor-neutral, agent- and human-friendly markdown — "just
markdown, just files, just YAML frontmatter." Emitting OKF makes the compiled
bundle portable (shippable as a tarball / git repo, indexable by any tool) and
interoperable with other OKF consumers. The only hard OKF requirement is a `type`
field on every concept; everything else is convention. Spec + sample bundles:
`GoogleCloudPlatform/knowledge-catalog`.

## Why this serves a coding agent (first-principles)

- A fresh, navigable wiki of the codebase is exactly the durable project knowledge
  the [KB design](./knowledge-base-design.md) wants — **auto-generated** instead of
  hand-written, so the vault is useful on day one and stays current as code moves.
- It is dual-use: the human reads it (in `/kb` or any editor), and the agent reads
  it back as context (via the KB's P2 `KbContextProvider`). The agent that wrote
  the wiki works better because it has the wiki.
- The note↔code links (`[name](src/…#L…)`, standard markdown) are the
  coding-specific edge no general wiki tool has.

## Architecture — the agent IS the compiler

There is **no new core subsystem**. Compilation is a structured agent task,
driven by a bundled skill, using the agent's existing tools. The cli only
provides the skill and a trigger.

| Piece | What | Where |
|---|---|---|
| **`okf` skill** | The compilation pipeline (survey → plan → generate → index → verify; incremental; anti-hallucination rules). A `kind: instruction` skill — this *is* the capability. | `skills/okf.md` |
| **Skill loader** | Always materialized to `~/.a3s/cli/skills/okf/SKILL.md` and added to the session skill dirs, so the capability is available in every project (not login-gated or project-local). The obsolete `~/.a3s/cli-skills/` layout is removed after the canonical directory is written. | `src/tui/system/skills.rs` `ensure_builtin_skills_dir` → `skill_dirs()` (`mod.rs`) |
| **`/okf` trigger** | The skill itself surfaces in the `/` menu as **`/okf`** (selecting it asks the agent to apply the skill); it also auto-applies when the user asks for the wiki/docs in prose. *No separate slash command* — that would just duplicate the skill's menu entry. | the slash menu's skill listing (`panels/menu.rs`) |
| **Fan-out** | Pages generate concurrently via `parallel_task` when available, else sequentially. | the agent's existing `parallel_task`/`task` tools |
| **Output** | `.a3s/kb/wiki/*.md` — the KB vault's compiled subtree. | the agent's `write` tool, routed through `ctx.resolve_workspace_path` |

## Pipeline

1. **Survey** — map the repo (top-level dirs, manifests, module entry points,
   existing `.a3s/kb/` notes). In a monorepo, each crate/package is a module concept.
2. **Plan the bundle** — a bounded concept set + directory layout (e.g. `modules/`,
   `concepts/`, `decisions/`), each directory with an `index.md`, plus the root
   `index.md`. Deterministic kebab-case slugs. The layout is shown to the user first.
3. **Generate** — per concept, read its sources then write an OKF file (required
   `type`, the standard fields, an *explanation* with `[file](path#Lline)` code
   links and `[name](/dir/other.md)` concept links), grounded entirely in what was
   read. Fan out with `parallel_task`.
4. **Index** — each directory's `index.md` and the root `index.md` link every
   concept (OKF's hierarchical navigation); concepts also link each other.
5. **Verify** — every markdown link must resolve; dangling links fixed or dropped.

### Concept file format (OKF)

```yaml
---
type: Architecture Decision      # REQUIRED by OKF (exactly one type per concept)
title: Why microVM, not container
description: Rationale for libkrun MicroVMs over containers.
resource: crates/box/src/runtime.rs   # the concept's canonical source (path or URL)
tags: [architecture, security]
timestamp: 2026-06-30T12:00:00Z       # ISO 8601
# OKF permits extra fields; we keep these for incremental recompile + provenance:
source: compiled                 # agent-generated, NOT a human note
sources: [crates/box/src/runtime.rs, crates/box/README.md]
source_digest: <sha of the concatenated sources>
---
```

Body: synthesized prose + **standard markdown links** between concepts
(`[name](/dir/other.md)`) and to code (`[runtime.rs](crates/box/src/runtime.rs#L42)`)
+ a final `## Sources` list. Navigation is OKF's reserved `index.md` per directory;
an optional top-level `log.md` holds the chronological compile history.

### Incremental recompile

Before regenerating a concept, the skill recomputes the digest of its `sources`
and compares it to the stored `source_digest`. Unchanged ⇒ skip; changed ⇒ rebuild
(plus the affected `index.md`). This is the "compile" semantics — cheap re-runs, a
bundle that tracks the code.

## Guardrails

- **Anti-hallucination** — state only what the code says, cite file+line, mark
  uncertainty, never invent APIs. A hallucinated page is worse than no page.
- **Provenance & boundaries** — `source: compiled` on every page; the agent owns
  `.a3s/kb/wiki/`, the human owns `.a3s/kb/*.md`; neither clobbers the other; the
  agent memory (`~/.a3s/memory/`) is untouched.
- **No secrets** — never copy tokens/keys/`.env` into a page.
- **Bounded** — document modules + concepts, not every file; compile large repos
  by area and report coverage.
- **Reward-hacking** — the agent generates knowledge it is later fed. Provenance +
  user review (in `/kb`) mitigate it, and the compiled wiki is kept **out of any
  self-evolution fitness signal** (carried from the KB P2 caveat).

## Relation to the KB phases

- Works **today** as the `okf` skill — invoke it via **`/okf`** (the menu entry) or
  by asking in prose; it writes plain `.md`, browsable with `/ide` or any editor —
  independent of the KB panel.
- **One format, end to end:** both the compiled bundle (`.a3s/kb/wiki/`) and any
  human-authored notes (`.a3s/kb/*.md`) are **OKF** — standard markdown links, a
  required `type` field. There is no second link syntax to reconcile; the KB's P1
  resolver follows OKF markdown links only.
- Gets better as the KB lands: P1 backlinks/search index the bundle's links so it
  is navigable in `/kb`; P2 `KbContextProvider` feeds compiled concepts into the
  agent's context.

## Open questions

- Bundle the skill always-on (current choice) vs. opt-in install?
- Should `/okf` recompile-all by default, or only stale pages? (Skill defaults to
  incremental; ask in prose for a full rebuild.)
- A periodic/scheduled recompile via the agent's cron — later, once usage shows the
  cadence.
