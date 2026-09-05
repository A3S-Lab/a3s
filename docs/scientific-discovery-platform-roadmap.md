# A3S Scientific Discovery Platform Roadmap

- Status: accepted product direction; implementation is staged
- Decision date: 2026-09-05
- Owners: A3S Code, A3S Use, and A3S Desktop
- Code architecture follow-up: [A3S Code Core Optimization Roadmap](a3s-code-core-optimization-roadmap.md)
- Related: [Native Code/Use boundary](https://github.com/A3S-Lab/Use/blob/main/docs/adr-003-native-code-harness-boundary.md), [Code evaluation substrate](https://github.com/A3S-Lab/Code/blob/main/manual/EVALUATION_SUBSTRATE.md), [Desktop product contract](../apps/desktop/DESIGN.md), [Science catalog](../packages/science/README.md)

## Product outcome

A3S Scientific Discovery is a local-first, reproducible research workbench.
It lets a researcher move from a question to an evidence-backed, validated,
and publishable result while keeping the source data, code, environment,
artifacts, decisions, and review history inspectable.

The product is not a single autonomous scientist. It is a governed system in
which an agent can plan and execute bounded work, domain packages contribute
validated methods, and a researcher can inspect, approve, revoke, fork, or
reproduce every consequential step.

The target workflow covers:

```text
question / hypothesis
  → source and dataset admission
  → evidence and claim graph
  → experiment or analysis plan
  → reproducible workflow execution
  → figures, tables, and model artifacts
  → independent validation and reviewer pass
  → manuscript/report publication
  → reproducible export and follow-up
```

Claude Science is a useful product reference because it combines domain
connectors, specialist agents, reproducible artifacts, managed compute, and a
separate reviewer. A3S adopts the product requirements, not its runtime or
APIs; the A3S implementation remains native Code + Use + Desktop.

## First principles and ownership

1. **One fact has one authority.** Code owns execution facts, Use owns package
   state, Desktop owns presentation state, and Cloud/providers own external
   business or infrastructure state.
2. **Evidence precedes synthesis.** A claim, number, figure, or conclusion is
   publishable only when its inputs and transformations have inspectable
   evidence or an explicit unresolved-gap record.
3. **Reproducibility is an identity, not a promise.** Inputs, source revisions,
   workflow, code, environment, model, random seeds, provider, and outputs are
   content-addressed and bound to one provenance receipt.
4. **Automation has a ceiling.** Agents and packages receive only the
   workspace, data, compute, network, and write authority explicitly granted
   for the current project and run.
5. **Research state is durable.** A project is more than a transcript. Sources,
   claims, workflows, artifacts, findings, decisions, and revisions survive a
   process restart and can be forked without mutating the parent.
6. **Native contracts over compatibility.** A3S Code does not emulate DSH or
   another foreign Harness. A3S Use remains a package manager, not a generic
   runtime or agent loop.
7. **Local-first, portable later.** A project works offline with local files
   and providers; remote compute, collaboration, and Cloud storage are typed
   provider extensions, not hidden prerequisites.

## Responsibility model

| Component | Owns | Must not own |
| --- | --- | --- |
| **A3S Code** | Native agent loop, research run scopes, model/tool calls, bounded context, workspace and retrieval, evidence facts, artifact references, evaluator execution, checkpoint/recovery, and provider-neutral research projections | Package installation, SemVer resolution, package trust, domain rubric authority, Cloud business lineage, or UI state |
| **A3S Use** | Signed scientific packages, catalog/lock/dependency closure, Grants, environment and provider requirements, installation generations, capability projection, lease/drain, upgrade/rollback, and package recovery | Agent orchestration, model prompts, scientific conclusions, data interpretation, or direct provider execution |
| **A3S Desktop** | Project/workbench UX, source and artifact inspection, notebook/report editing, review decisions, approval/revocation, visualizations, accessibility, and localized presentation | Filesystem/package authority, browser-supplied evidence, hidden permissions, or a second run/session store |
| **Flow/Runtime/Box/Cloud/Knowledge providers** | Workflow execution, Task/Service lifecycle, isolation, remote placement, durable external objects, and knowledge publication through typed ports | Rewriting Code evidence, selecting package trust, or silently changing project semantics |
| **Science catalog/packages** | Domain skills, connectors, parsers, validators, workflows, visualizers, and reference material distributed as signed Use packages | Ambient credentials, arbitrary package build scripts, unreviewed native code, or mutable latest-version behavior |

## Platform architecture

```text
Desktop Research Workbench
  project · sources · evidence · notebook · runs · artifacts · review · publish
             │ authenticated native API / bounded event stream
             ▼
A3S Code Research Kernel
  project projection · Agent scopes · tools · retrieval · evidence · evaluators
             │ exact capability snapshot + leased provider bindings
             ▼
A3S Use Capability Plane
  signed package graph · environment lock · Grants · generation · cutover
             │ typed provider ports
             ▼
Runtime / Flow / Knowledge / Box / Cloud / external scientific systems
```

The research project is the product aggregate. It is not stored in the
browser and it is not conflated with an Agent transcript.

```text
ResearchProject
 ├─ ProjectRevision
 ├─ SourceSet
 │   └─ SourceRecord → extracted document/data → source provenance
 ├─ EvidenceGraph
 │   ├─ Claim
 │   ├─ Citation
 │   ├─ Measurement / DatasetSlice
 │   └─ Derivation / Relation
 ├─ WorkflowPlan
 │   └─ WorkflowRun → StepRun → Artifact
 ├─ ReviewSet
 │   └─ ReviewFinding → Decision / Resolution
 └─ Publication
     └─ ProvenanceReceipt
```

Every node carries an immutable identity, content or semantic digest, source
revision, owner project revision, and logical observation time. Mutable
presentation (selection, panel layout, draft cursor) is a Desktop projection
keyed by the durable project/run identity.

## A3S Code plan: research execution and evidence kernel

### Code capabilities to retain and extend

The current Code foundation already supplies the important generic mechanisms:
bounded Agent execution, governed Tools, workspace retrieval, Skills/MCP/Flow
projection, Session/Run/Turn/Subtask scopes, exact Use leases, checkpoints,
logical-time events, and the provider-neutral Evaluation Substrate. The
scientific product should compose these mechanisms rather than add another
agent loop.

### Code-owned research mechanisms

1. **Project/run admission.** Admit a `ResearchRun` with project revision,
   source/evidence snapshot, Code catalog digest, Use cursor, model/provider,
   authority ceiling, budget, and reproducibility mode. Reject mixed or stale
   identities before the first provider call.
2. **Evidence fabric.** Record source reads, extracted spans, claim proposals,
   citations, measurements, transformations, and tool outputs as bounded
   digest-addressed facts. Preserve unresolved, conflicting, and unavailable
   evidence explicitly; never infer a green state from missing data.
3. **Research context.** Build model context from selected claims and source
   spans, not an unbounded transcript. Retrieval uses the workspace/catalog
   policy and can combine lexical, semantic, graph, and exact-source evidence.
   zvec-grep remains a retrieval data-plane implementation.
4. **Workflow bridge.** Convert an approved `WorkflowPlan` into typed Flow or
   Runtime invocations. Code supervises the research Run and evidence binding;
   Flow/Runtime own workflow scheduling and Task/Service execution.
5. **Artifact and provenance binding.** Bind every figure, table, dataset slice,
   notebook output, report, and model artifact to input digests, source spans,
   executable/code digests, environment lock, provider, parameters, seed,
   execution receipt, and validation status.
6. **Reviewer execution.** Use the Evaluation Substrate for isolated,
   cancellable, bounded reviewer runs triggered at a planned boundary. A
   reviewer cannot broaden the parent ceiling or turn its output into an
   implicit approval. Findings are host-defined and persisted through an
   immutable result sink.
7. **Recovery and fork.** Checkpoint at safe workflow boundaries. Resume only
   from the exact historical project/evidence/capability generation. Forking
   creates a new project revision and provenance branch; it never rewrites the
   parent evidence ledger.
8. **Scientific safety hooks.** Expose typed gates for destructive data writes,
   external submissions, human-subject or regulated data, network egress,
   expensive compute, and publication. The policy is host/project-owned;
   prompts do not grant authority.

### Code contracts to add

The following are proposed versioned, generated contracts. They are narrow
projections, not a second package or runtime protocol:

```text
a3s.code.research-run.v1
a3s.code.evidence-fact.v1
a3s.code.provenance-receipt.v1
a3s.code.review-finding.v1
a3s.code.science-event.v1
```

The contracts must be strict, bounded, canonical, and available to the Rust,
Node.js, Python, and Go SDKs. Plaintext prompts, credentials, local package
paths, and unrestricted arbitrary JSON are excluded from portable records.

## A3S Use plan: scientific package and environment plane

### Package composition

Use keeps its six native surface kinds. A scientific package composes them:

| Use surface | Scientific contribution | Execution owner |
| --- | --- | --- |
| Skill | Domain method, planning guidance, reviewer policy, or analysis protocol | Code/host |
| Tool Task/Service | Parser, simulator, statistical method, converter, validator, or compute service | Runtime/Box/host |
| MCP | External database, instrument, repository, or lab connector | Runtime/Gateway/host |
| OKF Knowledge | Curated references, ontologies, protocols, and validated background knowledge | Knowledge host |
| Flow | Reproducible multi-step workflow and retry/observation policy | Flow |
| UI | Domain viewer, plot inspector, structure viewer, or workflow form | Desktop/Web host |

There is no seventh “science runtime” surface. A package profile describes
domain, input/output schemas, method version, resource needs, licensing,
validation evidence, and reproducibility guarantees across the existing six
surfaces.

### Scientific package profile

Add a strict catalog/planning record (without weakening the current package
manifest) with:

- `domain` and capability taxonomy IDs;
- typed input/output data models and supported format versions;
- method and validator identities, references, and expected tolerances;
- environment lock digest (OS, architecture, runtime, libraries, models);
- CPU/GPU/memory/disk/time/network requirements and egress class;
- data classification and required Grant ceiling;
- deterministic/stochastic behavior and seed requirements;
- artifact types and provenance fields produced; and
- independent test fixtures and validation report digests.

Proposed contracts:

```text
a3s.use.science-package-profile.v1
a3s.use.environment-lock.v1
a3s.use.science-capability-binding.v1
a3s.use.science-validation-evidence.v1
```

Use stores immutable package/environment bytes by digest, verifies signatures,
freezes the complete dependency closure, and records exact provider
assignments. It does not execute package build scripts or install an ambient
Python/Node environment. A provider receives an immutable environment lock
and performs its own typed preflight.

### Package lifecycle for research

```text
discover signed package/profile
  → resolve method + connector + validator closure
  → freeze lock and environment
  → review requested Grants and resource budget
  → provider preflight
  → prepare package surfaces and workflow bindings
  → publish one capability generation
  → lease generation for ResearchRun
  → drain and reverse-retire on upgrade/removal
```

An upgrade cannot change a running ResearchRun. Code retains the old exact
lease until all Tasks, evidence writes, reviewer runs, and artifact receipts
settle.

## A3S Desktop plan: research workbench

Desktop turns the current research execution state and aggregate report into a
persistent project workbench. Native Rust remains authoritative; React is a
projection and command client.

### Primary surfaces

1. **Project home.** Hypothesis/question, project revision, active method and
   environment, recent runs, unresolved evidence gaps, and next safe action.
2. **Sources.** Import local files, URLs, datasets, and references; show
   identity, license, revision, extraction status, access failures, and source
   provenance. A browser preview never becomes authoritative evidence.
3. **Evidence graph.** Browse claims, citations, measurements, relations,
   derivations, conflicts, and source spans. Selecting a claim highlights every
   dependent artifact and review finding.
4. **Notebook/workflow.** Edit a typed plan, parameters, dataset selections,
   code/Skill/Flow steps, resource budget, and approval checkpoints. A plan
   diff is reviewed before execution.
5. **Runs and compute.** Show queued/running/failed/paused/completed steps,
   logs, resource use, checkpoints, provider, environment, and cancellation.
   Local, Box, and remote providers use the same event vocabulary.
6. **Artifacts.** Render figures, tables, structures, datasets, models, and
   reports with input/code/environment/provenance inspectors. Edits create a
   new artifact revision instead of hiding the original.
7. **Review.** Display citation, numeric, method, reproducibility, and
   figure/code findings with evidence links, severity, status, owner, and
   resolution. Approval, reject, waive, and request-revision are explicit
   commands.
8. **Publication.** Assemble Markdown/HTML/PDF/manuscript packages with
   citations, figures, tables, methods, data/code/environment references, and a
   machine-readable provenance receipt.

### Desktop state and interaction rules

- Project/run/artifact/review identities come from the native API; frontend
  state stores only bounded projections and user preferences.
- Every consequential action shows scope, provider, data class, cost, and
  approval requirement before submission.
- Event streams are append-only and resumable. A reconnect never duplicates a
  finding, artifact, or run transition.
- Long-running work remains inspectable when partial or failed. Missing source,
  retained-gap, degraded provider, and stale generation states are visible.
- Fork, compare, and restore operations preserve the parent and show exact
  revision lineage.
- The workbench supports keyboard navigation, reduced motion, localization,
  and bounded rendering for large evidence graphs and artifact lists.

## Cross-component data flow

```text
1. Desktop creates/opens ResearchProject revision P
2. Use resolves signed scientific packages and publishes capability generation N
3. Code admits ResearchRun R(P, N), captures source/evidence snapshot E
4. Code plans and requests explicit human approval for consequential effects
5. Flow/Runtime/Box executes steps; Code records facts and artifacts
6. Reviewer package runs through Code Evaluation Substrate against E/artifacts
7. Desktop renders findings; researcher resolves or requests revision
8. Code re-runs only affected steps when possible, preserving lineage
9. Publication commits artifact set + provenance receipt atomically
10. Use drains N only after every dependent run/lease settles
```

Canonical event keys use the repository convention of lowercase dot-separated
names:

```text
research.project.created
research.source.admitted
research.source.extracted
research.claim.proposed
research.claim.supported
research.claim.conflicted
research.plan.approved
research.run.admitted
research.run.checkpointed
research.step.completed
research.artifact.published
research.review.finding
research.review.resolved
research.publication.committed
```

Each event binds project revision, run identity where applicable, Code catalog
digest, Use cursor, logical sequence, actor/agent identity, and relevant
content digests. Events are observations; authorization and publication still
use the owning command path.

## Delivery roadmap

The order favors one complete, local-first vertical slice before broad domain
coverage.

| Phase | Code | Use | Desktop | Exit gate |
| --- | --- | --- | --- | --- |
| **P0 Contract freeze** | Define research run/evidence/provenance ownership and generated schemas | Define science package profile and environment-lock extension | Define project identity and event projection | Architecture review; no duplicate authority or second runtime |
| **P1 Durable project** | Project/run admission, source/evidence facts, artifact references, checkpoint binding | Install one reviewed Skill + Tool + Knowledge package graph | Project home, source list, run history, artifact inspector | Restart/fork preserves exact revisions; local-only flow passes |
| **P2 Evidence fabric** | Claim/citation/measurement graph, source spans, retrieval/context selection, conflict and gap states | Signed source connectors, parsers, ontology/OKF bundles | Sources and evidence graph with traceable selection | Every published claim has source/gap evidence; provenance completeness is measured |
| **P3 Reproducible compute** | Workflow bridge, step receipts, resource budgets, deterministic/stochastic seed binding | Environment locks, validator packages, Flow/Runtime provider preflight | Notebook/workflow editor, run/compute monitor, artifact gallery | Same fixture + lock reproduces normalized outputs; crash/retry leaves no orphan effects |
| **P4 Reviewer and validation** | Reviewer dispatch, bounded evidence, finding result schema, affected-step re-run lineage | Signed reviewer/validator packages and method evidence | Review panel, finding lifecycle, approval/revision/waiver | Citation/number/figure-code fixtures detect seeded faults; human decision is explicit |
| **P5 Publication** | Atomic artifact set and provenance receipt, export/re-import validation | Citation/style/rendering packages with verified assets | Manuscript/report editor, citation inspector, PDF/HTML export | Export round-trip, source/code/environment links, accessibility and layout checks |
| **P6 Ecosystem and scale** | SDK parity, remote provider recovery, long-horizon resource controls | Registry governance, package review, revocation, multi-platform artifacts | Collaboration/fork/compare, notifications, project sharing | Linux/macOS/Windows, offline/remote matrix, security and domain qualification |

### First vertical slice

The first release should target one narrow, non-regulated literature-plus-data
workflow:

1. import local Markdown/PDF and a curated web source;
2. normalize source identity and citations;
3. extract claims and quantitative observations into an evidence graph;
4. run one deterministic analysis workflow with a pinned environment;
5. generate one figure/table and a Markdown report;
6. run a separate reviewer for citation fidelity, numeric consistency, and
   figure/code provenance;
7. show findings in Desktop and require an explicit researcher decision; and
8. export the report, artifacts, code, environment, and provenance receipt.

This slice validates the complete mechanism without claiming broad scientific
validity. A second domain pack is added only after the first fixture has
independent expected results and a documented validation protocol.

## Verification strategy

### Contract and lifecycle tests

- canonical encode/decode and generated Rust/Node/Python/Go parity;
- mixed-generation, stale-lease, duplicate-identity, and path-boundary
  rejection;
- install/upgrade/rollback/drain/recovery with shared package dependencies;
- project fork and checkpoint recovery without parent mutation;
- cancellation, timeout, backpressure, provider loss, and process restart;
- no residual tasks, sockets, files, leases, or unowned provider effects.

### Scientific correctness tests

- source identity, citation resolution, deduplication, and span extraction;
- claim-to-source completeness and explicit unsupported/conflicting claims;
- deterministic workflow outputs and tolerance-bound stochastic outputs;
- dimensional/unit checks, schema validation, statistical method checks, and
  independent reference results;
- seeded citation, number, and figure/code faults for reviewer detection;
- artifact/code/environment round-trip and provenance receipt verification;
- blinded comparison against a human-reviewed fixture and domain expert signoff.

### Product and operational tests

- Desktop native `cargo test --lib`, frontend tests, typecheck, package build,
  and authenticated event-stream reconnect;
- Code focused evaluation, provider, retrieval, checkpoint, and SDK parity
  gates from the owning repository;
- Use package, lock, registry, signature, offline, and platform tests;
- Linux, macOS, and Windows with local-only and remote-provider profiles;
- performance budgets for source ingestion, graph projection, reviewer queue,
  artifact rendering, memory, file descriptors, and disk retention;
- privacy review for PII/regulated data, secret redaction, network egress, and
  package-native-code policy.

Promotion requires all required evidence, not merely a successful model answer.
The product must report `incomplete`, `degraded`, `unsupported`, and
`unreviewed` states instead of converting them to a green score.

## Explicit non-goals

- DSH/Cordis compatibility or embedding another Harness runtime.
- An unrestricted autonomous scientist or a claim that AI review replaces
  domain expertise, peer review, clinical judgment, or regulatory validation.
- A general package/runtime/DI framework in Code or a second package manager in
  Desktop.
- Silent downloads, mutable `latest` packages, arbitrary build scripts, or
  unbounded native execution.
- Making Cloud, a specific LLM provider, a specific database, or an HPC vendor
  mandatory for the local-first product.
- Treating retrieval similarity, model confidence, or a reviewer token as
  scientific truth without source and method evidence.
