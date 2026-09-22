# a3s-vec first-principles review (2026-09-22)

This review judges the six engine invariants in
[`crates/vec/ARCHITECTURE.md`](../crates/vec/ARCHITECTURE.md) section 1 against
the current `crates/vec` tree, then judges the cross-project retrieval boundary
those docs already state. It is a source review of published `main` plus a
separate reading of the uncommitted working tree. It does not replace
[`retrieval-platform-architecture-review.md`](retrieval-platform-architecture-review.md),
which examined an older Vec revision.

## 1. Integration state

| Record | Revision | What it is |
| --- | --- | --- |
| Published Vec `main` (`crates/vec` `HEAD`) | `880d547be07b2b863cf960a960809d21b37564ac` | Engine tree this review judges. `Cargo.toml` version is `0.1.4`. Commit subject: `docs: close 0.1.4 post-publish checklist with tag and checksum.` |
| Parent `a3s` gitlink for `crates/vec` | `e6d067fcd4ff5ac536c5c9ee2fc8ea837f193576` | Composition pin recorded by `git ls-tree HEAD crates/vec`. |
| Engine doc pin inside `crates/vec/ARCHITECTURE.md` section 2 | `13585ccd3f956f6cb7d669b2ee6acc7096fca03d` | The sentence that says which revision that file documents. |
| Parent architecture and roadmap pin | `416140ec5f9bd6fc8030f9f17735c0b10d099c99` | What the three parent retrieval docs still call the current engine (`a3s-vec` 0.1.1). |
| Older audit pin | `fbb1081d8ccdf926ceef36cbdd51fc1e891e0924` | Revision examined by `docs/retrieval-platform-architecture-review.md`. |

The gitlink and the checkout are not the same commit. That is integration
drift. It is not an engine invariant failure, and this review does not move
the pin.

## 2. Published `main` (`880d547`)

Judgments below use only files at published `main`. Each label is one of
`holds`, `does not hold`, or `not evidenced`.

### 2.1 Documents are the authority — `holds`

Recovery loads the snapshot named by the manifest and applies only committed
WAL records. `StorageHandle::open` reads `snapshot::read`, replays WAL from
`checkpoint_revision + 1`, and rejects a record whose `revision` is not the
next monotonic value (`crates/vec/src/storage/mod.rs`). After documents and
schema are recovered, `Collection::open` rebuilds the index registry from
those documents when the derived cache misses
(`crates/vec/src/collection.rs`). `IndexRegistry::build` constructs vector,
scalar, and FTS generations from `DocumentMap`
(`crates/vec/src/index/mod.rs`). The derived cache and DiskANN sidecar are
written after that recovery and are not the manifest commit point
(`crates/vec/src/storage/mod.rs`, `write_index_cache` /
`write_diskann_file`).

### 2.2 Schema owns meaning — `holds`

Writes coerce JSON adapter values to the schema field type or reject them.
`normalize_doc` replaces `FieldValue::Json` through `coerce_json_field`, which
returns `JSON adapter value is incompatible with {data_type}`
(`crates/vec/src/collection/validation.rs`). `matches_field_type` accepts only
`FieldValue::Null` or a value whose `data_type()` is the schema type; it does
not treat every JSON value as every field. Query execution checks the schema
dimension before search: `validate_dense_payload` returns
`query vector dimension mismatch: expected {expected}, got {}`
(`crates/vec/src/collection/query_contract.rs`). Altering a field's data type
or dimension is rejected because it would invalidate existing data
(`crates/vec/src/collection.rs`).

### 2.3 A query observes one revision — `holds`

`Collection::query` calls `snapshot_state` before planning or scoring
(`crates/vec/src/collection/query_api.rs`). `snapshot_state` copies `schema`,
`docs` (`Arc`), `revision`, and `indexes` (`Arc`) while holding the state read
lock (`crates/vec/src/collection.rs`). Planning and `execute_query_with_candidates`
then use that snapshot. A later writer replaces the `Arc` values in
`CollectionState` only after `storage.append` returns
(`crates/vec/src/collection/mutation.rs`); it does not mutate the snapshot the
query already copied.

### 2.4 Durability is explicit — `holds`

`StorageHandle::append` refuses a non-sequential revision, writes the WAL
record, then publishes the manifest with the new revision and committed WAL
byte boundary before it returns (`crates/vec/src/storage/mod.rs`). Document
mutations assign `state.docs`, `state.indexes`, and `state.revision` only
after that append returns (`crates/vec/src/collection/mutation.rs`). The
default policy is `Durability::Always`, whose comment is `Sync the WAL file
before acknowledging each mutation` (`crates/vec/src/config.rs`).
`Durability::Interval` is `Sync after the configured operation/byte
threshold`. `should_checkpoint` is true only when `wal_max_ops` or
`wal_max_bytes` is set and reached (`crates/vec/src/storage/mod.rs`).
`maybe_checkpoint` then calls `storage.checkpoint(..., true)`
(`crates/vec/src/collection/checkpoint.rs`). `Durability::Manual` is `Only
sync when Collection::flush is called`, and `flush` calls
`storage.checkpoint(..., true)` (`crates/vec/src/collection.rs`). Those are
the three named policies, not an implied fsync.

A checkpoint writes a new generation snapshot, then publishes `manifest.json`
by atomic rename (`crates/vec/src/storage/manifest.rs`, `fs::rename`). Prune
of older WAL and snapshot generations runs only after that manifest commit
(`crates/vec/src/storage/mod.rs`). `StorageHandle::create` returns
`cannot create a collection through a read-only handle` before it creates
files when `read_only` is set.

`Durability::Interval` with neither `wal_max_ops` nor `wal_max_bytes` set
does not add an extra sync, because `should_checkpoint` uses `is_some_and`
on those options (`crates/vec/src/storage/mod.rs`). That matches the written
threshold rule. The default remains `Always`.

### 2.5 Approximation never breaks correctness — `holds`

`build_kind` constructs `HnswIndex`, `IvfIndex`, and `DiskannIndex` for those
index types. Only `IndexType::Flat` constructs `FlatIndex`
(`crates/vec/src/index/build.rs`). HNSW builds a hierarchical graph
(`crates/vec/src/index/hnsw.rs`). IVF trains centroids and ordinal postings
(`crates/vec/src/index/ivf.rs`). DiskANN builds a Vamana graph and an optional
product quantizer (`crates/vec/src/index/diskann_index.rs`).

`IndexRegistry::candidates` returns `None` when `index.source_revision` is not
the captured collection revision, when `is_linear` is set, or when the
requested metric differs. The comment on that function states that `None` is
the caller's signal to use the exact oracle
(`crates/vec/src/index/mod.rs`). `execute_vector` exact-scores the candidate
set, or scans every document when there is no candidate set
(`crates/vec/src/collection/query_engine.rs`). Quantized or missing
coordinates make `exact_unquantized_f32_score_at` return `None`, so
`rerank_unquantized_candidates` returns `None` and `execute_vector` exact-scores
the same candidate documents. FTS search returns `None`
when `source_revision` does not match (`crates/vec/src/index/fts.rs`).
`plan_fts_candidates` then publishes `fts_scores: None` and `used_fts_index:
false`, and `execute_fts` tokenizes the captured documents and scores them
with scan BM25 (`crates/vec/src/collection/query_engine.rs`).

ANN candidate selection can omit documents. The public score of a returned
document is still the exact score of the captured document generation, and a
stale or missing generation does not answer by itself.

### 2.6 The portable path is the default — `holds`

`Cargo.toml` sets `default = []`. `jieba` and `async` are optional features.
`zvec-core` is pinned with `default-features = false`. The default query
backend is `IoBackend::Positioned`
(`crates/vec/src/config.rs`). `into_random_access` selects
`RandomAccessReader::Positioned` for that backend
(`crates/vec/src/storage/derived_file.rs`). Mmap is an explicit
`IoBackend::Mmap` selection.

`score_f64.rs` uses NEON on `aarch64` and SSE2 on `x86_64`, which are the
baseline ISAs of those targets, and calls `f32_scalar` / `f64_f32_scalar` /
`f64_scalar` on every other target. Correctness does not require AVX2,
`io_uring`, a C compiler, or the mmap backend. `crates/vec/ARCHITECTURE.md`
section 1 names runtime AVX2; the kernel that is actually compiled is SSE2 or
NEON. That sentence is ahead of the code. It does not make the portable
default false.

The Tokio module is compiled only under `#[cfg(feature = "async")]`
(`crates/vec/src/collection.rs`). `query_async` documents that it uses the
same snapshot, planner, fallback, and scoring path as `query`
(`crates/vec/src/collection/async_api.rs`). `crates/vec/src/lib.rs` keeps the
external kernel private: a `compile_fail` doctest rejects `use a3s_vec::core`.

## 3. Uncommitted working tree

These paths are modified in the `crates/vec` checkout and are not part of
`880d547`. None of the judgments in section 2 depend on them. This review
does not commit, revert, or restage them.

| Path | What the uncommitted diff does | Released? |
| --- | --- | --- |
| `.gitignore` | Adds `/target-wsl/`. | No |
| `BENCHMARKS.md` | Adds a local Windows and WSL smoke table dated 2026-09-20. | No |
| `Cargo.toml` | Adds `[profile.test] opt-level = 1`. | No. This changes test builds, not the published library profile. |
| `RELEASE.md` | Documents that test profile. | No |
| `src/index/ordinal_map.rs` | Removes `#[cfg(test)]` from `get_mut`. | No |
| `src/index/vamana.rs` | Updates reverse edges in place, caches RobustPrune source distances, and tracks the bounded-search pool with a `HashSet`. | No |

On the overflow path, the uncommitted `vamana.rs` inserts the reverse edge
into the graph and then calls `robust_prune` with an empty candidate slice.
`robust_prune` still starts from `graph.get(source)` chained with that slice
(`crates/vec/src/index/vamana.rs`). The published function, by contrast,
passes the cloned neighbor list in the slice before the graph is updated.
This review does not claim those two constructions select the same neighbors.
That equivalence is `not evidenced` here, and it is not a property of
published `880d547`.

## 4. Cross-project retrieval boundary

The boundary judged here is the one already stated for this engine: one
workspace source revision, chunk catalog, and collection describe the same
bytes; exact search stays authoritative; Vec is not the public result source
and does not own CLI, provider, or serving policy.

| Obligation | Judgment | Evidence in this tree |
| --- | --- | --- |
| Workspace source revision, chunk catalog, and collection describe the same bytes | `not evidenced` | `Doc` stores `pk`, `score`, `internal_id`, `fields`, and `vectors` (`crates/vec/src/doc.rs`). It has no workspace revision or chunk catalog. `mutate_documents` persists the caller-supplied `&[&Doc]` batch (`crates/vec/src/collection/mutation.rs`). Those paths do not compare the bytes to a workspace source revision. |
| Exact search stays authoritative | `holds` | Section 2.5. Returned vector scores come from the captured document generation or from unquantized coordinates derived from it. A stale ANN or FTS generation is not used; the query exact-scores the captured documents instead. |
| Vec is not the public result source and does not own CLI, provider, or serving policy | `holds` | `crates/vec/src/embedding.rs` states that the database never downloads a model or performs network I/O implicitly. Dense and sparse embedding traits are caller-owned. `crates/vec/src` contains no `vgrep` command, CLI, or provider client. |

Source-file verification (a current file read, digest, and byte range) is also
`not evidenced` in this tree. The parent architecture assigns that check to
Code. The engine's exact score is authoritative for stored vectors, not for
workspace files.

`crates/vec/ARCHITECTURE.md` section 2 still says the delivered Code adapter
is pinned to Vec `13585ccd` and that only the Memory result is published.
That pin is not `880d547`. The ownership claim matches the engine tree's lack
of a serving surface. The commit it names is not this checkout.

## 5. Where the parent retrieval docs disagree with this tree

### 5.1 `docs/retrieval-platform-architecture.md`

- Section 4.1 says the current root pin is Vec `416140ec` (`a3s-vec` 0.1.1)
  and that tagging remains blocked on a macOS 12 Intel runtime report. The
  checkout is `880d547` at crate version `0.1.4`. The parent gitlink is
  `e6d067f`, which is neither of those commits.
- Section 5 draws the engine as `api/`, `planner/`, `codec/`, and `testkit/`,
  with an index list of flat, scalar, FTS/BM25, HNSW, IVF/SOAR, and DiskANN.
  The checked-in tree is the `src/` layout in `crates/vec/ARCHITECTURE.md`
  section 3 (`collection/`, `index/`, `storage/`, `tests/`). There is no
  `planner/`, `codec/`, or `testkit/` directory.
- Section 7.2 makes `x86_64-apple-darwin` with deployment target 12.0 the hard
  platform gate and says Vec `416140ec` provides a self-hosted `a3s-macos-12`
  workflow. `crates/vec/ROADMAP.md` (VEC-R1) and `crates/vec/ARCHITECTURE.md`
  say Monterey is unsupported and that workflow has been removed.
  `crates/vec/.github/workflows/ci.yml` runs `macos-15-intel`. No
  `a3s-macos-12` workflow is in that directory.
- Section 8's migration diagram still places "advanced ANN/persistent cache"
  after stable `vgrep`. Published `main` already builds HNSW, IVF, RaBitQ,
  Vamana, and DiskANN, and it already has a derived index cache that is
  ignored when stale (`crates/vec/src/index/cache.rs`).

Section 5.1's publication rule (snapshot plus WAL are authoritative, a stale
index is ignored, a write is acknowledged only after the configured WAL
policy, the manifest is the commit point) matches section 2 of this review.
Section 4's ownership row for `a3s-vec` also matches section 4 above: the
engine does not own workspace traversal, credentials, model downloads, or UI.

### 5.2 `docs/retrieval-platform-roadmap.md`

- The 2026-09-04 status block and section 1 say the current Vec pin is
  `416140ec` / `0.1.1`, and that macOS 12 Intel runtime run `33811715564` is
  queued. That is not the checkout, and Monterey is not an engine exit gate
  in `crates/vec/ROADMAP.md`.
- Section 2.1 still marks the cross-platform row as partly closed because an
  actual macOS 12 Intel runtime remains open, and it cites hosted CI run
  `33810337678` for pin `416140ec`.
- The historical 2026-08-30 block says real ANN, indexed FTS, and
  scale-bearing quantization remain future work. That block is labeled
  historical. Read as a description of `880d547`, it is false: section 2.5
  cites live HNSW, IVF, DiskANN, and indexed FTS. The later "current
  baseline" paragraphs in the same file do say those indexes exist, but they
  attach that claim to `416140ec`.

P7 in that roadmap ("remove duplicate SQLite/BM25") is a product gate. Nothing
in the engine tree shows that removal. Leaving P7 open does not disagree with
an engine invariant.

### 5.3 `docs/retrieval-platform-architecture-review.md`

That file examined Vec `fbb1081` and reports five passing tests, a rustfmt
failure, and 533 Clippy diagnostics. Its executive decision treats the engine
as a pre-migration prototype. Those observations are about that revision.
They are not observations of `880d547`. This review did not re-run fmt,
Clippy, or the test suite; the non-goal is a fresh benchmark or test gate.
The source checks below are the carried-forward defect re-check.

| Old ID | Old claim | Against `880d547` |
| --- | --- | --- |
| VEC-P0-01 | HNSW, IVF, and DiskANN delegate `build` and `search` to `FlatIndex`. | **Withdrawn.** `crates/vec/src/index/build.rs` dispatches those types to `HnswIndex::build`, `IvfIndex::build`, and `DiskannIndex::build`. |
| VEC-P0-02 | Collection FTS always re-tokenizes every document; postings are not the query authority. | **Withdrawn.** `crates/vec/src/index/fts.rs` searches postings when the revision matches. `execute_fts` scans only when indexed scores are absent (`crates/vec/src/collection/query_engine.rs`). |
| VEC-P0-03 | Checkpoint renames one `snapshot.json` and then writes `manifest.json`, so a crash pairs the old checksum with the new snapshot. | **Withdrawn.** Checkpoint writes generation `snapshot-<generation>` and publishes the manifest afterward. Recovery reads the snapshot the manifest names (`crates/vec/src/storage/mod.rs`). |
| VEC-P0-04 | WAL replay does not carry a revision, so restart can disagree with the manifest. | **Withdrawn.** `WalRecord` has `revision`, and `open` rejects a non-monotonic frame (`crates/vec/src/storage/mod.rs`). |
| VEC-P0-05 | `create` writes files when `read_only` is true, and read-only close cannot checkpoint. | **Withdrawn.** `create` returns `permission_denied` before creating the collection when `read_only` is true (`crates/vec/src/storage/mod.rs`). |
| VEC-P0-06 | WAL and snapshot reads allocate without a budget. | **Withdrawn.** Manifest reads are capped (`MAX_MANIFEST_BYTES` in `crates/vec/src/storage/manifest.rs`). Snapshot, WAL replay, index-cache, and DiskANN sizes go through `StorageCeilings` (`crates/vec/src/storage/mod.rs`). |
| VEC-P0-07 | The default build reaches `zstd-sys` through Jieba. | **Withdrawn.** `default = []` and `jieba` is optional (`crates/vec/Cargo.toml`). |
| VEC-P1-01 | Schema WAL replay checks the name and does not apply the schema. | **Withdrawn.** `apply_operation` validates and installs `WalOperation::Schema` and `SchemaOnly` (`crates/vec/src/storage/mod.rs`). |
| VEC-P1-02 | A dimension mismatch is skipped and looks like an empty result. | **Withdrawn.** `validate_dense_payload` returns `invalid_argument` (`crates/vec/src/collection/query_contract.rs`). |
| VEC-P1-03 | Any `FieldValue::Json` matches every schema field. | **Withdrawn.** `matches_field_type` does not special-case JSON, and `coerce_json_field` rejects incompatible JSON (`crates/vec/src/collection/validation.rs`). |
| VEC-P1-04 | Quantized codecs have no exact re-score. | **Withdrawn** as a current search defect. Quantized coordinates do not supply the public score; `rerank_unquantized_candidates` returns `None` and the document path scores the stored vector (`crates/vec/src/collection/query_engine.rs`). |
| VEC-P1-05 | `IoBackend` is recorded and not executed. | **Withdrawn.** `IoBackend::Positioned` is the default, and `derived_file.rs` opens that reader. |
| VEC-P1-06 | The architecture's `api/`, `planner/`, `codec/`, and `testkit/` layout and the named integration tests are absent. | **Withdrawn** for the engine tree. `crates/vec/ARCHITECTURE.md` section 3 matches `src/` and `tests/` (including `durability.rs` and `concurrency.rs`). **Still true of the parent architecture:** section 5 of `docs/retrieval-platform-architecture.md` still draws the old layout. |
| VEC-P1-07 | `pub mod core { pub use zvec_core::*; }` is public. | **Withdrawn.** `crates/vec/src/lib.rs` has no `zvec_core` re-export, and a `compile_fail` doctest rejects `use a3s_vec::core`. |
| P2 fmt/Clippy/five tests | rustfmt fails, Clippy reports 533 diagnostics, and only five tests exist. | **Withdrawn** as a description of this tree. `crates/vec/tests/` contains the durability, concurrency, ANN, and FTS suites named in `crates/vec/ARCHITECTURE.md` section 3. A fresh fmt/Clippy result is `not evidenced` in this review because those commands were not re-run. |

Copying VEC-P0-01 forward as an open engine defect would be a stale finding.
It is withdrawn.

## 6. Ordered plan

No engine change is justified by a failed invariant on published `880d547`.
The items below are documentation and ownership corrections. They are ordered
by dependency and by the cost of being wrong. Each one names the finding, the
invariant it makes more true, and the observable outcome.

### 6.1 Withdraw the stale engine audit in the parent retrieval docs

- **Finding:** section 5.3. `docs/retrieval-platform-architecture.md` still
  calls `docs/retrieval-platform-architecture-review.md` the open-blocker
  record, and that record's P0 rows describe `fbb1081`.
- **Invariant:** approximation never breaks correctness. The same edit also
  stops the stale durability, schema, and default-feature claims from being
  the operative contract.
- **Depends on:** nothing.
- **Cost of being wrong:** highest. Implementing VEC-P0-01 again would replace
  live HNSW, IVF, and DiskANN with a flat facade, or would refuse to use
  indexes that already fail closed.
- **Done when:** the three parent retrieval docs no longer tell a reader that
  HNSW, IVF, or DiskANN delegate to `FlatIndex`, that collection FTS is only a
  full scan, that checkpoint publishes one `snapshot.json` before
  `manifest.json`, or that the default build requires Jieba/`zstd-sys`. They
  point engine-invariant questions at this review and at checkout `880d547`.

### 6.2 Make the parent engine map and platform gate describe this tree

- **Finding:** sections 5.1 and 5.2. The parent architecture's module diagram
  and the Monterey `a3s-macos-12` gate do not exist in `crates/vec`.
- **Invariant:** the portable path is the default. Monterey is not required
  for that invariant. A second `planner/` / `testkit/` layout would also put
  a second structure next to the document snapshot, which weakens "documents
  are the authority."
- **Depends on:** 6.1, so the architecture and roadmap are edited once, against
  the withdrawn audit rather than against it.
- **Cost of being wrong:** high. A release process that still waits on a
  removed Monterey runner reports an engine failure that the engine refuses
  to treat as a gate (`crates/vec/ROADMAP.md`, VEC-R1).
- **Done when:** parent section 5's tree matches `crates/vec/src` as listed in
  `crates/vec/ARCHITECTURE.md` section 3, and the parent platform gate says
  macOS 12 Monterey is unsupported. `macos-15-intel` in
  `crates/vec/.github/workflows/ci.yml` remains the hosted Intel job.

### 6.3 Keep same-bytes and publication in Code

- **Finding:** section 4. The same-bytes obligation and source-file
  verification are `not evidenced` in `crates/vec`. The engine architecture
  assigns them to Code at Vec `13585ccd`, which is not this checkout.
- **Invariant:** a workspace source revision, chunk catalog, and collection
  describe the same bytes. Putting that check in Code, and not in
  `crates/vec`, also keeps Vec from becoming the public result source or the
  owner of CLI, provider, or serving policy. Adding a workspace walker to the
  engine would make that second obligation less true.
- **Depends on:** 6.1, so the ownership table is not edited to satisfy the
  stale prototype decision.
- **Cost of being wrong:** medium. A second chunker inside the engine can
  store bytes the catalog does not describe.
- **Done when:** the parent ownership table states that a Code-owned check,
  not `crates/vec`, shows one catalog generation's bytes are the collection
  documents, and that only the Code/Memory result is published. `crates/vec`
  still has no workspace walker, CLI, or provider client. The Vec commit named
  by that Code note is a commit the note actually depends on, not `13585ccd`
  presented as `880d547`.

## 7. Refusals

These do not make one of the six engine invariants, or the three boundary
obligations in section 4, more true. They are not scheduled.

- Native async file reads, direct file-backed mmap, binary ANN, positional
  phrase postings, Alibaba C++ storage compatibility, language bindings, and
  network embedding providers. Published `main` already refuses them in
  `crates/vec/ROADMAP.md` (VEC-R2). No failed invariant requires them.
- A new index family, or a rewrite of HNSW, IVF, or DiskANN to match
  withdrawn VEC-P0-01.
- Runtime AVX2 dispatch added only so the section 1 sentence matches
  `score_f64.rs`. SSE2 and NEON already cover the baseline targets, and the
  scalar kernels cover the others.
- Restoring a public `zvec_core` re-export, or making `jieba` a default
  feature.
- Reopening macOS 12 Monterey as an engine release gate.
- Committing, reverting, or restaging the six uncommitted `crates/vec` paths,
  and describing the Vamana micro-optimization, `[profile.test] opt-level = 1`,
  or the local Windows/WSL benchmark notes as `0.1.4` behavior.
- Advancing the parent `crates/vec` gitlink from `e6d067f` to `880d547`.
  The mismatch is recorded in section 1. Moving it does not make an engine
  invariant more true, and it is a separate compatibility-lock change.
- Teaching `crates/vec` to walk a workspace, select an embedding model, or
  expose `vgrep`.
- Re-running the benchmark or full test suite as the closure for this review.

## 8. Source notes

Engine behavior for the six invariants was read from the `crates/vec` paths
cited above. Fmt, Clippy, and the test suite were not re-run. A passing or
failing historical command in
`docs/retrieval-platform-architecture-review.md` is not reused as a result for
`880d547`.
