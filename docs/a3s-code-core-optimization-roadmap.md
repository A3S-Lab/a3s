# A3S Code Core Optimization Roadmap

- Status: accepted engineering plan
- Decision date: 2026-09-05
- Owner: A3S Code, with A3S Use and A3S Desktop integration gates
- Baseline: Code `main` at `bfd79f4c` and the repository integration at
  `b40626bf`
- Scope: simplify and strengthen the Code execution kernel; do not add a
  foreign Harness runtime

This document turns the first-principles architecture review into an ordered
implementation plan. It supplements the
[scientific discovery platform roadmap](scientific-discovery-platform-roadmap.md)
and the Code-owned [roadmap](../crates/code/ROADMAP.md).

## 1. Decision summary

A3S Code already has the required capability breadth: governed Agent
execution, exact capability generations, scoped cancellation, workspace and
semantic retrieval, durable memory, workflows, evaluation, checkpoints,
replay, and cross-language SDKs. The next performance and reliability gains
come from reducing parallel authorities and duplicated lifecycle machinery.

The target is one small execution kernel with typed host adapters:

```text
Host API / SDK / Harness
          │
          ▼
ExecutionCoordinator
  Run · Turn · Tool · Model · Workflow
          │
          ├── CapabilitySnapshot + AuthorityCeiling
          ├── CoreEventLog + EvidenceCursor
          ├── WorkspaceSourceSnapshot
          └── Artifact/CAS + Checkpoint
          │
          ▼
Typed adapters: Use · MCP · Flow · Memory · Search · Sandbox · Cloud
```

Code remains the execution and evidence authority. A3S Use remains the
package, grant, environment, and generation authority. Desktop remains the
project and human-decision projection. No component may create a second
session/run store or silently reinterpret Code evidence.

## 2. First-principles invariants

Every change in this roadmap must preserve these invariants:

1. **One fact, one authority.** A fact is appended once and projected into
   Agent, evaluation, research, graph, and SDK views.
2. **Authority is temporal.** A Run can only use the exact capability,
   workspace, policy, and provider generation admitted for that Run.
3. **Evidence precedes synthesis.** Missing, stale, conflicting, or redacted
   evidence is explicit; it never becomes an implicit success.
4. **Boundedness is a contract.** Bytes, tokens, tasks, queues, descriptors,
   files, sockets, retries, and shutdown time are bounded before work starts.
5. **Cancellation follows ownership.** Every spawned future belongs to a
   Session, Run, Turn, or Subtask and settles before that owner releases its
   leases.
6. **Reproducibility is identity.** Source, workflow, code, environment,
   model, parameters, seed, provider, and output are content-addressed.
7. **Trust is explicit.** Model, tool, web, package, host, secret, and derived
   artifact data have different trust and egress rules.
8. **Adapters do not become authorities.** Search, memory, Flow, MCP, and
   remote providers implement typed ports and cannot rewrite Code history.

## 3. Baseline observations

The current architecture is strong but has measurable consolidation pressure:

- Runtime facts are represented by `AgentEvent`, `RunEventRecord`,
  `EventEnvelopeV1`, `harness_evidence`, the evaluation fact journal,
  research contracts, and State Graph events.
- Lifecycle state is coordinated by `AgentSession`, `AgentLoop`, Run stores,
  capability supervisors, evaluation supervisors, queues, the task scheduler,
  and the protocol Harness/Host.
- `SessionStore` still exposes aggregate snapshots and legacy fragment APIs.
- The capability projection is correct and generation-exact, but compatibility
  registries remain beside the projected registry until `CAP-GA1`.
- Workspace manifest, LSP documents, retrieval chunks, semantic indexes, and
  zvec indexes each maintain related source/revision state. The persistent
  zvec-grep work is a derived-cache integration and is not a new source of
  truth.
- Large implementation units remain in `tools/task.rs`, `run.rs`,
  `workspace/local.rs`, and `evaluation/evidence.rs`; the Core Cargo manifest
  also couples model, browser, search, memory, Flow, sandbox, zvec, S3, and
  QuickJS concerns.
- The native research contracts are now strict Rust values, but still need an
  adapter into the existing Run, Event, Store, Evaluation, and SDK planes.

These observations are architecture risks, not reasons to remove working
capabilities. Refactors must be incremental and behavior-preserving.

## 4. Optimization tracks

### KRN-1 — Unified identity, clock, and event fabric

Introduce a small `CoreIdentity` layer containing typed `OperationId`,
`SourceRevision`, `CapabilityStamp`, `EvidenceCursor`, `ArtifactRef`, and an
injectable logical clock. Add one append-only `CoreEventLog` with canonical
encoding, domain-separated digests, causality, retention gaps, and bounded
payload references.

The existing Agent, evaluation, research, and State Graph values become
validated projections. Do not create another audit database. Preserve the
current wire envelopes as compatibility projections until the migration gate
is complete.

**Exit gate:** replaying any retained log twice is idempotent; duplicate,
reordered, stale-generation, and cursor-skipping writes fail closed; all
projections expose the same operation, source, capability, and evidence
identity.

### KRN-2 — One execution coordinator

Create an internal `ExecutionCoordinator` that owns Run admission, Turn
creation, cancellation, task registration, Tool/Model/Workflow invocation,
event append, checkpoint acknowledgement, and close. Existing facades remain
public API adapters.

Detached work must use an explicit durable/external-run contract. A dropped
`JoinHandle` must never be the accidental way to outlive a Run. Close must
settle child Tasks, evaluation work, refresh jobs, stream bridges, and
capability effects before releasing the Use lease.

**Exit gate:** one state machine covers normal, streaming, direct-tool,
delegated, protocol, and recovery paths; no unowned task or lease remains in
the lifecycle qualification matrix.

### KRN-3 — Capability plane convergence

Finish the migration from compatibility registries to the immutable
`CapabilityProjection` path. Keep `CapabilitySet` as the identity plane and
separate three concerns currently close together:

- `AuthorityCeiling` — what may be accessed;
- `ResourceBudget` — how much work may be done; and
- `ReadinessStamp` — whether a dependency is usable.

Use exact generation leases for all Use-backed surfaces. Use lazy runtime
handles for expensive MCP, Flow, UI, and Knowledge values while retaining
generation-exact identity and atomic publication.

**Exit gate:** one catalog, one admission transaction, no shadowing or
piecemeal reconciliation, bounded cutover/retirement, and unchanged old-Run
behavior across an N → N+1 publication.

### KRN-4 — Source snapshot and derived data plane

Define `WorkspaceSourceSnapshot` as the single revision authority for local,
remote, and S3 content. It binds repository revision, file content digest,
document/LSP revision, eligibility policy, chunk revision, and index
generation.

Manifest scanning, LSP, BM25, zvec, semantic vectors, context selection, and
research evidence must consume this snapshot. Indexes are rebuildable derived
caches published by atomic generation/CAS; stale results cannot cross a
source snapshot boundary.

**Exit gate:** a search hit, symbol result, model context item, and evidence
fact can be traced to the same source snapshot; concurrent edits never yield a
false current result; zvec startup, crash recovery, and platform loading pass
the locked qualification matrix.

### KRN-5 — Model and Tool middleware

Normalize blocking, streaming, structured, repair, compaction, and auxiliary
model calls through one `ModelCallMiddleware`:

```text
admission → budget → cancellation → capability/evidence bind
→ provider call → usage/cost → retry/repair → event append
```

Normalize every Tool, MCP, Flow, and Use Runtime Task through one
`ToolInvocation` state machine. Add typed trust/taint labels so redaction and
egress checks are applied at the value boundary instead of repeated in each
adapter.

**Exit gate:** every provider and Tool path produces the same admission,
cancellation, usage, error, and evidence semantics; model output cannot
change authority; untrusted content cannot become an instruction without an
explicit host policy.

### KRN-6 — Durable log, CAS, and artifact lifecycle

Extend `SessionStoreCapabilities` to advertise aggregate CAS, append-only log,
lease fencing, encryption, watch, and artifact-GC guarantees. Move the file
adapter toward WAL plus periodic immutable snapshots while retaining the
current API during migration.

Make artifact retention reference-aware: a provenance receipt, review finding,
checkpoint, or publication keeps its referenced content alive. Garbage
collection must never remove an object reachable from a retained identity.

**Exit gate:** crash/restart/replay tests prove atomic visibility, no partial
generation, no lost event, no duplicate side effect, and no premature artifact
deletion under concurrent writers.

### KRN-7 — Research and evaluation convergence

Adapt `ResearchRunV1`, `ResearchEvidenceFactV1`,
`ResearchProvenanceReceiptV1`, `ResearchReviewFindingV1`, and
`ResearchEventV1` onto KRN-1 rather than adding a Research Store. Add a typed
reproducibility manifest for provider parameters, model revision, environment
lock, code/workflow digests, seeds, tolerances, and output artifacts.

Reviewer execution uses the generic Evaluation Substrate for isolation,
cancellation, bounded evidence, and immutable results. Research review is a
typed projection with host-owned rubric, threshold, human approval, and
publication decisions.

**Exit gate:** one research run survives restart and fork; every published
claim, number, figure, table, and report has source/evidence/provenance links;
review findings cannot imply approval; Rust/Node/Python/Go receive the same
strict schema.

### KRN-8 — Workflow and scheduler convergence

Unify Planning, orchestration, Dynamic Workflow, Flow, and State Graph around
one internal `ExecutionPlan`/Step model. Keep their public formats as
adapters. Unify `a3s-lane`, Session queues, and the task scheduler around one
admission scheduler with priority, fairness, per-Run quotas, provider quotas,
and starvation protection.

Checkpoint and idempotency identity must be shared by model calls, Tool calls,
workflow steps, and evaluator dispatches.

**Exit gate:** sequential, parallel, resumable, delegated, and Flow-backed
plans use one cancellation and checkpoint semantics; no orphan work remains
after parent cancellation or process restart.

### KRN-9 — SDK and host contract generation

Generate Event, Capability, Evaluation, Research, Run Control, and error
schemas from one catalog. Add research capability discovery and exact schema
version negotiation to Node, Python, and Go. Replace FFI runtime `.expect()`
paths with fallible initialization and stable error codes.

**Exit gate:** generated declarations and negative fixtures pass parity checks;
all SDKs agree on cancellation, backpressure, replay, unknown-field, and error
semantics.

### KRN-10 — Kernel modularization and release profiles

Keep `a3s-code-core` as a compatibility facade, but split internal ownership
into these dependency layers:

```text
a3s-code-kernel       identity · clock · events · capabilities · scopes
a3s-code-engine       run · turn · model · tool · workflow
a3s-code-data         workspace · retrieval · memory · artifacts · CAS
a3s-code-evaluation   evidence · evaluator · research projections
a3s-code-host         MCP · Use · Flow · SDK · Serve · Cloud adapters
```

Publish explicit feature profiles (`minimal`, `local-code`, `scientific`,
`server`, `full`). Native zvec, browser, S3, QuickJS, and telemetry must be
selected by a product profile rather than silently increasing every embedder's
build and supply-chain surface.

**Exit gate:** kernel builds without browser/S3/zvec dependencies; release
profiles have documented size, startup, memory, and feature behavior; public
API compatibility tests remain green.

## 5. Delivery phases

| Phase | Focus | Required deliverables | Exit gate |
| --- | --- | --- | --- |
| **P0 — Freeze and measure** | Establish one baseline before refactoring | Machine-readable capability inventory; schema/owner map; event/identity inventory; lifecycle leak matrix; source/index consistency matrix; compile/startup/RSS/latency measurements | Architecture review approves owners and budgets; no behavior change is hidden in a refactor |
| **P1 — Kernel convergence** | KRN-1, KRN-2, KRN-3 | Core identities, logical clock, event-log adapter, ExecutionCoordinator skeleton, separated ceiling/budget/readiness, compatibility-path telemetry | Existing full Core tests pass; replay/cancellation/lease tests pass through both old and new adapters |
| **P2 — Data and durability** | KRN-4, KRN-6 | WorkspaceSourceSnapshot, derived-index generation/CAS, unified artifact refs, store capability expansion, WAL/CAS prototype | Concurrent edit/index/restart fixtures prove current-source results and atomic persistence |
| **P3 — Execution simplification** | KRN-5, KRN-8 | Model middleware, ToolInvocation FSM, shared plan/step model, unified scheduler and idempotency | Provider/tool/workflow matrix has identical budget, cancellation, evidence, and retry semantics |
| **P4 — Scientific vertical slice** | KRN-7, KRN-9 | Research-to-Run adapters, reproducibility manifest, reviewer projection, artifact/provenance graph, generated SDK contracts | Local literature-plus-data fixture supports restart, fork, deterministic analysis, reviewer fault detection, explicit approval, and export |
| **P5 — Modularization and scale** | KRN-10 plus remaining KRN-9 | Internal crate split, feature profiles, remote provider recovery, FFI hardening, operational dashboards | Linux/macOS/Windows, offline/remote, memory/disk, security, and release qualification pass without API drift |

## 6. Dependency order

```text
P0
 └─> P1 Kernel convergence
       ├─> P2 Source + durability
       │     └─> P4 Scientific vertical slice
       ├─> P3 Model/Tool/workflow convergence
       │     └─> P4 Scientific vertical slice
       └─> P3 + P2 ──> P5 Modularization and scale
```

Do not start broad domain packages, remote compute, or Desktop graph features
before P2 establishes source and artifact identity. Do not claim scientific
reproducibility before P4's fixture and reviewer gates pass.

## 7. Measurement and rollback policy

Every phase records before/after values for:

- admission-to-first-token and Tool round-trip latency;
- event append and replay throughput;
- peak RSS, open files/sockets, queue depth, and retained bytes;
- cancellation-to-settlement and shutdown deadlines;
- index rebuild/publication time and stale-result rate;
- duplicate side effects, replay conflicts, and orphan-task count; and
- SDK wire parity and schema rejection behavior.

The proposed default performance guard is no more than 10% regression in a
qualified local profile unless the change removes a documented correctness or
security risk. A phase rolls back to its previous adapter when any invariant,
resource ceiling, or cross-platform gate fails. Compatibility adapters may
remain during one migration period, but they must emit usage metrics and cannot
become a new authority.

## 8. Explicit non-goals

- Embedding DeepSeek Harness, Cordis, or another foreign runtime.
- Turning Code into a package manager, general dependency-injection framework,
  or scientific policy authority.
- Adding a second event journal, research store, Cloud audit store, or Desktop
  session store.
- Making a specific model, browser, vector database, Cloud deployment, or HPC
  provider mandatory for local Code.
- Treating similarity, model confidence, reviewer tokens, or successful
  execution as scientific truth without evidence and human/domain policy.
- Removing a working compatibility surface before its migration and rollback
  gates are recorded.

## 9. Progress and immediate next slice

The first P1 slices are now delivered on Code `main`:

- **P1.1 Core identity and event adapter** (`f6995c73`): typed operation,
  source, capability, evidence-cursor, artifact, and logical-clock values are
  validated at the boundary; Run events and Evaluation Journal reuse the
  canonical encoding; and `ResearchEventV1` is an explicit projection rather
  than a second authority (`baeac00f`).
- **P1.2 coordinator boundary** (`ae23dff7`): blocking and streaming Agent
  paths share one internal `ExecutionCoordinator` for Run control, checkpoint
  identity, and cancellation/invocation assembly. Event forwarding and
  terminal cleanup remain mode-specific adapters until their state machines
  can be migrated without changing behavior.
- **P1.2 admission convergence** (`f6c84262`): conversation, recovery, and
  manual-save entrypoints use the coordinator for close checks, task-scheduler
  leases, single-flight admission, and detached stream supervision. The
  low-level `RunAdmission` remains a primitive; facades no longer duplicate
  admission policy.
- **P1.2 terminal convergence** (`7e968054`): blocking and streaming
  lifecycles use one cancellation-first `RunTerminalTransition`, with an
  atomic one-shot RunStore settlement. Lifecycle adapters retain only their
  mode-specific persistence, event-drain, and cleanup ordering.
- **P1.2 task-admission convergence** (`d5c03fa5`): synchronous and
  event-streaming direct-tool calls reuse the coordinator's canonical
  TaskScheduler lease/error adapter while retaining their independent
  control-plane lifetime (they do not acquire the transcript Run lease).
- **P1.2 terminal transition sink** (`e4b160a5`): the coordinator,
  cancellation handles, session close, and admission-failure paths now apply
  one typed terminal transition through the Code-owned Run store. Completed
  Runs remain finalized by the authoritative `End` event, while cancellation
  and failure retain monotonic first-terminal semantics.
- **P3/KRN-5 structured call cancellation seam** (`58670be5`): structured
  blocking, streaming, repair, auxiliary, `generate_object`, and delegated
  schema calls now accept one explicit cancellation boundary. Provider I/O
  receives a child token, while the caller-owned token remains the lifecycle
  authority; legacy helpers remain compatible and route through the same path.
- **P3/KRN-5 typed ModelCallMiddleware seam** (`35c683cd`): completion and
  structured provider calls now enter `LlmInvoker` through one typed request
  and outcome contract. Evidence-kind selection, prompt estimation, provider
  dispatch, budget, cancellation, usage, and error phases are no longer
  repeated by each non-streaming call shape; streaming retains its explicit
  proxy-receiver ownership boundary.
- **P3/KRN-5 streaming middleware seam** (`9a562e35`): streaming and
  structured-streaming setup now use typed requests/outcomes through the same
  `LlmInvoker` lifecycle. Evidence classification and prompt estimation are
  shared, while the proxy receiver remains the owner of provider cancellation
  and usage completion.
- **P3/KRN-5 ToolInvocation FSM** (`2c72c913`): built-ins, MCP, Flow, and
  Use Runtime Task dispatch now cross one explicit lifecycle state machine
  from admission through gate evaluation, execution, and a single terminal
  outcome. Rejection, cancellation, success, failure, and budget denial are
  typed without changing public Tool/ToolEnd protocol shapes.

The local qualification slice passed `cargo fmt --all -- --check`, focused
identity/research/coordinator tests, blocking and streaming lifecycle tests,
protocol Host/Harness tests, and the complete Core test suite (3106 passed,
0 failed, 13 ignored; 3119 total). The root integration PRs are #307, #308,
#309, #310, #311, #312, #313, #314, #315, and #316; coordinator integrations
follow these Code changes.

The middleware follow-up passed `cargo fmt --all -- --check`, Code compilation,
and 13 focused `agent::llm_invoker` tests. The streaming follow-up passed the
same format/compile checks and 14 focused `agent::llm_invoker` tests. The
intentionally filtered integration binaries were not rerun because the prior
complete Core suite already covers their unchanged paths.

The ToolInvocation follow-up passed `cargo fmt --all -- --check` and focused
FSM, executor-registry, and nested-governance tests (4 + 1 + 13 passed). The
full suite was not repeated because this slice only adds an internal lifecycle
guard around existing tool execution paths.

The next Code-side slice is **P3/KRN-5 trust/taint and idempotency**:

1. bind typed trust/taint labels at model/tool value boundaries and enforce
   egress/redaction at one value boundary;
2. add shared idempotency identity across model calls, Tool calls, workflow
   steps, and evaluator dispatches while preserving replay/fencing;
3. route repair, compaction, and auxiliary model calls through the typed
   middleware phases with caller-owned cancellation.

This remains an incremental refactor: no second Run store, event journal,
package manager, or foreign Harness runtime is introduced.
