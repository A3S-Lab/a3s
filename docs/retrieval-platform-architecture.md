# A3S Local Retrieval Platform Architecture

Status: proposed cross-project architecture. The engine-level details for
`a3s-vec` live in [`crates/vec/ARCHITECTURE.md`](../crates/vec/ARCHITECTURE.md);
the delivery gates live in
[`retrieval-platform-roadmap.md`](retrieval-platform-roadmap.md).
The 2026-09-22 engine judgment is
[`vec-engine-first-principles-review.md`](vec-engine-first-principles-review.md).
[`retrieval-platform-architecture-review.md`](retrieval-platform-architecture-review.md)
records the older `fbb1081` snapshot. Where that snapshot disagrees with the
current contract below, the current contract wins.

## Current engine contract (2026-09-22)

The checked-in engine layout is `crates/vec/src`: `lib.rs`, `collection.rs`
and `collection/`, `config.rs`, `doc.rs` and `doc/`, `embedding.rs`,
`error.rs`, `index/`, `iterator.rs`, `multi_query.rs`, `query.rs`,
`schema.rs` and `schema/`, `score_f64.rs`, `stats.rs`, `storage/` and
`storage_ceilings.rs`, `text.rs` and `text/`, and `types.rs`, with integration
tests under `crates/vec/tests/`. There is no `api/`, `planner/`, `codec/`, or
`testkit/` directory.

The following are not current engine defects or open engine gates: HNSW, IVF,
or DiskANN delegating to a flat index; collection FTS as only a full scan; a
checkpoint publishing one `snapshot.json` before `manifest.json`; the default
build requiring Jieba/`zstd-sys`; WAL replay without a revision (VEC-P0-04);
read-only create writing the collection (VEC-P0-05); unbounded WAL or snapshot
reads (VEC-P0-06); or a five-test / 533-Clippy description of the current tree.
macOS 12 Monterey is unsupported and is not an open engine gate.

A workspace source revision, chunk catalog, and collection that describe the
same bytes, and publication of the public result, are owned by Code. The
engine has no workspace walker, CLI, embedding-provider client, or `vgrep`.
Vec commit `13585ccd` is not checkout `880d547` and is not the commit this
contract names for that Code check.

Enterprise GA is a hosted-green revision, a git tag, and a crates.io checksum
that matches the release artifact. Enterprise GA for `0.1.1` is closed.
`a3s-vec` `0.1.4` is tag `0.1.4` at revision
`9a07e9a33726dd187080b00a44505f9bbd31bd97` with crates.io SHA-256
`15c4220df078de9c350aea98e0f9187890cec066e4d3ee242170a2ab762ed80f`. GitHub
Actions run
[`35510190796`](https://github.com/A3S-Lab/Vec/actions/runs/35510190796)
completed with conclusion `success` on that revision, so the `0.1.4` hosted
gate is green. Checkout `880d547be07b2b863cf960a960809d21b37564ac` and the
dirty `crates/vec` working tree are not the GA artifact. The parent gitlink
for `crates/vec` remains `e6d067fcd4ff5ac536c5c9ee2fc8ea837f193576`.

This document is the ownership and dependency contract for the local retrieval
capability used by A3S Code and the future built-in `vgrep` tool. The
Memory-authoritative A3S Vec shadow projection is implemented in the Code main
merge `1cd4423e`,
including the Code-owned vector contract and revision-CAS adapter, and recorded
below;
the full serving migration and removal of the
legacy workspace paths are not complete. The root Cloud lock now records the
verified Code 8.2.0 component graph; serving migration still requires the
remaining cross-platform and rollback gates.

## 1. Problem and outcome

The product needs one workspace search surface that can answer three different
questions without confusing their guarantees:

| Question | Required guarantee | Route |
| --- | --- | --- |
| "Where is this exact text or symbol?" | Exhaustive, source-authoritative matches | `rg`/`grep` and `glob` |
| "Which indexed chunks contain these words?" | Deterministic lexical ranking | FTS/BM25 |
| "Which chunks express this idea?" | Bounded semantic candidates with source verification | dense vector search |

`vgrep` is a router and result contract over these routes. It is not an answer
model, a general memory store, or a remote search service. An Agent or LLM may
interpret the returned evidence, but that interpretation is outside the
retrieval engine.

The target outcome is:

```text
one chunk catalog + one vector/lexical engine + one verified result contract
       -> CLI, TUI, Desktop, SDK, and Agent tool surfaces
```

## 2. First-principles invariants

1. **One source of truth.** A workspace source revision, chunk catalog, and
   `a3s-vec` collection must describe the same bytes. No route may invent a
   second chunk boundary or silently reread a different generation.
2. **Exact search remains authoritative.** Semantic similarity can discover a
   candidate; only a current file read, digest, and byte-range check can expose
   source evidence.
3. **Models are dependencies of an adapter, not of storage.** The vector
   engine accepts caller-supplied vectors. It never selects a model, downloads
   an artifact, opens an HTTP connection, or reads a workspace by itself.
4. **Approximation is optional.** Flat exact search and lexical fallback are
   always available. An ANN or reranker failure may reduce recall or coverage,
   but cannot return unverifiable data or make `grep` unavailable.
5. **A query has one revision.** A query runs against an immutable snapshot.
   Reconciliation and index publication either precede or follow the query;
   they cannot produce a mixed result.
6. **Isolation is explicit.** Session, workspace, tenant, and provider
   boundaries are part of the collection/partition identity. No process-global
   mutable workspace index is allowed in the first integration.
7. **Egress is a capability.** Remote Embedding and any future remote Reranker
   require explicit host policy and are independently observable. A chat-model
   credential never grants source-code egress.
8. **Portable correctness first.** The required baseline is stable Rust,
   scalar/portable CPU math, ordinary POSIX/Windows file I/O, and no mandatory
   C/C++, `io_uring`, or architecture-specific SIMD. Optimizations are selected
   only after a benchmark proves their value.
9. **Deterministic failure.** Invalid dimensions, stale generations, malformed
   filters, budget exhaustion, cancellation, and provider failures have typed
   outcomes and a deterministic fallback order.
10. **Observability is metadata-only.** Status may expose revisions, counts,
    timings, model identity, and fallback reasons; it must not expose source
    text, vectors, prompts, credentials, or provider response bodies.

## 3. Planes and dependency direction

The platform is split into five planes. Arrows point from a consumer to the
contract it may depend on.

```text
                         presentation plane
             ┌──────────────┬──────────────┬──────────────┐
             │ a3s CLI      │ a3s TUI      │ Desktop      │
             └──────┬───────┴──────┬───────┴──────┬───────┘
                    │               │              │
                    v               v              v
             host policy / typed tool invocation / status DTOs
                                    │
                                    v
                         a3s-code-core (product plane)
          workspace admission · chunking · session lifecycle · vgrep
                     │                 │                 │
                     v                 v                 v
              a3s-vec engine     embedding ports    code intelligence
          (data/index plane)      (model plane)      (optional channel)
                     │                 │
                     v                 v
              files/snapshots       host adapters
                                    │
                                    v
                     a3s-test / Bench (verification plane)
```

The web `a3s-search` project remains a separate retrieval product. Cloud,
Runtime, Box, Gateway, and Flow own execution/control-plane concerns and do not
become owners of a local workspace index.

## 4. Subproject ownership

The following table is normative for the migration. A project may consume a
contract in the next column, but it must not reimplement a responsibility from
another row.

| Project | Owns | Public boundary | Must not own |
| --- | --- | --- | --- |
| `a3s-vec` (`crates/vec`, hosted by the external `A3S-Lab/Vec` repository) | Typed collections, schema, documents, WAL/snapshots, scalar/FTS/vector indexes, query planning, fusion primitives, and collection statistics | `Collection`, `Doc`, schema/index builders, `SearchQuery`, `MultiQuery`, iterator, typed errors, and `EmbeddingProvider`-agnostic vector APIs | Workspace traversal, chunk policy, provider credentials, model downloads, Agent sessions, or UI |
| `a3s-code-core` (`crates/code/core`) | Workspace admission, one canonical chunk catalog, chunk IDs/ranges/digests, manifest reconciliation, session-scoped collection lifecycle, the Memory-authoritative Vec shadow projection, source verification, Code-specific channel policy, and the model-facing `search`/`vgrep` contract | `WorkspaceServices`, `WorkspaceRetrieval`, `vgrep` request/result/status DTOs, typed provider ports, and shadow diagnostics | A second serving authority, direct provider-specific model code, hidden filesystem access outside the workspace service, or Cloud lifecycle |
| `a3s-code-core::embedding` | Provider-neutral embedding descriptor, bounded batching, cancellation, timeout/retry, response validation, and redacted errors | `EmbeddingProvider` and `EmbeddingDescriptorV1` | Vector persistence, file discovery, ACL parsing, or model artifact installation |
| `a3s-code-core::rerank` | Code-aware RRF/MMR policy, range overlap and identifier precedence, bounded scratch accounting, and optional host reranker port | `WorkspaceReranker` plus versioned `WorkspaceRerankStatus` | A mandatory neural model, source egress, or generic database storage |
| `a3s-memory` | Long-term/episodic memory stores and their memory semantics | `MemoryStore`, `MemoryItem`, memory search | Workspace chunks, BM25 policy, model runtime, or the authoritative Code index. Its current `VectorIndex` is a migration reference and is removed from Code after parity |
| `a3s-cli` (`crates/cli`) | ACL/config validation, explicit feature flags, local artifact admission, remote-egress authorization, `vgrep` command routing, and process-level provider reuse | CLI commands, ACL schema, provider adapters, and non-sensitive status output | Indexing/chunking algorithms, direct database internals, or an implicit model download |
| `a3s-tui` (`crates/tui`) | Terminal event loop, rendering, progress, search-result navigation, and accessible fallback/error presentation | UI events and view-model inputs | Filesystem scanning, ranking, provider calls, or persistence |
| `apps/desktop` | Native workbench presentation and host integration using the same Code/CLI contract | IPC/API view models | A second `vgrep` protocol, duplicate index, or model lifecycle |
| `a3s-search` | External web retrieval, source normalization, web rank fusion, and provider health | `Search`/`SearchResults` | Workspace files, local vectors, or Code's source verification |
| `a3s-test` and `a3s-bench` | Deterministic contract tests, fault injection, cross-platform smoke, quality/latency/resource evidence, and release reports | Versioned fixtures and machine-readable evidence | Product fallbacks that mask a failed gate |
| Cloud/Runtime/Box/Gateway/Flow | Agent execution identity, scheduling, isolation, transport, and usage/control-plane contracts | Existing platform protocols | Local workspace indexing, Embedding authorization decisions, or retrieval ranking |

### 4.1 Repository packaging rule

`A3S-Lab/Vec` hosts the crate and the A3S repository consumes it through the
`crates/vec` git submodule. The parent gitlink is
`e6d067fcd4ff5ac536c5c9ee2fc8ea837f193576`. That pin is integration state. It
is not the `0.1.4` GA artifact, and this document does not move it. Enterprise
GA is the record in the current engine contract at the top of this file.
Tagging is not blocked on a macOS 12 Intel runtime report.

Source changes are committed in the Vec repository first; the integration
repository advances the gitlink only after engine checks and compatibility
review pass.

No root `Cargo.toml` or root Rust workspace is introduced.

## 5. `a3s-vec` engine architecture

`a3s-vec` is an embedded, synchronous-by-default database. Its internal
modules are replaceable behind contracts, while the logical data model is
stable:

```text
crates/vec/src/
├── lib.rs
├── collection.rs
├── collection/
├── config.rs
├── doc.rs
├── doc/
├── embedding.rs
├── error.rs
├── index/
├── iterator.rs
├── multi_query.rs
├── query.rs
├── schema.rs
├── schema/
├── score_f64.rs
├── stats.rs
├── storage/
├── storage_ceilings.rs
├── text.rs
├── text/
└── types.rs
```

### 5.1 Authority and publication

The document snapshot plus WAL are authoritative. ANN, scalar, and FTS files
are derived and carry the source revision, schema digest, metric, dimensions,
and format version. A write is acknowledged only after the configured WAL
durability policy. A checkpoint publishes data and index files first, then
atomically publishes the manifest. Recovery validates the manifest and replays
only complete WAL frames after its checkpoint.

HNSW, IVF, and DiskANN build their own graphs or centroid postings. They do
not delegate `build` or `search` to a flat index. Indexed FTS searches term
postings when the generation matches and uses a document scan only as the
fallback. A checkpoint writes `segments/snapshot-<generation>.bin` and then
publishes `manifest.json` by rename; it does not publish one `snapshot.json`
before `manifest.json`. The default Cargo features are empty, so the default
build does not require Jieba or `zstd-sys`.

An index built against another revision is ignored and replaced by an exact
scan. This rule applies equally to an in-memory index and a future persistent
index.

### 5.2 Query pipeline

```text
request
  → schema/filter/limit validation
  → immutable revision snapshot
  → scalar pre-filter
  → exact / FTS / dense / sparse route execution
  → candidate deduplication
  → RRF or weighted fusion (rank domains stay separate)
  → optional exact re-score / group / radius policy
  → typed projection and deterministic tie break
```

`a3s-vec` may provide generic fusion primitives. Code remains responsible for
protecting exact identifiers, enforcing a two-chunks-per-file policy, applying
source-range verification, and deciding whether an optional reranker is
admitted.

### 5.3 Embedding and reranking boundaries

The engine stores vectors supplied by the caller. A Code session obtains them
through `EmbeddingProvider`, whose descriptor includes model/provider identity,
dimension, metric, normalization, revision, and limits. A descriptor change
requires a new collection generation or an explicit rebuild.

Reranking is a separate, optional second stage. The baseline uses BM25 rank,
vector rank, and RRF; it does not require a Cross-Encoder or LLM. A future
host-injected neural reranker receives only a bounded, already-verified
candidate set and has its own timeout, memory, cancellation, and egress policy.

## 6. Workspace and `vgrep` architecture

### 6.1 Canonical record

For each admitted text file, Code creates one immutable catalog generation. A
chunk record contains at least:

```text
chunk_id, workspace_id, session_id, relative_path,
start_byte, end_byte, start_line, end_line, language,
symbol/breadcrumb metadata, content_digest, source_revision, text
```

The same record is projected into `a3s-vec` fields. A file maps to one logical
partition so replacement and deletion are atomic:

```text
file change → tombstone old partition → read/chunk/digest
            → publish lexical data → embed bounded batch
            → publish vector partition → mark generation ready
```

The first Code integration keeps this collection session-scoped and
memory-resident. Persistent workspace caches are deferred until locking,
encryption, stale-generation, and multi-process ownership are specified in a
separate ADR.

The delivered integration is a differential shadow, not an authority switch.
One validated embedding batch is published to the existing Memory index and
mirrored into the Vec collection under a shared publication gate. Memory owns
hits, ordering, scores, searched-record counts, truncation, fallbacks, and
errors; Vec contributes only comparison counters and bounded diagnostics. A
shadow failure or mismatch therefore cannot change serving results. The
schema-4 `workspace-retrieval-v3` qualification on 2026-09-02 matched 120/120
queries in both hybrid arms, retained 25,000 records per arm, and released both
engines on close. Promotion still requires the later cross-platform, RSS/disk,
recovery, privacy, and rollback gates.

### 6.2 Routes and guarantees

| Route | Index use | Model use | Guarantee/fallback |
| --- | --- | --- | --- |
| `rg` | none | none | Exhaustive direct scan; works before indexing |
| `fts` | `a3s-vec` FTS | none at query time | Deterministic lexical rank; requires an existing indexed catalog |
| `vector` | `a3s-vec` dense index | query Embedding | Semantic candidates only from the matching descriptor/generation |
| `hybrid` | FTS + dense (+ exact/structural channels) | query Embedding | RRF, optional bounded Code rerank, source verification |

The existing Code BM25 implementation is retained as a golden reference while
`a3s-vec` FTS is brought to parity. After the migration gate it is removed from
the workspace path. The BM25 *semantics* remain available through `a3s-vec`;
SQLite and `sqlite-vec` are not hidden fallback dependencies.

### 6.3 Tool contract

The model-facing tool remains one governed `search`/`vgrep` tool with an
explicit route, limit, path/type filters, freshness state, and status. It
returns file-relative paths, byte/line ranges, channel ranks, revision/digest
evidence, and a bounded preview. It never returns a vector as source evidence
and never logs query text or snippets in diagnostics.

If semantic coverage is partial or unavailable, the result explicitly reports
`building`, `degraded`, or a typed fallback and still returns exact/FTS hits.
The host can require `ready` with a bounded wait, but session construction and
ordinary exact search never wait for a model.

## 7. Model plane and Intel macOS policy

### 7.1 Provider lifecycle

- **Local provider:** the host admits a digest-locked artifact and runtime
  capabilities before session creation. Installation is explicit; construction
  never downloads a model.
- **Remote provider:** the host supplies endpoint, credential reference, and a
  separate source-egress grant. The engine receives vectors, not credentials.
- **Reranker:** absent by default. If enabled, it is another typed provider;
  Embedding authorization does not imply Reranker authorization.

All providers use bounded batches, cancellation, timeout, retry budgets,
dimension/finite-value validation, and redacted errors. Query and document
vectors must come from the same descriptor.

### 7.2 Platform gate

macOS 12 Monterey is unsupported and is not an open engine gate. The former
self-hosted `a3s-macos-12` workflow is not a release requirement. Hosted Intel
CI is `macos-15-intel` with deployment target 15.0. Exact and lexical search
stay available without a model. A local semantic provider is a Code and host
concern and is not an engine correctness gate.

## 8. Migration safety

The migration is a staged replacement, not a flag flip:

The first dual-projection stage is now implemented as the Code developer
shadow described in section 6.1. The arrows after that stage remain planned
gates; they are not implied by a successful shadow build.

```text
current Code catalog + BM25 + a3s-memory VectorIndex
          │ freeze fixtures and status contract
          v
dual projection (same chunks, one embedding call, compare results)
          │ parity + recovery + resource gates
          v
a3s-vec-backed opt-in vgrep
          │ host/SDK/TUI qualification and rollback rehearsal
          v
remove Code workspace BM25 + SQLite/sqlite-vec vector path
          │ retain a3s-memory for long-term Agent memory
          v
stable optional vgrep; the checked-in HNSW, IVF, and DiskANN indexes stay derived state
```

During dual projection, both systems consume the same catalog and provider
vectors; they must not independently scan files or double Embedding requests.
The old path remains a read-only oracle and rollback target until all gates are
green. Removal is allowed only after a release reports parity for correctness,
stale-source rejection, privacy, latency, memory, and close. macOS 12
Monterey is not that gate.

## 9. Architectural decisions still requiring an ADR

These choices are intentionally not hidden in implementation details:

- native `a3s-vec` format versus an importer/exporter for Alibaba zvec files;
- session-ephemeral versus encrypted persistent workspace indexes;
- the first supported Intel local Embedding runtime and artifact license;
- whether a true neural Reranker is worth its latency/egress cost;
- the stable `vgrep` wire/SDK schema and whether it eventually becomes a
  standalone `A3S-Lab/Vgrep` repository;
- compatibility-lock entries and release support policy for the new crate.
