# A3S Local Retrieval Platform Roadmap

This roadmap implements the architecture in
[`retrieval-platform-architecture.md`](retrieval-platform-architecture.md).
The latest first-principles audit is recorded in
[`retrieval-platform-architecture-review.md`](retrieval-platform-architecture-review.md),
with reproducible command evidence in
[`retrieval-platform-review-evidence.json`](retrieval-platform-review-evidence.json).
It is gate-based rather than calendar-based: a phase is complete only when its
exit evidence exists for the same revision. Work can be parallelized only when
the dependency graph says so.

**Current integration status (2026-09-03):** Code `96be2ce34695341f477644bd3c36cf4aa6c39d4f`
now contains the Code-owned A3S Vec shadow adapter. It mirrors the already
validated Memory embedding batch into a session-scoped temporary Vec
collection, keeps Memory authoritative, and exposes bounded status and
differential diagnostics across the Rust, Node.js, Python, and Go surfaces.
The adapter's implementation pin is Vec `019fdb929a57dee1803691e6def60df3946d9561`;
the root submodule is advanced to Vec `41283f6315906a2737b5a8e8612ac876a8dc9c04`
for the corresponding release review, public API contract, gated
release-candidate artifact workflow, IVF SOAR execution, Binary32/Binary64
exact L2/Hamming queries, complete feature matrix, query-builder/ordinal
contracts, compact schema-only WAL revisions, the locally verified dependency
audit, the 53-row feature-matrix plus concurrent-reader/mixed-workload/scale/lifecycle
performance artifact gates, Vamana RobustPrune controls and standalone scalar
quantization, and the paired a3s-vec/zvec scale harness. The
methodology revision's revision-bound CI run `33690839419` passed all hosted quality,
MSRV, recovery, cross-platform,
performance, and package jobs, including the lifecycle matrix. The
Windows x86-64 schema-4
qualification compared 120/120 queries with zero mismatch or failure in both
hybrid arms, retained 25,000 records per arm, and released both engines on
close. Exact, RRF-only, and deterministic-rerank p95 were 6.7343, 50.7850, and
49.7348 ms. This closes the developer-shadow P4 implementation evidence; Vec
promotion, old-path removal, Intel macOS 12 runtime evidence, and the broader
release gates remain open.

**Historical engine review baseline (2026-08-30):** `a3s-vec` was then a
pre-migration prototype, but Vec `0236e0d0cd9d4c203a689567e52a0591697260a2` closes the
numbered P1 contract findings for the current exact surface. Native dense and
sparse FP16, INT4, INT8, INT16, and binary payloads now have strict physical-
type, range/chunk, typed-access, and lossless storage contracts. Every numeric
native form executes exact L2/IP/cosine/MIPS-L2 with `f64` intermediates;
binary search and scale-bearing index quantization remain explicit future work
rather than implied behavior. Format 3 prevents the new sparse-FP16 bit layout
from being reinterpreted by an older reader. Retained runtime controls have
execution owners, future index/query/schema controls fail before mutation, and
the external algorithm kernel remains private. Independent references now
cover current dense/sparse exact scoring, filters, radius/top-k ordering, and
scan-BM25 corpus statistics; unsupported advanced FTS syntax fails closed.
Concurrent public-API fixtures now prove serialized disjoint updates,
revision-pinned iterators, and atomic multi-document publication to readers.
Strict rustfmt, Clippy, three 60-test feature-matrix runs plus four compile-fail
doctests, rustdoc, and the default suite on the declared Rust 1.75 MSRV are
green. Real ANN/indexed FTS, broader generated and FTS/filter evidence, the
durability fault/fuzz matrix, cross-platform evidence, and migration benefit
remain open. Code's existing workspace BM25 and `a3s-memory` vector path remain
the golden reference; P7 removal has not started.

## 1. Current baseline and target

The current checkout contains three relevant implementations:

- `crates/vec` is the `A3S-Lab/Vec` git submodule at `41283f6` (complete
  FP16/INT8/INT4 Vamana reopen coverage over Vamana control execution and the
  hosted-evidence/test-count refresh over the borrowed
  exact-score and one-query-norm performance kernel over comparison-methodology
  documentation over hosted benchmark pin `7f3e2a9`, CI hardening over
  the scale-control, lifecycle, and performance-matrix revisions and implementation
  commit `dbd4a75`). Format-10
  storage/recovery, exact dense/sparse/binary search, scalar and FTS indexes,
  HNSW, IVF with optional SOAR dual assignment, HNSW/IVF RaBitQ, and
  metric-aware Vamana/PQ DiskANN (L2, inner product, cosine, and MIPS-L2) are
  implemented behind exact fallbacks and revisioned
  generations. Namespace-only upstream CRUD, vector-search, and schema-builder
  fixtures run as executable gates. The crate remains a release candidate: its
  hosted matrix and package gate do not substitute for an actual macOS 12 Intel
  runtime result.
- `a3s-code-core` has a session-local chunk catalog, incremental BM25,
  `a3s-memory` exact-vector partitions, deterministic RRF/MMR, provider ports,
  source verification, and a Memory-authoritative Vec shadow adapter. The
  [Code migration note](../crates/code/manual/WORKSPACE_RETRIEVAL_VEC_MIGRATION.md)
  records the adapter contract and evidence.
- `a3s-memory` remains the long-term Agent memory project. Its workspace-vector
  capability is a temporary integration reference, not the target owner.

The target is one optional `vgrep` capability in Code backed by `a3s-vec`, with
exact and lexical behavior available when no Embedding model is installed.

The Vec pin now carries a three-test public feature matrix and an asserted
performance matrix. The all-feature suite currently passes 270 unit/integration
tests plus four doctests (267 unit/integration tests on both the default and
no-default feature sets, plus four doctests). The integration tests compare
every public query
route, all six ANN families across their supported metrics, lifecycle/cache/
sidecar behavior, Binary32/Binary64 exact L2/Hamming execution, and explicit
binary-ANN boundaries; the 53-row benchmark records p50/p95/p99 latency and
throughput for synchronous, ANN, mutation, sidecar, and Tokio paths, while the
scale benchmark records build/search/Recall@10 rows for a selected corpus and
the lifecycle matrix records 16 management/resource/maintenance rows. CI runs
the smoke scale and retains five CSVs as revision-bound artifacts. The detailed
fixture and same-host baseline are in
[`crates/vec/BENCHMARKS.md`](../crates/vec/BENCHMARKS.md).

## 2. Gates and evidence policy

Every gate must produce:

- deterministic fixtures and a machine-readable report;
- a bounded failure/fallback result rather than a skipped assertion;
- platform and feature information for the tested revision;
- no source text, vectors, credentials, or prompts in logs or reports.

Real DeepSeek or remote-provider runs are supplementary evidence. They cannot
replace deterministic fake-provider, brute-force, crash-recovery, and privacy
tests.

### 2.1 Architecture-review gate

Before promoting the P4 shadow projection or exposing semantic `vgrep` as a
serving path, the findings in the
[architecture review](retrieval-platform-architecture-review.md) must be
closed on the same pinned component graph. At minimum this means:

| Gate | Required evidence | Current state |
| --- | --- | --- |
| P0 correctness | Real index/recovery behaviour, monotonic revisions, atomic manifest publication, read-only lifecycle, and bounded deserialization | **Engine gate closed** at Vec implementation pin `dbd4a75` (Code's shadow adapter remains separately pinned at `019fdb9`): format-10 snapshots/WAL, all 18 injected publication boundaries, bounded recovery fuzzing, lock ownership, and read-only lifecycle are executable gates |
| P1 contract | Schema WAL replay, typed dimension/type errors, native codec semantics, wired configuration, private kernel boundary, and promised integration tests | **Closed for the advertised engine surface** at root pin `41283f6` (complete FP16/INT8/INT4 Vamana reopen coverage over Vamana controls/scalar quantization, the test-count correction, and performance kernel): generated vector/FTS/filter and Binary32/Binary64 Hamming oracles, advanced FTS, concurrency, private-kernel compile failures, typed unsupported paths, IVF SOAR/cache contracts, public `Send + Sync` contracts, metric-aware Vamana/DiskANN contracts, dense/binary/FTS query-builder execution, include-doc-id persistence checks, schema-only WAL compaction, the complete feature matrix plus concurrent-reader/mixed-workload/scale tail-latency gates, and the 16-row lifecycle/resource/maintenance matrix are tested |
| Strict quality | `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` for Vec | **Passed locally and hosted for test-coverage pin `41283f6` in [Vec CI run `33705867979`](https://github.com/A3S-Lab/Vec/actions/runs/33705867979)** (all ten jobs succeeded: quality, performance, MSRV, recovery fuzz, Linux x86-64/ARM64, Windows x86-64, macOS ARM64/Intel, and versioned release candidate); the exact-revision macOS 12 Intel qualification workflow remains available |
| Cross-platform | x86_64 macOS 12.0 build, smoke, runtime, and offline exact/FTS evidence | **Partially closed**: hosted Linux x86-64/ARM64, Windows x86-64, and macOS ARM64/Intel pass, and Intel builds target 12.0; an actual macOS 12 Intel runtime remains open |
| Migration benefit | Differential quality, latency, memory, startup, recovery, lifecycle, and privacy report against the frozen Code baseline | **Shadow differential passed** for 120 queries and lifecycle/resource checks; RSS, recovery, cross-platform, and serving-promotion evidence remain open |

An open row blocks the dependent phase. Passing a superficial API or compile
check cannot substitute for the required runtime evidence.

## 3. Phased delivery

### P0 — Freeze contracts and repository ownership

**Dependencies:** none.

**Deliverables**

- Approve the cross-project ownership table and the `vgrep` request/result
  schema.
- Inventory current Code BM25, `a3s-memory::VectorIndex`, SQLite features, SDK
  adapters, CLI ACL blocks, TUI views, and existing test fixtures.
- Freeze `ChunkRecordV1`, `EmbeddingDescriptorV1`, `RetrievalStatusV1`, typed
  fallback reasons, and collection/partition identity rules.
- Record the external `A3S-Lab/Vec` repository, submodule pin, and temporary
  path-development workflow without adding a root workspace.

**Exit gate**

The same fixture can be rendered by exact, BM25, semantic, and hybrid paths;
each result carries a revision and digest; ownership review finds no duplicate
watcher, chunker, model lifecycle, or persistence authority.

### P1 — `a3s-vec` reference engine

**Dependencies:** P0.

**Deliverables**

- Stable Rust public API for configuration, collection lifecycle, schema,
  typed documents, nulls, dense/sparse vectors, filters, projections, and
  deterministic iterators.
- Exact flat dense/sparse search for L2, inner product, cosine, and radius/top-k
  semantics.
- FTS tokenizer/postings/BM25 implementation and generic RRF/weighted fusion.
- A reference brute-force evaluator used by every later index test.
- Close the P0/P1 findings from the architecture review before handing the
  engine to a Code adapter.

**Exit gate**

CRUD, type/dimension/nullability errors, filters, FTS golden cases, hybrid
fusion, and deterministic tie-breaking pass on Linux and macOS Intel with no
model runtime or native database extension.

**Progress at Vec `0236e0d` (2026-08-30)**

- Added a centralized schema-derived query contract for route/type/dimension,
  sparse-index, metric, radius, top-k, and tokenizer validation. Every current
  numeric dense type and metric has dimension-error evidence; unsupported
  binary and sparse source-ID routes fail explicitly.
- Defined JSON as an adapter-only input and canonicalized every supported
  scalar/non-binary array type before validation and persistence. Added
  incompatible, numeric-boundary, binary, typed-backfill, and replacement-
  upsert fixtures.
- Made the default feature graph independent of Jieba, `zstd-sys`, and `cc`;
  Jieba remains an explicit feature and never silently falls back when absent.
- Removed the public `zvec-core` escape hatch and added a compile-fail doctest
  that keeps dependency types outside the A3S public contract.
- Removed inert process/collection runtime knobs and orphaned log/backend types;
  wired process-default inheritance, collection durability overrides, and WAL
  checkpoint thresholds with execution tests.
- Closed VEC-P1-05 by allowing only Flat metric/radius and scan-FTS tokenizer
  controls with execution consumers. Future index builds, query tuning, FTS
  operator/filter/extra controls, segment sizing, schema concurrency, and
  physical optimize return typed errors without mutation. Exact fusion is no
  longer counted as ANN, and scan FTS is not reported as a built index.
- Closed VEC-P1-04 by separating native storage encodings from future index
  quantization. Dense/sparse FP16 preserve raw half bits, INT4 enforces
  `-8..=7`, INT8/INT16 preserve integer coordinates, and Binary32/Binary64
  enforce chunk-aligned bit dimensions. Strict schema-type writes, typed
  getters, storage round trips, exhaustive finite-FP16 bit round trips,
  conversion error bounds, all four dense/sparse metrics, and non-narrowing
  FP64 accumulation have deterministic fixtures. Binary queries and
  scale-bearing/packed index quantizers remain `NotSupported` until their
  later-phase executors and exact re-score evidence exist.
- Advanced manifest, snapshot, and WAL framing to format 3 because sparse FP16
  now persists raw half bits; format 2 fails closed before payload decode.
- Made the declared Rust 1.75 default-feature MSRV executable by constraining
  `zvec-core`'s broad Rayon dependency and the test-only tempfile dependency to
  compatible versions. The current Jieba feature still requires newer Cargo
  because its dependency chain contains Rust 2024 manifests.
- Added independent dense/sparse reference scans across all exact metrics,
  filters, radius/top-k semantics, exact FP64 ordering, and primary-key ties.
  Added independent scan-BM25 reference cases for nullable/missing and empty
  text fields. Negative L2 radius and unimplemented advanced FTS syntax now
  fail explicitly instead of changing query meaning.
- Added deterministic cloned-handle concurrency fixtures: disjoint updates
  retain both patches and monotonic revisions, iterators retain one captured
  revision, and synchronized readers racing repeated two-document upserts see
  only complete batches. The focused suite is stable across 20 repeated runs.
- Remaining before P1 exit: broader generated and FTS-filter golden corpora,
  multi-process lock diagnostics, and supported-platform runs.

### P2 — Durability and crash correctness

**Dependencies:** P1.

**Deliverables**

- Framed CRC WAL, atomic snapshots, manifest generations, single-writer/
  multi-reader locks, read-only mode, and explicit durability policies.
- Schema/DML WAL replay, partial-tail handling, checksum diagnostics, WAL
  pruning, and fault-injection hooks.
- Recovery and reopen APIs that never publish a mixed revision.
- Add bounded frame/document recovery budgets and fault-injection evidence for
  every snapshot, manifest, WAL, lock, and read-only boundary.

**Exit gate**

Restart tests cover every DML and schema operation; interrupted append,
checkpoint, rename, lock contention, and corrupted earlier frames fail or
recover deterministically. Manifest metadata is durable after every acknowledged
WAL append.

**Progress through Vec `0236e0d` (2026-08-30)**

- Landed generation snapshots, manifest-committed WAL byte boundaries,
  monotonic DML/schema identities, manual-flush synchronization, and bounded
  recovery reads. The original format-2 durability layout advanced to format 3
  when sparse FP16 persistence became raw-bit authoritative; the manifest,
  snapshot, and WAL versions advance together and format 2 is rejected.
- Added deterministic tests covering restart sequences, read-only lifecycle,
  partial/uncommitted tails, committed truncation/checksum failure, orphan
  generations, oversized snapshots, aligned format markers, and failure-closed
  format compatibility.
- Remaining before P2 exit: deterministic hooks at every write/fsync/rename/
  prune boundary, explicit prune-crash and lock-contention matrices,
  per-document/field budgets, recovery fuzzing, and supported-platform runs.

### P3 — Index implementations and resource bounds

**Dependencies:** P1 and P2.

**Deliverables**

- Scalar equality/range indexes and safe scan fallback.
- HNSW and IVF behind the common `VectorIndex` contract, with exact re-score
  and generation checks.
- PQ/RaBitQ/FP16/INT8/INT4 codecs, then DiskANN/Vamana only when a benchmark
  justifies the complexity.
- Index statistics, build/drop/rebuild/optimize progress, memory budgets, and
  portable I/O selection.

**Exit gate**

Every approximate result has an exact reference comparison and stale-index
fallback test. Reopen/checksum tests pass on Linux, Windows, macOS arm64, and
macOS 12 Intel; no optimization is required for correctness.

### P4 — Code catalog adapter and shadow migration

**Dependencies:** P2, plus the existing Code catalog/reference path.

**Deliverables**

- A Code-owned adapter that projects the existing canonical chunks into one
  session-scoped `a3s-vec` collection, one file per logical partition.
- Shared manifest reconciliation, tombstones, source revisions, digest/range
  verification, and file-atomic replacement.
- Shadow mode that sends one provider batch and compares old/new lexical,
  vector, and hybrid results without exposing the new result yet.
- Query and status diagnostics that identify the active engine and fallback.

**Exit gate**

Dual reads agree on eligible IDs and source ranges across create/change/delete/
rename/lag races; no duplicate file read or Embedding request occurs; session
close leaves zero vectors, tasks, handles, or sockets.

**Progress through Code `96be2ce` (2026-09-02)**

- Delivered the Code-owned adapter and shared publication gate. One validated
  provider batch is published to Memory and mirrored to Vec; Vec failures and
  mismatches degrade only shadow diagnostics and never change serving results.
- Added deterministic partition/key mapping, revision and digest fencing,
  bounded filter construction, cancellation-safe blocking operations, rollback
  isolation, and zero-state close checks.
- Added backward-compatible status fields and Rust/Node.js/Python/Go mapping
  tests. The release benchmark schema 4 (`workspace-retrieval-v3`) retained
  25,000 records in each hybrid arm, matched 120/120 comparisons, and reported
  zero mismatch/failure with 54,500,008 Vec-accounted bytes per arm.
- Focused adapter tests, the replacement soak, workspace checks, strict
  Clippy, SDK checks, and the Windows release benchmark passed. The hosted Vec
  matrix is green; actual macOS 12 Intel runtime evidence is still external.

The P4 developer-shadow implementation is delivered. The exit gate for
serving promotion remains tied to the later cross-platform, RSS/disk,
recovery, privacy, and rollback evidence; P7 must still remove the duplicate
workspace backends only after that review.

### P5 — Model plane and policy

**Dependencies:** P4.

**Deliverables**

- Keep `EmbeddingProvider` in Code Core as a provider-neutral port; move all
  local/remote implementation details to hosts.
- Typed local artifact admission with digest, tokenizer, dimension,
  normalization, license, CPU-feature, and offline checks.
- Typed remote provider with explicit source-egress authorization, bounded
  retries, cancellation, endpoint identity, and redacted diagnostics.
- Optional `WorkspaceReranker` port; deterministic Code RRF/MMR remains the
  default and a neural model remains off.

**Exit gate**

Deterministic provider faults (wrong dimension, NaN, panic, timeout, 429/5xx,
cancellation, partial batch, descriptor drift) degrade semantic coverage only.
No provider call occurs before policy admission; no model is downloaded during
session construction.

### P6 — Built-in `vgrep` and host surfaces

**Dependencies:** P4 and P5.

**Deliverables**

- One governed Code tool with `rg`, `fts`, `vector`, and `hybrid` routes,
  path/type filters, limits, freshness, previews, and structured fallback.
- `a3s` CLI `vgrep` command and ACL/config blocks; default disabled unless the
  user explicitly enables indexed/semantic behavior.
- TUI progress/readiness/degraded states and keyboard navigation using the same
  result DTO; no ranking logic in TUI.
- Desktop IPC/view-model integration using the same host contract.
- Rust SDK examples and compatibility tests; language SDK wrappers only expose
  typed provider/options objects.

**Exit gate**

The same request produces equivalent result identities and fallback metadata in
direct CLI, TUI, Desktop, and SDK paths. `--rg` works with no index or model;
`--fts` works without query-time Embedding; semantic routes reject missing or
incompatible descriptors clearly.

### P7 — Remove duplicate SQLite/BM25 integration

**Dependencies:** P6, the architecture-review gate, and every
migration-benefit gate.

This phase is deliberately **not complete** in the current checkout. It must
not be started merely because the `a3s-vec` API exists or because a semantic
provider test passes.

**Deliverables**

- Make `a3s-vec` FTS/BM25 the sole workspace lexical implementation.
- Remove Code's workspace dependency on `a3s-memory::VectorIndex` and any
  SQLite/`sqlite-vec` workspace path. Keep `a3s-memory`'s durable memory APIs
  intact for Agent memory consumers.
- Delete obsolete query-time corpus BM25 code, duplicate status fields, and
  stale docs/examples; retain a versioned migration note and rollback switch
  for one release if packaging permits.
- Replace release profiles that optimize the removed vector backend with the
  `a3s-vec` dependency and update exact compatibility pins together.

**Exit gate**

Search over the locked corpus has no SQLite/sqlite-vec or old Code BM25 runtime
dependency; correctness, quality, resource, privacy, and lifecycle reports are
equal to or better than the frozen baseline; rollback to exact/FTS remains
available through configuration.

### P8 — Cross-platform qualification and release

**Dependencies:** P7.

**Deliverables**

- Add `A3S-Lab/Vec` external repository, submodule, release tags, README, API
  docs, and compatibility-lock entries.
- `a3s-test` suites for API, CRUD/query, durability, indexes, concurrency,
  provider policy, vgrep output, TUI interaction, and cleanup.
- Benchmarks for exact/hybrid p95, indexing throughput, memory, cold/warm model
  load, cancellation recovery, and request amplification.
- Intel macOS 12 build/smoke/runtime gate, plus Linux, Windows, and macOS
  arm64 CI; verify installer and Homebrew metadata only after artifacts exist.
- Operator runbook for status, stale indexes, model installation, remote-egress
  consent, and rollback.
- Publish the architecture-review report and machine-readable evidence for the
  exact component graph used by the release.

**Release gate**

Formatting, strict Clippy, unit/integration/fuzz smoke, crash recovery,
security/egress, deterministic relevance, host E2E, and all supported-platform
smoke tests pass for one pinned component graph. No feature is advertised from
an unqualified platform or model runtime.

### P9 — Post-stable optimizations (separate decisions)

**Dependencies:** P8 and measured production evidence.

Possible work, each requiring an ADR and an A/B report:

- encrypted persistent workspace index and background daemon;
- true DiskANN/mmap or architecture-specific SIMD;
- local Cross-Encoder reranker;
- multimodal/non-text knowledge-compiler handoff;
- standalone `a3s-vgrep` crate for non-Code consumers.

None of these is allowed to expand the first release's trust boundary or block
the model-free exact/FTS path.

## 4. Parallel work graph

```text
P0 ──> P1 ──> P2 ──> P3 ───────────────┐
                  └──> P4 ──> P5 ──> P6 ──> P7 ──> P8 ──> P9
                         │       │      │
                         └───────┴──────┴── SDK / CLI / TUI / Desktop slices

P0 ──> fixtures, a3s-test contracts, and documentation (parallel)
P1 ──> reference benchmarks (parallel)
P4 ──> shadow migration only after P2 recovery is green
```

The model plane and host surfaces may proceed in parallel with ANN optimization
after P4. They must consume the stable adapter contracts and may not wait for
DiskANN or a neural reranker.

## 5. Rollout and rollback

1. **Preview:** compile the adapter and expose diagnostics; default remains
   disabled.
2. **Shadow:** build the `a3s-vec` projection and compare against the old path;
   do not double-call an Embedding provider.
3. **Opt-in beta:** enable `vgrep` through trusted ACL/SDK options; exact and
   FTS routes remain first-class.
4. **Stable:** remove duplicate workspace backends only after P8 evidence.

Rollback is configuration-only: stop new semantic sessions, cancel and close
existing projections, hide semantic routes, and continue with exact/FTS/Code
Intelligence. Long-term Agent memory is unaffected. A rollback must report the
engine generation and bounded reason; it must not silently present stale vector
hits.

## 6. Definition of done

The program is complete only when all of the following are true:

- `a3s-vec` is an externally pinned, documented Rust crate with durable
  correctness evidence;
- Code has one canonical catalog and one workspace index owner;
- `vgrep` is optional, governed, source-verified, and usable from CLI/TUI/
  Desktop/SDK through one contract;
- no workspace path depends on SQLite, `sqlite-vec`, or duplicate Code BM25;
- Embedding and Reranking are explicit provider capabilities, not hidden model
  dependencies;
- Intel macOS 12 support is backed by actual artifacts and runtime evidence,
  not just a target triple;
- every release claim links to a reproducible `a3s-test`/benchmark report.
- the [architecture review](retrieval-platform-architecture-review.md) has no
  unresolved release-blocking finding, and the migration-benefit report is
  attached to the release record;
- removal of the old Code workspace BM25/SQLite/vector path is explicitly
  recorded as complete only after P7's differential and rollback evidence,
  never inferred from a successful build.
