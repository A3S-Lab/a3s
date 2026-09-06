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
- **P3/KRN-5 trust/taint and execution identity** (`a22bea94`): model and
  Tool request boundaries now derive replay-stable, domain-separated
  identities; typed trust, taint, and sanitization labels travel with values,
  and model/tool/task/command/event egress uses one redaction boundary without
  promoting untrusted or secret data.
- **P3/KRN-5 identity propagation and ledger adapters** (`eee3ec95`): Flow
  and evaluation dispatches now carry one typed `ExecutionClaimV1` through
  claim, renewal, completion, and release while retaining legacy ledger keys
  for replay compatibility. Canonical semantic identities are validated and
  traced without persisting prompt, tool-argument, or instruction plaintext.
- **P3/KRN-5 evidence-bound claim results** (`f8cf19d7`): evaluation dispatch
  claims now persist a bounded `ExecutionResultReceiptV1` containing canonical
  execution identity, evidence snapshot digest, terminal outcome, and an
  optional result digest/byte count. Identity-aware claim, renewal, release,
  and completion operations fence stale workers while legacy ledger keys and
  old receipt files remain replay-compatible.
- **P3/KRN-5 workflow result convergence** (`c48363a0`): resumable workflow
  checkpoints now carry bounded workflow-step result receipts bound to
  canonical step specifications; changed step identities, tampered outputs,
  expired workers, and unreadable checkpoints fail closed, while old
  checkpoints without receipts remain loadable. Flow decision claims use the
  same identity-aware claim/renew/release/complete boundary and persist a
  digest-only decision receipt.

The local qualification slice passed `cargo fmt --all -- --check`, focused
identity/research/coordinator tests, blocking and streaming lifecycle tests,
protocol Host/Harness tests, and the complete Core test suite (3106 passed,
0 failed, 13 ignored; 3119 total). The root integration PRs are #307, #308,
#309, #310, #311, #312, #313, #314, #315, #316, #317, #318, and #319;
coordinator integrations follow these Code changes.

The middleware follow-up passed `cargo fmt --all -- --check`, Code compilation,
and 13 focused `agent::llm_invoker` tests. The streaming follow-up passed the
same format/compile checks and 14 focused `agent::llm_invoker` tests. The
intentionally filtered integration binaries were not rerun because the prior
complete Core suite already covers their unchanged paths.

The ToolInvocation follow-up passed `cargo fmt --all -- --check` and focused
FSM, executor-registry, and nested-governance tests (4 + 1 + 13 passed). The
full suite was not repeated because this slice only adds an internal lifecycle
guard around existing tool execution paths.

The trust/taint and identity follow-up passed `cargo fmt --all -- --check`,
`cargo check -p a3s-code-core`, and focused tests for execution identity (3),
security boundaries (32), model middleware (14), ToolInvocation (5), and task
projection (118). The complete suite was not repeated because the changes are
boundary-local and the prior complete Core qualification remains valid.

The identity propagation follow-up passed `cargo fmt --all -- --check`,
`cargo check -p a3s-code-core`, and focused Flow dispatcher (14), evaluation
supervisor (5), and ExecutionIdentity (4) tests. The legacy Flow and
evaluation ledger keys remain unchanged, so historical receipts and replay
behavior stay compatible.

The evidence-bound result follow-up passed `cargo fmt --all -- --check`,
`cargo check -p a3s-code-core`, the complete evaluation module (49), Flow
dispatcher (14), evaluation supervisor (6), ledger fencing/receipt (2), and
execution identity (5) tests. Result receipts contain digests and bounded byte
counts only; legacy claim records deserialize without the new optional fields.

The workflow result convergence follow-up passed `cargo +stable fmt --all
-- --check`, `cargo +stable check -p a3s-code-core`, the complete Core library
suite (3153 passed, 0 failed, 13 ignored), Flow decision and ledger tests (18),
orchestration tests (42), persisted-schema integration tests (11), file-store
receipt restart coverage (1), and all integration-target compilation. The
workflow and Flow paths retain digest-only receipts, reject stale or corrupt
resumable state before executing steps, and keep legacy checkpoint/ledger JSON
loadable.

The scheduler and plan convergence follow-up landed in Code `c21c49b3`
(`A3S-Lab/Code#93`). Dynamic Flow history now projects into the canonical
`ExecutionPlan`, including steps that predate a resumed observer; plan identity
excludes mutable progress, and duplicate lifecycle delivery cannot regress a
terminal status. Flow step bodies use a cancellation-aware per-workflow gate,
standalone adapters can layer the agent-wide priority scheduler with a
digest-only step identity, and delegated tasks carry their canonical identity
through the same admission boundary. Local qualification passed the complete
Core library suite (3162 passed, 0 failed, 13 ignored), all integration-target
compilation, `--all-features` checking, documentation tests, and focused Flow,
scheduler, planning, task, and QuickJS coverage. Session-bound workflows keep
the enclosing lease so a max-active=1 scheduler cannot deadlock on nested task
fan-out.

The mixed-generation Code-side slice landed in Code `e6e794e1`
(`A3S-Lab/Code#94`):

1. new dynamic workflow runs pin the exact Code runtime build in the durable
   Flow `WorkflowSpec`, while an explicit compatibility set can retain an
   older worker and the bounded migration window still reads legacy unpinned
   histories;
2. Code reconstructs a digest-only continuation identity from the persisted
   Run/step definitions, source hash, input digest, runtime build, retry
   policy, and canonical plan identity, rejecting changed source/input,
   duplicate or malformed definitions, and non-contiguous history before Flow
   can admit a step body; and
3. terminal replay, changed-generation reuse, and legacy unpinned replay are
   covered by local qualification, including the no-duplicate-side-effect
   boundary.

The parent-cancellation and worker-takeover Code-side slice landed in Code
`22977bc4` (`A3S-Lab/Code#95`):

1. each dynamic workflow continuation derives one stable, digest-only claim
   identity that excludes evolving plan progress and retry history;
2. local workspaces use an atomic `.a3s/workflow/leases` sidecar, while remote
   hosts can inject a shared `FlowDecisionLedger`; live workers heartbeat and
   stale owners are fenced before workflow/step admission and completion;
3. parent cancellation propagates through the child `ToolContext`, waits for
   bounded settlement, releases only after execution stops, and leaves an
   unsettled lease fenced until expiry; and
4. generated run IDs, competing workers, expired-owner fencing, cancellation
   at a retry boundary, pre-admission lease loss, and digest-only lease records
   are covered by focused qualification plus the complete Core test suite.

The durable cancellation and cross-process recovery slice landed in Code
`21dfabf5` (`A3S-Lab/Code#96`):

1. `DynamicWorkflowControl` is the single host-facing boundary for bounded,
   redacted inspection, trusted history, driving, and request/force
   cancellation; it uses the same Flow terminal transition and worker lease as
   model-visible workflow execution;
2. `CrossProcessFlowEventStore` protects the local JSONL adapter with a
   process lock, while typed `with_flow_event_store` injection and the
   registration helper support a host-owned remote/database store without
   creating a shadow journal; and
3. independent-process qualification kills an owner, waits for lease expiry
   and takeover, verifies busy inspection and cancellation settlement, and
   injects event conflicts to prove bounded retry and convergence.

The scheduler fairness and bounded control observability slice landed in Code
`e4772cc0` (`A3S-Lab/Code#97`):

1. the agent-wide scheduler keeps the established occupancy `stats` wire shape
   and adds an actor-owned health projection with bounded admitted, released,
   cancelled, rejected, aging-promotion, peak-occupancy, and wait-time
   counters; health reads also evaluate elapsed aging so the observed fairness
   state cannot lag behind an eligible promotion;
2. dynamic workflow tools and control handles share a process-local,
   payload-free metrics block for claim attempts, busy/conflict outcomes,
   takeovers, lease renewal/loss, terminal settlement, cancellation, and
   in-flight claims; `DynamicWorkflowControl::diagnostics` composes those
   counters with optional scheduler health without creating a second journal or
   workflow authority; and
3. resumed-workflow admission under sustained newer interactive arrivals is
   covered by a starvation qualification, while typed Agent/Session surfaces
   expose scheduler health consistently through Node, Python, and Go adapters.
   Local qualification passed the complete Core library suite (3178 passed,
   13 ignored), locked Node/Python binding checks, Go SDK and bridge tests, and
   TypeScript declaration checks.

The per-run quota and admission-identity slice landed in Code `ade9dce4`
(`A3S-Lab/Code#98`):

1. the existing scheduler actor now accepts a validated
   `TaskSchedulerQuota`, retains only a domain-separated digest and live
   active/pending counters, skips a quota-blocked owner when another owner can
   use global capacity, and prunes idle owner state;
2. governed `ToolContext` values carry the exact run identity into detached
   Task admission, while direct globally admitted Flow steps use the stable
   continuation claim identity; the two boundaries remain explicit so nested
   fan-out never reacquires an outer single-slot lease; and
3. host diagnostics expose a live quota projection, with adversarial coverage
   for mixed-owner progress, priority blocking, cancellation, malformed and
   overlong scopes, identity/limit conflicts, idle-state pruning, dynamic
   diagnostics, and detached run fan-out.

Local qualification passed `cargo +stable check -p a3s-code-core
--all-features`, strict clippy, the complete Core library suite (3185 passed,
0 failed, 13 ignored), and documentation tests (8 passed, 6 ignored). The
Code PR was merged without waiting for long-running hosted soak checks, per
the development-efficiency policy; the local gates are recorded above.

The provider-aware model-generation admission slice landed in Code
`d75b88f0` (`A3S-Lab/Code#99`) and is pinned by this repository's `crates/code`
gitlink:

1. provider adapters can publish a typed, digest-only `ModelGenerationPool`
   keyed by provider/model/endpoint origin and an optional non-secret account
   scope; credentials, URL paths/queries, prompts, outputs, and transport
   headers never enter scheduler state;
2. regular, streaming, structured, repair, direct-tool, delegated-child, and
   dynamic-workflow generations reserve the pool through quota-only admissions
   in the existing priority scheduler, while the owning Run still consumes its
   global orchestration slot; local client limits and shared provider limits
   are intersected without introducing a second queue or authority; and
3. stream EOF/terminal/cancellation/drop, structured repair, nested workflow
   limits, and mixed-provider progress release reservations deterministically,
   including the max-active=1 nested-call boundary.

Local qualification passed strict formatting, `--all-features` Clippy, rustdoc,
focused multi-quota/provider/stream lifetime tests, and the complete Core
library suite (`3200 passed, 0 failed, 13 ignored`). The Code PR was merged
without waiting for long-running hosted checks under the development-efficiency
policy; provider rate limits, billing, and host policy remain outside Code.

The provider-pool health hardening slice landed in Code `386d75b1`
(`A3S-Lab/Code#100`) and is pinned by this repository's `crates/code` gitlink:

1. the existing scheduler actor exposes `TaskScheduler::quota_health`, retaining
   at most 64 recent idle digest-only quota epochs with admission, release,
   cancellation, rejection, peak-occupancy, and bounded wait counters;
2. `ModelGenerationPoolHealthSnapshot` composes that shared projection with
   local reserved/available permits, and `AgentSession::model_generation_pool_health()`
   provides a host-facing Rust read surface; nested runtime rebinds preserve
   the exact provider pool identity while allowing tighter local limits; and
3. focused qualification covers idle retention, cancellation, provider-pool
   isolation, hard retention bounds, credential/routing redaction, and nested
   rebind composition without adding a queue, store, or foreign runtime.

Local qualification passed formatting, `--all-features` check, strict Clippy,
rustdoc, and all focused health tests. The full Core run reached 3,204 passed,
one pre-existing BM25/zvec native-index failure, and 13 ignored; no provider
pool-health test failed. The Code PR was merged without waiting for long-running
hosted checks under the development-efficiency policy. Provider rate limits,
billing, and host policy remain outside Code.

The cross-host qualification slice landed in Code `c4ee7d25`
(`A3S-Lab/Code#101`) and is pinned by this repository's `crates/code` gitlink:

1. Node.js, Python, and Go sessions expose the same read-only
   `ModelGenerationPoolHealthSnapshot`; the versioned Go JSONL bridge advertises
   `session_model_generation_pool_health`, and capability discovery lists it
   before a host opts into the diagnostic surface;
2. an eight-cycle noisy-neighbor qualification with twelve blocked waiters per
   cycle proves an independent provider pool continues to admit work under a
   full global budget while cancellation, release, and bounded retention
   counters settle; and
3. local qualification passed Core strict Clippy, all three host binding
   `--all-features` checks, Go SDK tests, Go bridge tests, and the focused
   noisy-neighbor test. Node TypeScript runtime checks remain a package-local
   follow-up when dependencies are installed; no second scheduler or metrics
   store was introduced.

The BM25 replacement qualification correction landed in Code `a471e944`
(`A3S-Lab/Code#102`) and is pinned by this repository's `crates/code` gitlink:

1. the persistent replacement regression now rejects the old source content,
   rather than rejecting a valid result from the same path after the catalog
   has published the replacement generation;
2. the test continues to cover the reconciliation race and native source
   verification boundary without changing the zvec-grep integration or adding
   another index authority; and
3. ten repeated parallel BM25 suites passed locally after the correction.

The earlier full-suite BM25 failure was therefore a false-positive assertion,
not stale content escaping source verification.

The reviewer composition binding landed in Code `e4d7e286`
(`A3S-Lab/Code#103`), and its bounded response batch landed in Code `2c6c1d17`
(`A3S-Lab/Code#104`); the latter is now pinned by this repository's
`crates/code` gitlink. `ResearchReviewFindingV1::bind_evaluation_record` links
a finding to one immutable `EvaluationRecordV1`, verifies the evaluator and Run
identities, requires the evaluator's evidence snapshot digest, and keeps the
finding `open` until host policy or a human explicitly resolves or waives it.
`ResearchReviewBatchV1` prevents partial or mixed responses by requiring one
project/Run/evaluation-record/evidence identity, canonical finding order, and a
digest update after each explicit resolution. Legacy unbound v1 findings retain
their prior digest identity; bound findings include the evaluator record digest,
preventing result replacement from masquerading as the same review observation.

The research execution identity hardening landed in Code `91d4e3cf`
(`A3S-Lab/Code#105`). Hosts can validate a `ResearchRunV1` against the exact
admitted `ExecutionTargetV1`, and `ResearchEventV1::from_core_event_for_run`
requires the bare Run id instead of guessing it from an opaque operation id.
This closes a cross-Run projection ambiguity before evidence or reviewer facts
are published; the host adapter still owns source capture, artifact storage,
and evaluator policy.

The P3/KRN-8 host fixture qualification landed in Code `7e4af9fb`
(`A3S-Lab/Code#106`) and is now pinned by this repository's `crates/code`
gitlink. The versioned `sdk/evaluation/model-generation-pool-health-v1.json`
contract is consumed by the public Node.js, Python, and Go Session surfaces;
each adapter checks digest-only identity, local reservation conservation,
shared/local capacity bounds, a bounded aggregate field set, and recursive
redaction. Go also has an opt-in real Rust JSONL bridge check. The default
fixture uses an unreachable endpoint and sentinel credential, so it makes no
provider request and cannot turn a health read into a billing operation. Core
admission tests remain the authority for active, cancelled, released, and
retained scheduler epochs; no second scheduler or metrics store was added.
Flow remains the sole lease authority and Gateway/hosts own rate limits and
billing.

The RESEARCH-EXEC1 qualification landed in Code `25039b42`
(`A3S-Lab/Code#107`) and is now pinned here. The
`research_execution_qualification` integration fixture drives one research
Run through exact Code/Use binding, contiguous evidence, Run-aware event
projection, checkpoint serialization/restart, terminal cancellation,
create-only content-addressed artifact replay, and file-backed evaluator
dispatch/result recovery. It also projects a bound, still-open reviewer batch;
review policy and human disposition remain host-owned.

Reviewer provenance binding landed in Code `3a085232` (`A3S-Lab/Code#108`) and
is now pinned here. `ResearchReviewFindingV1::bind_provenance_receipt` binds a
finding to the exact project, Run, artifact digest, and retained input evidence
used by its reproducibility receipt. The optional field preserves legacy
finding identities; once present, the receipt digest is part of the finding
identity and cannot drift without producing a new observation. Code still does
not own reviewer rubrics, thresholds, approval, or publication policy.

Reviewer location validation landed in Code `79a2300c`
(`A3S-Lab/Code#112`), with zero-column coverage in follow-up `e797f69b`
(`A3S-Lab/Code#113`), and is now pinned here. Finding anchors remain bounded
text, while optional line/column coordinates are one-based and a column must be
line-bound; malformed positions fail before a finding can be published or
rebound.

Run-aware reviewer provenance fencing landed in Code `872b3173`
(`A3S-Lab/Code#115`, following `A3S-Lab/Code#114`) and is now pinned here.
Hosts can pass the admitted `ResearchRunV1` to
`bind_provenance_receipt_for_run`; Code then rejects a provenance receipt from
another project revision, provider, or random seed in addition to project, Run,
artifact, and evidence-input mismatches. The object-only compatibility binding
remains available for older integrations, while the qualification fixture now
covers all three reproducibility-drift rejections.

Run-aware evaluator binding landed in Code `9f420e87`
(`A3S-Lab/Code#116`) and is now pinned here. Hosts can pass the admitted
`ResearchRunV1` to `bind_evaluation_record_for_run`; Code verifies the finding
and Run project namespace before accepting an evaluator record, closing the
cross-project Run-id reuse case that the legacy object-only binding cannot
observe. The qualification fixture covers this rejection alongside the
provenance checks.

Run-aware review-batch validation landed in Code `883f0fff`
(`A3S-Lab/Code#117`) and is now pinned here. Hosts can construct or validate a
`ResearchReviewBatchV1` with the admitted Run and exact evaluator record;
Code closes the batch project/Run namespace and rejects a batch evidence
digest that differs from the evaluator record. The legacy digest-only
constructor remains available for wire compatibility.

The create-only artifact-store boundary landed in Code `8bf60f34`
(`A3S-Lab/Code#109`) and is now pinned here. `ArtifactStore::put_content_addressed`
makes immutable replay explicit: exact writes are idempotent, URI/content or
metadata collisions fail closed, manifest reopen rejects conflicting duplicate
entries, and the Tool artifact fallback uses the same operation. The mutable
`put` API remains available for compatibility cache callers; retention and
eviction policy remain in the store rather than being duplicated in Core.

Persistent lexical index integrity fencing landed in Code `d68bdc4`
(`A3S-Lab/Code#110`), with strict schema-v2 field rejection in follow-up
`a16210b2` (`A3S-Lab/Code#111`), and is now pinned here. Schema-v2 generations
retain and verify per-chunk text digests, stable chunk IDs, canonical source
digests, valid ranges, duplicate-ID constraints, and closed manifest fields
before exposing a reopened native index. Corrupt or incompatible generations
fail closed and are rebuilt from the current catalog; no stale or partially
trusted postings are served.

This remains an incremental refactor: no second Run store, event journal,
package manager, or foreign Harness runtime is introduced.
