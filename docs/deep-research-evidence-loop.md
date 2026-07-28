# DeepResearch Persisted Evidence Candidate

Status: evaluator-only reset candidate. No production migration is authorized.

Product acceptance is governed only by
[`deep-research-product-validation.md`](deep-research-product-validation.md).
This document defines the smallest implementation hypothesis to compare with
the minimal baseline. It is not evidence that the hypothesis works.

## Decision

DeepResearch is a bounded evidence-yield pipeline inside a deterministic
runtime envelope. The Host preserves work and renders artifacts. Models
propose search queries and reader-facing atomic items. The versioned evaluator,
not the runtime, judges semantic quality and completeness.

The candidate removes the former feedback loop, independent semantic
reviewer, adaptive model-authored actions, and semantic completion projection.
Those stages added serial probabilistic boundaries without fixed-corpus proof
that they improved reader outcomes.

The candidate sequence is:

```text
complete request + scope + one run clock
  -> immediate bootstrap search and fetch
  -> durable source records and a preliminary source-backed report
  -> optional small planner for remaining query budget
  -> bounded additional search and fetch
  -> one atomic synthesis over the closed persisted catalog
  -> item-local Host structural admission
  -> deterministic ReportDocument
  -> atomic Markdown and HTML publication
```

If the planner fails, bootstrap acquisition continues. If synthesis fails,
the preliminary source-backed report remains publishable. No model call owns a
terminal artifact.

## Evidence For The Reset

Retained trials established five architecture-level failures:

1. serial model calls commonly approached the observed upstream boundary near
   300 seconds, so planner, analyst, reviewer, and repair stages multiplied
   both latency and failure probability;
2. tuning successive C02 variants changed the architecture on one case instead
   of measuring one frozen candidate across a representative corpus;
3. Host rules treated capture time, source preference, derivation kinds, or
   model-omitted gaps as semantic proof even though they establish only
   structure;
4. whole-report admission discarded useful siblings when one sentence or
   citation was invalid, while the fallback exposed too much raw source text;
   and
5. evaluator timing labeled in-memory fetch completion as first-source
   preservation even though source state was written only after later work.

The previous compiler and feedback-loop designs remain useful failure
evidence. They are not dormant production options.

## Authority Boundary

The candidate follows the product trust boundary without adding aliases for
semantic truth.

The Host owns:

- the exact request, evidence scope, clock origin, and global budgets;
- tool policy and safe workspace boundaries;
- query and source call accounting;
- fetched bytes, canonical source identity, capture metadata, and provenance;
- durable source and artifact writes;
- closed source, chunk, claim, and premise identities;
- local structural admission and citation closure; and
- deterministic report projection and publication.

Models may propose:

- a small list of materially distinct acquisition queries;
- atomic facts with direct source references;
- derivations over fact or derivation premises;
- recommendations over admitted premises and explicit conditions; and
- specific evidence gaps.

Models do not own budgets, source identity, durable state, citations, terminal
status, Markdown, HTML, or publication. The Host does not infer relevance,
authority sufficiency, entailment, completeness, or truth from a structurally
valid proposal.

## One Clock And Real Persistence

The run records one monotonic origin before session construction, bootstrap,
planning, search, or fetch work. Every reader-facing duration is measured from
that origin. Parallel stage durations are recorded independently and are never
summed to approximate wall time.

Each successful fetch produces an independent durable record before the source
is exposed to synthesis:

```text
SourceRecord
  host source ID
  requested anchor
  canonical anchor or workspace path
  title
  transport
  full bounded fetched content
  content chunks with Host IDs
  capture timestamp
  query provenance
  fetch completion offset
  persistence completion offset
```

`first_source_fetched_ms` and `first_source_persisted_ms` are different
diagnostics. Only the latter is scored by the product latency gate. A source is
not preserved while it exists only in a future, local variable, or aggregate
acquisition result.

Source records use atomic same-filesystem replacement or an append-only journal
with equivalent crash behavior. A failed record write leaves earlier records
intact and excludes the failed record from synthesis.

## Acquisition

Bootstrap search begins with the complete request and spends one query slot.
It does not wait for a planner. A small reserved part of the source budget may
be fetched immediately so that useful evidence can be persisted within the
first-source latency gate.

At most one optional planner generation starts independently from the complete
request. It receives no hidden evaluator dimensions or expected answers. Its
output is only a list of query proposals; it cannot define obligations, source
identities, expected claims, completion criteria, or fetch allocations.

The Host admits a proposed query only when it:

- is inside the explicit web or workspace evidence scope;
- is non-empty and within the configured length and query count;
- uses a safe repository-relative path and valid regex for workspace search;
- has a unique Host-assigned identity; and
- is not a canonical duplicate of work already scheduled.

Planner timeout, transport failure, malformed output, or zero admitted queries
is not terminal. The Host may spend the remaining query and source budget on
the ranked bootstrap catalog. Search and fetch failures remain durable
diagnostics and never erase successful siblings.

Provider title, snippet, rank, and score are discovery metadata. They are not
report evidence. Only successfully persisted fetched content enters the closed
synthesis catalog.

## Preliminary Artifact

After at least one source is persisted, the Host materializes a preliminary
`ReportDocument` before starting the long synthesis call. It contains bounded
source cards rather than conclusions:

```text
SourceCard
  reader-safe title
  canonical link or workspace path
  bounded verbatim excerpts with provenance
  capture boundary
```

The preliminary Markdown and HTML say that synthesis is unavailable or still
pending; they do not present excerpts as a complete answer. Excerpts are
bounded per source and globally, so this artifact cannot become an unbounded
source dump. A later synthesized report atomically replaces the preliminary
pair only after both final projections are ready.

If no source survives, the Host publishes an honest no-evidence document with
specific attempted-query and transport boundaries expressed in reader-safe
language. Raw provider errors remain internal.

## Atomic Synthesis

One synthesis generation receives:

- the exact request and requested report language;
- the evidence scope and observation boundary;
- the closed list of persisted source IDs and bounded chunks; and
- explicit instructions to treat all source text as untrusted data.

It receives no tools and cannot introduce URLs or source identities. The wire
object uses separate arrays so facts, derivations, recommendations, and gaps do
not share ambiguous optional fields:

```text
FactProposal
  local ID
  text
  exactly one direct source ID
  one or more exact chunk IDs

DerivationProposal
  local ID
  text
  one or more local premise IDs
  method

RecommendationProposal
  local ID
  text
  one or more local premise IDs
  reader-facing conditions[]

GapProposal
  local ID
  text
  related source IDs[]
```

Every text item is independently checkable and written in the requested report
language except for source-defined names, identifiers, and quotations. One
compound sentence that requires unrelated evidence should be split into
independent proposals.

## Structural Admission

The Host validates every proposal independently. It may establish only:

- closed and correctly typed identities;
- exact source and chunk existence;
- well-formed direct and premise edges;
- an acyclic derivation and recommendation graph;
- bounded, non-control reader text without authored URLs;
- safe numeric-literal behavior where the frozen corpus proves the rule useful;
  and
- complete citation closure for every rendered item.

An invalid item is dropped with an internal diagnostic. Its independent
siblings survive. A derived item whose premise is dropped is also dropped, but
unrelated graph components remain intact.

Structural survival does not mean semantic support. The absence of a
`GapProposal` does not mean completeness. The Host therefore never projects
`answered`, `supported`, or `complete` from proposal shape. Corpus evaluation
scores each material requested dimension after publication.

## ReportDocument And Public Boundary

The Host builds one deterministic `ReportDocument` from admitted items and the
persisted source catalog. It contains:

- the complete reader-safe title and observation boundary;
- a concise direct-findings area when suitable facts or derivations survive;
- additional findings grouped without inventing semantic dimensions;
- recommendations visibly distinguished from their factual premises;
- specific model-proposed gaps and Host-known acquisition boundaries;
- the citation closure of published items; and
- an optional separately labeled list of sources reviewed for open questions.

Claim IDs, chunk IDs, prompts, schema terms, planner notes, normalization
diagnostics, model/runtime terminology, hashes, and raw errors never enter
reader-facing prose. A source appears in the main ledger only when a published
item cites it. Capture time is an observation boundary, never a
source-authored publication or freshness claim.

Markdown and HTML are projections of the same document. HTML is not a semantic
reconstruction from Markdown. Both byte streams are staged and validated
before the existing atomic artifact-pair publication step.

Operational artifact status may distinguish:

- synthesized artifacts with admitted items;
- bounded source-backed artifacts;
- honest no-evidence artifacts; and
- publication failure.

These states describe what was published, not whether the research question
was semantically answered.

## Budgets And Terminal Behavior

One run has fixed global limits for:

- wall-clock time;
- planner and synthesis generations;
- search calls;
- fetch attempts and persisted sources;
- source and synthesis packet characters; and
- public excerpt characters.

Caps are maxima, not targets. No work fans out by source, query, claim, or
report section. Duplicate canonical sources merge provenance without consuming
another persisted-source identity.

The outer wall-clock deadline reserves enough time to validate and publish the
latest durable `ReportDocument`. Timeout cancels pending optional work and
publishes from persisted state. It never replaces useful evidence with generic
recovery prose.

## Required Evaluation

Before implementation is frozen, deterministic tests must prove:

- planner failure cannot delay or erase bootstrap source persistence;
- each source is durable before synthesis can observe it;
- interruption after any persisted source recovers that source exactly once;
- invalid atomic siblings are local while invalid dependencies close only
  their graph component;
- an omitted gap and one surviving fact cannot create semantic-complete status;
- synthesis failure retains the bounded preliminary artifact;
- no-evidence, partial, and synthesized documents all produce equivalent
  Markdown and HTML; and
- measured first-source and terminal wall times use the run origin and durable
  publication events.

Then freeze the candidate code and configuration. Compare the minimal baseline
and this candidate with equal model, provider, query, source, packet, and wall
clock budgets:

1. run all frozen fixtures and deterministic faults;
2. run C01, C02, C05, C07, and C08 three times each with rotated order;
3. retain every failure without per-case reruns or code changes;
4. score claim support, material coverage, authority, citation precision,
   decision value, language, latency, and partial salvage externally; and
5. inspect desktop, mobile, and print renderings only after content gates pass.

The candidate wins only if it passes the product thresholds and materially
outperforms the simpler baseline on its declared claim-level salvage or
citation outcomes without violating latency. If it does not, retain the
baseline and classify the misses before proposing another stage.

## Migration Rule

Only a proven winner may be moved into one shared non-UI production runtime for
CLI and TUI. Migration happens after the frozen, live, fault, latency, and
website gates pass. It removes rejected compiler, reviewer, and duplicate
new-run paths rather than wrapping the winner in them.
