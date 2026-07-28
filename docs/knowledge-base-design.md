# a3s code TUI — Knowledge Base ("Vault") design

> Status: historical design note, superseded by the current `/kb` + `/okf`
> boundary. Audience: a3s maintainers.
> Method: synthesized from a 4-way design judge-panel (minimal / Obsidian-faithful / agent-native / hybrid), grounded in the live codebase.
> Current boundary: `/kb` is the local personal knowledge base at `.a3s/kb`;
> `/okf` manages shareable OKF knowledge-package assets under `.a3s/okf` and
> publishes them through OS Knowledge service. Do not store OKF package assets
> under the personal KB vault and do not add compatibility aliases.

---

## Motivation & first-principles scope

### Why a coding agent needs a KB at all

A coding agent accumulates durable, project-specific knowledge that does not belong in code comments and does not belong in episodic memory logs: architecture rationale, "why we did it this way" decisions, gotchas, and the map between concepts and the files that implement them. Today that knowledge has two bad homes — it is either lost between sessions or dumped into `~/.a3s/memory/` (per-user, append-only, header-keyed, not human-curated). A KB gives it a **third home that is project-scoped, git-committable, human-editable, and agent-readable**: a folder of plain markdown notes in the workspace.

The differentiator versus Obsidian: the same `.a3s/kb/*.md` files are a **shared substrate** — human-edited in the TUI, agent-written via existing file tools, agent-read as prompt context. It is not a human-only notebook, and it is not the agent's private memory. That dual-use, plus OKF markdown links that connect notes to real workspace code files, is the only reason this feature earns its place against Rule 1.

### First-principles gate (Rule 1)

1. **Core mission:** a coding agent that is trustworthy and effective in a real workspace.
2. **Does this serve it?** Yes — durable, searchable, linkable project knowledge that both the human and the agent read/write, with note↔code links no other store has.
3. **Architecture impact:** *strengthens* it only if it stays an Extension composed from existing pieces. It *weakens* it the moment it grows a bespoke graph DB, a plugin runtime, or its own editor — those are hard stops.
4. **Real or hypothetical?** Real: there is no project-scoped, human-curated, linkable note store today (verified — see "what we drop").
5. **Simpler alternative?** The simplest useful version is "a folder of `.md` + a markdown read-mode bolted onto the panel we already have." That is precisely the MVP. Everything beyond it is phased and independently gate-able.

### Obsidian features we deliberately DROP, and why

| Dropped feature | Reason |
|---|---|
| **Global / force-directed graph view** | A string-grid TEA terminal cannot legibly render it (`render_ide` budgets a 1/3-width tree + a width-truncated right pane). An agent navigates by search + tree + backlinks, not by staring at a node cloud. Backlinks answer every "what connects to this" question. |
| **1-hop ASCII "local graph"** | Considered and **rejected**. It is honestly just a neighbor list with arrows — net-new overlay code for marginal value. The backlinks list is the honest, sufficient form. |
| **Canvas / whiteboard** | Pure spatial GUI affordance with no ANSI representation. Orthogonal to the mission. |
| **Live-preview WYSIWYG** | We already inherit a cleaner split from `/ide`: rendered read-mode (`Markdown::render`) ⇄ raw vim edit-mode (the `IdeFile` engine), toggled by one key. Re-running comrak per keystroke and merging it with cursor-accurate editing is a large build for negative clarity. |
| **Daily notes / calendar / periodic notes** | **DRY violation.** The agent memory layer already writes journal-style `memory/YYYY-MM-DD.md` daily logs (`crates/memory/src/sqlite/markdown.rs`). A coding KB is topic-keyed (architecture, gotchas), not date-keyed. We do not duplicate the journal. |
| **Templates / Templater JS** | Arbitrary templating is a scope+security hole. If a note needs scaffolding, ask the agent — it already authors files. |
| **Community plugin ecosystem** | a3s already has a skills/plugins system. A second plugin runtime violates Minimal-Core. KB extensibility is the `MemoryStore` / `ContextProvider` traits + skills. |
| **Sync / publish / encryption** | The vault is plain workspace content. Source-control, encryption, and publishing are handled outside the TUI boundary. |
| **Dataview / properties query DSL** | A whole sub-product. Tag filter (P1) + full-text/semantic search (P2) covers the real need. |
| **Embeds / transclusion `![[ ]]`** | Recursive render + cycle handling for marginal benefit. Following a link covers reuse-of-content. YAGNI until asked. |
| **Themes / per-note CSS** | `/theme` already cycles the syntect theme the comrak renderer respects (`SYNTAX_THEME`, `syntax.rs:156`). Per-note CSS is meaningless in ANSI rows. |
| **PDF / media / attachments** | Coding notes are text. Only the half-block image preview path (`ide.rs:121-128`) exists, and that is enough. |

---

## Architecture

### Where the vault lives

**Default vault root: `<workspace>/.a3s/kb/`.**

This is a deliberate, verified choice:

- **Inside the workspace backend** so the agent's workspace-bounded file tools can write it. Agent file I/O is normalized through `ctx.resolve_workspace_path` (`crates/code/core/src/tools/types.rs:121`); a vault *outside* the workspace backend is silently unreachable by the agent. This is a hard constraint.
- **Under `.a3s/`** (alongside the already-committed `.a3s/agents/` and `.a3s/skills/`, discovered by the same cwd walk-up in `src/tui/config.rs`) so the feature is opt-in, project-scoped, **committable/team-shared**, and does not pollute the repo root.
- **Chosen over `~/.a3s/`** so it is project-scoped and survives clone, and **kept strictly separate from `~/.a3s/memory/`** (the per-user, append-only agent memory). Different formats, different owners. **Do not fuse them.**
- Path is overridable via a `kb_dir` key in `.a3s/config.acl` (A3S ACL is preferred over TOML, per AGENTS.md). Created on first `/kb` if missing.

The `.a3s/kb/` directory is visible to the `/ide` tree walker (`ide_children`, `mod.rs:630`, which seeds from cwd and does not skip `.a3s/`) with **zero new wiring**.

### On-disk layout & note format

```
.a3s/kb/
├── architecture.md
├── gotchas/
│   └── libkrun-env-quoting.md
└── decisions/
    └── why-microvm-not-container.md
```

- One plain CommonMark `.md` file per concept. Nesting allowed (the tree handles it for free).
- **The vault format is Google's Open Knowledge Format (OKF v0.1)** — the single,
  vendor-neutral knowledge format for this feature (see
  [knowledge-compilation.md](./knowledge-compilation.md)). OKF requires exactly one
  frontmatter field — `type` — plus standard optional fields, parsed by the
  **existing** skills technique (`splitn(3, "---")` + `serde_yaml`,
  `crates/code/core/src/skills/mod.rs:145`):

```yaml
---
type: Architecture Decision           # REQUIRED by OKF
title: Why microVM, not container
description: Rationale for libkrun MicroVMs over containers.
resource: crates/box/src/runtime.rs   # the concept's canonical source (path or URL)
tags: [architecture, security]
timestamp: 2026-06-30T12:00:00Z        # ISO 8601
source: compiled | user                # provenance (see Risks)
---
```

The body is normal CommonMark with **standard markdown links** (the OKF graph).

### Links model — OKF standard markdown links

OKF turns the directory into a graph via **normal markdown links** — there is no
`[[wikilink]]` syntax, so comrak renders the links natively (no separate
extraction pass, no renderer fork). A `[[wikilink]]` engine is explicitly **not**
built — one knowledge mechanism, OKF.

- **Concept links**: `[name](/dir/other.md)` (bundle-relative) connect concepts.
  "The file path is the concept's identity" (OKF), so resolution is just the path;
  an `index.md` per directory provides hierarchical navigation (OKF reserved name).
- **Note↔code links** (the coding-agent differentiator): a markdown link to a
  workspace file, e.g. `[runtime.rs](crates/box/src/runtime.rs#L42)`, opens that
  file **read-only** in the same viewer. No other knowledge store has this edge.
- **Backlinks & search are computed on demand with `rg`, NOT a persistent index.**
  A backlink index is a cache you must invalidate on every edit; `rg` over a
  personal-scale vault is sub-100 ms. Backlinks = `rg -l --fixed-strings
  "<slug>.md" .a3s/kb/`; search = `rg -n --color=never <query> .a3s/kb/`. Reach for
  a stored, feature-gated index only when a **profiler** (not a hypothesis) demands it.
- **Following links from the rendered view**: read-mode ANSI rows are
  width-wrapped and lose source byte positions, so link-follow runs off a parallel
  extraction of the raw body, surfaced as a **numbered strip** followed with
  **digit keys** (reusing the existing HITL digit-key precedent) — sidestepping
  cursor hit-testing in width-wrapped CJK ANSI.
- **Tags**: frontmatter `tags:` plus inline `#tag` as plain text. `rg '#tag' .a3s/kb/`
  enumerates them. No tag DB, no tag pane in P0/P1.

---

## TUI integration

The KB **is** the `/ide` panel, re-seeded at the vault root, with a markdown read-mode. Reuse is near-total.

### Command plumbing

> **Current shipped boundary:** `/kb` is local personal-KB ingestion, search, and
> vault browsing only. `/kb add <text>` and `/kb import <path>` write local notes
> and copied text sources under `.a3s/kb/sources/`; `/kb search <query>` searches
> that personal vault; `/kb vault` opens it in the local file browser. Shareable
> OKF knowledge-package assets live under `.a3s/okf` and are handled only by
> `/okf <description>`, `/okf clone`, `/okf review`, `/okf publish`,
> `/okf deploy`, `/okf status`, `/okf list`, and `/okf activity`.

- Add one entry to `SLASH_COMMANDS` (`src/tui/mod.rs`):
  `("/kb", "browse/edit the project knowledge base")`.
- Add a `"/kb" =>` arm to the `match trimmed` dispatch (`mod.rs:2730`), mirroring the ~12-line `/ide` arm at `mod.rs:2898`. It calls a new `open_kb_in_ide(root)` helper cloned from `open_config_in_ide` (`mod.rs:3641`) / `open_readonly_in_ide` (`mod.rs:3674`), seeding `entries` from `.a3s/kb/` (created if absent) instead of `self.cwd`.
- `/kb` is read/edit-only and does not mutate the conversation, so it need **not** be added to `IDLE_ONLY` (`mod.rs:145`).

### Reused code (exact references)

| What | Where | Reused for |
|---|---|---|
| `Ide` / `IdeFile` / `IdeEntry` / `EditMode` structs | `mod.rs:619 / :360 / :343 / :354` | The note browser + buffer model, verbatim |
| `ide_key` (tree + editor dispatch) | `panels/ide.rs:7` | Tree nav, Esc layering, Tab-to-editor, open branch |
| `IdeFile::edit_key` vim engine (Normal/Insert, motions, `dd`/`yy`/`p`, undo, yank, multibyte-safe) | `panels/ide.rs:398` (impl `:265-779`, tests `:781-935`) | The note editor — **no new editing code** |
| `render_ide` split-pane renderer (1/3 tree clamped 16..38 cols + right viewer + footer hint) | `panels/ide.rs:154` | The two-pane vault UI |
| Ctrl+S save + `touch_workspace_file_path_for_manifest` | `panels/ide.rs:50-72`, `mod.rs:1143` | Note save; auto-registers in the "recently touched" manifest → free "recent notes" affordance |
| `ide_children` dir walker (skips `.git`/`target`/etc., dirs-first sort) | `mod.rs:630` | The vault tree source |
| `a3s_tui::markdown::Markdown::render` (comrak + syntect; tables/tasklist/headings) | `crates/tui/src/markdown.rs:40` (`with_width :30`, `with_theme :35`) | **Read-mode rendering** — closes the gap where `/ide` shows `.md` as plain text |

### The single behavioral delta vs `/ide`

`lang_of` (`syntax.rs:5`) has **no markdown case**, so `/ide` shows `.md` as plain highlighted text. The KB open branch instead routes the `.md` body through `Markdown::render` once on open.

To get read-mode behavior with **the most surgical change possible** (grafted from the Agent-native proposal — cleaner than a whole new `NoteView` mode enum): add **one field** to `IdeFile` (`mod.rs:360`):

```rust
rendered: bool,   // pre-rendered ANSI read-mode buffer (markdown), not editable
```

OR it into the **two existing `f.image` checks**:
1. the "show raw rows / no line numbers" render branch (`panels/ide.rs:228-230`), so the rendered ANSI lines display as-is; and
2. the "block edits" key branch (`panels/ide.rs:76`), so the buffer is read-only.

A rendered markdown buffer then behaves exactly like the image preview without being one. `e` (or Tab) re-creates the buffer from the raw file as a normal editable `IdeFile` to drop into vim edit-mode; Esc returns to the tree.

**Render once, never per frame.** (Grafted, to avoid the known O(render-per-frame) jitter trap on resize/large notes.) On open we compute `Markdown::render(raw)` a single time into the read-only buffer — the lines **are** the ANSI. We do not re-render in the draw loop. (Re-render only on explicit re-open or width change.)

### New code, contained

A new file `src/tui/panels/kb.rs` (sibling of `panels/ide.rs`, well under the 500-line limit per AGENTS.md) holds `open_kb_in_ide`, the OKF link-follow pass (resolve a standard markdown link under the cursor to a bundle file or code path; `index.md` handling), and the `rg`-backed `backlinks`/`search`. `mod.rs` gains only the `SLASH_COMMANDS` entry, the dispatch arm, the `rendered` field, and `mod kb;`. P1 link rendering / numbered strips live in `kb.rs` (or a sibling `kb_links.rs` if it grows). All output is pre-wrapped width-aware ANSI — no HTML, no graph.

### Graceful degradation

If `rg` is not on PATH, backlinks/search set a one-line `flash` footer ("install ripgrep") rather than reinventing a recursive walker. (`rg` is already the project's search primitive — the `grep` builtin requires it.)

---

## Agent integration

Three layers, **phased**, each independently shippable. The agent needs **zero new tools**.

### Write path (P0/P1) — convention + one skill, no new capability

The vault is just `.a3s/kb/*.md` inside the workspace backend, so the agent **already** has read/write/edit/grep/glob over it via the capability-gated builtin file tools (`crates/code/core/src/tools/builtin/{write,read,edit,grep,glob_tool}.rs`, registered `builtin/mod.rs:37-58`, all routed through `ctx.resolve_workspace_path`). `WriteTool` creates the note + parent dirs.

The only addition is a shipped built-in **`kb` SKILL.md**, loaded by the existing skills registry (`registry.rs:124` `load_from_dir`), mirroring the `crates/box/integrations/skills/` pattern. It teaches the agent the **convention**, not a capability:

- where the vault is (`.a3s/kb/`);
- the OKF package format (required `type` frontmatter + CommonMark + standard markdown links);
- *when* to capture (a settled decision, a gotcha hit, an architecture map) and to link related notes with `[name](/dir/other.md)` and the code it touched with `[runtime.rs](crates/box/src/runtime.rs#L42)`.

This captures the auto-curation value with **zero new tool**, which is why the skill ships in **P0/P1, not P2**. Because the links are standard markdown (which P1 follows + backlinks via `rg`), the agent participates in the OKF link graph using only `WriteTool`.

### Read path (P2) — retrieval into context via the existing trait

A `KbContextProvider` implements the **existing** `ContextProvider` trait (`crates/code/core/src/memory.rs:328`, exactly like `MemoryContextProvider` at `:294`). Each turn it surfaces the top 3–5 task-relevant notes into the prompt (substring + recency for the default impl; reuse `MemoryContextProvider`'s relevance/freshness shape and item→context conversion), tagged `kb://<slug>`, capped and threshold-gated to avoid token bloat. It is registered next to `MemoryContextProvider` only when `.a3s/kb/` exists, so notes auto-surface without manual `@`-mention.

Per the typed-extension-options rule (CLAUDE.md:401), it takes a **typed `KbStore` object** (default `FsKbStore` scanning the dir; a feature-gated `MemoryStore`-backed FTS5/`sqlite-vec` index as the swap-in), never a raw `kbDir: string`.

### Relation to the existing memory system — keep the boundary

There are **two distinct stores and they stay distinct**:

| | `.a3s/kb/` (this feature) | `~/.a3s/memory/` (existing) |
|---|---|---|
| Owner | human-curated, agent-assisted | agent auto-curated |
| Scope | project, git-committed | per-user, cross-project |
| Format | OKF: `type` frontmatter + CommonMark + standard markdown links | append-only `## ts · type · importance` blocks (`crates/memory/src/sqlite/markdown.rs`), **no** frontmatter, **no** links |
| Authority | the `.md` files themselves | SQLite is authoritative; `.md` is a mirror |

**Do not fuse them.** Auto-promoting memory items into KB notes is explicitly out of scope (it would duplicate the append-only writer and risk link-less dumps). The `kb` SKILL.md and docs must state this boundary so neither the human nor the agent writes to the wrong place.

---

## MVP (P0) — smallest shippable slice

P0 is essentially "the minimal `/ide` reskin" and ships value with **zero new agent code**. Concrete task list:

1. **`src/tui/mod.rs`**
   - Add `("/kb", "browse/edit the project knowledge base")` to `SLASH_COMMANDS` (`:103`).
   - Add a `"/kb" =>` dispatch arm (next to `/ide` at `:2898`) that resolves the vault root (`.a3s/kb/`, `kb_dir` override from `.a3s/config.acl`), `create_dir_all`s it, and calls `open_kb_in_ide(root)`.
   - Add `open_kb_in_ide` (clone of `open_config_in_ide`, `:3641`) seeding `entries = ide_children(root, 0)`.
   - Add the `rendered: bool` field to `IdeFile` (`:360`) and OR it into the two `f.image` checks (`panels/ide.rs:228-230` render branch, `:76` block-edits branch).
   - `mod kb;` next to `mod ide;`.

2. **`src/tui/panels/kb.rs`** (new file)
   - `open_kb_in_ide(root)` helper + a thin `kb_children` wrapper over `ide_children` filtering to dirs + `.md` (single point of change).
   - Read-mode open branch: when a `.md` is opened in the KB panel, build a `rendered`/`readonly` `IdeFile` from `Markdown::render(raw)` **once**; bind `e`/Tab to re-open the raw editable buffer; Esc returns to the tree.
   - In-panel `/` substring filter over note titles + bodies (no index, no deps).

3. **Built-in `kb` SKILL.md** (for example under root `skills/`, shipped and installable like `crates/box/integrations/skills/`)
   - Teaches vault path, note format, the human-KB vs agent-memory boundary, when to capture, how to link. Agent writes via existing `WriteTool` — **no new tool**.

4. **Footer hint**: branch the `render_ide` footer (`panels/ide.rs` hint string) on the KB panel to advertise `e edit · / filter` (and, once P1 lands, `b backlinks`).

5. **Tests** (`cargo test -p a3s-cli`, fmt + clippy clean): `kb_children` filters to dirs+`.md`; frontmatter parse round-trips; read-mode renders a sample note; Ctrl+S writes + touches the manifest.

**P0 explicitly has NO link-following, NO backlinks, NO graph, NO new agent tools, NO context provider** — just an OKF-vault browser/editor.

---

## Roadmap (P1 / P2) — trait-based extensions

### P1 — links & navigation (the only substantial net-new code)

- **OKF link resolution** in `kb.rs`: parse the standard markdown links in a
  concept's **raw body** (`[text](/dir/other.md)` and `[text](path/to/code.rs#Lnn)`);
  resolve bundle-relative paths to files (the path is the concept identity — no
  alias/title fuzzy-matching needed). comrak already renders the links; this pass
  is only for *following* them.
- **Note↔code links**: `[foo](crates/box/src/runtime.rs#L42)` opens the workspace
  file read-only in the same viewer.
- **Follow via numbered strip + digit keys** from the raw buffer (reuse HITL
  digit-key precedent); reading view stays display-only.
- **Backlinks on demand via `rg`** (no stored index): a `b` toggle lists
  `rg -l --fixed-strings "<slug>.md" .a3s/kb/` results, opened read-only; graceful
  `rg`-missing flash.
- **Search on demand via `rg`**: `/kb <query>` → `rg -n` over the vault; Enter on a
  hit opens the note.
- Unit tests for the resolver (a link span at the cursor, multiple links per line,
  bundle-relative vs. code paths).

### P2 — agent retrieval & provenance (feature-gated)

- **`KbContextProvider`** implementing `ContextProvider` (`memory.rs:328`), registered beside `MemoryContextProvider`, top-N capped + threshold-gated, taking a typed `KbStore`.
- **Feature-gated index**: back search with a `MemoryStore` (`crates/memory/src/lib.rs:201`) — `FileMemoryStore` default (dep-free), `sqlite` FTS5 / `sqlite-vec` semantic behind Cargo features so embedders pay nothing. Only build this when a profiler shows `rg` is too slow at real vault scale.
- **Provenance & anti-reward-hacking** (carried from the Agent-native caveat): write `source: agent|user` frontmatter; expose agent-authored notes for user review via `/kb`; **keep the KB out of any self-evolution fitness signal**, since an agent that both edits and is fed the KB is a reward-hacking surface. This is a documented constraint, not optional.

---

## Risks & open questions

1. **Render-vs-source mapping (top correctness risk).** Rendered ANSI rows lose source byte positions. **Mitigated by decision:** follow links from the raw buffer only via a numbered strip; reading view is display-only. Must hold this line or link hit-testing in width-wrapped CJK ANSI gets fiddly.
2. **Render jitter.** Re-rendering comrak per frame would jitter on large notes/resize. **Mitigated:** render once into the read-only buffer on open; never per frame.
3. **`rg` dependency.** Backlinks/search shell out to `rg`. **Mitigated:** one-line "install ripgrep" flash; never reinvent a walker.
4. **OKF link edge cases:** a renamed/moved concept file leaves dangling relative links. **Mitigated:** links are plain file paths (no fuzzy stem/alias matching to get wrong), the compiler's verify step drops danglers, and `rg` finds referrers. Rename-with-link-rewrite is **not** built — the agent does it more cheaply with grep+edit on request.
5. **Context-feed noise / token bloat (P2).** Auto-surfacing notes can pad/distract. **Mitigated:** cap top-3–5, relevance threshold + freshness weighting, same tuning as `MemoryContextProvider`.
6. **Two near-identical panels (`/ide` vs `/kb`).** DRY pressure + user confusion. **Mitigated:** `kb.rs` reuses `Ide`/`IdeFile`/`ide_key`/`render_ide`; the only intended deltas are the `.md` filter, the `rendered` field, and the link passes.
7. **Two markdown "memory" systems.** Risk of writing to the wrong store. **Mitigated:** document the `.a3s/kb/` ↔ `~/.a3s/memory/` boundary in the `kb` SKILL.md and docs; never fuse.
8. **Co-curation write conflict.** An agent edit landing while the user has a note open could clobber unsaved changes. Low risk (single user); rely on the dirty/manifest model and warn on external change.
9. **Reward-hacking surface (P2).** Agent edits what it is later fed. **Mitigated:** `source` provenance + user review + KB excluded from fitness signals.
10. **Scope discipline.** The pull toward daily notes / templates / graph / dataview once the panel exists is exactly what Rule 1 guards against. Re-run the pruning audit (CLAUDE.md:367-375) after each phase; if the KB starts growing a bespoke graph DB, plugin host, or its own editor, it has failed Rule 1/2 and must be cut back.

**Open questions for maintainers:**
- Confirm `.a3s/kb/` (committed, team-shared) over a gitignored agent-private variant — current recommendation is committed-by-default with `kb_dir` override.
- Confirm the `kb` SKILL.md ships built-in/auto-installed vs opt-in install.
- P2 only: pick `FsKbStore` (rg/scan) as the permanent default and gate the SQLite/vec index strictly behind a profiler-proven need.
