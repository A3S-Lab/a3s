# A3S Workflow Platform Architecture

Status: Target integration architecture; Phases 0 and 1 verified, Phase 2 active

This document defines the target composition of A3S Form, A3S Flow, A3S Boot,
A3S ORM, and A3S Cloud. It is an integration plan, not an availability claim.
Component READMEs, roadmaps, release notes, and the Cloud compatibility lock
remain authoritative for delivered behavior and exact revisions.

## 1. Objective

Deliver one self-hosted workflow platform that supports deterministic workflow
definitions, durable execution, governed human tasks, version-pinned forms,
tenant-aware management, and production recovery without creating duplicate
engines, queues, identity stores, or persistence paths.

The first complete product slice is intentionally narrow:

```text
input -> human_decision -> output
```

An authorized user must be able to publish one form and one workflow, start a
run, claim the generated task, submit the pinned form, and observe the workflow
complete after crashes and duplicate delivery. Broader node catalogs and scale
work follow only after this path is proven.

## 2. Architecture Principles

1. **One authority per concern.** Cloud owns business state and policy, Flow
   owns durable execution history, Form owns form semantics, Boot owns task
   processing, and ORM owns relational access.
2. **Immutable inputs for every run.** A run pins exact Workflow, Plan, Form,
   capability, policy, and compiler revisions and digests.
3. **No distributed transaction.** Cloud transactions and Flow appends are
   joined by durable Outbox records, idempotent commands, expected-sequence
   writes, and recovery reconciliation.
4. **No client-trusted validation.** Cloud repeats form compilation, schema
   validation, authorization, assignment, expiry, and optimistic-version checks
   inside the protected command path.
5. **No direct browser-to-Flow control.** Browsers submit authenticated Cloud
   commands. Flow hook identities and callback tokens remain internal.
6. **Bounded durable data.** Flow history stores deterministic outputs and
   immutable references, not secrets, files, unbounded evidence, or raw audit
   archives.
7. **Gate-driven delivery.** A contract or table does not count as a product
   capability until real PostgreSQL, process-death, authorization, and recovery
   gates pass.

## 3. Component Authority

| Concern | Sole authority | Responsibilities | Must not own |
| --- | --- | --- | --- |
| Form semantics | A3S Form Core | Document profile, canonicalization, compilation, rules, validation, patches, digests, and conformance fixtures | Durable storage, tenants, assignments, actions, or authorization |
| Form presentation | A3S Form UI | Designer, Renderer, React/Vue/Web Component adapters, accessibility, and controlled host ports | Publication policy, submissions, or side effects |
| Form lifecycle | Cloud Forms context | Drafts, immutable releases, promotion, permissions, retention, submissions, and audit references | A second form compiler or browser-only validation |
| Workflow semantics | Cloud Workflow context | Definitions, revisions, goals, plans, runs, step projections, human tasks, and decisions | Flow event history, worker queues, or provider execution |
| Durable orchestration | A3S Flow | Replay, steps, timers, hooks, retries, cancellation, progress, and execution history | Tenancy, task assignment, Form storage, or product APIs |
| Application task management | A3S Boot | Queue processors, job state, retry, timeout, retention, deduplication, and shutdown | Workflow semantic state or business authorization |
| Relational persistence | PostgreSQL through A3S ORM | Parameterized SQL, transactions, typed rows, and checksummed migrations | Product policy or an alternate event authority |
| Integration facts | Cloud Outbox and A3S Event | Transactional facts, asynchronous delivery, and acceleration | Business truth or inferred completion |
| Identity and grants | Cloud Identity | Principals, memberships, resource grants, credentials, and revocation | Form-local roles or Flow-local authorization |
| Visibility | Cloud Search and projections | Authorized task and run queries, rebuildable indexes, and operational views | Mutation authority or a copied Flow transcript |

The dependency direction is one-way:

```text
Cloud Web -----------------------> A3S Form UI
                                        |
                                        v
Cloud API/Worker ------------> portable A3S Form Core
       |
       +-----------------------> A3S Flow
       |                              |
       |                              +------> A3S Boot
       |                              +------> A3S ORM (optional stores)
       +------------------------------+------> A3S ORM / PostgreSQL
```

Flow never depends on Cloud or Form. Form never depends on Cloud runtime
services or Flow. Cloud composes the released, versioned contracts.

## 4. Portable Form Semantic Core

Cloud is a Rust control plane, so it must not implement an independent Form
compiler or validator or trust a browser-produced plan. A3S Form now publishes
one portable semantic core with two build targets:

- a native Rust library for Cloud server-side compilation and validation; and
- a WASM build consumed by the TypeScript package for browser, Worker, CLI, and
  server-side JavaScript use.

The former TypeScript compiler remains test-only reference evidence. Browser,
Node, Worker, CLI, native Rust, and Cloud adapters use the same Form Core. Its
bounded versioned protocols return diagnostics, normalized document bytes,
`FormPlan`, schema profile, compiler revision, SHA-256 digests, canonical
submitted values, validation errors, and optional evaluation traces.

The pinned parity suite covers:

- canonical key ordering, Unicode, numbers, arrays, and object boundaries;
- every accepted Schema Profile 1 keyword and every rejected keyword;
- visible, enabled, computed, and validation rule graphs;
- revision and digest mismatch behavior;
- 100, 500, and 1,000-node resource limits; and
- malformed, oversized, cyclic, and adversarial documents.

The Phase 0 compatibility gate pins the exact native core revision, evaluation
protocol sources, and byte-identical interaction and submitted-value fixtures
consumed by Cloud. Durable drafts, releases, and accepted submissions remain
Cloud lifecycle responsibilities rather than Form Core storage.

Cloud product configuration remains ACL-only. A Form document is an immutable
typed asset owned by A3S Form, not a second Cloud configuration language. Cloud
ACL stores only an exact `FormReleaseRef`; the referenced Form bytes retain the
Form-owned media type, compiler revision, schema profile, and digest.

## 5. Canonical Cloud Resources

The Cloud implementation should add a `forms` bounded context and extend the
existing `workflow` context. Both follow the repository's four-layer DDD
structure and persist only through A3S ORM repositories.

### 5.1 Form resources

`FormDraft`

- Mutable aggregate with optimistic versioning.
- Stores the current Form-owned document bytes and editing metadata.
- Cannot be referenced by a published Workflow revision.

`FormRelease`

- Immutable release produced by server-side Form Core compilation.
- Stores exact normalized document bytes, plan bytes, compiler revision, schema
  profile, content digest, compatibility metadata, and actor provenance.
- Can be promoted or superseded but never updated or deleted in place.

`FormReleaseRef`

- Contains organization, project, form, release, revision, compiler revision,
  schema profile, and content digest identities.
- Is embedded by identity and digest in a Workflow human-decision
  configuration.

`FormSubmission`

- Immutable accepted submission bound to one HumanTask generation and exact
  FormReleaseRef.
- Stores bounded canonical output; files and large values use immutable object
  references.
- Stores actor, authorization decision, task version, idempotency identity,
  submission digest, and timestamp.

Invalid attempts produce redacted audit facts and diagnostics but do not become
accepted submissions.

### 5.2 Workflow resources

`WorkflowRun`

- Pins one PlanRevision and records the correlated Operation and Flow run IDs.
- Owns semantic status and current step projections, not a copy of Flow history.
- Uses a deterministic Flow run ID derived from the WorkflowRun identity.

`WorkflowStepProjection`

- Records the current semantic step state, attempt generation, exact child
  identity, result digest, and bounded evidence references.
- Is rebuildable from Workflow state, Flow history, and owning-context facts.

`HumanTask`

- Is owned by Workflow and guarded by Cloud Identity and Resource Grants.
- Pins the Workflow run, step, attempt generation, FormReleaseRef, assignment
  policy, due time, and active Flow hook identity.
- Uses optimistic versioning for claim, release, reassign, complete, expire, and
  cancel transitions.

`WorkflowDecision`

- Is immutable and records approve, reject, submit, expire, or cancel outcome.
- References the accepted submission and policy decision by exact identity and
  digest.
- Produces the bounded deterministic payload used to resume Flow.

Recommended initial state models:

```text
WorkflowRun: pending -> running -> waiting -> cancelling
             -> completed | failed | cancelled | timed_out

HumanTask: pending_activation -> ready -> claimed
           -> completed | expired | cancelled
```

Claim release and reassignment change ownership and aggregate version without
inventing extra terminal states.

## 6. Human Interaction Contract

The current Form interaction envelope must be replaced by a request-bound
contract before Cloud accepts production submissions. Validation must receive
the original request, the exact Form release, the candidate submission, current
time, and the authenticated principal.

The contract must bind at least:

- WorkflowRun, step, attempt generation, HumanTask, and Flow hook identities;
- Form URI, release revision, digest, compiler revision, and schema profile;
- assignment-policy revision, claimed principal, and task aggregate version;
- creation, due, expiry, and submission timestamps;
- submission idempotency key and canonical value digest; and
- allowed outcome and output-mapping policy.

It must reject unsupported API versions, wrong Form modes, stale task versions,
cross-run or cross-step submissions, changed digests, expired tasks, revoked
access, non-claimants, duplicate terminal decisions, and oversized values.

Data-source resolution, asynchronous validation, and actions are Cloud
application ports. A Form document stores only approved registry keys. Cloud
performs authorization, SSRF prevention, Secret resolution, rate limiting, and
audit before invoking a provider.

## 7. Consistency and Recovery

Cloud business transactions and Flow history appends are separate consistency
boundaries. The platform obtains exactly-once semantic outcomes through stable
identity and reconciliation, not a cross-system SQL transaction.

### 7.1 Starting a run

1. The authenticated Cloud command validates the pinned revisions.
2. One ORM transaction creates WorkflowRun, Operation, idempotency, audit, and
   an Outbox start request.
3. A reconciler calls `FlowEngine::start_with_id` with the deterministic run ID.
4. Repeated delivery succeeds only when WorkflowSpec and input match.
5. Cloud advances the run projection only after reading the matching Flow state.

### 7.2 Creating a human task

1. The Flow runtime reaches a stable human-decision step and invokes the Cloud
   HumanTask application port with a stable step-generation identity.
2. One ORM transaction idempotently creates `pending_activation` task state,
   audit, and Outbox facts.
3. Flow creates a stable hook whose metadata contains bounded task and digest
   identities but no PII or Secret material.
4. Reconciliation verifies the active hook and changes the task to `ready`.
5. A task is never shown as completable until both authorities agree.

### 7.3 Completing a human task

1. The Cloud command locks the task aggregate and checks principal, grant,
   claim, expected version, due time, and FormReleaseRef.
2. Portable Form Core validates and canonicalizes the submitted value.
3. One ORM transaction creates FormSubmission and WorkflowDecision, terminates
   HumanTask, and commits idempotency, audit, and an Outbox resume request.
4. The Outbox consumer resumes the exact Flow hook with a bounded decision
   payload and stable idempotency identity.
5. Late or duplicate resume is treated as the already-committed outcome only
   when every identity and digest matches.
6. Recovery projection advances WorkflowRun after observing the matching Flow
   event; missing acknowledgement remains pending and is retried.

### 7.4 Required crash points

Real PostgreSQL tests must kill the owning process after each of these points:

- WorkflowRun commit before Flow start;
- Flow start before Cloud run projection;
- HumanTask commit before hook creation;
- hook creation before task activation;
- submission and decision commit before Flow resume;
- Flow resume before WorkflowRun projection; and
- task claim or expiry racing with submission.

Every restart must converge without a second run, task, submission, decision,
hook completion, or external side effect.

## 8. Security and Data Handling

- The browser receives Cloud task identity, FormReleaseRef, document/plan, and
  scoped host capabilities, never a raw Flow callback token.
- Task listing and mutation are tenant-scoped and grant-checked at the
  application layer and repository boundary.
- Submission values are classified, encrypted where required, bounded, and
  excluded from logs, metric labels, hook metadata, and error strings.
- File uploads produce immutable object references after content, size, media
  type, malware, retention, and authorization checks.
- Revocation and last-owner protections take effect on the next command.
- Data sources and actions use typed registries; arbitrary URLs, JavaScript,
  headers, credentials, and environment injection are rejected.
- Flow observer delivery never implies authorization or business completion.

## 9. Versioning and Evolution

Every WorkflowRun pins:

- WorkflowRevision and PlanRevision;
- FormReleaseRef for each human interaction;
- Flow WorkflowSpec version and runtime build identity;
- every selected capability, connector, Agent, MCP, model, Tool, and policy
  revision; and
- the relevant protocol and compiler revisions.

Published revisions never mutate in place. A new release affects only new runs
unless an explicit, validated instance-migration command records source and
target revisions, compatibility result, actor, and rollback boundary.

Before supporting in-place production upgrades, Flow needs:

1. worker/runtime build identities and compatible-build routing;
2. deterministic patch markers for replay-safe code changes;
3. bounded history segmentation or continue-as-new semantics;
4. first-class child workflow lifecycle and cancellation propagation; and
5. named signal, query, and update contracts where hooks and snapshots are not
   sufficient.

Cloud owns migration policy and authorization. Flow owns replay enforcement.
Neither silently selects a newer revision for an existing run.

## 10. Visibility and Operations

Cloud should expose authorized, rebuildable projections for:

- Workflow definitions, releases, runs, and current step state;
- task inboxes, candidate and claimed tasks, due times, and SLA breaches;
- Form releases and accepted submissions;
- Flow correlation, open suspensions, retry state, and terminal outcomes; and
- bounded audit, diagnostic, and evidence references.

The first observability contract must correlate request ID, organization,
project, WorkflowRun, Operation, Flow run, step generation, HumanTask,
submission, and decision identities. High-cardinality identity belongs in logs
and traces, not metric labels.

Required metrics include command latency, run-start lag, runnable backlog,
scheduled-wakeup lag, task activation lag, submission-to-resume lag, retry and
dead-letter counts, replay non-determinism, SLA breaches, and terminal outcome
counts. OpenTelemetry spans and metrics must cross Cloud, Flow, Boot, and owning
step providers without copying business payloads.

## 11. Development Plan

Implementation order, verification gates, workstream dependencies, and release
sequencing are defined in the companion [Workflow platform development
plan](workflow-platform-development-plan.md).
