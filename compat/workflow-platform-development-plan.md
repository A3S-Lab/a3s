# A3S Workflow Platform Development Plan

Status: Phases 0 and 1 verified; Phase 2 is the active delivery stage

This plan implements the boundaries and consistency model in the [A3S Workflow
Platform Architecture](workflow-platform-architecture.md). It is gate-driven;
phase numbers define dependency order, not availability dates.

## 1. Delivery Policy

- Start with one complete human-decision workflow before expanding the node
  catalog.
- Land domain invariants, persistence, APIs, clients, recovery evidence, and
  documentation together for each Cloud slice.
- Release Form and Flow contracts before Cloud pins them.
- Keep unsupported or unverified paths explicitly unavailable.
- Do not publish an integration claim until the exact root compatibility lock
  and Cloud contract fixtures pass from a clean checkout.

## 2. Ordered Development Phases

### Phase 0: Integration baseline and contract freeze

Deliverables:

- Upgrade Cloud to the released Flow baseline that includes A3S Boot task
  management and A3S ORM-backed SQL stores.
- Freeze portable Form Core input/output and cross-runtime golden fixtures.
- Freeze request-bound Form interaction, HumanTask, Submission, Decision, and
  Flow-resume contracts.
- Publish exact Form and Flow revisions before Cloud consumes them.
- Add Form and the new protocol levels to the root compatibility gate once the
  component has an immutable release or registered gitlink.

Exit gate:

- Browser, Node, native Form Core, and Cloud return byte-identical normalized
  output, digest, and diagnostics for the conformance corpus.
- Cloud builds and tests against exact released dependencies.
- The clean root compatibility gate passes with no revision drift.

Verified on 2026-08-09 by compatibility-lock digest
`sha256:c990b4d84667b37c32ffcf6e198ea0e8ab773e5927407308a3a94426b7f24068`.
The gate pins Form `8d73dba5e88ded0de7ae0e1c7b1e599a5d9134de`, Cloud
`db556223af6dfc964cac0ef45519972a50039d2f`, Flow `0.11.0`, Boot `0.1.4`,
and ORM `0.2.0`; it verifies both Form interaction and submitted-value
evaluation fixtures byte for byte.

### Phase 1: Cloud Form lifecycle

Deliverables:

- Add the Forms DDD module, ORM migrations, repositories, commands, and queries.
- Implement draft create/get/list/update and immutable publish/get/list.
- Compile and validate every publish on the server.
- Expose the same handlers through REST/OpenAPI, maintained client, CLI, and
  applicable Management MCP tools.

Exit gate:

- Optimistic conflicts, replayed idempotency, unauthorized access, malformed
  documents, compiler failure, process death, release immutability, and
  cross-tenant isolation pass against PostgreSQL 17.

Verified on 2026-08-09 by compatibility-lock digest
`sha256:a3219a0c6fc43b8d9559a613945eeee48edb19fb6ff8f46d2e960a2f01e4bb9e`.
The gate pins Cloud `c99ac0dc573fee12a7757e11c6800728f15244f0` and verifies
project-scoped canonical drafts, immutable native owner-compiled releases,
optimistic aggregate versions, idempotent replay, tenant isolation, audit and
Outbox writes, and one CQRS authority across REST `1.13.0`, the maintained
TypeScript client, CLI, and seven Management MCP tools against PostgreSQL 17.

### Phase 2: Minimal WorkflowRun execution

Deliverables:

- Persist WorkflowRun and WorkflowStepProjection through A3S ORM.
- Compile PlanRevision into one exact Flow WorkflowSpec.
- Execute input, transform, branch, and output steps.
- Add start, list, get, wait, cancel, output, and bounded history surfaces.
- Correlate every run with one Operation and one Flow run.

Exit gate:

- A deterministic workflow survives API and worker death at every commit
  boundary and produces one terminal output.
- Cancellation, timeout, duplicate start, replay drift, and invalid plan
  references fail explicitly.

The current implementation baseline was published on 2026-08-10 by
compatibility-lock digest
`sha256:9667805dd2d5327db19b7f5f56cecbe5fd86972ed493d06eb7f0598b0f05c57a`.
The gate pins Cloud `ae9f145e322315e45d0bf1d1239caf7a347e9e9d`, Flow
`0.11.0`, Boot `0.1.4`, and ORM `0.2.0`. Migration `080` atomically persists
the Goal/Plan-bound WorkflowRun, Operation, semantic step projections,
idempotency, audit, and Outbox records through A3S ORM/PostgreSQL. One A3S Flow
run executes Workflow-local `input`, `transform`, `branch`, and `output` steps;
REST `1.14.0`, the maintained TypeScript client, CLI, and Management MCP share
the same lifecycle. Phase 2 remains active until the named API/worker
process-death and expanded cross-surface evidence is complete. This baseline
does not claim protected Form submission or HumanTask availability.

### Phase 3: Human-task vertical slice

Deliverables:

- Add HumanTask, FormSubmission, and WorkflowDecision persistence.
- Bind `human_decision` configuration to an exact FormReleaseRef.
- Implement ready, claim, release, complete, reject, expire, and cancel paths.
- Add a minimal Cloud task inbox and embed the controlled A3S Form Renderer.
- Resume Flow through Outbox delivery and recovery reconciliation.

Exit gate:

- The `input -> human_decision -> output` slice passes real browser, API,
  PostgreSQL, Boot worker, Flow, duplicate delivery, process-death, expiry,
  revocation, stale-version, and cross-tenant tests.
- No browser path can complete Flow without a committed Cloud decision.

This is the first milestone that may claim an end-to-end human workflow.

Current implementation status on 2026-08-13: compatibility-lock digest
`sha256:9bab802156275a29c2cdddc4d7fd57a8a17188e7d7ef4cb5e763b0d8675cfa83`
pins Cloud `b3b5a823eaa1a82bef6dd98abc6d6c85957241a9`, which persists the internal
authority-bound HumanTask decision loop and exposes protected list/detail,
versioned claim/release, and exact native A3S Form submission through REST
`1.23.0`, the maintained TypeScript client, CLI, and Management MCP. Form
remains the sole semantic evaluator; Identity re-reads current credential,
membership, and Resource Grant authority and records immutable decision
evidence in the existing audit store. Workflow atomically persists the
decision reference, Form submission, idempotency result, audit, Outbox, and
Flow-resume delivery. Deadline expiry now derives one exact authority-bound
terminal decision from the immutable Run, Plan, task, and Flow timeout evidence
and settles it through the same Outbox/resume worker without another scheduler
or terminal ledger. Parent cancellation now uses that same coordinator,
decision transaction, and resume Outbox; cancellation candidates preempt
expiry, bind the exact cancelling Principal and Flow
`RunCancellationRequested`/`RunCancelled` pair, and settle from an immutable
`RunCancelled` receipt. A controlled task inbox and renderer, the named clean
cross-surface evidence, and end-to-end availability remain open, so the Phase 3
exit gate is not yet met.

### Phase 4: Enterprise human work and Form governance

Deliverables:

- Candidate users and groups, claim policies, reassign, delegate, substitute,
  due dates, calendars, reminders, escalation, and separation-of-duty rules.
- Comments, immutable attachments, review evidence, and notification delivery.
- Complex nested forms, repeatable groups, wizards, file/signature extensions,
  draft/release diffs, rollback, promotion, and migration tooling.
- Tasklist filtering, bulk-safe operations, audit queries, and operator views.

Exit gate:

- Approval, onboarding, order-entry, and inspection reference workflows pass
  assignment, accessibility, recovery, retention, and security suites.

### Phase 5: Typed executable steps

Deliverables, in dependency order:

1. finite Execution steps;
2. versioned business Service connectors;
3. Subworkflow steps with exact child identity;
4. Agent and Tool steps;
5. admitted MCP and model steps; and
6. compensation and bounded evidence references across owning contexts.

Each adapter calls the owning application port. Workflow never writes another
context's tables, starts Runtime directly, or stores provider credentials.

Exit gate:

- Each step type has idempotency, timeout, cancellation, retry, compensation,
  process-death, stale-receipt, authorization, cleanup, and conformance evidence
  against its exact provider revision.

### Phase 6: Safe evolution and workflow visibility

Deliverables:

- Flow runtime build routing, patch markers, history segmentation,
  continue-as-new, and first-class child lifecycle.
- Cloud run migration policy, authorized search attributes, instance migration,
  replay diagnostics, schedules, statistics, and operator repair commands.
- Named signal/query/update contracts only after their consistency and
  authorization boundaries are frozen.

Exit gate:

- Mixed-version workers, long histories, rolling upgrades, migration, rollback,
  replay, child loss, and repair pass without non-deterministic completion.

### Phase 7: Production scale and disaster recovery

Deliverables:

- Multi-node placement, Cloud role HA, worker partitioning, leader fencing,
  quotas, rate limits, admission control, and backpressure.
- PostgreSQL and object backup/restore, retention, corruption handling, and
  disaster-recovery runbooks.
- End-to-end OpenTelemetry, SLOs, capacity models, load tests, soak tests, and
  mixed-version upgrade evidence.

Exit gate:

- Node, worker, API, database connection, and availability-zone failures meet
  published recovery and durability limits without duplicate business effects.
- Restore reconstructs Cloud authorities, Flow histories, task inboxes, and
  immutable Form references with verified digests.

## 3. Parallel Workstreams and Dependencies

| Workstream | May start after | Primary output |
| --- | --- | --- |
| Form portable core | Plan approval | Native/WASM semantic authority and fixtures |
| Flow/Cloud compatibility | Released Flow baseline | Exact pins and cross-repository gate |
| Cloud Forms context | Portable Form contract freeze | Draft/release persistence and APIs |
| Cloud WorkflowRun | Flow compatibility baseline | Durable semantic run lifecycle |
| Human-task integration | Form lifecycle and WorkflowRun | First end-to-end product slice |
| Cloud Web task experience | Stable task/query contracts | Task inbox, Form rendering, run view |
| Typed capability steps | WorkflowRun and owning provider gates | W0.4 execution breadth |
| HA and recovery | Stable semantics and operational metrics | W0.5/H0/S0 production evidence |

Phase 1 and Phase 2 may proceed in parallel after Phase 0. Phase 3 requires both.
Phase 4 and Phase 5 may then proceed in parallel. Phase 6 requires stable run
and provider semantics. Phase 7 closes only after the earlier recovery gates.

## 4. Verification Matrix

Every completed phase must cover the applicable rows:

| Area | Required evidence |
| --- | --- |
| Contracts | Golden fixtures, unsupported-version rejection, digest parity, size limits, and compatibility decision |
| Persistence | Real PostgreSQL 17, ORM-only access, migrations, rollback, idempotency, optimistic conflicts, and corruption tests |
| Recovery | Process death at every commit/acknowledgement boundary, replay, duplicate delivery, lost events, and reconciliation |
| Security | Authentication, grants, revocation, cross-tenant denial, token redaction, SSRF, Secrets, PII, and file handling |
| Execution | Timeout, cancellation, retry exhaustion, compensation, stale receipts, cleanup, and provider loss |
| UX | Keyboard, screen reader, responsive layout, stale state, loading, empty, error, retry, and concurrent edits |
| Operations | Logs, traces, metrics, alert thresholds, backlog visibility, backup, restore, upgrade, and runbooks |

Tests must leave no files, tasks, hooks, sockets, or temporary databases behind.
Unverified capability must fail explicitly rather than degrade to an in-memory
or browser-only path.

## 5. Release and Compatibility Order

1. Merge and release the owning Form or Flow contract implementation.
2. Record immutable revision, package version, protocol level, and checksum.
3. Update Cloud exact dependencies, Cargo lock, migrations, contract fixtures,
   client, and documentation together.
4. Pass Cloud focused, PostgreSQL, process-death, and cross-surface gates.
5. Update the root gitlinks and `compat/cloud-stack.acl` together.
6. Run `just cloud-stack-check` from a clean recursive checkout.
7. Publish the integration claim only after the exact lock passes.

Form is a registered compatibility component. Cloud may advertise only the
HumanTask slices whose exact lock and evidence have passed; source-level
adapters and demos remain development evidence only.

## 6. Claim Boundaries

- A Form Renderer is not a HumanTask service.
- A Flow hook is not an assignment, authorization, or Tasklist system.
- A persisted PlanRevision is not an executable WorkflowRun.
- An Outbox fact is not proof that Flow accepted a resume.
- A rebuilt Search projection is not mutation authority.
- Unit coverage is not process-death or production-scale evidence.
- The platform must not claim broad industry parity until Phases 0 through 7
  pass their exact gates.
