# DeepResearch Evidence-First Redesign

Status: implemented through the shared typed runner for new CLI, TUI, and Code
Web DeepResearch runs. Exact-query bootstrap acquisition and one bounded
semantic outline run concurrently. The
outline declares `focused` or `comprehensive` scope, freshness and workspace
requirements, one to four evidence tracks, and at most seven supplemental
plain-text queries. The Host preserves
the exact query, rejects URL-shaped supplements and duplicates, and separately
promotes at most three explicit HTTP(S) references from the user query to
normalized direct retrieval seeds. Credential-bearing references are rejected,
and local-only scope remains network-free. The Host owns all transport budgets,
merges bootstrap and supplemental evidence, and applies generic source and
publication gates. It contains no topic dictionary or domain-specific report
path. Open-ended query expansion,
research-brief fan-out, and feedback-loop experiments remain rejected because
they multiply probabilistic boundaries. Their retained evidence is documented
in [`deep-research-evidence-loop.md`](deep-research-evidence-loop.md). Corpus
and website evaluation remain release gates rather than runtime authority.

Product acceptance is governed by
[`deep-research-product-validation.md`](deep-research-product-validation.md).
This document records the active publication architecture and the evidence
that motivated it. A passing implementation test or durable workflow state is
not a substitute for the product gates.

The terminal-authority rule remains central: report-generation failure must not
erase fetched evidence, and the Host must own publication. Candidate selection
controls bounded fetch opportunities, but claim evidence is admitted only by a
closed semantic selection over fetched chunks. The Host then validates exact
IDs, track-relevance edges, completion-criterion coverage, source roles, and
publication provenance.

## Implementation Boundary

The active reusable implementation is the independent
`A3S-Lab/DeepResearch` repository, integrated here as the
`a3s-deep-research` crate. It owns:

- the asynchronous `DeepResearchEngine` stage machine;
- the domain-neutral planner contract and fallback plan;
- embedded retrieval and generation workflow assets;
- source-catalog and report admission;
- report quality and citation gates; and
- Markdown and HTML artifact construction.

The CLI owns only the A3S product adapter. `CodeDeepResearchRuntime` implements
the engine's structured-generation, workflow-execution, publication, and
progress ports by calling an isolated AgentSession, Flow-backed workflow, and
workspace artifact surfaces. `CodeDeepResearchRunner` is shared by headless
CLI, TUI, and Code Web; a new evidence-first run crosses this boundary through
one `DeepResearchEngine::execute_request` call. Typed engine events are
projected into the version-2 run journal for Web refresh. Historical Inquiry
events remain readable through compatibility paths; the former sectioned-report
executor and report-resume transaction are not active product paths.

## Decision

DeepResearch terminal publication must require zero successful report-synthesis
calls. Once semantically admitted fetched text exists, the Host can publish a
traceable source-backed Markdown report and equivalent HTML site without
waiting for a report writer, reviewer, repairer, or presentation model. Web
acquisition uses one closed semantic candidate decision before fetch and one
closed semantic evidence protocol over fetched chunks. Small catalogs use one
direct selector. Larger catalogs use complete source-local JSON windows capped
at 32 KiB and an exact-ID per-source reduction capped at four excerpts. A
failed candidate decision may choose bounded fallback URLs for transport
resilience, but that fallback text remains audit-only unless the fetched-text
selector admits its exact chunk IDs. Provider metadata never becomes evidence.

Source admission has no publisher allowlist, protected-host table,
query-token-overlap score, language/script routing, or topic-specific branch.
The selector returns closed obligation-relevance edges, completion-criterion
indexes, and typed supporting, primary, and independent roles. The Host rejects
unknown or malformed edges and preserves partial relevance without pretending
that a completion criterion is closed.

The normal path may make at most two attempts at one structured report proposal
over a closed, Host-owned source packet: an initial attempt and one retry for a
transient generation failure. This is an optional quality upgrade, not a
terminal dependency. It receives no tools and no source URLs. The active
`deep_research_typed_claim_graph` protocol proposes fact, inference, and
recommendation claims; exact source/chunk support; basis and derivation edges;
contradiction relations; and typed gaps. The Host compiles that graph into one
`ReportDocument`, source ledger, and pair of renderings. Exhausted attempts or
invalid output select the deterministic source-backed report already prepared
from the source catalog.

The following stages are not admitted to the active path:

- model-authored planning before the original query is searched;
- mandatory model evidence extraction;
- open-ended per-source extraction or per-question review generations;
- per-section writing;
- model semantic self-audit;
- model report repair, open-ended retries, or replay of a completed attempt; and
- model-authored editorial, guidance, or presentation frames.

One semantic outline generation is admitted only because it runs beside, not
before, exact-query bootstrap and has an exact-query-only fallback. It may
propose up to seven supplemental queries for the evidence tracks it defines.
It may not return URLs, sources, facts, conclusions, or budgets. The Host does
not infer a topic when closing the outline: it validates shape, preserves query
identity, removes transport authority from the planner, and applies fixed caps.
Adaptive search loops and unbounded model-authored expansion remain excluded.

## Evidence For The Decision

The retained multi-stage Tokio versus async-std run completed in 214.68 seconds
but used only three sources for maintenance, ecosystem compatibility, and
production selection. Its provisional quality score is 1.85 out of 4, and it
fails the reader-boundary and language gates. Earlier v26 and v33 runs show
larger model fan-out producing serial latency, late search, and recovery output.

The untuned one-generation frozen baseline produced these results with the
configured `openai/glm5.1-w4a8` model:

| Case | Model time | Outcome | Product evidence |
| --- | ---: | --- | --- |
| F01 contradiction | 283.69 s | Returned | Preserved both conflicting primary records and cited each one. |
| F02 derivation | about 300.15 s | Transport failure | No semantic result and no artifact; the connection closed before completion. |
| F05 prompt injection | about 300.07 s | Transport failure | No semantic result and no artifact; the connection closed before completion. |
| F04 Chinese report | 280.23 s | Returned | Core facts were correct, but raw source identity, English website chrome, one uncited claim, and missing semantic headings passed the syntactic Host check. |

F01 and F04 prove that one closed-evidence generation can produce useful
synthesis. F02 and F05 prove that even one call is not reliable enough to own
terminal publication. All four calls took roughly five minutes or reached the
observed upstream boundary. Reducing fan-out is necessary, but it is not a
fallback strategy.

F04 also proves that the current admission check measures the wrong thing. One
known citation token and an H1 yielded an empty violation list even though the
artifact failed citation recall, language, reader-boundary, and website gates.
More model review calls would not repair that contract error.

## Root-Cause Audit Of The Replaced Path

The legacy failure was not located in one prompt, timeout, reducer event, or
renderer branch. It was an authority error across the former control flow:

1. `execute_inquiry_pipeline` starts useful bootstrap acquisition immediately,
   but also starts an optional semantic planner and then commits the resulting
   plan into the required Inquiry contract.
2. A valid raw acquisition packet is persisted, but publication cannot consume
   that packet directly. The former path next required one batched model
   extraction to convert raw source text into accepted evidence.
3. When extraction fails, `extraction_workflow_output` correctly preserves the
   raw acquisition, but produces no accepted evidence. The TUI interprets an
   empty accepted-evidence ledger as terminal degradation, while the CLI
   rejects any output without a reportable Outlining Inquiry.
4. When extraction succeeds, publication still required the legacy sectioned
   report transaction. That transaction fans out through section writing,
   revision, editorial/guidance/presentation framing, semantic audit, and
   repair or resume paths before the Markdown/HTML pair becomes publishable.
5. The final recovery path can list source anchors, but it does not recover the
   fetched excerpts as the reader's result. It therefore replaces useful
   source text with generic workflow-oriented recovery prose.

The value-destroying transition is therefore:

```text
durable fetched source text
  -> mandatory model-authored evidence representation
  -> mandatory model-authored report transaction
  -> publishable artifact
```

Reducing the extraction to one batch did not remove either mandatory semantic
transition. It reduced call count while preserving the architectural failure:
model output still decides whether already-fetched evidence is user-visible.

The replacement must cut the dependency at the source catalog, not add another
recovery branch after it:

```text
durable fetched source text
  -> Host report document
  -> publishable Markdown/HTML

optional model proposal
  -> typed claim-graph admission
  -> replace the staged document only with an admitted graph
```

This audit rejects the following as root-cause fixes:

- increasing a client timeout beyond the observed upstream connection boundary;
- retrying mandatory extraction, review, or repair, and retrying report
  generation without a fixed bound and an already-staged artifact;
- generating accepted-evidence placeholders after extraction failure;
- retaining the sectioned transaction but reducing its number of sections;
- adding another model reviewer to catch report defects; and
- improving recovery wording without publishing the preserved source text.

Each may alter a symptom, but none changes terminal authority.

## Candidate Runtime

```text
run created
  |
  |-- exact-query bootstrap search/fetch -------------------|
  |                                                        |
  `-- bounded semantic outline (in parallel)                |
          |-- invalid/slow -> exact-query-only fallback     |
          `-- 0..3 validated supplemental queries           |
                                                           v
merge bootstrap + supplemental evidence under Host budgets
  |
  |-- generic candidate admission and source-quality fallback
  |-- fetch, canonicalize, bound, and persist source text
  v
durable Host source catalog
  |
  |-- stage deterministic extractive Markdown + HTML
  `-- optional typed claim graph (initial attempt + one retry)
          |-- failed or timed out ---------------------------|
          |-- invalid graph items removed by Host ------------|
          `-- valid claims, relations, and gaps retained -----|
                                                              v
Host-owned report document + source ledger
  |
  `-- atomic Markdown and localized HTML publication
```

The deterministic report exists before model synthesis becomes a terminal
risk. A process restart resumes from persisted source and artifact state; it
does not replay completed search, fetch, or generation effects.

## 1. Acquisition

The exact user query is sent to the configured search provider immediately.
At the same time, one structured planner identifies the semantic scope and
evidence obligations of the request. Search-engine selection and generic
fallback follow `config.acl`; the default avoids AnySearch unless the user opts
in.

The planner returns:

- `focused` or `comprehensive` research scope;
- whether freshness or workspace evidence is required;
- one to four coherent evidence tracks with completion criteria and typed
  source requirements; and
- zero to seven supplemental plain-text queries.

The planner cannot return URLs, seed sites, facts, answers, stop conditions, or
budgets. The Host prepends the unchanged exact query, rejects blank,
duplicate, whitespace-mutated, or URL-shaped supplements, and caps the complete
query set at eight. For web-capable scopes, the Host separately extracts at
most three explicit HTTP(S) references from the user query, removes fragments,
rejects credentials, and sends the normalized unique references as direct
retrieval seeds. Local-only scope never emits them. Planning failure produces
one generic track and the exact query only. Unknown semantic breadth and temporal intent select the stronger
publication contract: the fallback is always `comprehensive` and
`freshness_required = true`. That fallback deliberately performs no topic or
freshness inference from query text.

For a comprehensive comparison of named subjects, the outline must include a
central shared track whose atomic baselines can be supported by at least two
separately attributable records and synthesized together. Distinct atomic
criteria may reuse a question role; the required establish, challenge, and
compare-or-explain mix is validated once across the complete plan.

Planned retrieval reuses the durable bootstrap packet. If bootstrap retained
web evidence, the planned pass skips the exact query and searches only the
validated supplements. If bootstrap did not retain evidence, the exact query
remains available to the planned pass. Both packets are merged before semantic
source and chunk selection. Seed URLs, when explicitly supplied by another
validated caller, remain sufficient to run discovery even when there are no
supplemental queries. One typed coverage-gap pass may run within its own fixed
caps; it cannot become an open-ended search loop.

The merged candidate catalog is passed to one closed semantic candidate
selection before web fetch. A valid selection may contain only exact candidate
IDs from that catalog. A real step failure or timeout degrades acquisition to a
bounded cross-query fallback set, but that fallback is transport resilience,
not evidence admission. Search rank, publisher names, hostnames, TLDs,
language/script detection, and token overlap do not reorder candidates into a
claim-authority tier. An explicit empty selection stays empty, and an
out-of-catalog ID is never accepted.

After safe fetch, canonicalization, URL validation, and content sanitization,
all web text passes a closed semantic evidence selector. It returns exact chunk
IDs plus obligation-relevance, completion-criterion, and typed source-role
edges. Fallback web text remains audit-only unless this selector admits it.
Partial relevance may retain a useful chunk without manufacturing complete
criterion coverage. PDF extraction may use three trusted ranges; ordinary HTML
uses the initial range plus at most one trusted continuation so hydration and
navigation tails cannot consume the closed chunk catalog. When a selector
packet would exceed 32 KiB, the Host partitions it only by source identity and
UTF-8 byte budget. Every source-local window is semantically considered, and a
closed exact-ID reduction chooses at most four retained excerpts for that
source.

Provider result titles, snippets, ranks, engine names, and dates remain
acquisition metadata. Only safely fetched, structurally bounded, semantically
admitted text becomes report evidence. Sanitization removes element blocks,
markup, control characters, and inline transport targets through structural
parsing and fixed byte limits; it does not classify visible text by
vocabulary. Sources that lack closed semantic-selection provenance remain
auditable but cannot support conclusions. The Rust publication boundary
revalidates exact selection mode, source IDs, chunk IDs, criterion indexes, and
the durable source-role wire shape. Every source record owns:

- a Host-generated stable identity;
- requested and canonical anchors;
- reader-facing title;
- capture time and acquisition provenance;
- bounded immutable text chunks; and
- typed obligation relevance plus separate completion-criterion and source-role
  coverage; and
- a content digest.

Successful siblings survive search or fetch failures. Source persistence is
not conditional on a later report proposal. Eligibility and source roles come
only from the closed semantic selection: exact obligation-relevance edges admit
atomic claims, while separate criterion-scoped coverage edges carry completion
and role information. Host, publisher, title, path, and provider vocabulary
carry no authority.

The Host has no query keyword table, domain classifier, named-entity branch, or
topic-specific retrieval template. Topic meaning belongs to the bounded
semantic outline; transport correctness remains deterministic.

## 2. Deterministic Extractive Report

For every non-empty fetched source catalog, the Host stages a complete report
without model output. It contains:

- the user query as the bounded title or subject;
- a protocol-defined explanation that synthesis did not complete when fallback
  is selected;
- source-grouped excerpts admitted by exact chunk IDs and retained in source
  order;
- exact canonical source links;
- explicit acquisition and evidence limitations; and
- a deduplicated source ledger.

Fetched text is escaped as untrusted data. Instructions, Markdown, or HTML
inside a source cannot alter the report template. The fallback never prints
workflow statuses, packet IDs, model errors, hashes, local journal paths, or
retry instructions.

A source-backed extractive artifact is explicitly `SourceBacked`: it retains
useful evidence but has no admitted direct answer, Findings, claims, or
citations. It must never be promoted to `Qualified` or `Synthesized` merely
because sources were fetched. A run with no safely publishable evidence uses a
separate localized `NoEvidence` boundary artifact.

This fallback is intentionally less polished than a valid synthesized report.
Its purpose is to preserve user value and make model failure non-destructive,
not to claim that excerpts are equivalent to analysis.

## 3. Optional Closed Report Generation

One `deep_research_typed_claim_graph` proposal receives:

- the exact query, current date, and validated report scope;
- the frozen material dimensions;
- bounded titles and at most four exact admitted chunks from each source that
  passed closed semantic evidence selection;
- opaque dimension, source, and chunk IDs, but no publication authority; and
- the closed fact, inference, recommendation, relation, derivation, and gap
  schema.

It receives no tools. Source content is explicitly untrusted evidence data.
The Host compiles the schema for the current packet: dimension, source, and
chunk enums contain only exact IDs from the validated run, and audit-only
sources are not addressable by the proposal. Claim IDs may be referenced only
by `basis_claim_ids`, derivation input IDs, and contradiction endpoints. The
model cannot author a source URL, citation number, artifact path, or new
dimension.

Each attempt has a 90-second active bound. A transient stream or generation
failure may retry once under the same closed evidence and durable workflow
identity. There is no validation-driven rewrite, reviewer, repair wave, or
third attempt. When the second attempt fails, times out, or returns no
publishable proposal, the Host keeps the source-backed artifact that was
already staged.

## 4. Host Admission And Salvage

The Host treats the model response as an untrusted proposal. It applies these
rules without another model call:

1. Decode one closed object with no unknown fields and enforce graph,
   cardinality, character, and control-character bounds.
2. Resolve every dimension, source, chunk, claim, relation, and gap reference
   by exact ID membership. Reject only a malformed item while retaining valid
   siblings.
3. Admit facts only from exact cited chunks whose source provenance reaches the
   claim dimension. Inferences and recommendations must name admitted basis
   claims; a derivation must retain its method and exact admitted inputs.
4. Admit a contradiction only between two admitted claims in the same
   dimension. Preserve both claims and both citation paths.
5. Derive material coverage from the admitted graph. Complete material
   coverage yields `Synthesized`; deeply analyzed resolved dimensions plus a
   material typed gap yield `Qualified`. If every dimension remains bounded,
   exactly one partial conclusion may yield `Qualified` only when it satisfies
   the same two-source analytical chain and retains a typed gap. No admitted
   generated graph retains the staged `SourceBacked` artifact.
6. Apply fixed scope metrics. Focused reports require one cited direct-answer
   claim, at least one admitted claim, and one cited eligible source.
   Comprehensive reports additionally require five findings, six admitted
   claims, two cited sources, and 1,200 substantive characters. Every resolved
   material dimension independently requires two facts across two sources, two
   inferences, three analytical claims, one cross-source synthesis, and 800
   substantive characters.
7. Accept reader-facing labels by closed schema and budget only. Escape labels,
   claim text, titles, and excerpts as inert Markdown or HTML data.
8. Build numeric citations and one deduplicated source ledger from exact
   admitted support. Model prose cannot author ledger identity, citation
   numbering, or artifact paths.
9. Validate a closed narrative plan that contains only natural section
   headings and ordered claim-ID groups. Every finding must appear exactly once
   in authored order; the plan cannot add or rewrite evidence.
10. Classify the Markdown/HTML pair only through matching exact versioned
   artifact markers and persist the closed publication enum in the receipt.

If the complete proposal does not satisfy this closed structural and typed
coverage contract, the Host retains the source-backed artifact staged before
generation. It does not ask the same model to audit or repair itself.

An ineligible source retained for audit cannot support a claim, but its presence
does not poison valid claims from eligible sources. A focused report is not
forced to invent a second finding when one cited direct answer is structurally
sufficient. `Qualified` is not a synonym for report failure: it requires at
least one useful admitted claim and an explicit material gap, and it survives
the publication receipt and restart projection.

Atomic entailment, freshness, quantitative scope, and source-role meaning are
bounded semantic model contracts, not string predicates in deterministic Host
code. Corpus evaluation remains the release authority for semantic citation
precision. Production self-review is not admitted merely to create the
appearance of semantic certainty.

## 5. Deterministic Website

Markdown and HTML consume the same Host report document. The renderer owns:

- localized titles, evidence labels, limitations, and navigation;
- actual cited-source and admitted-Findings counts, excluding source headings;
- a truthful read-time label, including a one-minute or sub-minute report;
- numeric inline citations, densely numbered in first-citation order from only
  the sources actually used, with visible `[n]` labels;
- one matching deduplicated linked source ledger without a second list number;
- semantic H1-to-H2 heading order;
- a sticky left action menu, centered report, and sticky right table of
  contents on desktop, with a stacked narrow-screen layout;
- fixed edit, save-HTML, print, focus, and overflow behavior; and
- a fixed reading design until visual evaluation justifies variation.

No art-director generation is permitted. F01 and F04 already show that content
shape must not drive unvalidated counters or English-only renderer chrome.

## Budgets

The product-validation contract owns release latency thresholds. The active
path adds these architectural ceilings:

| Resource | Active ceiling | Rule |
| --- | ---: | --- |
| Required report-synthesis generations | 0 | A fetched catalog can publish without a successful report proposal. |
| Report proposal attempts | 2 maximum | One initial closed-evidence proposal and at most one transient retry. |
| Semantic outline generations | 1 maximum | Runs concurrently with exact-query bootstrap and may propose bounded supplements; failure selects the exact-only fallback. |
| Semantic audit or repair generations | 0 | Host salvage replaces model self-review. |
| Original-query search | 1 | Starts immediately. |
| Supplemental planned searches | 3 maximum | Plain queries must be distinct, URL-free, and additive to the unchanged exact query. |
| Complete planned query set | 4 maximum | The exact query occupies the first slot. |
| Fetched sources | 8 | Shared deterministic acquisition budget. |
| Bootstrap acquisition stage | 150 seconds | Covers discovery, one source-admission attempt, and actual fetches. |
| Planned retrieval stage | 300 seconds | Reuses bootstrap evidence, executes only needed planned discovery, and includes the bounded typed-coverage pass. |
| Web source-admission active time | 60 seconds | The Flow step is not retried; failure degrades acquisition only while leaving room for bounded fetches. |
| Report-model active time | 90 seconds per attempt | The two-attempt stage and whole Host run remain independently bounded. |

Model admission wait, active generation, search, fetch, first-source
persistence, fallback readiness, and terminal publication are timed separately.

## Failure Semantics

| Failure | Required behavior |
| --- | --- |
| Semantic outline fails or is invalid | Use one generic track and only the unchanged exact query; record the fallback mode. |
| Bootstrap acquisition fails | Preserve the error and let bounded planned retrieval retain or retry the exact query as needed. |
| Every configured search provider fails | Publish an honest no-evidence boundary artifact. |
| Web candidate selection fails or times out | Fetch only the bounded cross-query fallback set, record fallback provenance, and keep its web text audit-only unless closed semantic chunk selection admits it. |
| Fetched-text semantic selection fails or returns invalid IDs | Promote none of that failed selection's web text; preserve valid sibling sources and publish `SourceBacked` when safe evidence remains or `NoEvidence` otherwise. |
| One search or fetch sibling fails | Preserve successful sources and continue. |
| First report attempt fails transiently | Retry once with the same closed evidence and durable identity. |
| Second report attempt fails, times out, or is invalid | Publish the staged `SourceBacked` report. |
| One graph item has an invalid reference or shape | Remove that item and retain valid siblings. |
| No generated claim graph passes publication gates | Publish the staged extractive report. |
| HTML enhancement fails | Publish the same report through a deterministic minimal HTML document. |
| Publication is interrupted | Recover the atomic pair from staged state without repeating external effects. |

## State And Authority

The durable source catalog and Host report document are terminal authorities.
The semantic outline is untrusted structured input until the Host validates and
closes it into a bounded plan. Model prose remains a proposal, not publication
authority.

New runs need only record:

- run identity, exact query, explicit evidence scope, budgets, and start time;
- the validated semantic outline or exact-query fallback mode;
- search and fetch attempts;
- immutable source records and failure siblings;
- at most two report attempts and their bounded terminal reasons;
- admitted typed claims, relations, gaps, and cited source identities; and
- staged and committed Markdown/HTML artifacts.

Historical Inquiry events remain readable through strict graph replay. Their
event vocabulary cannot resume or authorize the retired report pipeline.

## Active Path And Compatibility

Retain:

- bootstrap search and fetch transports;
- URL safety, canonicalization, and bounded source persistence;
- exact source/excerpt restoration;
- atomic artifact-pair publication;
- CLI/TUI result handoff and browser opening, plus Code Web report serving; and
- generic event-journal replay needed for diagnostics.

The new-run control flow no longer uses:

- mandatory batched evidence extraction;
- accepted-evidence gating before raw sources can be reported;
- section generation and section revision;
- editorial, guidance, and presentation-frame generations;
- semantic-audit waves and model repair waves;
- a resumable legacy report transaction; and
- generic recovery prose when fetched source text exists.

The old planner, extraction, section writer, reviewer, repair, and report-resume
executors have no consumers and are removed. Compatibility is data replay, not
executable business logic.

The active seam is the standalone engine publication envelope. CLI, TUI, and
Code Web settlement validate its exact query, status, artifact identity,
artifact content, and typed quality metrics. Code Web exposes artifacts only
through validated run identity and artifact kind. A failed envelope cannot be
reclassified through Inquiry convergence or an accepted-evidence count.

## Next Decision Gates

Before release promotion:

1. run the live corpus for C01, C02, C05, C07, and C08 with persisted failures;
2. preserve the current active-entry F01-F08 outcomes and graph semantics:
   F01/F02/F04/F05/F07/F08 synthesized, F03 qualified, and the F06 typed
   timeout source-backed;
3. measure direct-answer, Findings, citation, source-relevance, and latency
   gates from the terminal artifact rather than workflow completion;
4. compare exact-only bootstrap with bounded semantic supplements across
   unrelated domains, including planner failure and misleading-query cases; and
5. pass real desktop and 390-pixel website inspection for synthesized,
   qualified, source-backed, and no-evidence artifacts.

The production architecture is selected; release readiness remains governed
by the corpus and visual gates above.
