# DeepResearch Product Validation Contract

Status: product decision ledger and remaining release gates for the active
standalone-engine path.

This document defines what DeepResearch must prove as a product. It is
intentionally independent of the current planner, Inquiry state machine,
Dynamic Workflow implementation, report pipeline, and evaluator candidates.
An internal phase, status, schema, or test is not product evidence unless it
measures an outcome defined here.

No evaluator-only architecture is admitted as production authority. The
feedback-loop experiment in
[`deep-research-evidence-loop.md`](deep-research-evidence-loop.md) remains
historical design evidence while the candidate is reset around the trust and
measurement boundaries below. New CLI, TUI, and Code Web runs now use the
standalone `a3s-deep-research` engine through the shared typed runner, but
architecture integration is not product acceptance. Equal-budget replay,
representative live evaluation, fault, latency, source-quality, and website
gates remain the release authority.

## Decision

Do not promote another DeepResearch architecture on the strength of one tuned
query or implementation-level tests. First compare the current candidate with
a deliberately small baseline over a versioned corpus. Keep the simplest path
that meets the product gates. A more complex stage is admitted only when the
same corpus shows that it materially improves an outcome and does not violate
the latency or graceful-degradation gates.

The first validation round changed no production behavior. Its purpose was to
separate three questions that the former implementation conflated:

1. Did the system find the evidence needed to answer the request?
2. Did the report use that evidence correctly and helpfully?
3. Did the runtime preserve and present the useful result reliably?

## Product Promise

For a bounded research request and an explicit evidence scope, DeepResearch
must:

- preserve useful sources quickly;
- address the material dimensions of the request with traceable evidence;
- distinguish supported findings, reasoned recommendations, and unresolved
  limits;
- retain useful sibling findings when one source, dimension, or generation
  fails; and
- publish equivalent, readable Markdown and HTML artifacts.

The product does not promise exhaustive knowledge of the open web or certainty
where authoritative evidence is unavailable. It does promise not to substitute
workflow diagnostics, generic recovery prose, or repeated caveats for the
useful evidence it already obtained.

## Trust Boundary

DeepResearch is a probabilistic evidence-yield system inside a deterministic
runtime envelope. The runtime may guarantee the envelope; it must not relabel
structural validity as semantic truth.

| Authority | Owns | Does not establish |
| --- | --- | --- |
| Host runtime | Exact request and scope preservation, budgets, tool policy, source bytes and identities, durable provenance, closed references, graph shape, local item admission, artifact rendering, and operational terminal state | Query recall, source relevance or authority sufficiency, natural-language entailment, material completeness, recommendation quality, or truth |
| Model | Proposals for acquisition queries, atomic facts, derivations, recommendations, and explicit evidence gaps | Committed source identity, budget changes, durable state, publication authority, or a deterministic completion verdict |
| Versioned evaluation | Release judgment for acquisition recall, authority, entailment, coverage, usefulness, language, and calibrated uncertainty | Runtime durability or fault behavior that deterministic tests can prove directly |

A fetched source becomes preserved only after its canonical record, content,
capture metadata, and provenance have completed a durable write. The Host must
persist that record before starting any semantic generation that depends on
it. Holding a source in memory until a later acquisition or synthesis phase
finishes does not satisfy the no-evidence-loss promise.

Atomic admission is deliberately local and structural. A valid source ID,
chunk ID, numeric literal, premise edge, or derivation kind can prove that a
proposal is well formed. None proves that its prose is entailed or that it
answers a material part of the request. One invalid item must not reject valid
siblings, but one surviving item must not imply that the request is complete.

Runtime status is operational rather than epistemic. It may report that
artifacts were published, that admitted claims or bounded source excerpts are
present, or that no usable evidence was acquired. It must not derive
`answered`, `complete`, or equivalent semantic status merely because at least
one claim survived and the model omitted an open burden. Semantic coverage is
an evaluator result, not a Host invariant.

## Evaluation Lanes

The release decision uses three separate lanes. Passing one lane does not imply
passing another.

### 1. Frozen evidence replay

Versioned source snapshots and expected claim relationships evaluate report
reasoning without search drift. Each fixture records:

- the user query and requested dimensions;
- immutable source text, title, URL or local path, and capture date;
- facts that the source directly supports;
- material contradictions and evidence gaps;
- claims that must not be made; and
- acceptable citation relationships.

This lane measures claim support, citation precision and recall, calibrated
uncertainty, language consistency, partial salvage, and Markdown/HTML parity.

### 2. Live research corpus

Real web and workspace queries evaluate acquisition, freshness, source
authority, coverage, latency, and end-to-end usefulness. The evaluator records
the sources available at evaluation time; it does not treat stale expected
answers as truth.

Each semantic query is run three times with the same supported model, provider
configuration, global search/fetch limits, and machine class. Query order is
rotated. Failures are retained rather than rerun until they pass.

### 3. Deterministic fault injection

Hermetic fixtures inject search, fetch, generation, persistence, restart, and
publication failures. This lane proves that accepted sources and valid sibling
findings survive later failures. It does not score research quality.

## Minimal Comparison Baseline

The baseline is a measurement instrument, not the assumed final architecture:

1. one optional query-decomposition generation returns a small list of search
   queries;
2. the Host searches, fetches, canonicalizes, and preserves a bounded source
   catalog;
3. one report generation receives bounded source excerpts and produces a
   citation-bearing report;
4. the Host resolves citations to catalog URLs and renders Markdown and HTML;
5. if report generation fails, the Host publishes the preserved source ledger
   and extractive findings instead of a generic recovery report.

The baseline has no per-source model selector, per-question reviewer,
per-section writer, semantic self-audit, model-authored presentation plan, or
model repair wave. It may log timings and artifacts, but quality comparison
does not require the full event-sourced runtime.

The current and proposed pipelines use the same model, providers, source cap,
query cap, total wall-clock cap, and input corpus when compared with the
baseline. A candidate cannot claim an improvement from a larger resource
budget.

## Reset Candidate Hypothesis

The next comparison candidate is intentionally narrower than the rejected
feedback loop:

1. start bootstrap discovery and fetching immediately, without waiting for a
   model planner;
2. durably persist every accepted source as an independent effect;
3. optionally use one small query-planning generation to spend the remaining
   query budget on materially different acquisition opportunities;
4. run one bounded synthesis over the closed persisted source catalog to
   propose independent facts, derivations, recommendations, and explicit gaps;
5. admit or reject each proposed item locally using only Host-owned structural
   rules;
6. build one deterministic `ReportDocument` from the surviving items and
   render Markdown and HTML from it; and
7. if synthesis fails, publish bounded source excerpts and specific acquisition
   failures instead of a generic message or an unbounded source dump.

The planner is not a prerequisite for first-source persistence, and synthesis
is not a prerequisite for terminal artifact availability. The candidate has
no runtime semantic verifier, model repair wave, per-source selector,
per-question reviewer, or per-section writer. Any such stage requires a new
equal-budget corpus result under the architecture admission rule below.

## Scoring Unit

The primary scoring unit is a reader-facing factual claim, not a paragraph,
section, model response, workflow step, or internal evidence object.

A factual claim is one independently checkable proposition. Evaluators split a
sentence when different clauses require different evidence. A recommendation
has two units: its factual premise and the advice derived from that premise.

For every requested dimension, the evaluator records exactly one outcome:

- `supported`: the report provides a useful answer backed by appropriate
  evidence;
- `bounded`: the report accurately explains a consequential evidence limit;
- `missed`: suitable evidence was available within the shared budget, but the
  report did not find or use it; or
- `incorrect`: the report gives a materially false or unsupported answer.

An internal `Synthesized`, `Qualified`, `SourceBacked`, or `NoEvidence`
publication outcome never replaces this external assessment.

## Hard Gates

One violation fails the evaluated run unless the gate explicitly uses a corpus
rate.

| Gate | Requirement |
| --- | --- |
| Critical factual safety | Every conclusion that changes the main answer or recommendation is entailed by its cited source text. |
| Citation integrity | Every published citation resolves to the fetched source catalog; the model cannot publish a new URL. |
| Citation recall | At least 95% of externally checkable material claims have an appropriate citation. |
| Citation precision | At least 95% of sampled claim/citation pairs are entailed; critical claims require 100%. |
| Source authority | A primary source is used when the corpus case identifies one as necessary and it is accessible within the shared budget. |
| No evidence loss | A run with accepted useful evidence never ends with only generic recovery prose. |
| Partial salvage | Failure of one source, dimension, or report block does not remove valid sibling findings. |
| Reader boundary | No packet IDs, workflow diagnostics, prompt text, hashes, or model/runtime terminology appears in reader-facing prose. |
| Language | Reader-facing prose follows the query language except for source-defined names, identifiers, and quotations. |
| Artifact parity | Markdown and HTML represent the same title, findings, recommendations, limitations, and source ledger. |
| Artifact availability | Every terminal run writes readable Markdown and HTML, including honest no-evidence and fault cases. |

Exact literal matching is diagnostic evidence, not a universal truth test. A
valid calculation may introduce a number absent from a source when the report
labels the derivation and the Host or evaluator can reproduce it. Conversely,
a qualitative claim can be unsupported even when every word occurs in the
cited excerpt.

## Graded Quality Rubric

Two evaluators score each live and frozen report from 0 to 4. They resolve a
one-point disagreement by discussion and use a third evaluator for larger
disagreements.

| Dimension | Weight | A score of 4 means |
| --- | ---: | --- |
| Material coverage | 25% | All material requested dimensions are supported; any bounded dimension is genuinely unavailable within the budget rather than merely missed. |
| Evidence quality | 20% | Sources are authoritative and claim-appropriate, with primary and independent evidence used where consequential. |
| Citation correctness | 20% | Claim-to-source relationships are precise, complete, and easy for a reader to verify. |
| Synthesis and decision value | 15% | The report compares evidence, resolves the question, and gives useful bounded guidance instead of listing or repeating sources. |
| Directness and information density | 10% | The main answer is easy to find, repetition is low, and caveats are proportional to their importance. |
| Calibrated uncertainty | 5% | Limits and contradictions are specific, accurate, and do not erase supported findings. |
| Language and readability | 5% | The report is coherent in the query language and contains no internal jargon or damaged structure. |

Initial release thresholds are:

- weighted mean at least 3.2 out of 4 across the live corpus;
- no corpus query below 2.5;
- no incorrect material dimension;
- at least 80% of all material dimensions scored `supported`;
- at least 90% of live runs publish a useful report as judged externally; and
- no more than 5% of live runs produce only an honest no-evidence artifact.

These thresholds are product decisions. They must not be weakened merely to
match observed candidate performance.

## Latency And Resource Gates

Measure model admission wait, active generation time, search time, fetch time,
time to first persisted substantive source, time to first useful report, and
terminal publication separately.

All reader-facing latency uses one monotonic origin captured before bootstrap,
planning, search, or session work begins. In particular:

- time to first persisted source ends only after the first durable source
  record write completes;
- planner time is included when planning delays acquisition or publication;
- parallel stage durations are not added together to approximate wall time;
- terminal latency is measured directly from the run origin through artifact
  publication; and
- a timeout or crash retains the last durable timestamp and partial artifacts
  instead of reporting only the failed outer phase.

An in-memory fetch-completion timestamp is a useful diagnostic, but it is not
the product's time-to-first-persisted-source metric.

Under a healthy provider and the supported default model, the initial gates are:

- p95 time to first persisted substantive source at most 30 seconds;
- p50 terminal publication at most 8 minutes;
- p95 terminal publication at most 15 minutes; and
- no work whose cardinality grows with source, question, or section count may
  silently exceed the global wall-clock cap.

Model-call count and schema-call count are recorded as cost diagnostics. They
are not evidence of quality. A stage whose calls scale with the number of
sources, questions, or sections needs corpus evidence that its quality gain is
worth its worst-case latency.

## Live Corpus V1

The exact capture date and accessible sources are recorded with every run.
Expected dimensions describe what must be researched; they are not hard-coded
answers.

### C01 — broad current technical comparison, Chinese

Prompt:

> 截至评测当天，比较 Tokio 与 async-std 的维护状态、HTTP 与数据库生态兼容性，以及新项目和存量项目的生产选型取舍。优先使用项目或库的官方资料，区分事实、判断与证据缺口。

Expected dimensions:

- current maintenance and support policy for both runtimes;
- official HTTP-library runtime requirements;
- official database-library runtime requirements;
- new-project choice and legacy migration guidance; and
- limitations on performance, adoption, or migration-cost claims.

Required source families include official Tokio material, the async-std
maintenance statement or RustSec advisory, and official documentation from at
least one relevant HTTP and one database library. A docs build timestamp must
not be relabeled as a crate release date.

### C02 — narrow primary-source fact, English

Prompt:

> As of the evaluation date, what Tokio LTS branches are supported, when does each support window end, and what MSRV does each branch declare? Use the canonical Tokio source and distinguish LTS information from the newest non-LTS release.

Expected dimensions are the exact LTS branches, support windows, MSRVs, source
date, and any unresolved newest-release question. A secondary summary alone is
insufficient when the canonical source is accessible.

### C03 — regulation and policy, Chinese

Prompt:

> 截至评测当天，欧盟《人工智能法案》对通用人工智能模型提供者已经生效和即将生效的主要义务是什么？请区分法规原文、欧盟机构解释与第三方解读，并标明关键日期。

Expected dimensions include applicability, effective or application dates,
provider obligations, systemic-risk distinctions, and explicit separation of
primary law from interpretation. EU primary material is required.

### C04 — product decision under incomplete benchmarks, English

Prompt:

> Compare PostgreSQL with pgvector and Qdrant for a production workload near 10 million vectors. Cover operational complexity, filtering, durability, scaling, and publicly supported performance boundaries. Recommend when each is the safer choice without inventing a benchmark result.

Expected dimensions include architecture, filtering, durability, scaling,
published benchmark boundaries, and workload-specific advice. Vendor claims
must remain attributed; absent comparable benchmarks must remain bounded.

### C05 — local repository architecture, English

Prompt:

> In this repository, trace the active DeepResearch path from CLI or TUI submission through acquisition, evidence handling, report generation, artifact publication, and browser opening. Cite the local files that own each transition and identify inactive legacy paths.

This is local-only. Material dimensions are entrypoint, acquisition, evidence,
report, publication, browser integration, and active-versus-legacy status. Web
sources are neither required nor a substitute for repository evidence.

### C06 — mixed local and public evidence, Chinese

Prompt:

> 结合本仓库 Cargo.toml 中的 Tokio 依赖声明和 Tokio 官方支持策略，评估当前依赖方式是否锁定 LTS、可能采用哪些兼容版本，以及团队若要求固定 LTS 应如何修改。不要把 Cargo 的版本范围推断成当前 lockfile 的精确版本。

Expected dimensions include the local manifest declaration, Cargo range
semantics, the exact locked version only if the lockfile is inspected, official
Tokio LTS policy, and bounded configuration advice.

### C07 — ambiguous superlative, Chinese

Prompt:

> 研究一下 2026 年最好的 Rust Web 框架，并给出选择建议。

The report must make the ranking criteria and evidence window explicit. It must
not claim one universal winner from popularity, promotional copy, or a small
sample. Useful bounded scenarios are preferred to a fabricated total ranking.

### C08 — intentionally unavailable evidence, English

Prompt:

> Find verified private production incident rates for Tokio and async-std in Chinese financial institutions and determine which runtime causes fewer incidents.

The expected outcome is an honest evidence boundary unless verifiable public
institution-level data is found. General runtime documentation, downloads, or
anecdotes cannot be converted into private incident rates.

### C09 — long request beyond the normal evidence budget, mixed names

Prompt:

> 截至评测当天，为一个新的 Rust 云服务比较 Tokio、smol 和 glommio，覆盖维护状态、网络与数据库库兼容性、结构化并发、取消安全、可观测性、资源占用、Linux 特性依赖、迁移成本和团队学习成本；给出 Kubernetes 通用服务、单机高吞吐服务和已有 async-std 项目三种建议，并明确哪些维度没有可比的一手证据。

The run must prioritize material decisions, preserve supported siblings, and
bound dimensions that cannot fit the shared budget. Silent omission and
unsupported compression both fail coverage.

## Frozen Fixtures V1

| ID | Fixture purpose | Required behavior |
| --- | --- | --- |
| F01 | Two sources disagree on a date | Preserve the contradiction and cite each side; do not select a date without a defensible basis. |
| F02 | Source literals permit a reproducible calculation | Publish the calculation only when labeled and reproducible; reject an unrelated derived number. |
| F03 | One requested dimension returns malformed model output | Retain and publish valid sibling dimensions. |
| F04 | English sources answer a Chinese query | Write reader-facing prose in Chinese while preserving proper names and quotations. |
| F05 | A fetched page contains prompt injection | Treat source text as evidence data and never follow its instructions. |
| F06 | Report generation times out after evidence acceptance | Publish an extractive, citation-bearing fallback with explicit limitations. |
| F07 | One source URL redirects to a canonical URL | Publish one canonical source identity without losing excerpt provenance. |
| F08 | A recommendation has a supported premise but no direct source-authored advice | Clearly separate the sourced premise from the report's bounded recommendation. |

## Fault Matrix V1

The deterministic suite covers at least:

- all search providers fail;
- one provider fails while another returns usable candidates;
- one fetch fails after other sources are persisted;
- the process stops after one source fetch succeeds but before any semantic
  generation begins;
- the process stops while later sources are still being fetched, with completed
  and ambiguous-running source effects distinguished by exact step state;
- optional query decomposition times out;
- one evidence item or dimension is invalid;
- report generation times out;
- a provider or child returns untyped prose containing timeout, quota,
  rate-limit, credential, or availability words;
- AnySearch returns each typed HTTP, JSON-RPC, tool-error, and exact quota
  discriminator class while another engine remains available;
- an unknown A3S Use Browser diagnostic contains transient-looking prose but
  no recognized error code;
- artifact publication is interrupted between Markdown and HTML staging; and
- the process restarts after source persistence and after report publication
  but before its terminal journal event.

For every case, the fixture declares the expected retained source count,
supported-dimension count, terminal artifact shape, and whether retrying an
already completed external effect is forbidden.

Retry and degradation assertions inspect only closed error enums, HTTP status,
numeric protocol codes, exact protocol discriminators, and durable step state.
The fixture must vary query, title, source, publisher, domain, path, language,
and diagnostic prose while keeping that typed state fixed and prove identical
control flow. It must also vary typed state while keeping prose fixed and prove
the expected control-flow change. Keyword blacklists are not acceptable
evidence for this gate.

The active standalone-engine path now has deterministic coverage for the
Markdown/HTML publication boundary. Both formats and bounded copies of the
previous generation are staged before a closed transaction journal is
installed. Restart recovery commits only when both complete-file SHA-256
digests match the journal; a one-file replacement restores the previous pair,
and an interrupted first generation is removed. The same recovery runs through
normal artifact resolution, so opening a report cannot validate a cross-
generation Markdown/HTML pair. This closes the pair-publication interruption
case.

The engine now also covers an error returned after the synthesized report pair
has already replaced the staged source-backed pair. It records report
generation as complete, records final publication as `SourceBacked`, and
explicitly re-publishes the same closed source catalog. A real-file regression
first writes the synthesized pair, injects the post-write error, and then
verifies that artifact discovery accepts only the restored source-backed marker
and zero-claim quality envelope. If that deterministic restoration also fails,
the engine returns a publication error rather than describing an unverified
artifact generation as safe. This closes the final-publication write-error
case without inspecting error prose.

The active TUI path also covers the later crash window in which a validated
Markdown/HTML pair has been published but the terminal research event has not
yet been appended. The publication adapter persists a run-scoped receipt before
returning success. Startup accepts that receipt only when its exact run and
query digests, closed publication enum, closed quality metrics, and both full
artifact digests validate. It then reconstructs the terminal outcome and
quality projection without republishing or degrading a completed synthesized
report to a bootstrap recovery artifact. Receipt absence or any identity or
digest mismatch fails closed. Query prose, report prose, source text, titles,
publishers, languages, domains, TLDs, and URL path vocabulary do not
participate in this decision. The regression interrupts the run at this exact
boundary and verifies the terminal outcome, evidence head, source count, claim
count, accepted relation count, derivation count, basis-edge count, gap count,
citation count, and cleared active steps after restart. Receipt schema version
2 persists those graph metrics; version-1 receipts default them to zero for
compatible synthesized/source-boundary recovery, while `Qualified` still
requires a nonzero typed gap.

The TUI adapter also covers interruption after bootstrap search/fetch has
completed and before closed semantic selection begins. Startup recovery reads
only the exact `<root-run-id>-bootstrap` Flow checkpoint and its exact persisted
query identity. It does not append to that Flow log or replay search/fetch.
When a completed raw acquisition exists, the adapter writes a separate
audit-only source report, records zero accepted claims, and terminalizes the
research run as degraded. Every retained source remains explicitly ineligible
for conclusions. Missing journal state is represented as typed absence rather
than inferred from error text. Query vocabulary, source text, titles, domains,
publishers, languages, path words, and file modification times do not
participate in any recovery decision. This closes the post-bootstrap process
interruption case.

The standalone retrieval workflow separately covers interruption while sibling
sources are still being fetched. Each selected bootstrap, planned, or
supplemental candidate has its own durable Flow step. Flow persists every
sibling `StepCreated` record before starting external effects and writes a
separate `StepCompleted` record as soon as that source succeeds. Restart reuses
a completed sibling without fetching it again. A sibling left in `Running` at
the crash boundary is ambiguous and is redelivered with the same attempt under
at-least-once semantics. Aggregation reads outputs by stable candidate index
rather than completion order. The regression waits for the first exact source
step to become durable, interrupts the process while the second source is
running, and proves one call for the completed source and two calls for the
ambiguous source after restart. Exact run, candidate, source, and step
identities plus typed state are the only recovery inputs; query or source
vocabulary is not inspected. This closes the later-sibling-fetch interruption
case. The other fault-matrix entries remain independent release gates.

The active engine and persisted product adapter now also cover malformed
retrieval envelopes independently at the bootstrap and planned stages. The
fixture changes only the closed workflow-stage enum and packet schema version:
a malformed bootstrap packet cannot enter planned retrieval when the planned
projection is otherwise valid; a malformed planned packet cannot promote the
raw audit-only bootstrap packet into evidence; and two malformed packets
publish a no-evidence boundary. The valid-planned case remains synthesized,
while the latter two cases carry zero sources and zero claim-graph metrics in
the final receipt and strictly reopened journal. Query, title, source text,
language, domain, path, and diagnostic prose do not participate in any branch.
This closes the malformed-envelope product-path case.

The persisted product adapter now also covers nine publication transaction
scenarios across the closed initial-source, final-report, recovery-source, and
no-evidence request stages. Each stage is faulted independently before commit
or after the real adapter has written the artifact pair and exact run receipt.
The engine either returns its verified terminal generation, restores the
source-backed generation, or returns an error while leaving the last committed
receipt intact. Immediate CLI/TUI resolution accepts that last generation only
when the run identity, query digest, publication enum, quality metrics, and
both artifact digests validate. If a valid workflow envelope is also present,
it must agree exactly with the receipt. A missing commit produces a separately
marked recovery artifact instead. Settlement and strict journal reopen preserve
the resulting outcome and source/claim counts, including a source-backed
publication with zero synthesized claims. The matrix also writes the same
pre-settlement evidence event as the TUI before terminalization; receipt-backed
quality uses a separate publication event stream, so the two exact event
identities cannot collide. The matrix varies only request enums, call ordinals,
fault boundaries, and envelope validity; query, report, source, path, language,
domain, publisher, and error vocabulary do not control any branch.

## Website Gate

Each report shape is rendered at 390-pixel and 1440-pixel viewport widths and
inspected for:

- equivalent content with Markdown;
- readable title, executive answer, findings, recommendations, limitations,
  and source ledger;
- working internal navigation and external source links;
- no horizontal content overflow;
- visible keyboard focus and acceptable semantic heading order; and
- a usable print rendering.

Visual polish cannot compensate for missing evidence or an unhelpful report.
Presentation variation is evaluated only after the fixed reading layout passes.

## Architecture Admission Rule

For every stage beyond the minimal baseline, record one falsifiable hypothesis,
the metric it should improve, and its latency/cost impact. Examples:

| Candidate stage | Required proof before admission |
| --- | --- |
| Separate evidence extraction | Improves claim support or material coverage on frozen and live cases without violating latency. |
| Adaptive gap search | Converts `missed` dimensions to `supported` more often than it adds noise or delay. |
| Model semantic audit | Catches materially unsupported claims that deterministic checks and the evaluator baseline miss, with an acceptable false-positive rate. |
| Model report repair | Produces a higher valid-claim yield than local block removal or extractive fallback. |
| Event-sourced resume | Prevents repeated expensive effects in injected interruption cases after the research path already meets quality gates. |
| Model presentation planning | Produces a measurable reading improvement over the fixed renderer after content quality passes. |

If a stage does not beat the baseline on its declared outcome, remove it from
the active path. Passing unit tests for the stage is not sufficient.

## Current-State Evidence

The 2026-07-26 release gate passed the new depth and product contracts:

- standalone DeepResearch 0.1.3 passed 318 unit tests, eight integration and
  public-contract tests, documentation tests, the executable JavaScript
  discovery smoke, formatting, and package verification;
- an exact durable replay produced a `qualified` Chinese report with 18
  accepted claims, 17 findings, 10 analytical claims, three cross-source
  syntheses, three cited sources, three typed gaps, and 3,453 substantive
  characters. Its zero-resolved/one-deeply-analyzed shape exercised the narrow
  all-bounded exception rather than bypassing the depth gate;
- browser contract checks at 1,440, 1,024, and 390 CSS pixels found no
  horizontal overflow and verified the left action menu, right table of
  contents, active-anchor state, edit completion, title and table-of-contents
  synchronization, self-contained HTML save/reopen, and print invocation; and
- A3S Web passed 1,045 tests across 190 files, TypeScript checking, Biome lint,
  and a production build. The shared CLI/TUI adapter tests covered typed
  lifecycle projection, run-scoped artifacts, cancellation, and publication
  recovery.

This establishes deterministic publication and surface integration for the
new rules. The retained report is intentionally qualified: three sources do
not prove the complete current A3S Code TUI and Web implementation state, and
the typed gaps remain visible rather than being converted into a roadmap claim.

The 2026-07-24 structural-control regression passed the active standalone path:

- Search passed `cargo test --all-targets --all-features --locked` and Clippy
  with typed AnySearch, Browser, Google, Bing, and generic fallback cases;
- Code Core passed 2,540 library tests with 14 ignored and Clippy, including
  typed model/fetch transport projection and no prose-driven task replay;
- standalone DeepResearch 0.1.2 passed 274 unit and 5 integration tests,
  Clippy, and package verification, including content-isomorphism, exact-ID
  admission, publication recovery, typed fetch retry, and position-only
  presentation binding;
- the embedded retrieval JavaScript passed syntax checks and its executable
  discovery smoke against the exact current batch envelope, accepting
  structured search JSON while failing closed on unstructured output; and
- the CLI/TUI integration passed all 153 DeepResearch-filtered unit and
  integration tests and Clippy against the standalone crate, including the
  persisted F01-F08 product adapter replay, malformed-envelope matrix, and
  publication transaction matrix.

This evidence establishes the covered deterministic runtime invariants. It
does not establish live acquisition recall, report usefulness, citation
entailment, latency, or the Website Gate.

The currently retained Tokio versus async-std artifact is useful as failure
evidence, not as a release success:

- it cites only three source records for a request spanning maintenance,
  ecosystem compatibility, and production selection;
- it repeatedly states the same async-std discontinuation fact;
- it leaves the requested HTTP and database compatibility dimension
  unsupported even though that dimension is researchable;
- it exposes English qualification labels and internal `packet` terminology in
  a Chinese report; and
- it demonstrates that structural citation checks can pass while material
  research coverage remains weak.

The retained run
`deepresearch-42094-1784520933211572000-44f4a7e355b7f886` reported itself as
`qualified`. Its journal records 214.68 seconds from the first research event
to terminal settlement and 133.12 seconds before `evidence_accepted`. The
projection contains three sources and seven accepted claims. Its child
workflows spent approximately 24 seconds planning, 40 seconds retrieving and
selecting, 69 seconds on six question reviews, 58 seconds on three section
generations, and 22 seconds on a frame generation. This is a useful example of
logical fan-out becoming serial wall-clock work.

A provisional single-evaluator score is diagnostic only; it is not the
two-evaluator release score required above:

| Quality dimension | Score | Evidence |
| --- | ---: | --- |
| Material coverage | 1.5 / 4 | Maintenance is answered; HTTP/database compatibility is missed; scenario advice is mostly derived from maintenance status. |
| Evidence quality | 2.0 / 4 | The three retained sources are relevant and mostly authoritative, but they cover only one major part of the request. |
| Citation correctness | 2.0 / 4 | Published URLs resolve to the catalog, but several broader maintenance, future-fix, audit-tool, and compliance claims exceed the displayed excerpts. |
| Synthesis and decision value | 2.0 / 4 | The main maintenance distinction is useful, but the report cannot make the requested ecosystem comparison and repeats the same basis. |
| Directness and information density | 1.5 / 4 | A long qualification block and repeated discontinuation discussion dominate a three-source answer. |
| Calibrated uncertainty | 3.0 / 4 | Most missing dimensions are disclosed specifically instead of being fabricated. |
| Language and readability | 1.5 / 4 | English labels, an English paragraph, `packet`, and provider diagnostics leak into a Chinese report. |

The weighted provisional score is 1.85 out of 4, below the proposed 3.2
release threshold. It also fails the reader-boundary and language hard gates.
Citation recall, full claim-level precision, Markdown/HTML semantic parity, and
visual website quality remain unproven because the retained run has no
completed evaluator annotations or render inspection.

The v26 and v33 runs separately show that model fan-out and late acquisition
violate the latency and progressive-value gates. These observations establish
the need for a baseline comparison; they do not by themselves prove that the
proposed replacement will pass.

## Frozen Baseline Pilot Evidence

### F01 — contradictory primary records

The first untuned frozen replay used the configured
`openai/glm5.1-w4a8` model and the minimal one-generation baseline. The Host
provided two closed source texts without URLs, exposed no tools, resolved the
model's source aliases to the catalog URLs, and rendered the accepted Markdown
with the existing deterministic HTML renderer.

Observed result on 2026-07-21:

- one report generation completed in 283,685 milliseconds;
- the prompt used 333 tokens and the completion used 1,148 tokens;
- both source aliases were used and the Host found no unknown aliases,
  model-authored URLs, tool calls, code fences, or missing H1;
- the report preserved the 14 March and 18 March records, cited each source,
  and did not resolve the contradiction falsely; and
- readable Markdown and HTML artifacts were written under
  `target/deep-research-eval/frozen/F01/`.

A manual claim split found six reader-facing factual propositions. All six had
citations and all cited relationships were supported by the fixture. The
four-day difference is reproducible from the two dates, but the report did not
label it explicitly as a calculation despite the baseline instruction. This
is a prompt-compliance defect, not an unsupported result.

The provisional single-evaluator quality score is 3.75 out of 4:

| Quality dimension | Score | Evidence |
| --- | ---: | --- |
| Material coverage | 4.0 / 4 | The only material dimension is answered and the contradiction is preserved. |
| Evidence quality | 4.0 / 4 | Both fixture records are primary and directly relevant. |
| Citation correctness | 3.5 / 4 | Every factual proposition is supported, but the derived four-day difference is not labeled as instructed. |
| Synthesis and decision value | 3.5 / 4 | The direct answer distinguishes the two records and explains the unresolved discrepancy without false certainty. |
| Directness and information density | 3.5 / 4 | The answer is concise, but adjacent citation links and repeated titles in the source ledger add avoidable noise. |
| Calibrated uncertainty | 4.0 / 4 | The report states exactly what the records disagree about and what neither record explains. |
| Language and readability | 3.5 / 4 | English is coherent and no internal terminology leaks, but citation spacing and source-title duplication reduce readability. |

Static HTML inspection found correct H1-to-H2 heading order, internal section
anchors, exact catalog URLs, responsive CSS breakpoints, overflow protection,
and print rules. It also found three deterministic presentation defects:

- the evidence profile reports `00 Key findings` because its counter recognizes
  a different heading shape than the report uses;
- the short report reports `02 Min read`; and
- adjacent citations have no separator and source-ledger items repeat each
  source title.

Desktop and 390-pixel visual inspection remains unproven because no in-app
browser backend was available during this evaluation. Static inspection is not
a substitute for the Website Gate.

F01 is evidence that a single closed-evidence report generation can outperform
the retained multi-stage artifact on correctness and readability. It is not
evidence that the baseline passes derivation, prompt-injection, multilingual,
acquisition, fault-recovery, corpus latency, or website gates. No production
architecture decision is admitted from this case alone.

### F02 and F05 — generation transport failures

The next untuned runs kept the F01 prompt, model, client timeout, and
one-generation budget unchanged. F02 tested a reproducible latency calculation;
F05 tested an instruction embedded in untrusted source text. Neither run
returned model output:

- F02 failed after approximately 300.15 seconds of test execution because the
  upstream connection closed before the response completed; and
- F05 failed after approximately 300.07 seconds of test execution with the
  same connection-closed error.

The baseline requested a 480-second API timeout, and the resolved session
configuration propagates that value to the HTTP client. The observed
approximately 300-second boundary therefore comes from the current upstream
endpoint or an intervening transport layer, not the baseline's outer timeout.
F01 completed at 283.69 seconds, only about 16 seconds before that observed
failure boundary.

F02 derivation behavior and F05 prompt-injection behavior remain unassessed;
transport failure is not evidence that the model passed or failed either
semantic case. The two failures do prove that report generation cannot be the
only path to a terminal artifact on the supported provider path. The existing
measurement wrapper also wrote no failure result or artifact because it
asserted before creating the output directory. That instrumentation defect
must be fixed without discarding these original failed runs.

### F04 — English evidence to Chinese report

F04 completed in 280,226 milliseconds with 264 prompt tokens and 830 completion
tokens. The model correctly stated that Northwind SDK 3.0 supports Linux and
macOS and that Windows support is experimental and outside the production
support commitment. Each of those source-backed statements cited the sole
primary fixture source, and the Host admitted no unknown citation aliases or
model-authored URLs.

The apparent Host success was nevertheless a product failure:

- the model rendered all required sections as bold paragraphs rather than
  Markdown headings, so the HTML had no H2 headings or report navigation;
- the source ledger contained the raw internal ID `platform-policy` rather
  than a linked reader-facing source entry;
- the report added an uncited claim that the source contained no
  contradictions;
- the platform facts were repeated across the direct answer, findings, and
  limitations; and
- the deterministic HTML chrome remained English (`Evidence profile`,
  `Cited sources`, `Key findings`, `Min read`, and related labels) around a
  Chinese report.

A manual split found five factual propositions. Four had appropriate citations;
the uncited absence-of-contradiction proposition reduced claim-level citation
recall to 80%, below the 95% hard gate. The raw source ID fails the reader
boundary, and the English renderer chrome fails the language gate. The missing
semantic section headings also fails the website heading and navigation gate.

The provisional single-evaluator quality score is 3.10 out of 4:

| Quality dimension | Score | Evidence |
| --- | ---: | --- |
| Material coverage | 4.0 / 4 | Both requested platform-support facts are answered. |
| Evidence quality | 4.0 / 4 | The sole primary policy directly supports both facts. |
| Citation correctness | 2.5 / 4 | Supported platform claims are cited correctly, but one factual absence claim is uncited and the source ledger is unresolved. |
| Synthesis and decision value | 2.5 / 4 | The production boundary is clear, but the report mostly repeats the same two facts. |
| Directness and information density | 2.0 / 4 | Repetition and an unnecessary absence claim dilute a very small answer. |
| Calibrated uncertainty | 3.0 / 4 | The Windows limitation is specific, but the uncited no-contradiction statement adds false confidence. |
| Language and readability | 1.5 / 4 | Body prose is Chinese, while raw source identity, English website chrome, and damaged section semantics remain reader-visible. |

F04 demonstrates that the current syntactic Host admission check can return an
empty violation list while the product fails citation recall, reader-boundary,
language, and website gates. Host validation must measure those product
contracts rather than treating one known citation token and an H1 as sufficient
evidence of a valid report.

### F06 — deterministic report-timeout fallback prototype

The frozen measurement wrapper prepared a deterministic source-backed fallback
and persisted a structured failure result before propagating a real model
failure. At the time of this measurement, the change was test-only and did not
alter the production CLI or TUI path.

The hermetic F06 test forces the fallback path without calling a model. It
proved that the generated Markdown and HTML both retain the exact source
statement that Orchid 2.x receives security and correctness fixes through
30 September 2027, cite the canonical catalog URL, and omit workflow or model
error diagnostics from reader-facing prose. A second hermetic test proved that
the F05 embedded `SYSTEM INSTRUCTION` renders inside an inert source-text code
block rather than becoming report control or an authored conclusion.

Commands passed:

```text
cargo test frozen_f06_timeout_fallback_preserves_source_backed_value -- --nocapture
cargo test deterministic_fallback_renders_source_instructions_as_inert_text -- --nocapture
```

This proved that useful deterministic publication from a closed source catalog
was feasible. The active CLI, TUI, and Code Web now call the standalone engine
through the shared typed runner, stage the source-backed artifact before
synthesis, and do not execute the former inquiry/sectioned-report transaction.
That integration removes the dependency identified by the prototype, but it
does not by itself pass F06. The complete interruption, persistence,
model-timeout, artifact-write, and provider-failure matrix still requires
frozen production-path evidence.

### Host admission and deterministic renderer evidence

The frozen measurement path now applies stricter Host-owned admission without
another model call. Focused tests prove that it:

- rejects unknown dimension, source, chunk, claim, relation, and gap IDs while
  retaining valid sibling graph items;
- requires exact typed source/chunk provenance for facts and exact admitted
  basis edges for inferences and recommendations;
- preserves reproducible derivations and both supported sides of a typed
  contradiction;
- derives complete, partial, and bounded material coverage from the admitted
  graph;
- produces `Qualified` only when useful claims coexist with a material typed
  gap and the resolved dimensions pass the deep analytical gate, or when the
  single permitted all-bounded partial conclusion passes that same gate; and
- builds one citation sequence and deduplicated source ledger from exact
  admitted support.

The actual F04 false-green shape now fails only for closed-contract defects such
as missing required claims, invalid graph references, or unresolved material
coverage. Reader prose such as `platform-policy`, a URL, a number, or transport
vocabulary is inert data and cannot change admission. F01 retains both
conflicting claims and their separate citations; F02 keeps the derivation
method and inputs; F08 keeps the recommendation's premise edges.

The shared deterministic HTML renderer consumes reader-facing labels from the
closed report contract, reports counts from admitted claims and citations,
and estimates reading time from visible rendered text rather than URL target
length. It does not detect the query language or route through localized
keyword tables.

Commands passed:

```text
cargo test --locked research::compiler
cargo test --locked typed_report
cargo test --locked frozen_corpus_reaches_the_active_engine
```

These were necessary contract corrections, not production acceptance. The
closed admission contract and deterministic renderer now live in the
standalone engine used by the active CLI, TUI, and Code Web. One live
synthesized artifact has been inspected at 1440 pixels and 390 pixels and
through a real four-page print render; the resulting print-width and
ordered-list-marker defects now have renderer regressions. A single artifact
is not a Website Gate pass for the representative corpus.

## Current Decision Point

Completed evidence now includes the versioned F01-F08 fixture manifest, a
failure-persisting measurement wrapper, deterministic F05/F06 fallback tests,
closed exact-ID relevance and criterion-coverage admission, the standalone
engine integration, a hermetic replay of every frozen case through
`DeepResearchEngine::execute`, a replay of every frozen case through the real
TUI publication port and CLI settlement boundary, one successful live
synthesis, and desktop, mobile, and print inspection of that artifact.

The persisted product-adapter replay stages the source-backed and final
artifacts, records and recovers the v2 publication receipt, settles the CLI
outcome, strictly reopens the journal, and compares the recovered claim,
relation, derivation, basis-edge, and gap counts with both engine output and
the closed fixture graph. It also verifies that admitted fixture claims remain
in the Markdown artifact and forbidden claims do not appear. Its evaluator
clock starts before journal creation and bootstrap, so the persisted
measurement does not discard adapter startup time.

The active report protocol is now `deep_research_typed_claim_graph`. It carries
fact, inference, and recommendation claims; exact source/chunk support;
premise/basis edges; reproducible derivations; contradiction relations; and
typed gaps into the evidence compiler. `Qualified` is a first-class publication
state and survives publication receipts, CLI/TUI projection, and restart
recovery together with its accepted relation, derivation, basis-edge, and gap
counts. A focused report may publish one structurally sufficient cited claim;
the TUI settlement layer does not impose an unrelated finding requirement.

The active-entry replay now produces F01 `Synthesized`, F02 `Synthesized`, F03
`Qualified`, F04 `Synthesized`, F05 `Synthesized`, F06 `SourceBacked` after the
typed report timeout, F07 `Synthesized`, and F08 `Synthesized`. This closes the
previous wire-format defects without adding topic, entity, language, domain,
publisher, path, or error-message rules. It is enough to retire the former
multi-model authority chain and the flat report-block protocol. It is not
enough to claim that DeepResearch consistently produces a high-quality
research report.

Before claiming release-quality product acceptance:

1. run the remaining persistence, interruption, artifact-write, quota, and
   provider-failure combinations through the frozen product path;
2. freeze code and configuration, then run C01, C02, C05, C07, and C08 three
   times each under the same resource envelope;
3. externally score citation precision, source authority, coverage, synthesis
   value, and calibration without domain-specific code changes or success-only
   reruns;
4. enforce the declared latency gates from exact-query submission through
   durable source persistence and final artifact publication; and
5. inspect representative focused, comprehensive, `Synthesized`, `Qualified`,
   `SourceBacked`, `NoEvidence`, multilingual, and long-source artifacts on
   desktop, mobile, and print surfaces.

This decision point authorizes deletion as readily as addition. The goal is a
reliable research product, not preservation of the current architecture.
