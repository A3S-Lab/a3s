# DeepResearch Evidence Compiler Contract

Status: the typed claim ledger, coverage projection, `ReportDocument`, and
dual renderer are active in the standalone engine's production report path.
The earlier proposal to replace the complete retrieval runtime with a
compiler-owned planner remains rejected after the live planner trials recorded
below. The persisted-evidence and feedback-loop experiments in
[`deep-research-evidence-loop.md`](deep-research-evidence-loop.md) remain
historical evaluator candidates, not production paths.

Product acceptance remains governed by
[`deep-research-product-validation.md`](deep-research-product-validation.md).
The earlier
[`deep-research-evidence-first-redesign.md`](deep-research-evidence-first-redesign.md)
correctly moved terminal publication authority away from model output, but the
live C01 pilot exposed a second, independent failure: research intent is not
preserved from acquisition through publication.

Subsequent live compiler proposals exposed the opposite boundary error. C01
produced useful dimensions but required nine query-target edges under an
eight-source cap, while C08 expressed the correct private-evidence limitation
but referenced a target outside one strict dimension edge. Neither run reached
acquisition. These are contract failures, not evidence that the semantic
research intent was poor.

## Decision

DeepResearch is an evidence compiler, not a multi-agent workflow and not a
free-form report writer.

One immutable dimension contract must connect planning, retrieval, claims,
coverage, and publication. A stage may add sources, claims, or explicit gaps;
it may never rename, replace, merge, or silently drop a dimension after the
contract is frozen.

The normal semantic pipeline is:

```text
request
  -> frozen ResearchSpec
  -> bounded QueryPlan
  -> Host-owned SourceCatalog
  -> flat ClaimLedger proposal
  -> Host structural admission and CoverageMatrix
  -> Host-owned ReportDocument
  -> Markdown and HTML
```

Model calls may propose a research plan and a claim ledger. They do not own
budgets, fetched sources, coverage identities, terminal status, citations,
Markdown, HTML, or publication. A model failure must not erase an already
fetched source or an already admitted sibling claim.

## Two Independent Correctness Axes

DeepResearch has to satisfy both axes. Neither one implies the other.

1. **Semantic continuity:** the final result addresses every material part of
   the request with an admitted claim or an explicit evidence gap.
2. **Publication authority:** useful fetched material remains publishable when
   planning, synthesis, persistence, or rendering fails.

The evidence-first candidate fixed the second axis by publishing a source
catalog without a successful model call. It did not fix the first axis. The
active `ResearchSpec` records no dimensions, bootstrap acquisition uses the
whole request as one provider query, and the report proposal reconstructs an
answer from whatever sources happened to be fetched. A stable source page can
therefore be published while material requested dimensions remain invisible.

The product invariant was not merely violated; it was absent from the state
model and therefore impossible for the Host to enforce.

## Active-Path Root-Cause Audit (2026-07-21)

The new-run CLI, TUI, and Code Web path currently executes this concrete
sequence:

```text
CLI, TUI, or Code Web submission
  -> CodeDeepResearchRunner::start
  -> DeepResearchEngine::execute_request
  -> bootstrap_workflow_args
  -> host_fallback_plan
  -> one search for the complete user request
  -> at most eight fetches
  -> source catalog
  -> report-shaped block proposal
  -> Markdown
  -> HTML parsed from Markdown
```

This is not yet the compiler pipeline described by this contract:

- `state_journal::ResearchSpec` stores the query, date, evidence scope, an
  empty `required_claims` vector, and timing budgets. It has no dimension,
  source-family, source-target, or per-query acquisition identity.
- `execute_evidence_first_research` never calls the semantic planner. It
  always starts `bootstrap_workflow_args`, whose Host fallback searches the
  complete request once.
- The compatibility Inquiry path can generate an outline, but
  `host_plan_from_outline` also replaces its retrieval plan with the complete
  request as one search. Its track identities therefore do not constrain
  acquisition.
- The active report proposal contains only report category, prose, and source
  aliases. It has no dimension identity, claim kind, factual premise graph,
  attempted query, or missing target.
- The flat-ledger experiment is adapted back into the old report-block shape;
  that adapter discards every dimension gap before publication.
- The current renderer derives HTML from Markdown instead of projecting both
  artifacts from one `ReportDocument`.

The architectural root cause is therefore precise: **new-run state has no
durable semantic intermediate representation**. Each stage receives prose and
reconstructs identities that should have been frozen upstream. Search can
return useful pages and synthesis can produce fluent text, while the Host has
no state from which to prove that the requested dimensions survived.

At the time this rejected hypothesis was evaluated, three representations
coexisted:

1. the now-retired event-sourced Inquiry/report pipeline;
2. the active evidence-first, one-query, report-block pipeline; and
3. the test-only evidence-compiler contracts.

Adapters between these shapes preserve transport compatibility while losing
semantic information. Adding another adapter would increase implementation
surface without making the requested result more reliable. The compiler
artifacts would have had to replace the new-run source of truth together. The
retired executors have since been removed; only generic event data remains
inspectable.

The audit also separates symptom fixes from missing invariants:

| Observed symptom | Local change that cannot solve it | Missing invariant |
| --- | --- | --- |
| A requested topic is absent from the report | Rewrite or retry the report prompt | The topic had no frozen dimension identity before retrieval |
| A SQLx page is used for an HTTP question | Raise a global relevance score | Candidate admission was not allocated to one declared source target |
| A migration recommendation exceeds the evidence | Add lexical claim checks | Recommendations have no admitted premise edges |
| Model failure leaves only a fallback | Increase model timeout or retries | Publication authority and semantic coverage are independent |
| Markdown and HTML drift | Patch the Markdown parser or CSS | Both artifacts lack a shared Host-owned document |

No production-path change is justified merely because it improves one of
these symptoms. It must restore an upstream identity or authority invariant
and pass the corresponding corpus case.

## Non-Negotiable Invariants

1. Every run freezes exactly one `ResearchSpec` before targeted retrieval is
   admitted.
2. Every material dimension appears in at least one query-plan edge, unless
   the run records a planning gap for that dimension.
3. A query may cover several dimensions only when it seeks one coherent source
   family capable of answering them.
4. Every fetched source records the exact `(query_id, source_target_id)` edges
   that admitted it. Independent query and target arrays are invalid because
   their implicit cross-product can manufacture provenance that never occurred.
   Search rank, title, snippet, and provider date remain discovery metadata,
   not evidence.
5. Every admitted factual claim belongs to exactly one dimension and cites
   exact Host-owned source excerpts.
6. Every admitted inference identifies its admitted factual basis.
7. Every derived value absent from cited text is visibly labeled and identifies
   its admitted inputs and derivation method.
8. Every admitted recommendation identifies its admitted factual or inference
   basis. Advice is never relabeled as a sourced fact.
9. Contradictory admitted facts remain independently cited and connected by a
   typed relation; one is not silently selected as truth.
10. Every dimension ends with one of: claims only, claims plus a bounded gap, or
   a bounded gap only. Silent omission is invalid.
11. Invalid claims, relations, and gaps are rejected independently. Valid
    siblings survive.
12. The Host never labels a claim semantically `supported` merely because its
    aliases, numbers, or words occur in a cited excerpt.
13. The report writer cannot introduce a dimension, claim, source, URL, or
    evidence gap that is absent from the admitted intermediate representation.
14. Markdown and HTML are projections of the same `ReportDocument`; neither is
    parsed from the other.

## Authority Boundary

| Artifact or decision | Proposal authority | Final authority |
| --- | --- | --- |
| Research dimensions and source targets | Planning model | Host schema and identity validation |
| Search text | Planning model or exact-query fallback | Host budget and query-plan validation |
| Exploratory candidate IDs | Optional closed-ID selection policy | Host identity, quota, and provenance validation; corpus evaluation judges selection quality |
| Search and fetch execution | None | Host |
| Source identity, text, anchor, and provenance | None | Host |
| Facts, inferences, recommendations, and gap wording | Synthesis model | Host structural admission |
| Semantic entailment and product quality | No production component can prove it deterministically | Versioned corpus evaluation |
| Dimension presence and structural coverage | None | Host |
| Citations and source numbering | None | Host |
| Report structure, Markdown, HTML, and publication | None | Host |

Host admission deliberately has a limited claim. It can prove that identities
are closed, excerpts are authentic, required fields are present, relationships
are valid, and siblings are independently salvageable. It cannot prove natural
language entailment with token overlap. Frozen and live corpus evaluation is
the release authority for semantic precision and recall.

## 1. ResearchSpec

`ResearchSpec` is the immutable semantic identity of a run.

```rust
struct ResearchSpec {
    version: u32,
    query: String,
    language: String,
    current_date: String,
    evidence_scope: EvidenceScope,
    dimensions: Vec<ResearchDimension>,
    source_targets: Vec<SourceTarget>,
    budget: ResearchBudget,
}

struct ResearchDimension {
    id: String,
    question: String,
    material: bool,
    source_target_ids: Vec<String>,
}

struct SourceTarget {
    id: String,
    source_family_id: String,
    role: SourceRole,
    transport: AcquisitionTransport,
    match_policy: TargetMatchPolicy,
}

enum AcquisitionTransport {
    Web,
    Workspace,
}

enum TargetMatchPolicy {
    Named { identity: SourceIdentity },
    Exploratory { selection_goal: String },
}

enum SourceIdentity {
    Repository(String),
    Domain(String),
    Url(String),
    WorkspacePath(String),
}

enum SourceRole {
    Canonical,
    Official,
    Primary,
    Independent,
}
```

The concrete Rust representation may differ, but these semantics may not.

Dimension and target IDs are Host-validated stable ASCII identities. Display
questions and exploratory selection goals remain in the query language. The
model cannot set timeouts, search counts, fetch counts, retries, or terminal
policy.

`source_family_id` describes a coherent publication authority, not a topic
keyword or transport host. One dimension may legitimately require several
families and roles. EU primary law, EU institutional guidance, and independent
interpretation may all address one obligation dimension without becoming the
same authority. Likewise, one mixed local/public dimension may require a
workspace manifest and an official web policy. A singular family field on a
dimension would force these distinctions to be lost or turn source classes
into artificial reader dimensions.

A named target is used when the request or bounded planner can identify a
canonical repository, domain, URL, or workspace path before evidence fetch.
An exploratory target is used when the identity itself must be discovered,
such as an ambiguous market landscape. Exploratory does not grant permission
to claim exhaustiveness: the selection goal, candidate policy, admitted sample,
and resulting scope gap remain explicit.

There is no global `Exact`/`Decomposed` route in semantic state. C02 needs one
exact canonical web lookup, C05 may need several workspace discovery queries,
C06 mixes workspace and web transports, and C07 is exploratory. These are
query-level acquisition decisions, not different kinds of user request. If
planning fails, the Host freezes a one-dimension fallback over the complete
request and a planning limitation. That preserves publication authority but is
not evidence of complete semantic coverage.

## 2. QueryPlan

```rust
struct QueryPlan {
    spec_digest: String,
    queries: Vec<ResearchQuery>,
    planning_gaps: Vec<PlanningGap>,
}

struct ResearchQuery {
    id: String,
    text: String,
    transport: AcquisitionTransport,
    mode: QueryMode,
    dimension_ids: Vec<String>,
    source_target_ids: Vec<String>,
    fetch_slots: usize,
}

enum QueryMode {
    Exact,
    Discovery,
}

struct PlanningGap {
    dimension_id: String,
    missing_source_target_ids: Vec<String>,
    reason: String,
}
```

The Host rejects a plan when:

- it references an unknown dimension or source target;
- a query mixes target transports or conflicts with its declared transport;
- one material dimension has no query edge and no planning gap;
- it exceeds the global query budget;
- its Host-assigned fetch allocations exceed the global fetch budget;
- it changes the original request or invents a factual answer;
- duplicate broad queries compete for the same bounded slots; or
- one query combines unrelated source families merely to reduce call count;
- an `Exact` query references an exploratory target; or
- a planning gap references an unknown dimension or undeclared target, leaves
  an unscheduled target unnamed, or presents an unverified factual absence as
  its reason.

Candidate admission is balanced first across query IDs and then across source
targets within each query. One dimension may appear on several query edges when
different publication authorities are required. A high-ranked result from one
query cannot consume all fetch slots while another material dimension receives
none. URL and semantic-target deduplication happen before fetch admission.

### Budget Feasibility And Target Admission

A structurally valid plan is still invalid when its required targets cannot fit
the Host budget. For every admitted plan:

1. each query receives an explicit fetch-slot allocation;
2. the number of named source targets for that query is no greater than its
   allocation, unless the plan explicitly leaves one as a planning gap;
3. the sum of query allocations is no greater than the global fetch budget;
4. every candidate admitted for a named target is identity-matched before it
   can consume that target's slot;
5. a named match uses a distinctive target identifier, not merely a
   generic family term such as `runtime`, `Rust`, `official`, or
   `documentation`;
6. each named target receives one admission opportunity before any target
   receives a second slot;
7. an exploratory target has a separately recorded bounded candidate policy
   and cannot consume slots allocated to named targets;
8. provider rank and general domain authority may participate only in the
   declared within-target or exploratory policy; they cannot substitute an
   unrelated candidate for an unfilled named target; and
9. unused slots may remain unused when no target-matching or policy-admissible
   candidate exists.

Semantic source families and transport families are different identities. A
GitHub repository and its docs.rs crate page may be one semantic target even
though they use different transports. Conversely, two GitHub repositories are
different targets even though they share a host. Transport deduplication is
applied only after target allocation.

Schema maxima are safety ceilings, not desired output counts. A planner must
derive dimensions from material clauses and decision scenarios in the request;
it must not expand examples into new dimensions to fill `maxItems`. The C01
pilot uses six dimensions and at most two required targets per query because
that is the largest plan that can satisfy its four-search/eight-fetch budget.

An original-query search may run concurrently with planning to reduce
time-to-first-source, but it consumes the same global search and fetch budgets.
The C01 pilot must compare `original + three focused` against `four focused`
queries before production chooses either policy. Immediate search is a latency
hypothesis, not permission to increase the shared budget.

## 3. SourceCatalog

```rust
struct SourceCatalog {
    spec_digest: String,
    sources: Vec<SourceRecord>,
    attempts: Vec<AcquisitionAttempt>,
}

struct SourceRecord {
    id: String,
    title: String,
    requested_anchor: String,
    canonical_anchor: String,
    captured_at: String,
    provenance: Vec<SourceProvenance>,
    chunks: Vec<SourceChunk>,
    content_digest: String,
}

struct SourceProvenance {
    query_id: String,
    source_target_id: String,
}
```

The Host owns every field. Each provenance edge must resolve to a declared
query-target edge and a real successful fetch attempt; one valid edge cannot
hide an unrelated target. Source text is immutable after admission. A model
sees bounded aliases and excerpts, never publication URLs. Provider snippets
may help candidate selection but never enter the claim packet as evidence.

Failed search and fetch attempts are retained because a bounded gap must state
what this run failed to establish without claiming that the fact does not
exist.

## 4. Flat ClaimLedger

The synthesis boundary is a flat graph, not a nested report-shaped object.

```rust
struct ClaimLedgerProposal {
    claims: Vec<ClaimProposal>,
    relations: Vec<ClaimRelationProposal>,
    gaps: Vec<GapProposal>,
}

struct ClaimProposal {
    id: String,
    dimension_id: String,
    placement: ClaimPlacement,
    kind: ClaimKind,
    text: String,
    evidence_refs: Vec<EvidenceRef>,
    basis_claim_ids: Vec<String>,
    derivation: Option<DerivationProposal>,
}

enum ClaimPlacement {
    DirectAnswer,
    Finding,
}

enum ClaimKind {
    Fact,
    Inference,
    Recommendation,
}

struct EvidenceRef {
    source_id: String,
    chunk_ids: Vec<String>,
}

struct DerivationProposal {
    method: String,
    input_claim_ids: Vec<String>,
}

struct ClaimRelationProposal {
    id: String,
    dimension_id: String,
    kind: ClaimRelationKind,
    claim_ids: [String; 2],
}

enum ClaimRelationKind {
    Contradicts,
}

struct GapProposal {
    id: String,
    dimension_id: String,
    text: String,
    attempted_query_ids: Vec<String>,
    missing_source_target_ids: Vec<String>,
}
```

The wire schema stays flat even when dimension IDs are dynamic. It contains
arrays of independent claims and gaps; it never generates a dynamic nested
object keyed by dimension. This preserves sibling claims when one item is
malformed and avoids the observed failure where a model returned a claim array
for a document-shaped schema.

Admission rules are kind-specific:

- a fact requires at least one exact evidence reference and no basis claim;
- an inference requires at least one admitted factual basis and may also cite
  evidence directly;
- an inference that introduces a calculated value absent from its evidence
  excerpts requires a bounded derivation method and admitted input claims; it
  is rendered as a derivation, never relabeled as a sourced fact;
- a recommendation requires at least one admitted fact or inference basis;
- every basis reference must resolve to an independently admitted claim and
  the resulting graph must be acyclic; admission cannot depend on array order;
- a contradiction relation references exactly two independently admitted
  claims in the same dimension; an invalid relation never removes either
  factual sibling;
- a gap may reference only real acquisition attempts and declared source
  targets;
- a gap says only what the fetched catalog failed to establish;
- one malformed item is removed without rejecting siblings; and
- duplicate or compound claims are measured and rejected at claim granularity,
  not paragraph granularity.

The model may omit a dimension accidentally. The Host then inserts a
deterministic missing-output gap for that dimension. This makes the omission
visible, prevents a false completed state, and preserves valid siblings. The
inserted gap is a runtime diagnostic translated into reader-safe wording; it
does not claim that evidence is unavailable on the web.

## 5. CoverageMatrix

The Host derives a structural matrix from admitted ledger items.

```rust
enum StructuralCoverage {
    ClaimsOnly,
    ClaimsAndGap,
    GapOnly,
    Missing,
}
```

`ClaimsOnly` is not called `supported`. The Host knows that a closed reference
exists; it does not know that natural-language entailment is correct.
`ClaimsAndGap` represents a useful partial answer with a consequential
boundary. `GapOnly` represents a bounded dimension. `Missing` is invalid and
must be converted to a deterministic gap before publication.

Internal terminal outcomes use structural facts only:

- `Synthesized`: every material dimension has claims and no material gap;
- `Qualified`: useful incomplete work exists and every resolved dimension has
  passed the comprehensive depth gate; when all dimensions are bounded, exactly
  one partial conclusion must independently pass that gate and retain a typed
  gap;
- `SourceBacked`: sources exist but no claim ledger was admitted; and
- `NoEvidence`: no safely publishable source exists and the explicit boundary
  artifact was published.

Failure to publish a safe artifact is an execution error, not a fifth
publication outcome.

These outcomes describe runtime artifacts, not product correctness. Corpus
evaluation still assigns `supported`, `bounded`, `missed`, or `incorrect` per
dimension.

## 6. ReportDocument

```rust
struct ReportDocument {
    title: String,
    direct_answer_claim_ids: Vec<String>,
    dimensions: Vec<ReportDimension>,
    source_ledger: Vec<ReportSource>,
}

struct ReportDimension {
    dimension_id: String,
    claim_ids: Vec<String>,
    relation_ids: Vec<String>,
    gap_ids: Vec<String>,
}
```

The Host creates the document from the immutable spec, admitted ledger, and
source catalog. It owns headings, labels, ordering, citation numbering,
limitations, and source-ledger entries.

Direct answer content must be admitted claims, not a statement of research
intent such as “this report will compare.” A claim may appear once in the
reader document. A recommendation is rendered with citations expanded from
its basis claims. Derived values are visibly labeled with their admitted input
claims. Contradictory claims remain side by side with separate citations. Gaps
remain attached to their dimensions so a local absence cannot become a
report-wide absence claim.

Markdown and HTML consume this document directly. Global citation numbering is
assigned once, after final claim admission, so removing one claim cannot leave
stale or duplicated source numbers.

## C01 Paper Walkthrough: Broad Technical Comparison

Query:

> As of the evaluation date, compare Tokio and async-std maintenance, HTTP and
> database ecosystem compatibility, and production choices for new and legacy
> projects. Prefer official sources and separate facts, judgment, and gaps.

The plan uses four focused discovery queries because maintenance, HTTP
libraries, database libraries, and migration guidance have different
publication authorities.

| ID | Source family | Material question | Required source targets |
| --- | --- | --- | --- |
| `d1` | `runtime_maintenance` | What is the current maintenance status of Tokio and async-std? | Tokio releases; async-std maintenance statement; RustSec advisory |
| `d2` | `http_runtime_support` | What official evidence establishes Tokio HTTP integration? | axum and hyper official documentation or repository examples |
| `d3` | `http_runtime_support` | What official evidence establishes async-std HTTP support or adapters? | async-std, Tide, Surf, or compatibility-project official material |
| `d4` | `database_runtime_support` | What do official database libraries say about both runtimes? | SQLx and, budget permitting, another official database library |
| `d5` | `runtime_choice_guidance` | What bounded production choice follows for a new project? | admitted maintenance and ecosystem facts; official guidance when available |
| `d6` | `runtime_choice_guidance` | What bounded migration or coexistence guidance exists for a legacy project? | async-std migration statement and official compatibility guidance |

A four-query plan maps `d1` to maintenance sources, `d2` and `d3` to the HTTP
source family, `d4` to the database source family, and `d5` and `d6` to official
choice or migration guidance. It may not combine HTTP and database dimensions
in one query. The selector still balances query groups and source families; it
does not mark the whole HTTP or database dimension covered because one Tokio
or one SQLx example was fetched.

Using the retained seven-source pilot catalog, the structurally honest result
would be:

| Dimension | Admissible result |
| --- | --- |
| `d1` | Facts for the async-std discontinuation and current Tokio release activity, plus a bounded maintenance inference |
| `d2` | A fact about the fetched axum/hyper Tokio example, plus a gap against ecosystem-wide generalization |
| `d3` | Gap only; no fetched official async-std HTTP compatibility statement established this dimension |
| `d4` | SQLx facts, plus a gap because one library does not establish the whole database ecosystem |
| `d5` | A recommendation based on admitted `d1`, `d2`, and `d4` premises; performance and adoption claims remain absent |
| `d6` | The official recommendation to move from async-std to smol, plus a gap for concrete migration steps, cost, and compatibility impact |

This walkthrough prevents the observed false green in which the HTTP dimension
was called supported from one Tokio example while the async-std side remained
unresearched. It also prevents “Tokio is preferred” from being published as a
fact: it is either an attributed source statement or a recommendation with
explicit factual bases.

## C02 Paper Walkthrough: Narrow Canonical Fact

Query:

> As of the evaluation date, what Tokio LTS branches are supported, when does
> each support window end, and what MSRV does each branch declare? Use the
> canonical Tokio source and distinguish LTS information from the newest
> non-LTS release.

The plan uses one `Exact` query, even though it contains several dimensions,
because the dimensions seek one coherent canonical Tokio LTS source family.

| ID | Source family | Material question | Source target |
| --- | --- | --- | --- |
| `d1` | `tokio_lts_policy` | Which LTS branches are currently supported? | Canonical Tokio LTS source |
| `d2` | `tokio_lts_policy` | When does each support window end? | Canonical Tokio LTS source |
| `d3` | `tokio_lts_policy` | What MSRV does each branch declare? | Canonical Tokio LTS source |
| `d4` | `tokio_lts_policy` | How does the newest non-LTS release differ from the LTS list? | Canonical Tokio release and LTS sources |

One precise provider query maps to all four dimensions. The Host may follow a
canonical link to the current release record within the fetch budget; it does
not schedule four independent searches or a broad ecosystem planner.

The ledger keeps each branch, support end, and MSRV proposition independently
citable. If the canonical LTS page establishes branches and windows but not the
newest release, `d1` through `d3` survive and `d4` receives a specific gap. A
single missing newest-release fact cannot erase the LTS answer, and a secondary
summary cannot replace the required canonical source.

## Cross-Corpus Paper Audit

C01 and C02 are necessary but not sufficient architecture cases. Walking the
remaining product corpus without code exposed constraints that a
C01-specialized target registry would miss:

| Case | Contract pressure | Required compiler behavior |
| --- | --- | --- |
| C03 EU AI Act | One obligation dimension needs primary law, institutional explanation, and separately labeled third-party interpretation. | A dimension may reference several source families and roles; source classes do not become artificial reader dimensions. |
| C04 pgvector versus Qdrant | Vendor documentation, operational facts, and non-comparable benchmark claims feed one decision. | Preserve attribution and benchmark gaps; recommendations depend on admitted premises rather than vendor rank or prose similarity. |
| C05 repository trace | Every authoritative source is a workspace file, but several searches and reads may be needed. | Query transport is per query; workspace paths and line/range anchors are first-class source identities. |
| C06 local plus Tokio policy | Local manifest and lockfile facts must combine with canonical public policy without confusing a version range with a resolved version. | One dimension may cross workspace and web targets; no global route or transport can describe the run. |
| C07 “best framework” | Relevant project identities and ranking criteria are not fully known before discovery. | Use a bounded exploratory target, record the candidate policy and sample, and require a scope gap instead of fabricating an exhaustive winner. |
| C08 private incident rates | Search attempts may return adjacent public material but no evidence for the requested private rate. | Persist attempts and publish a gap only; nearby runtime facts cannot satisfy the dimension. |
| C09 over-budget request | More material dimensions exist than the shared search/fetch budget can investigate. | Freeze every material dimension, prioritize within the budget, and attach planning gaps to unscheduled dimensions; never silently delete them. |

The frozen fixtures add two graph requirements. F01 needs a typed contradiction
relation that preserves both cited facts. F02 needs a visibly labeled
derivation with admitted inputs when a calculated value is absent from source
text. These are why the ledger cannot be only a list of report paragraphs.

This audit changes the earlier design in three ways: acquisition mode is
query-local rather than a global route, dimensions may depend on multiple
source targets and families, and targets may be either named or explicitly
exploratory. The named-target admission invariant remains strict; exploratory
selection is bounded, measured policy rather than an implied canonical match.

## C01 Flat-Ledger Pilot Evidence

The first dynamic-dimension flat-ledger run used the retained seven-source C01
catalog and `openai/gpt-5.1`. It completed without a repair in 36.38 seconds,
using 4,004 prompt tokens and 2,435 completion tokens. The proposal produced 15
claims, the legacy block adapter admitted all 15, and all six planner dimensions
contained at least one claim and one explicit gap. The earlier dynamic nested
schema had failed after 26.67 seconds, so this is direct evidence that the flat
wire shape is materially more reliable.

The run is not semantically acceptable:

- `d3` used SQLx database-runtime evidence as support for the HTTP/Web
  dimension;
- `d2` generalized one axum/hyper example into a default integration claim;
- `d4` generalized one database library into a long-term ecosystem direction;
- `d6` proposed a smooth staged runtime migration while its own gap said that
  no concrete migration steps or impact evidence had been fetched; and
- the legacy report adapter discarded every explicit gap from reader-facing
  Markdown.

These defects refine rather than reject the compiler design. They prove that
query-to-dimension edges need an enforceable source-family identity, that
recommendations need explicit admitted basis claims, and that only a
Host-owned `ReportDocument` can preserve dimension gaps. They also prove that
flat shape and complete structural coverage alone are insufficient release
evidence.

## C01 Source-Family Acquisition Pilot Evidence

The four-family planner run retained at
`/tmp/a3s-deepresearch-live/C01/four-query-plan/run-5-source-family-registry/`
completed in 41.86 seconds and fetched eight sources. Its four searches were
structurally coherent, but its catalog was not fit for synthesis:

- the eight-item dimension ceiling encouraged two unrequested dimensions for
  HTTP middleware and database connection pools;
- every query declared three targets while round-robin allocation provided
  only two fetch slots;
- the maintenance family selected `tokio-async-std` documentation and
  async-std releases, omitting the available Tokio repository;
- the HTTP family selected a SQLx issue and a third-party Stackwise page;
- the database family spent both slots on SQLx representations and omitted
  available SeaORM material; and
- the guidance family omitted the fetched search candidate containing the
  async-std migration statement.

The selector scored each candidate against the union of all target terms in a
query and allowed authoritative hosts to win without a target match. It could
therefore satisfy its own numeric budget while failing the semantic budget.
This run is direct evidence for target-first admission and for rejecting an
infeasible target cardinality before search.

## C02 Exact-Query Continuity Evidence

The retained C02 exact-query acquisition found both the canonical LTS material
and the current Tokio release feed. A synthesized GPT-5.1 report correctly
published the `1.47.x` and `1.51.x` LTS windows and MSRVs, but did not explicitly
answer the requested distinction from the newest `1.53.1` non-LTS release.
The source-backed fallback visibly retained that release record, so this was
not an acquisition failure. It was a report-stage semantic omission that the
active path could not detect because no `newest_non_lts` dimension existed.

This case proves that an `Exact` query cannot mean “skip dimensions.” Several
frozen dimensions may share one coherent canonical target and one bounded
provider query. The report must publish an admitted newest-release fact or a
dimension-specific gap; silently omitting it is invalid even when every other
LTS fact is correct.

## Original Full-Runtime Candidate Sequence

1. Start the run and persist the original request and Host budget.
2. Optionally search the original request while one bounded plan proposal is
   generated; all effects share one global budget.
3. Validate and freeze `ResearchSpec` and `QueryPlan`, or freeze the explicit
   one-dimension fallback with a planning limitation.
4. Execute focused queries and balanced fetch admission.
5. Persist the immutable `SourceCatalog` before synthesis.
6. Stage a deterministic source-backed `ReportDocument` immediately.
7. Request one flat `ClaimLedger` proposal over the closed spec and source
   catalog.
8. Admit claims and gaps independently, derive structural coverage, and replace
   only the corresponding document blocks.
9. Render and atomically publish Markdown and HTML from the same document.
10. Record product-evaluation artifacts separately from internal terminal
    status.

The end-to-end planner replacement in this candidate was rejected. Its report
compiler boundary is now active and still has no per-question reviewer,
per-section writer, semantic self-audit, report repair wave, or
presentation-model call. The active path uses complete source-local selector
windows capped at 32 KiB and an exact-ID per-source reduction so late evidence
is not lost to a single oversized cross-source packet. See
[`deep-research-evidence-first-redesign.md`](deep-research-evidence-first-redesign.md).

## Implementation Minimality

The logical artifacts in this contract do not authorize six independently
persisted subsystems, six model calls, or six compatibility layers. The new-run
implementation has four aggregate boundaries:

1. `ResearchContract`, which persists the frozen `ResearchSpec` and validated
   `QueryPlan` atomically;
2. `SourceCatalog`, which persists acquisition attempts, immutable fetched
   text, and query/target provenance;
3. `ClaimLedger`, which persists independently admitted claims and gaps; and
4. `ReportDocument`, which is the sole publication input.

`CoverageMatrix` is a pure Host derivation from the frozen contract and claim
ledger. It is not model output and does not need a separately mutable source of
truth. Markdown and HTML are projections, not workflow state. A model proposal
may be retried only by an explicit future product policy; the initial candidate
uses at most one planning proposal and one claim-ledger proposal.

The four aggregates are a semantic minimum, not a framework. Combining them
into one untyped JSON packet would recreate the identity-loss defect. Splitting
them into additional report, question, section, reviewer, or presentation
transactions would recreate the orchestration defect.

## Existing-Code Reuse Boundary

The migration keeps proven transport and publication infrastructure while
replacing the new-run semantic protocol. It does not wrap the compiler in the
legacy Inquiry shapes.

| Existing capability | New-run disposition | Boundary |
| --- | --- | --- |
| CLI, TUI, and Code Web launch, session setup, absolute deadlines, and budget calculation | Retain | They transport one compiler run and do not decide research truth. |
| Web/local tool invocation, fetch retry, document-range restoration, and text chunking | Retain below acquisition | Candidate, fetched-source, and packet records must be extended so query, family, target, and attempt identities survive fetching. |
| URL/path validation, canonicalization, secret filtering, and source-text bounding | Retain | These remain Host-owned SourceCatalog admission rules. |
| Graph event store, optimistic concurrency, checkpoints, and strict replay | Retain as storage machinery | New runs persist versioned compiler aggregates. Existing Inquiry objects are replayed only for old run versions. |
| Atomic Markdown/HTML pair publication and safe report paths | Retain | Both byte streams must be rendered from one `ReportDocument` before the existing atomic write. |
| Responsive CSS and local browser/webview opening | Retain after semantic publication | CSS may style typed document sections; it may not recover structure by parsing Markdown. |
| Historical Inquiry/report event data | Generic graph inspection only | The retired executors are not adapters or fallbacks for a new run. |
| Evidence-first report blocks and binary `Synthesized`/`SourceBacked` status mapping | Replaced | The active typed graph carries dimension, claim-kind, basis, derivation, relation, and gap identity and adds `Qualified`. |
| Markdown-to-HTML semantic composition | Replaced | Markdown and HTML consume the same typed document directly. |
| Generic recovery prose after useful evidence exists | Replaced | The staged source-backed `ReportDocument` is the fallback authority. |

Version dispatch belongs at run loading and creation. There is deliberately no
converter from a compiler `ResearchContract` to legacy Inquiry tracks, from a
compiler `SourceCatalog` to `AcceptedEvidence`, or from a `ClaimLedger` to
report-shaped blocks. Such converters would preserve execution compatibility
by discarding the exact identities this design exists to protect.

### Ownership Boundary

The ownership correction and report-protocol migration are complete in the
independent `a3s-deep-research` crate. It owns the compiler contracts, workflow
assets, source/report admission, and artifact construction.
`src/commands/code/research_runtime.rs` and the TUI controller own argument
handling, `AgentSession` adaptation, progress presentation, journal settlement,
and browser opening only. The rejected part is the former proposal to move the
entire retrieval planner behind the compiler, not the active claim graph.

Historical Inquiry events remain readable as generic graph data. There is no
old-run dispatcher that can execute the former planner or report pipeline.

## Rejected Acquisition Heuristics

The C01 experiments also generated heuristics that are explicitly not product
rules:

- appending a vocabulary-derived token to model-authored search text;
- adding owner, repository, publisher, or domain seeds inferred from names;
- classifying requested artifacts from query words;
- preferring artifact shapes from URL or title vocabulary; and
- routing or budgeting from detected topic, language, or script.

Target-first admission itself is an invariant: an unrelated candidate cannot
consume a slot declared for another target. The mechanisms used to discover
and rank artifacts inside a target are empirical policies. A repository root
seed, for example, is a fallback fetch opportunity; it is not proof that the
root is the best document for the requested artifact.

None enters production. Meaningful expansion and admission belong to bounded
model contracts over closed packets; deterministic code validates IDs, shape,
provenance, and budgets only.

The generic compiler core contains no Tokio, async-std, axum, SQLx, SeaORM,
RustSec, EU, PostgreSQL, pgvector, or Qdrant special case. Named identities
belong only to versioned corpus fixtures and live planner output.

## Migration Boundary

The report-protocol migration is complete:

1. `deep_research_typed_claim_graph` replaced report-shaped block proposals;
2. the compiler admits claims, basis edges, derivations, contradiction
   relations, and typed gaps independently;
3. `CoverageMatrix` and `ReportDocument` are Host projections rather than model
   output;
4. Markdown and HTML render from the same document;
5. CLI, TUI, and Code Web settle `Synthesized`, `Qualified`, `SourceBacked`,
   and `NoEvidence` through receipts and replay while retaining accepted
   relation, derivation, basis-edge, and gap counts; and
6. the production F01-F08 engine replay exercises the same wire contract.

The original full planner migration remains rejected. Retrieval continues to
use exact-query bootstrap plus the bounded semantic outline and closed
source/chunk selection. Live C01-C09 ablation, equal-budget comparison, and
website inspection remain product validation work, not permission to add
query-vocabulary heuristics or a parallel runtime.

Do not add adapters back to the retired report-block representation. The typed
claim graph and `ReportDocument` are the single source of truth for a new
generated report; otherwise the same semantic drift would survive behind
another conversion layer.

## Validation Gates Before Production Work

The design advances only when all of these are true:

- C01 freezes the six material dimensions from the paper walkthrough without
  example-driven expansion, admits no more targets than its fetch allocations
  can satisfy, and acquires target-matching official sources for maintenance,
  HTTP, database, and migration families;
- C01 has one structural result for every frozen dimension, with async-std HTTP
  and ecosystem-wide claims bounded when the catalog does not establish them;
- C02 uses one exact query and retains valid LTS facts when one newest-release
  item is unavailable, and never silently omits the newest non-LTS dimension;
- C03 keeps primary law, institutional explanation, and third-party
  interpretation as distinct target roles without splitting one material
  obligation into source-shaped reader questions;
- C05 acquires only workspace evidence, while C06 retains both workspace and
  web provenance on the dimensions that require them;
- C07 records an exploratory sample and scope gap instead of declaring a
  universal winner; C08 publishes no adjacent fact as a substitute for the
  unavailable private rate;
- C09 freezes every material dimension and turns budget-excluded work into a
  planning gap rather than dropping it;
- a malformed claim or gap preserves every valid sibling item;
- F01 preserves both sides of a contradiction, and F02 labels every derived
  value with admitted inputs and a reproducible method;
- facts cannot cite a source from another semantic source family, inferences
  require admitted factual premises, and recommendations require admitted fact
  or inference premises through an acyclic graph;
- a report-generation failure publishes the staged source-backed document;
- no model output can add a URL, dimension, source, or report heading;
- global citation numbering remains correct after arbitrary claim removal;
- Markdown and HTML contain the same claims, gaps, and source ledger; and
- the live pilot measures semantic coverage, authority, latency, and artifact
  quality separately.
