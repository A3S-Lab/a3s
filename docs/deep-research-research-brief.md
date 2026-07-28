# DeepResearch Research Brief Contract

Status: retained representation and failed batch-acquisition experiment. The
first live C02 run falsified the one-shot query allocation and status semantics.
The later feedback loop was also rejected; the reset persisted-evidence
hypothesis is [`deep-research-evidence-loop.md`](deep-research-evidence-loop.md).
This file does not authorize a production-path migration.

This contract supersedes the full Evidence Compiler as the next architecture
hypothesis. The compiler correctly identified semantic continuity as a product
requirement, but required the planner to predict a complete source-target and
claim graph before acquisition. Live runs showed that structurally reasonable
research intent could therefore fail before any source was acquired.

## Decision

DeepResearch is a bounded evidence search, not a one-shot evidence compiler.
The only semantic identity that must survive every stage is the material
research dimension. Queries, attempts, fetched sources, report blocks, and
gaps refer to dimensions, but they do not form a model-authored proof graph.

The originally proposed candidate sequence was:

```text
request
  -> small ResearchBrief
  -> transport-specific discovery and acquisition
  -> dimension-scoped report blocks and explicit gaps
  -> Host-owned ReportDocument
  -> Markdown and HTML
```

The brief representation remains useful, but the live run proved that all
queries cannot be executed as a static batch with preassigned source slots.
The replacement retains at most one brief generation and one report-block
generation while moving planning behind bootstrap discovery and making later
acquisition depend on observed evidence.

## Planner Wire Contract

The model proposes only this bounded shape:

```text
dimensions[]
  id
  question
  material

queries[]
  id
  text
  transport
  path
  glob
  dimension_ids[]
  preferred_sources[]
    kind                  # repository | domain | url | workspace_path
    value

planning_gaps[]
  dimension_id
  reason
```

The request, report language, current date, evidence scope, query cap, and
source-attempt cap are Host input. The planner does not author source IDs,
source families, source roles, exact source-target edges, fetch allocations,
claim bases, Markdown, HTML, or terminal status.

Source preferences are ranking hints, never admission requirements. An invalid,
unsafe, out-of-scope, or unmatched preference is ignored without invalidating
its query. A query remains eligible for ordinary relevance-based selection.

## Host Normalization

Normalization is monotonic and local:

1. Retain every structurally valid dimension with a unique stable ID.
2. Drop only an invalid query, unsafe workspace path or pattern, unknown
   dimension edge, or invalid source preference.
3. Enforce the transport scope and the shared query cap.
4. Add a Host planning gap to every material dimension that has neither a
   surviving query edge nor a valid planner gap.
5. Assign source-attempt capacity after normalization. Every query receives
   one initial opportunity; remaining capacity is a shared reserve for a
   second relevant source or fetch backfill. The cap is a maximum, not a target
   that must be exhausted.
6. Never reject the complete brief because optional preferences, one query,
   one edge, or one gap is malformed.

If no model dimension survives, the Host may create one fallback dimension for
the complete request. That fallback preserves terminal progress but is a
qualified result, not evidence that semantic planning succeeded.

## Acquisition Semantics

Web and workspace acquisition share dimension identities and attempt records,
but not candidate-selection algorithms.

### Web

For each query, rank unique canonical candidates in this order:

1. valid preferred repository, domain, or URL match;
2. semantic relevance to the query and its dimensions;
3. first-party or canonical identity signals available in discovery metadata;
4. provider score and order.

An unmatched preference never makes the candidate set empty. Duplicate URLs
merge provenance and do not consume a query opportunity or source call. A
failed fetch consumes one source attempt; the Host may use remaining shared
capacity to try the next candidate.

### Workspace

Workspace search is code navigation, not web ranking. The executor must:

- constrain code-seeking queries to source paths when the brief supplies a
  safe path or glob;
- retain grep result order and matched context;
- rank owning code and call sites above documentation, lockfiles, generated
  files, and changelog prose;
- preserve the query-to-file provenance; and
- fetch the matched region or a bounded owning file view rather than treating
  a filename as evidence.

Sorting workspace anchors through a set before scoring is invalid because it
turns relevance into path order.

## Three Gap Types

The Host keeps three gap types distinct:

| Gap | Meaning | Required provenance |
| --- | --- | --- |
| `planning` | No valid query was scheduled within the brief and budget. | Dimension ID and planning reason. |
| `acquisition` | A query was attempted but no qualifying source was fetched. | Dimension ID and real attempt IDs. |
| `support` | Sources were fetched, but their text did not establish a safe answer. | Dimension ID, source or attempt IDs, and a bounded reason. |

A gap describes this run's evidence boundary. It must not claim that evidence
does not exist globally. In particular, an unavailable-evidence answer may say
that the bounded search found no qualifying public record and therefore cannot
support the requested comparison. It may not say that no such private record
exists.

## Report Proposal And Admission

The optional report generation proposes independent items:

```text
blocks[]
  dimension_id
  section                # answer | finding | recommendation | limitation
  text
  evidence_refs[]
    source_id
    chunk_ids[]

gaps[]
  dimension_id
  section                # answer | limitation
  gap_type               # planning | acquisition | support
  text
  attempt_ids[]
  source_ids[]
```

The historical proposal admitted or rejected each item independently through
identities, exact source and chunk membership, and attempt provenance. Numeric,
language, URL, and other prose checks were rejected because they confused text
shape with semantic support. The active runtime never matches report prose to
source prose; see
[`deep-research-evidence-first-redesign.md`](deep-research-evidence-first-redesign.md).

Every material dimension ends with at least one admitted block or one Host
gap. A rejected item cannot remove an admitted sibling. A gap-only answer is a
valid qualified report. The complete report is never rejected because a table
header, section transition, or bounded Host-authored evidence statement lacks
a source citation.

Markdown and HTML are projections of one Host-owned `ReportDocument`. The
model does not author either artifact or the source ledger.

## Paper Walkthrough

The shared envelope is one brief generation, at most four queries, at most
eight source attempts, and one optional report-block generation.

### C01: broad current technical comparison

The request naturally yields maintenance, HTTP compatibility, database
compatibility, new-project choice, legacy migration, and evidence-boundary
dimensions. Four queries are sufficient when recommendation and boundary
dimensions share the factual queries:

1. official Tokio and async-std maintenance records;
2. official HTTP-library runtime requirements;
3. official database-library runtime requirements; and
4. official async-std discontinuation or migration guidance.

Repository or domain preferences steer selection toward Tokio, async-std, an
HTTP library, and a database library without requiring a complete target graph.
Two relevant fetch opportunities per query fit the cap. Performance, adoption,
or migration-cost statements remain support gaps unless fetched primary text
establishes them.

### C02: narrow canonical fact

Five dimensions share two coherent first-party queries: the canonical Tokio
LTS policy and the canonical newest-release record. A `tokio-rs/tokio`
repository preference ranks both without excluding an equivalent first-party
page. The Host need not consume all eight source attempts. Linux LTS or other
high-ranked cross-project results cannot outrank a matching canonical
preference.

### C05: local active-path trace

Seven dimensions fit four source-scoped workspace searches: submission and
entrypoints, acquisition and source catalog, report materialization and
publication, and browser or presentation plus active-path gates. Searches are
restricted to `src` or the relevant DeepResearch source subtree. Grep context
and call-site ownership rank runtime files above README, changelog, lockfiles,
and architecture prose. If no browser-opening call is present, the browser
dimension becomes a support gap rather than an inference from component docs.

### C07: ambiguous superlative

The brief turns "best" into explicit criteria, evidence window, scenario
comparison, and bounded recommendations. Queries seek current first-party
documentation for a small viable framework set plus claim-appropriate
maintenance, adoption, or benchmark context only if those criteria are used.
Promotional comparisons cannot establish a universal winner. A report may
recommend different frameworks for different scenarios and must bound any
criterion without comparable evidence.

### C08: intentionally unavailable private evidence

Two dimensions share one or two exploratory queries for public,
institution-level incident-rate records. No canonical runtime repository is
misrepresented as capable of proving private institutional rates. If the
attempts return no qualifying primary record, the Host publishes an
acquisition gap and the conclusion that this run cannot determine which
runtime causes fewer incidents. Incidental runtime bugs, popularity, and
maintenance evidence are not admitted as substitutes.

## Falsifiable Admission Gates

The candidate is rejected unless equal-budget live runs show all of the
following:

1. C01 fetches primary runtime, HTTP, and database material and preserves all
   material dimensions as findings or specific gaps.
2. C02 selects the canonical Tokio record and does not admit cross-project LTS
   noise.
3. C05 traces owning source call sites instead of reconstructing the active
   path from README, changelog, or lockfile text.
4. C07 avoids an unsupported universal ranking and produces scenario-bounded
   advice from claim-appropriate sources.
5. C08 publishes a valid gap-only answer without converting anecdotes into
   incident-rate evidence and without whole-report rejection.
6. Duplicate candidates, malformed preferences, one invalid query, and one
   invalid report item are locally salvageable within the shared caps.
7. Markdown and HTML are non-empty, semantically equivalent, localized, and
   readable at desktop, mobile, and print widths.

Passing unit tests is not sufficient. Production migration requires the live
gates, fault matrix, and visual artifact inspection.
