# A3S Local Retrieval Platform Roadmap

This roadmap implements the architecture in
[`retrieval-platform-architecture.md`](retrieval-platform-architecture.md).
It is gate-based rather than calendar-based: a phase is complete only when its
exit evidence exists for the same revision. Work can be parallelized only when
the dependency graph says so.

## 1. Current baseline and target

The current checkout contains three relevant implementations:

- `crates/vec` is an untracked pure-Rust `a3s-vec` prototype with zvec-shaped
  API, storage, FTS, and vector/index facades. It is not yet an external
  submodule or a release-qualified crate.
- `a3s-code-core` has a session-local chunk catalog, incremental BM25,
  `a3s-memory` exact-vector partitions, deterministic RRF/MMR, provider ports,
  and source verification. This is the behavioural reference for migration.
- `a3s-memory` remains the long-term Agent memory project. Its workspace-vector
  capability is a temporary integration reference, not the target owner.

The target is one optional `vgrep` capability in Code backed by `a3s-vec`, with
exact and lexical behavior available when no Embedding model is installed.

## 2. Gates and evidence policy

Every gate must produce:

- deterministic fixtures and a machine-readable report;
- a bounded failure/fallback result rather than a skipped assertion;
- platform and feature information for the tested revision;
- no source text, vectors, credentials, or prompts in logs or reports.

Real DeepSeek or remote-provider runs are supplementary evidence. They cannot
replace deterministic fake-provider, brute-force, crash-recovery, and privacy
tests.

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
- Decide the temporary path dependency and the eventual external repository
  name (`A3S-Lab/Vec`) without adding a root workspace.

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

**Exit gate**

CRUD, type/dimension/nullability errors, filters, FTS golden cases, hybrid
fusion, and deterministic tie-breaking pass on Linux and macOS Intel with no
model runtime or native database extension.

### P2 — Durability and crash correctness

**Dependencies:** P1.

**Deliverables**

- Framed CRC WAL, atomic snapshots, manifest generations, single-writer/
  multi-reader locks, read-only mode, and explicit durability policies.
- Schema/DML WAL replay, partial-tail handling, checksum diagnostics, WAL
  pruning, and fault-injection hooks.
- Recovery and reopen APIs that never publish a mixed revision.

**Exit gate**

Restart tests cover every DML and schema operation; interrupted append,
checkpoint, rename, lock contention, and corrupted earlier frames fail or
recover deterministically. Manifest metadata is durable after every acknowledged
WAL append.

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

**Dependencies:** P6 and all migration gates.

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
