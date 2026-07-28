---
name: okf
description: "Compile the project's knowledge into an Open Knowledge Format (OKF) bundle — a directory of cross-linked markdown 'concept' files under .a3s/kb/wiki/ (the LLM-wiki pattern, DeepWiki-style, per Google's OKF v0.1). Use when the user asks to build / compile / refresh the knowledge base, the wiki, or project docs, or runs /okf. Reads the codebase plus existing notes and writes OKF concept files (required `type` frontmatter, standard markdown links, per-directory index.md); recompiles incrementally (only concepts whose sources changed)."
kind: instruction
allowed-tools: "read(*), grep(*), glob(*), ls(*), write(*), edit(*), bash(*), parallel_task(*), task(*)"
---

# Knowledge compilation → Open Knowledge Format (OKF)

Compile this project's knowledge — its code plus any existing `.a3s/kb/` notes —
into an **OKF v0.1 bundle**: a directory of markdown *concept* files the human
browses (in `/kb` or any editor) and that you read back as context. Google's Open
Knowledge Format formalizes exactly this LLM-wiki pattern — "just markdown, just
files, just YAML frontmatter." It is a *compile*: sources in, a cross-linked
bundle out, rebuilt incrementally — not a one-shot dump.

## Output contract — an OKF bundle, written ONLY here

- The bundle root is **`.a3s/kb/wiki/`** (create it if missing); everything you
  write lives under it. NEVER touch human-authored notes (`.a3s/kb/` *outside*
  `wiki/`) or the agent memory — link to them, don't rewrite them.
- **One file per concept.** A *concept* is anything worth capturing: a module,
  crate/package, data model, key abstraction, architecture decision, runbook, or
  API. The **file path is the concept's identity** (`modules/box-runtime.md`).
  Group related concepts in subdirectories.
- **Every concept file is markdown with YAML frontmatter. OKF requires exactly one
  field — `type` — plus these standard optional fields:**
  ```yaml
  ---
  type: Rust Crate                 # REQUIRED: the concept's kind (free-form string)
  title: a3s-box                   # optional
  description: Docker-like MicroVM runtime for Linux OCI workloads.   # optional
  resource: crates/box/            # optional: the concept's canonical source (path or URL)
  tags: [runtime, microvm]         # optional
  timestamp: 2026-06-30T12:00:00Z  # optional, ISO 8601
  # OKF permits extra fields — we keep provenance for the incremental recompile:
  source: compiled                 # marks it agent-generated, not a human note
  sources: [crates/box/src/runtime.rs, crates/box/README.md]
  source_digest: <hash of the concatenated sources>
  ---
  ```
- **Links are STANDARD markdown links, NOT `[[wikilinks]]`.** OKF turns the
  directory into a graph via normal links: reference another concept with
  `[a3s-box-cri](/modules/box-cri.md)` (bundle-relative) and reference code with
  `[runtime.rs](crates/box/src/runtime.rs#L42)`. End every file with a `## Sources`
  list of the files it was built from.
- **Reserved filenames:** every directory gets an **`index.md`** — its overview +
  links to the concepts under it (this is OKF's hierarchical navigation; the bundle
  root `index.md` is the wiki home). Optionally keep a top-level **`log.md`**
  (chronological compile history). These two names are OKF-reserved — don't use
  them for ordinary concepts.

## Pipeline

1. **Survey.** Map the repo before writing a word: `ls`/`glob` the top level and
   key dirs; read the root README + manifest(s) (`Cargo.toml`, `package.json`, …)
   and each module's entry point + README. In a monorepo, each crate/package is a
   module concept. Read existing `.a3s/kb/` notes so the bundle complements and
   links to them — never duplicates.
2. **Plan the bundle.** Choose a BOUNDED concept set + directory layout (e.g.
   `modules/`, `concepts/`, `decisions/`), each directory with an `index.md`, plus
   the root `index.md`. Deterministic kebab-case slugs. Show the planned layout to
   the user before generating.
3. **Generate concepts.** Per concept: read its sources, then write an OKF file —
   required `type`, the standard fields, and a synthesized explanation grounded
   entirely in what you read (key types/functions with `[file](path#Lline)` links,
   connections to other concepts with `[name](/dir/other.md)`), ending in
   `## Sources`. Fill `sources`/`source_digest` honestly. **Fan out with
   `parallel_task`** (one concept per subtask) when available; else do them one at
   a time.
4. **Index.** Write each directory's `index.md` and the root `index.md` last,
   linking every concept with a one-line summary, so the bundle is a navigable
   graph, not a flat pile.
5. **Verify.** Every markdown link must resolve to a file you wrote (or a real code
   path+line). Fix or drop dangling links. Report the concept count and any gaps.

## Incremental recompile (this is what makes it a *compile*)

On a re-run, before regenerating a concept read its frontmatter `source_digest`
and recompute the digest of its `sources` (e.g. `cat <sources> | shasum -a 256`).
Unchanged ⇒ **SKIP** it. Only regenerate concepts whose sources changed, plus the
affected `index.md` files. Report rebuilt vs. skipped — a dependency-tracked
rebuild that keeps recompiling cheap and the bundle fresh after code changes.

## Rules

- **Ground every claim** in a file you read; link file+line; mark genuine
  uncertainty ("appears to …"); never invent an API, type, or flow. A hallucinated
  concept is worse than none.
- **No secrets** — never copy tokens/keys/`.env` values into a file.
- **Bound the run** — document concepts + modules, not every file; for a very
  large repo compile by area and report what you covered.
- **Stay in your lane** — `source: compiled` on every file; you own
  `.a3s/kb/wiki/`, the human owns `.a3s/kb/*.md`. Don't clobber either.

> OKF v0.1 — spec, conformance criteria, and sample bundles:
> `GoogleCloudPlatform/knowledge-catalog`. The only hard requirement is the `type`
> field on every concept; everything else above is convention.
