# A3S Local Retrieval Platform Architecture

Status: proposed cross-project architecture. The engine-level details for
`a3s-vec` live in [`crates/vec/ARCHITECTURE.md`](../crates/vec/ARCHITECTURE.md);
the delivery gates live in
[`retrieval-platform-roadmap.md`](retrieval-platform-roadmap.md).

This document is the ownership and dependency contract for the local retrieval
capability used by A3S Code and the future built-in `vgrep` tool. It describes
how the existing Code retrieval implementation is migrated to `a3s-vec`; it
does not claim that the migration is complete.

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
| `a3s-code-core` (`crates/code/core`) | Workspace admission, one canonical chunk catalog, chunk IDs/ranges/digests, manifest reconciliation, session-scoped collection lifecycle, source verification, Code-specific channel policy, and the model-facing `search`/`vgrep` contract | `WorkspaceServices`, `WorkspaceRetrieval`, `vgrep` request/result/status DTOs, and typed provider ports | A second storage engine, direct provider-specific model code, hidden filesystem access outside the workspace service, or Cloud lifecycle |
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

`A3S-Lab/Vec` now hosts the crate and the A3S repository consumes it through
the `crates/vec` git submodule. Future source changes are committed in the Vec
repository first; the integration repository advances the gitlink only after
the engine checks and compatibility review pass. The remaining onboarding work
is to add the exact component revision to the appropriate release/compatibility
records before advertising a supported release.

No root `Cargo.toml` or root Rust workspace is introduced.

## 5. `a3s-vec` engine architecture

`a3s-vec` is an embedded, synchronous-by-default database. Its internal
modules are replaceable behind contracts, while the logical data model is
stable:

```text
a3s-vec/
├── api/          public lifecycle, collection, errors, iterators
├── schema/       fields, dimensions, metrics, index parameters
├── document/     typed values, nulls, vectors, projections
├── storage/      manifest, WAL, snapshots, locks, recovery
├── index/        flat, scalar, FTS/BM25, HNSW, IVF, DiskANN, quantizers
├── planner/      filters, route execution, fusion, group/radius semantics
├── codec/        versioned serialization and checksums
└── testkit/      reference scan, generators, corruption/fault injection
```

### 5.1 Authority and publication

The document snapshot plus WAL are authoritative. ANN, scalar, and FTS files
are derived and carry the source revision, schema digest, metric, dimensions,
and format version. A write is acknowledged only after the configured WAL
durability policy. A checkpoint publishes data and index files first, then
atomically publishes the manifest. Recovery validates the manifest and replays
only complete WAL frames after its checkpoint.

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

### 7.2 macOS 12 Intel release matrix

The hard platform gate is `x86_64-apple-darwin` with macOS deployment target
12.0:

1. `a3s-vec` core, flat search, FTS, filters, WAL, and recovery compile and
   pass runtime smoke without C/C++, `io_uring`, or required SIMD.
2. `vgrep --rg` and `vgrep --fts` work with no model runtime.
3. A local semantic provider is advertised only after an actual Intel Monterey
   artifact, CPU feature, cold/warm, RSS, cancellation, and offline test.
4. If no local provider qualifies, an explicitly authorized remote provider may
   be used; otherwise semantic mode reports unavailable while exact/FTS remain
   truthful. If product policy requires full semantic support on Intel, the
   release gate is blocked rather than silently claiming support.

## 8. Migration safety

The migration is a staged replacement, not a flag flip:

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
stable optional vgrep; advanced ANN/persistent cache only by ADR
```

During dual projection, both systems consume the same catalog and provider
vectors; they must not independently scan files or double Embedding requests.
The old path remains a read-only oracle and rollback target until all gates are
green. Removal is allowed only after a release reports parity for correctness,
stale-source rejection, privacy, latency, memory, close, and Intel smoke.

## 9. Architectural decisions still requiring an ADR

These choices are intentionally not hidden in implementation details:

- native `a3s-vec` format versus an importer/exporter for Alibaba zvec files;
- session-ephemeral versus encrypted persistent workspace indexes;
- the first supported Intel local Embedding runtime and artifact license;
- whether a true neural Reranker is worth its latency/egress cost;
- the stable `vgrep` wire/SDK schema and whether it eventually becomes a
  standalone `A3S-Lab/Vgrep` repository;
- compatibility-lock entries and release support policy for the new crate.
