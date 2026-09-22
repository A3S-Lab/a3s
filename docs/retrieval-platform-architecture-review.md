# Local Retrieval Platform Architecture Review

Status: review of an explicitly recorded checkout snapshot. The component
revisions below were tested from their owning repositories and are not implied
to be the root repository's current gitlinks. This is an engineering review
and a migration decision record; it is not a release qualification.

Initial review date: 2026-08-29

## Current engine contract (2026-09-22)

The findings, follow-up dispositions, and remediation list below describe the
snapshots they name, principally Vec `fbb1081`. They are not current engine
defects. That includes VEC-P0-04, VEC-P0-05, and VEC-P0-06: `WalRecord` carries
a revision, read-only create returns `permission_denied` before it creates the
collection, and recovery reads are bounded. The current judgment is
[`vec-engine-first-principles-review.md`](vec-engine-first-principles-review.md).

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

## Follow-up: Code Vec shadow promotion candidate (2026-09-03)

Code candidate `1c117f87` (the qualification source revision, documented by
follow-up `3986489e`) advances the workspace-retrieval shadow dependency and
Code-owned vector contract to
Vec `41283f6315906a2737b5a8e8612ac876a8dc9c04` while retaining A3S Memory as
the serving oracle. The candidate's focused shadow tests, full Code CI matrix,
and release-profile qualification passed; the hosted records are
[CI `33747647083`](https://github.com/A3S-Lab/Code/actions/runs/33747647083)
and [performance `33747646942`](https://github.com/A3S-Lab/Code/actions/runs/33747646942).
The 25,000 x 384 Linux x86-64 release profile reports Memory-primary exact /
RRF-only / deterministic p95 9.5296 / 52.4909 / 53.0744 ms and Vec-primary
9.5806 / 55.6923 / 52.8478 ms. Both profiles produced 120/120 comparisons
with zero mismatch or failure; Vec-primary construction and logical bytes were
2,188.8 ms and 54,500,008 versus 2,167.3 ms and 40,177,548 for
Memory-primary. These are directional logical measurements, not RSS or a
cross-platform SLO. The full Code workspace strict rustdoc command also passes
on this candidate.

This is developer-shadow evidence, not serving promotion: the root Cloud
compatibility lock now records the verified Code 8.2.0 graph, while P7 has not
removed the duplicate workspace BM25/SQLite/vector paths. At that date,
macOS 12 Intel runtime, RSS/disk, formal tag/registry, and rollback evidence
were still treated as release gates. macOS 12 Monterey is unsupported and is
not an open engine gate.

## Follow-up: Vec worker-pool cap and candidate refresh (2026-09-03)

Vec `7e3b083e36ab5aeb300b2c45d6d59280971087da` adds a collection-local,
bounded Rayon executor for `AddColumnOption::concurrency` and
`AlterColumnOption::concurrency`. Backfills and candidate-schema validation
reduce results in primary-key order, and invalid nullable-to-required changes
are rejected before WAL publication. The execution-contract test covers
parallel backfill, successful alteration, atomic rejection, and an oversized
worker request; the lifecycle matrix records the parallel schema path as a
measured management operation;
the worker pool is capped by work size, host parallelism, and 256 workers.
The revision-bound Vec CI run
[`33763187419`](https://github.com/A3S-Lab/Vec/actions/runs/33763187419) passed
all ten quality, performance, MSRV, recovery-fuzz, platform, and release-
candidate jobs. The hosted Code release profile for Code `32a70cd7` and this
Vec pin reports Memory/Vec-primary exact p95 8.1480/8.1448 ms, RRF-only
57.8928/63.3172 ms, and deterministic 57.5917/58.7085 ms. Both arms compare
120/120 queries with zero mismatches or failures; Vec-primary construction is
3,128.9 ms versus 2,693.0 ms for Memory-primary, and logical Vec bytes are
54,500,008 versus 40,177,548. These are directional logical measurements, not
RSS or a cross-platform SLO.

At that follow-up, the external macOS 12 Intel runtime, process RSS/temp-disk
limits, formal release tag/registry publication, and removal of the Memory
compatibility path were still open. macOS 12 Monterey is unsupported and is
not an open engine gate. Enterprise GA for `0.1.4` is the record at the top of
this file, not this follow-up.

## Follow-up: borrowed exact-score performance kernel (2026-09-03)

Vec `41283f6315906a2737b5a8e8612ac876a8dc9c04` is now the root candidate;
it includes executable Vamana RobustPrune occlusion/saturation controls and
standalone FP16/INT8/INT4 index quantization with reopen/cache coverage for
every scalar quantizer, in addition to the borrowed
exact-score kernel from the preceding candidate.
The exact dense executor traverses borrowed native vector storage instead of
allocating a converted `f64` buffer for every candidate, and cosine query
norms are computed once per query across Flat, HNSW, Vamana, and DiskANN
full-vector traversal. The authoritative `f64` ranking contract and exact
re-ranking invariant are unchanged; focused vector-score equivalence tests,
the all-feature/no-default suites, Rust 1.75 tests, Clippy, rustdoc, package,
and audit pass locally. On the same 100,000 x 128 Windows one-worker fixture,
three-process medians moved from 43.42 to 34.58 ms Flat p50 and from 2.19 to
1.73 ms HNSW p50; HNSW total build moved from 240.74 to 188.29 s and Recall@10
remained 0.6000. zvec remains faster at this large scale (about 5.6x Flat and
5.1x HNSW p50) because its comparison side is a native C++ wheel; the ratio is
directional, not a universal or same-refiner claim. The revision-bound hosted
run is [Vec CI `33705867979`](https://github.com/A3S-Lab/Vec/actions/runs/33705867979)
and is the revision-bound hosted validation for this candidate;
the preceding documentation run `33698297563` and implementation run
`33696717206` also completed successfully with the platform CSVs and versioned
release-candidate artifacts. Neither hosted run was a macOS 12 Intel runtime
qualification. macOS 12 Monterey is unsupported and is not an open engine
gate.

## Follow-up: Vec scale, WAL, and release-gate evidence (2026-09-03)

The owning `A3S-Lab/Vec` repository now has methodology/documentation revision
`d6b83458e0a1042a59e877d5df1511297b60f2fa` over hosted benchmark pin
`7f3e2a98944f495048a821e3afb1744d580b1559` and CI-hardening revision
`9ed701ae72e45f7b8f7be9c7db943ed4b64f93f4` over scale-control revision
`9a9c0850cf09ec90f54a9f7e9d8be3ca12af95f1`, documentation follow-up revision
`c2344dda55cc36e4c3e3d3fbab3e3512a12b18e8`, lifecycle/performance revision
`9031943b53577e14f805692a3bfb3a3237b5072f`, and implementation revision
`dbd4a75df4de5e02b4ddb106700617e686186a56`. The lifecycle/performance
revision adds the asserted 16-row lifecycle/resource/maintenance performance
matrix. Hosted CI run
[`33690839419`](https://github.com/A3S-Lab/Vec/actions/runs/33690839419) passed
all ten jobs for the latest revision, including the corrected rounded-total
scale gate, zvec companion syntax check, and comparison-methodology
documentation. The underlying engine gate remains
[`33686399240`](https://github.com/A3S-Lab/Vec/actions/runs/33686399240) for
the CI-hardening revision. This is a later evidence record;
the findings and command counts in the historical sections below describe the
older review snapshots and must not be read as the current test count.

The current engine validation includes 270 passing unit/integration tests plus
four doctests with all features (267 unit/integration tests plus four doctests
with the default and no-default feature sets), a release-mode suite, Rust 1.75
MSRV checks, strict rustfmt/Clippy, rustdoc, locked packaging, and an audit with
no vulnerability or unsoundness advisory. Recovery fault boundaries and a
256-run libFuzzer smoke pass. Hosted CI run
[`33670661773`](https://github.com/A3S-Lab/Vec/actions/runs/33670661773) passed
all ten jobs, including Linux x86_64/arm64, Windows x86_64, macOS arm64 and
hosted Intel, the public/per-platform performance CSV gates, MSRV, recovery
fuzz, and the versioned release candidate. The later lifecycle follow-up run
[`33679131820`](https://github.com/A3S-Lab/Vec/actions/runs/33679131820) reran
the same hosted gates for the documentation-pinned revision; the subsequent
run `33685402239` exposed a three-decimal build-total tolerance edge case, which
`9ed701a` corrected and re-ran as `33686399240`; `d6b8345` then documented the
native-wheel and exact-refinement asymmetries in the cross-project comparison.

The scale harness now emits a shared 20-column CSV for a3s-vec and an opt-in
zvec companion, while the lifecycle harness emits 16 management-plane rows.
The pre-kernel table (superseded by the `e27524d` follow-up above) showed zvec
at about 7.0x lower flat query p50, 6.4x lower HNSW query p50, and 2.8x shorter
total HNSW build. The recorded a3s-vec side is Cargo's portable baseline against zvec's native
wheel, and a3s-vec exact-reranks authoritative HNSW candidates while the
zvec optional refiner is disabled; the HNSW ratio is therefore directional,
not a compiler-level or same-refiner claim;
a3s-vec's HNSW Recall@10 was 0.6000 versus zvec's 0.5719 median (range
0.5625-0.5781). The full table, controls, and lifecycle caveat are in
[`crates/vec/BENCHMARKS.md`](../crates/vec/BENCHMARKS.md). This is capacity
evidence, not a universal ranking or a migration approval. The separate
macOS 12 Intel runtime and formal publication gates were open in that
comparison because no qualifying self-hosted runner was registered. macOS 12
Monterey is unsupported and is not an open engine gate.

## Follow-up: Vec in-process snapshot concurrency (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`0236e0d0cd9d4c203a689567e52a0591697260a2` (feature commit
`2a1346a6ab0ec801414b241177f15408470fa16f`). The collection lock and snapshot
boundaries were audited and exercised through cloned public `Collection`
handles. Every mutation takes the per-collection writer mutex before the state
write lock, while a reader copies schema, documents, and revision under one
state read lock. No production-code correction was needed for this slice.

Three deterministic fixtures now cover the in-process contract. Simultaneous
disjoint updates to one document retain both patches and advance two separate
revisions. An iterator captured at revision 1 remains unchanged while another
handle publishes a complete replacement batch at revision 2. Two synchronized
readers racing 64 repeated two-document upserts observe equal epochs for both
documents in every round, never a partially published batch. Manual durability
is selected explicitly in these fixtures to isolate logical synchronization
from fsync latency; the same WAL/manifest commit path still executes.

The concurrency-only suite passed 20 consecutive runs. On arm64 macOS 26.6.2
with Rust 1.98.0, the default, no-default, and all-feature suites each contain
60 passing unit/integration tests plus four compile-fail doctests. Rustfmt, both
strict Clippy variants, and all-feature rustdoc pass; the complete default suite
also passes on Rust 1.75.

The coherent-reader/serialized-writer gate was **closed for that follow-up's
in-process exact surface**. Multi-process contention/stale-lock diagnostics,
the supported-platform matrix, larger generated differential corpora,
durability fault injection, real ANN/indexed FTS, migration benefit, and Code
shadow migration were open in that follow-up. That is not a current claim that
HNSW, IVF, or DiskANN delegate to a flat index.

## Follow-up: Vec differential exact-search oracle (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`fd8aecf972ed611eb39f88c20f4ccd6fd5aa319d` (feature commit
`90459f971e9f9e607464c32d4f4d8587bc41fd8a`). Two independent reference
fixtures now compute dense/sparse scores and scan-BM25 rankings without calling
the Vec scoring implementation. The vector fixture covers all four exact
metrics, scalar filters, L2 and similarity radius semantics, top-k, and
primary-key tie-breaking. FP64 candidates remain ordered by their exact `f64`
score; only selected results cross the checked public `f32` boundary.

The FTS fixture defines the BM25 corpus as documents that contain the queried
text field, including explicit nullable and empty-text cases. It preserves
valid average document lengths below one. Plain whitespace-separated
`query_string` terms execute, while boolean, phrase, wildcard, fielded, boost,
range, and related unimplemented syntax returns `NotSupported` rather than
being approximated as bag-of-words. Supplying both FTS expression forms is an
invalid ambiguous request.

The initial differential run reproduced four semantic failures: FP64 close
scores were narrowed before ordering, missing nullable text fields changed
BM25 corpus statistics, advanced FTS syntax was silently approximated, and a
negative L2 radius was squared and accepted. Expanded fixtures then reproduced
the subunit-average-length clamp and additional query syntax acceptance. The
implementation now ranks on finite `f64` scores before top-k/public narrowing,
uses precomputed text-bearing corpus statistics, preserves the real positive
average length, rejects negative L2 radius, and fails closed on unsupported FTS
syntax.

On arm64 macOS 26.6.2 with Rust 1.98.0, the default, no-default, and all-feature
suites each contain 57 passing unit/integration tests plus four compile-fail
doctests. Rustfmt, both strict Clippy variants, and all-feature rustdoc pass;
the complete default suite also passes on Rust 1.75.

The deterministic differential gate was **closed for that follow-up's exact
vector/scan-BM25 surface**. The broader P1 exit was still open then for
coherent-reader and serialized-writer concurrency evidence, larger generated
and FTS-filter golden corpora, and supported-platform runs. Real ANN/indexed
FTS, durability fault injection, migration benefit, and Code shadow migration
were open in that follow-up. They are not current statements that HNSW, IVF,
or DiskANN delegate to a flat index, or that collection FTS is only a full
scan.

## Follow-up: Vec native vector encoding contract (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`8e2544de8f30d33ae36545c8106411ad42e84628` (feature commit
`351fbe9367912abc551958006fd8f3d623d7935a`). The schema is now the sole
authority for a vector's physical representation: a write must use the exact
dense/sparse variant declared by the field instead of being accepted merely
because both variants are numeric.

Dense and sparse FP16 persist raw IEEE 754 half-precision bits. The f32 adapter
uses round-to-nearest-even, rejects non-finite and out-of-range inputs, and an
exhaustive unit fixture round-trips every finite FP16 bit pattern. Native INT4
accepts only `-8..=7`; INT8 and INT16 preserve their integer coordinates.
Binary32/Binary64 accept only complete 32-/64-bit chunks and schema dimensions
expressed in aligned bits. INT4 remains one signed coordinate per stored
element: it is not advertised as packed, scale-bearing ANN quantization.

The exact oracle now evaluates L2, inner product, cosine, and MIPS-L2 for every
native numeric dense type and both sparse types. Accumulation is in `f64`, so a
stored FP64 source is not narrowed before scoring; the public `f32` score is
the only checked narrowing boundary. Sparse scoring no longer treats every
metric as a dot product. In that follow-up, binary queries, packed or scaled
INT4 or INT8 index quantization, and refinement were `NotSupported`. That
sentence is not a current engine defect.

Changing sparse FP16 from misleading numeric `f32` values to raw bits required
an explicit compatibility boundary. Manifest, snapshot, and WAL markers now
advance together to format 3; format 2 is rejected before vector payloads can
be decoded or reinterpreted. Vector conversion/access code was split from the
document module, reducing `doc.rs` from 934 to 629 lines.

The initial five-test codec fixture reproduced three failures: cross-type
writes were accepted, INT4/FP16 invalid values were accepted, and binary chunk
widths were unchecked. After those passed, the expanded sparse metric fixture
reproduced L2 being evaluated as inner product. The final matrix contains 48
passing unit/integration tests plus four compile-fail doctests in each of the
default, no-default, and all-feature configurations on arm64 macOS 26.6.2 with
Rust 1.98.0. Rustfmt, both strict Clippy variants, and all-feature rustdoc pass.
The complete default suite also passes on the declared Rust 1.75 MSRV after
pinning `zvec-core`'s broad Rayon range and the test-only tempfile dependency to
compatible releases. That follow-up's optional Jieba chain used Rust 2024
manifests and needed a newer Cargo. The default build does not require Jieba
or `zstd-sys`.

VEC-P1-04 was **closed for that follow-up's exact surface**. The numbered P1
review findings were closed then, while the P1 delivery exit was still open
for differential FTS/vector fixtures, coherent-reader/concurrent-writer
evidence, and supported-platform runs. Real ANN/indexed FTS, the full
durability fault matrix, migration benefit, and Code shadow migration were
also open then. Those sentences are not current engine defects.

## Follow-up: Vec truthful index and query configuration (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`630f36414b6e3b304c4d11f041c7222dc0b775d7` (feature commit
`55fcf229bf2f070439d20252f1faa43e45755cfe`). The exact executor now owns a
small explicit configuration allowlist: Flat schema metrics and query
`metric`/`radius`; scan FTS owns only tokenizer selection. Only Flat is
reported as a ready index, with its revision and document count derived from
the authoritative collection state. Scan FTS is not reported as built, exact
multi-query fusion does not increment ANN telemetry, and the ANN count remains
zero while no ANN executor exists.

HNSW, IVF, RaBitQ, DiskANN/Vamana, inverted index, quantization, refinement,
FTS operator/filter/extra, and physical optimize requests now return
`NotSupported` before mutation. Their typed descriptors remain constructible
for adapter compatibility, but cannot enter a collection schema. Future query
setters do not mutate their payloads, and the execution boundary independently
rejects deserialized future parameters; unknown keys return `InvalidArgument`.
Non-zero segment sizing and add/alter concurrency also return `NotSupported`.
Persisted schemas are revalidated on open.

Six public tests first failed against the prior behavior, reproducing silent
acceptance and false telemetry. The final matrix contains eight option-
contract tests. On arm64 macOS 26.6.2 with Rust 1.98.0, each default/no-default/
all-feature run has 37 passing unit/integration tests plus four compile-fail
doctests; rustfmt, both strict Clippy variants, and rustdoc pass.

VEC-P1-05 was **closed** at that revision. VEC-P1-04 quantized/binary
semantics, real ANN/indexed FTS, differential/concurrency evidence, the
durability fault matrix, migration benefit, and the supported-platform matrix
were still open then. That follow-up called the engine a pre-migration
prototype. That call is not the current engine contract.

## Follow-up: Vec executable runtime configuration (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`ead2801ef5a6672654451455285aac3056c23be2` (feature commit
`8d36d95afb16b5b11a62a5af3f99a43a3b8e2b40`). Inert process and collection
controls were removed from the public surface: memory/thread/logging controls,
selectable I/O backends, mmap, buffer, segment, and the orphaned log/backend
types no longer imply implementations that do not exist.

The retained configuration has an execution owner. `ConfigBuilder` supplies
the process durability default and WAL operation/byte checkpoint thresholds;
`CollectionOptions` supplies read-only mode and an optional durability
override. Defaults are resolved and captured when a collection is created or
opened, so reinitializing the process cannot change an active handle's
acknowledgement policy. Storage tests prove both checkpoint thresholds are
consumed, and unit tests prove inheritance and explicit override precedence.

Three new compile-fail fixtures first reproduced the old misleading surface,
then passed after its removal. The full matrix was repeated on arm64 macOS
26.6.2 with Rust 1.98.0: each default/no-default/all-feature run has 29 passing
unit/integration tests plus four compile-fail doctests; rustfmt, both strict
Clippy variants, and rustdoc pass.

At that follow-up, VEC-P1-05 was partially closed. Process and collection
runtime configuration was truthful then. Future index and query parameters,
schema segment sizing, and schema-evolution concurrency controls lacked an
explicit `NotSupported` outcome or an implemented consumer in that snapshot.
Those items are not current engine defects, and macOS 12 Monterey is not that
gate.

## Follow-up: Vec kernel encapsulation (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`d965cbb8d6e4446fc1aff794d4a4f845f8c714d5` (feature commit
`26545aac41c05761bccd25e5e0341c2186d71085`). The public `a3s_vec::core`
re-export was removed after repository-wide consumer search found no callers.
`zvec-core` remains an internal filtering/tokenization/document-conversion
dependency, so replacing it no longer exposes dependency types as an A3S API
break.

A compile-fail doctest now guards that `use a3s_vec::core` does not compile. It
was first run against the old surface and failed because the import still
compiled, then passed after encapsulation. The full default/no-default/all-
feature matrix was repeated: each configuration has 26 passing unit/integration
tests plus the compile-fail doctest; rustfmt, both strict Clippy variants, and
rustdoc also pass on the same arm64 macOS 26.6.2 / Rust 1.98.0 host.

VEC-P1-07 was **closed** at that revision. VEC-P1-04/05, real ANN/indexed FTS,
the durability fault matrix, concurrency, migration benefit, and the supported-
platform matrix were still open then. That follow-up's pre-migration label is
not the current engine contract.

## Follow-up: Vec contract validation and portable default (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`10412b38c6d530a25960af89517e2cc74e551055` (contract feature commit
`40f83db36e3b200b211e44eafa9204a7082e614e`, nullability evidence commit
`56be158f6a2e1a4993e6d58e9044c344504f0bce`). The A3S composition update that
contains this note pins that gitlink. This follow-up extends the durability
baseline below; it is not a release qualification or permission to begin Code
shadow migration.

The exact query path now resolves its schema field and validates one route,
field type, dense dimension, sparse indices, top-k, radius, metric, and
tokenizer before scoring. The dimension fixtures cover FP16/FP32/FP64 and
INT4/INT8/INT16 fields with L2, IP, cosine, and MIPS-L2. Binary vector scoring
and sparse source-ID queries return explicit `NotSupported` errors instead of
empty results or implicit behavior.

`FieldValue::Json` is now an adapter input rather than an untyped stored value.
Writes canonicalize compatible values into every supported scalar and
non-binary array variant before validation and WAL append; incompatible,
out-of-range, nullability-breaking, and binary JSON values are rejected.
Recovery uses the same normalization and full-document validation. Typed
schema defaults and replacement upserts are also validated against the
resulting complete document.

The default Cargo feature set is now empty. Its normal/build dependency graph
contains no Jieba, `zstd-sys`, or `cc`; Jieba is explicit and retains its
native dictionary build chain. Requesting a Jieba tokenizer without that
feature returns `NotSupported` rather than silently changing tokenization.

Validation was repeated from `crates/vec` on arm64 macOS 26.6.2 with Rust
1.98.0:

| Command | Follow-up result |
| --- | --- |
| `cargo fmt --all -- --check` | **Passed** |
| `cargo clippy --all-targets -- -D warnings` | **Passed** |
| `cargo clippy --all-targets --all-features -- -D warnings` | **Passed** |
| `cargo test` | **26 passed**, 0 failed |
| `cargo test --no-default-features` | **26 passed**, 0 failed |
| `cargo test --all-features` | **26 passed**, 0 failed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | **Passed** |
| `cargo tree -e normal,build` | **Passed**; default graph has no Jieba/`zstd-sys`/`cc` |
| `cargo tree -e all --all-features -i cc` | **Observed** only through explicit Jieba → `include-flate` → `zstd-sys` |

The 26 tests comprise 11 unit/storage tests, eight query/write contract tests,
and seven public durability tests. Finding disposition added by this revision:

| Finding | Follow-up state | Remaining evidence |
| --- | --- | --- |
| VEC-P0-07 | **Snapshot only; not a current defect** | That follow-up still asked for x86_64 macOS 12 qualification. The current default build does not require Jieba/`zstd-sys`, and macOS 12 Monterey is not an open engine gate. |
| VEC-P1-02 | **Closed for the current exact query surface** | Every current numeric dense schema type and metric has negative dimension evidence; dense/sparse/FTS type routing and explicit binary/sparse unsupported paths are covered. Future index implementations must reuse this contract and add differential evidence. |
| VEC-P1-03 | **Closed** | JSON has a documented schema-aware adapter policy, all supported scalar/non-binary array variants have canonicalization evidence, and incompatible/overflow/binary cases fail before persistence. |
| Strict format/Clippy debt | **Closed at `10412b3`** | Both strict Clippy variants, rustfmt, rustdoc, and all three feature-matrix test runs pass. |
| Insufficient integration coverage | **Snapshot follow-up** | That revision had 15 public integration tests. Its remaining concurrency, differential, fault, recall, and platform suites were not closed then. This row is not a current engine gate. |

At that follow-up, VEC-P1-04/05/07, real ANN/indexed FTS, the full durability
fault matrix, concurrency, migration benefit, and supported-platform evidence
were still open. That is not a current claim that HNSW, IVF, or DiskANN
delegate to a flat index, that collection FTS is only a full scan, or that
the default build requires Jieba/`zstd-sys`. Code's retrieval implementation
remained the golden reference in that snapshot, and no old path was removed
by it.

## Follow-up: Vec durability baseline (2026-08-30)

`A3S-Lab/Vec` `main` advanced to
`78840cea784cd29052536519ffecc9217c654091` (feature commit
`0eb6915dec188b418c82f4e2074b9f5a943820b3`). The A3S composition update that
contains this note pins that gitlink. This follow-up is incremental evidence,
not a new release qualification and not permission to start Code shadow
migration.

The new Vec revision makes format version 2's manifest the single commit point,
uses immutable generation-specific snapshots, commits WAL byte boundaries and
monotonic operation revisions, replays schema/backfill state, and makes
read-only close side-effect-free. Recovery reads now have manifest, snapshot,
frame, and total-WAL byte limits. The unused private HNSW/IVF/DiskANN exact
wrappers were removed; the documentation now states that the live collection
path is an exact oracle and that ANN/indexed FTS are not implemented.

Validation was repeated from `crates/vec` on arm64 macOS 26.6.2 with Rust
1.98.0:

| Command | Follow-up result |
| --- | --- |
| `cargo fmt --all -- --check` | **Passed** |
| `cargo clippy --all-targets -- -D warnings` | **Passed** |
| `cargo clippy --all-targets --all-features -- -D warnings` | **Passed** |
| `cargo test` | **18 passed**, 0 failed |
| `cargo test --no-default-features` | **18 passed**, 0 failed |
| `cargo test --all-features` | **18 passed**, 0 failed |
| `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features` | **Passed** |

The 18 tests include restart coverage for insert/update/upsert/delete and
schema add/backfill/rename/drop, plus committed checksum/truncation failures,
partial uncommitted tails, orphan snapshot generations, oversized snapshots,
read-only create/open/close behavior, and checkpoint generation publication.

Finding disposition at that follow-up revision, not the current engine contract:

| Finding | Follow-up state | Remaining evidence |
| --- | --- | --- |
| VEC-P0-01 | **Snapshot only; not a current defect** | This row does not say that HNSW, IVF, or DiskANN delegate to a flat index in the current engine. |
| VEC-P0-02 | **Snapshot only; not a current defect** | This row does not say that collection FTS is only a full scan in the current engine. |
| VEC-P0-03 | **Snapshot follow-up** | That revision tested immutable generations. It is not a current claim that checkpoint publishes one `snapshot.json` before `manifest.json`. |
| VEC-P0-04 | **Closed for format 2** | WAL revisions/operation identities replay monotonically across every DML and schema sequence in the current suite. |
| VEC-P0-05 | **Closed on the review host** | Read-only create, missing-lock open, normal open, and close behavior have integration coverage. Cross-platform lock evidence remains in the platform gate. |
| VEC-P0-06 | **Snapshot follow-up** | That revision had byte budgets and oversized-input tests. This row is not a current engine gate. |
| VEC-P0-07 | **Snapshot only; not a current defect** | The current default build does not require Jieba/`zstd-sys`. macOS 12 Monterey is unsupported and is not an open engine gate. |
| VEC-P1-01 | **Closed for format 2** | Schema plus backfilled documents replay and checkpoint at one revision. A compact schema-delta encoding is optional future work, not current authority. |
| VEC-P1-06 | **Closed as documentation drift** | Vec architecture now describes the checked-in modules/tests and labels future index/planner modules as gated work. Broader API/query/concurrency coverage remains a release gate. |
| Strict format/Clippy debt | **Closed at `78840ce`** | The two strict Clippy variants and rustfmt pass. Three noisy API-annotation lints are explicitly deferred in Cargo lint policy while the prototype API changes; substantive `all`/`pedantic` lints remain enabled. |

At that follow-up revision, the other findings in this table were still open.
They are not the current engine gates in the contract at the top of this
file. In particular, that revision did not validate
query dimension errors, the JSON type escape hatch, quantization semantics,
configuration wiring, the public `zvec_core` escape hatch, real ANN/indexed
FTS, concurrency, migration benefit, or Intel macOS 12. Code's current
retrieval implementation remains the golden reference and no old path is
removed.

## Executive decision for the `fbb1081` snapshot

This decision is not the current engine contract. The `fbb1081` `a3s-vec`
revision was a useful API and portability prototype, but
it is not complete and it has not passed the quality bar required to replace
the Code workspace retrieval path. The existing Code and TUI baselines are
the available behavioral reference at this point; they are not a qualification
of the new engine.

The snapshot review made four decisions. They are not current engine gates:

1. That snapshot kept Code's workspace BM25 and `a3s-memory` vector path as
   the read-only golden reference and did not delete SQLite, SQLite-vector, or
   Code BM25 integration.
2. That snapshot treated `a3s-vec` as pre-migration until the P0/P1 findings
   below were closed. Those findings are not current defects.
3. Semantic Embedding and reranking stayed outside the storage engine.
   Providers had to be admitted by the host; storage did not download models,
   access the network, or discover workspace files.
4. That snapshot refused to advertise Intel macOS 12 support. macOS 12
   Monterey is unsupported and is not an open engine gate.

The revised roadmap is in
[`retrieval-platform-roadmap.md`](retrieval-platform-roadmap.md). A compact,
machine-readable record of the commands and observations is in
[`retrieval-platform-review-evidence.json`](retrieval-platform-review-evidence.json).

## 1. Review scope and pinned revisions

The review covers the local engine, the current Code workspace retrieval
implementation, and the CLI/TUI/SDK boundaries that would have to expose a
built-in `vgrep` capability. It does not approve a model runtime, a remote
provider, a persistent workspace cache, or a future standalone `vgrep` crate.

| Component | Revision examined | Role in this review |
| --- | --- | --- |
| A3S integration root | `55ae01ad44ad755711a758a2d33f33cb7a006cf5` | Composition repository and docs |
| `A3S-Lab/Vec` (`crates/vec`) | `fbb1081d8ccdf926ceef36cbdd51fc1e891e0924` | Candidate Rust retrieval engine |
| `A3S-Lab/Code` (`crates/code`) | `ad6426fb34a3a401c77d51bbdc5c71f2e3d38eec` | Current Code retrieval baseline |
| `A3S-Lab/CLI` (`crates/cli`) | `992fac5701caa2e3bcd77be840e8bad65d10b80a` | Host/configuration boundary |
| `A3S-Lab/TUI` (`crates/tui`) | `8e896bc5c6584ba79669d70d024f1606bdf812fd` | Presentation boundary |
| `A3S-Lab/Test` (`crates/test`) | `2876b1cf761b5768291be534b2119114f7769c4e` (local checkout; expected gitlink `00f601b4e095fec28b60d80abec51f49733cae1d`) | Initialized at a different revision and has a pre-existing modified fixture |

The root worktree contains unrelated, pre-existing changes. The review did
not reset, revert, or overwrite them. The root is not a Rust workspace and no
root `Cargo.toml` was introduced.

The tested component checkout and the root composition are intentionally
recorded separately. At the publication base (`origin/main`
`523c8d5071d001a4241a693a162ccbdc058d41e8`), the root gitlinks were Vec
`fbb1081d8ccdf926ceef36cbdd51fc1e891e0924`, Code
`85b2dc63bb4636ca8f3502a612f5e88c355ad765`, CLI
`650ccaf9b3f37f1f7e227a12ae269ced29b51d31`, TUI
`8e896bc5c6584ba79669d70d024f1606bdf812fd`, and Test
`13f4cfbb5a3ccde797d74723594b848568b1d109`. The newer Code, CLI, and Test
revisions in the table were checked out directly to run the review suites;
they were not silently advanced in the root. A compatibility-pin change and
its own verification are required before those revisions can be claimed as a
release composition.

## 2. Test environment and method

The executable checks were run on:

```text
macOS 26.3 (25D125), arm64 Apple Silicon
rustc 1.97.1, cargo 1.97.1
host: aarch64-apple-darwin
```

This environment is deliberately recorded because it cannot prove an Intel
Monterey claim. No source text, vectors, credentials, prompts, or provider
responses were placed in the evidence record. Real DeepSeek/remote-provider
execution is supplementary and was not used as a substitute for deterministic
correctness or portability tests in this review.

The following commands were used from the owning crate workspaces for the
`fbb1081` snapshot. They are not the current fmt, Clippy, or test status.

| Command | Observed result | Meaning |
| --- | --- | --- |
| `cargo test --manifest-path crates/vec/Cargo.toml --no-default-features` | **5 passed**, 0 failed | That snapshot had five unit tests; this is not the current suite |
| `cargo test --manifest-path crates/vec/Cargo.toml` | **5 passed**, 0 failed | That snapshot's default feature build on arm64; this does not qualify Intel Monterey and is not the current suite |
| `cargo test --manifest-path crates/vec/Cargo.toml --all-features` | **5 passed**, 0 failed | That snapshot's async plus Jieba run on arm64; coverage then was five unit tests |
| `cargo check --manifest-path crates/vec/Cargo.toml --release` | **Passed**, 49 warnings | Release compilation works on arm64; warnings include unused/dead code |
| `cargo fmt --manifest-path crates/vec/Cargo.toml -- --check` | **Failed** | That snapshot's Vec source was not rustfmt-clean |
| `cargo clippy --manifest-path crates/vec/Cargo.toml --all-targets -- -D warnings` | **Failed**: 533 diagnostics | That snapshot's strict lint run failed. This is not current release debt |
| `cargo test -p a3s-code-core --lib agent_api::retrieval_tests` | **12 passed** | Current Code API/retrieval baseline |
| `cargo test -p a3s-code-core --lib agent_api::retrieval_qa_tests` | **5 passed, 1 ignored** | Current Code QA/lifecycle baseline |
| `cargo test -p a3s-code-core --lib tools::builtin::bm25` | **14 passed** | Current lexical golden reference |
| `cargo test -p a3s-code-core --lib tools::builtin::search` | **8 passed** | Current route/tool golden reference |
| `cargo test --bin a3s workspace_retrieval` | **47 passed** | Current CLI host/configuration baseline |
| `cargo test --bin a3s workspace_search_view` | **3 passed** | TUI status projection baseline |
| `cargo test --manifest-path crates/tui/Cargo.toml --lib` | **1076 passed**, 0 failed | TUI rendering/event library baseline |
| `a3s-test check crates/test/examples/web-smoke.acl --json` | **Passed** | ACL validation only; the suite is unrelated to Vec and launches no surface |
| `a3s-test run images/runner/smoke/tui.acl --tui-executable "$PWD/images/runner/smoke/tui-fixture.sh" --tui-working-directory "$PWD" --json` (from `crates/test`) | **Passed** (1 scenario, 62 ms) | Owned PTY runner smoke and cleanup; it is not an `a3s-code` product-session test |
| `a3s-test capabilities --json` | **Infrastructure unavailable** | Installed Browser adapter is `0.1.1`, while `a3s-test 1.0.0` requires `>=0.4.0,<0.5.0`; no interactive session was started |

The Code, CLI, and TUI passes establish that the existing path is testable;
they do not imply that `a3s-vec` is wired into those products.

## 3. Dependency and portability evidence

`zvec-core 0.1.0` is resolved from crates.io and identifies
[`smarthi/zvec-rust`](https://github.com/smarthi/zvec-rust) as its repository.
It is not evidence that the Alibaba C++ engine has been ported or that the
Alibaba binary format is compatible.

With `--no-default-features`, the normal dependency tree has no `jieba-rs` or
zstd branch. The default `jieba` feature does add this build chain:

```text
jieba-rs → include-flate → zstd → zstd-sys → cc
```

That chain is important for the portability claim of this snapshot. The
`fbb1081` default feature set caused a native C compilation dependency.
Neither feature set was built on Intel macOS 12 during this review. That is
not the current default: `default = []`, and the default build does not
require Jieba or `zstd-sys`. macOS 12 Monterey is unsupported and is not an
open engine gate.

## 4. Findings

Severity describes the risk to a safe migration, not the amount of code
required to fix it.

### P0 — snapshot observations for `fbb1081`

These rows describe Vec `fbb1081`. They are not current defects. HNSW, IVF,
and DiskANN do not delegate to a flat index. Collection FTS is not only a
full scan. Checkpoint does not publish one `snapshot.json` before
`manifest.json`. The default build does not require Jieba/`zstd-sys`.
VEC-P0-04, VEC-P0-05, and VEC-P0-06 are withdrawn the same way: `WalRecord`
carries a revision, read-only create returns `permission_denied` before it
creates the collection, and recovery reads are bounded.

| ID | Observation on `fbb1081` | Consequence in that snapshot | Current status |
| --- | --- | --- | --- |
| VEC-P0-01 | On `fbb1081`, HNSW, IVF, and DiskANN types delegated `build` and `search` directly to `FlatIndex` (`crates/vec/src/index/hnsw.rs`, `ivf.rs`, `diskann.rs`). Parameters such as `m`, `ef`, `n_list`, and degree were not used by query execution. This is not a current defect. | The public index names overstated capability. | Historical request only, not a current gate. |
| VEC-P0-02 | On `fbb1081`, the collection FTS route re-tokenized every document and recomputed document frequency during each query (`crates/vec/src/collection.rs`, `execute_fts`/`bm25`). The standalone postings helper was not the collection query authority. This is not a current defect. | Query cost was a full corpus scan. | Historical request only, not a current gate. |
| VEC-P0-03 | On `fbb1081`, checkpoint wrote and renamed `snapshot.json`, then wrote `manifest.json` (`crates/vec/src/storage/mod.rs`). A crash between those publications left the old manifest checksum paired with the new snapshot. This is not a current defect. | Reopen could fail after a power-loss window. | Historical request only, not a current gate. |
| VEC-P0-04 | On `fbb1081`, WAL replay returned records but did not carry or apply a record revision. This is not a current defect. `WalRecord` carries a revision, and open rejects a non-monotonic frame. | That snapshot could disagree with the manifest after restart. | Historical request only, not a current gate. |
| VEC-P0-05 | On `fbb1081`, `StorageHandle::create` wrote files even when `read_only` was true. This is not a current defect. Read-only create returns `permission_denied` before it creates the collection. | Read-only mode mutated the filesystem in that snapshot. | Historical request only, not a current gate. |
| VEC-P0-06 | On `fbb1081`, WAL and snapshot reads deserialized unbounded byte buffers. This is not a current defect. Manifest, snapshot, WAL replay, index-cache, and DiskANN reads are bounded. | A corrupted local file could force unbounded memory growth in that snapshot. | Historical request only, not a current gate. |
| VEC-P0-07 | On `fbb1081`, the default dependency path reached `zstd-sys` and `cc` through the optional Jieba tokenizer. This is not a current defect: the default build does not require Jieba or `zstd-sys`, and macOS 12 Monterey is not an open engine gate. | That default build had no Monterey artifact. | Historical request only, not a current gate. |

### P1 — snapshot observations for `fbb1081`, not current engine gates

| ID | Observation on `fbb1081` | Consequence in that snapshot | Current status |
| --- | --- | --- | --- |
| VEC-P1-01 | On `fbb1081`, `WalRecord::Schema` replay only checked the collection name and did not apply the schema. This is not a current defect. Replay installs `WalOperation::Schema` and `SchemaOnly`. | A schema change could disappear in that snapshot. | Historical request only, not a current gate. |
| VEC-P1-02 | On `fbb1081`, dense query scoring could skip a dimension-mismatched vector and return an empty result. This is not a current defect. `validate_dense_payload` returns `invalid_argument`. | Callers could not distinguish “no match” from an invalid query in that snapshot. | Historical request only, not a current gate. |
| VEC-P1-03 | On `fbb1081`, `matches_field_type` treated any `FieldValue::Json` as compatible with every schema field. This is not a current defect. Incompatible JSON is rejected. | JSON could bypass scalar type checks in that snapshot. | Historical request only, not a current gate. |
| VEC-P1-04 | On `fbb1081`, binary, int4, and fp16 codecs did not yet establish exact re-score. This is not a current search defect. Quantized coordinates do not supply the public score. | API names could imply quantization that was only a conversion helper. | Historical request only, not a current gate. |
| VEC-P1-05 | On `fbb1081`, `IoBackend` and several index configuration fields were recorded without an execution path. This is not a current defect. `IoBackend::Positioned` opens a positioned reader. | Configuration was misleading in that snapshot. | Historical request only, not a current gate. |
| VEC-P1-06 | On `fbb1081`, the documented `api/`, `planner/`, `codec/`, and `testkit/` layout was absent. This is not a current defect. The checked-in layout is `crates/vec/src`. | That snapshot's module map could not be reproduced. | Historical request only, not a current gate. |
| VEC-P1-07 | On `fbb1081`, `pub use zvec_core` exposed the external kernel. This is not a current defect. `use a3s_vec::core` does not compile. | Replacing the kernel was a breaking change in that snapshot. | Historical request only, not a current gate. |

### P2 — quality and release debt recorded for the `fbb1081` snapshot

This list is that snapshot. It is not current release debt. The 2026-09-22
review withdrew this description of the later tree.

- In that snapshot, `cargo fmt --check` failed and Clippy reported 533
  diagnostics under `-D warnings`.
- That pinned checkout had five inline unit tests and did not yet show the
  later durability, concurrency, and index suites.
- That snapshot's Code performance evidence described session-local behavior.
  It did not establish a durable projection's latency or memory.
- That snapshot's FTS and filter paths parsed and scanned repeatedly. That is
  not a current claim that collection FTS is only a full scan.

## 5. Product-surface boundary matrix

The following is the observed boundary, not the desired end state.

| Surface | What exists in the pinned revision | What does not exist | Review implication |
| --- | --- | --- | --- |
| Code Core | Session-owned `WorkspaceRetrieval`, exact/BM25/semantic/hybrid routes, provider ports, source verification | No `a3s-vec` dependency or adapter | Keep as golden reference and migration oracle |
| CLI | ACL/config validation, provider admission, readiness/degraded status, workspace retrieval wiring | No `vgrep` command or route to `a3s-vec` | Do not document `vgrep` as shipped |
| TUI | Renders status, channel, ranking, and fallback metadata | No indexing, tokenization, ranking, or provider lifecycle | Preserve thin presentation ownership |
| Node/Python/Go SDKs | Wrappers around the existing `WorkspaceRetrieval` API | No stable `vgrep` DTO/engine selector | Freeze a typed cross-surface contract before migration |
| `a3s-test` | The Test project is present locally and can validate ACL syntax | The checked-out Test revision differs from the root gitlink; Vec-specific contract suites are absent, and the installed Web adapter reports an unsupported Browser version | Add deterministic Vec/host suites and pin the Test revision before a release claim |

The lack of a CLI command is not a bug to paper over by aliasing the old
search tool. It is a contract decision: `vgrep` must identify its route,
revision, freshness, and fallback reason consistently in every host.

## 6. Intel macOS 12 conclusion for the `fbb1081` snapshot

No Intel macOS 12 release claim was justified by this snapshot. macOS 12
Monterey is unsupported and is not an open engine gate.

What is known:

- The review host is Apple Silicon macOS 26.3, not Intel Monterey.
- The crate has a portable no-default-feature build that passed five unit
  tests on that host.
- In that snapshot, the default Jieba feature reached a C compiler through
  `zstd-sys`. The current default build does not require Jieba or `zstd-sys`.
- That snapshot recorded no x86_64 Monterey artifact, installer smoke run,
  runtime benchmark, CPU-feature check, or offline semantic-provider test.
  macOS 12 Monterey is unsupported and is not an open engine gate.

The snapshot's support matrix was:

| Capability | Intel macOS 12 status in that snapshot |
| --- | --- |
| Existing Code exact/FTS baseline | Qualified by the Code/CLI release pipeline, not by this engine gate |
| `a3s-vec` portable core | Snapshot host only. macOS 12 Monterey is unsupported and is not an open engine gate |
| `vgrep --rg` | Not shipped in the pinned CLI |
| `vgrep --fts` | Not shipped in that snapshot |
| Local semantic Embedding | Not an engine gate. macOS 12 Monterey is unsupported |
| Explicitly authorized remote semantic route | Host egress policy only; not a Monterey engine gate |

That snapshot treated a full Intel semantic requirement as a blocked release
gate. macOS 12 Monterey is unsupported and is not an open engine gate. Exact
search stays truthful.

## 7. Revised architectural decisions

The first-principles review confirms and tightens the cross-project contract:

1. **One canonical catalog.** Code owns workspace admission, chunking, source
   revisions, digests, and verification. Vec owns typed collection/index state.
2. **One revision per query.** Documents plus WAL are authoritative; derived
   FTS/ANN/scalar files carry a source revision and fail closed to an exact
   reference scan when stale.
3. **Model-free storage.** Vec accepts caller-supplied dense/sparse vectors. It
   never chooses or downloads an Embedding model and never reads workspace
   files directly.
4. **Provider policy at the host.** Local artifacts are digest/capability
   admitted; remote calls require a separate source-egress grant. Reranking is
   optional and receives only bounded, verified candidates.
5. **`vgrep` is a router and result protocol.** It unifies `rg`, FTS, vector,
   and hybrid routes and reports readiness/degraded/fallback metadata. It is
   not a database, model, memory store, or UI ranking implementation.
6. **Migration is staged.** Freeze the old path, dual-project the same chunks
   and provider batch, compare results, expose opt-in behaviour, and retain a
   rollback switch until the release gate is green.
7. **SQLite scope is narrow.** Removing SQLite/BM25 is allowed only for the
   Code workspace retrieval path. `a3s-memory` long-term memory SQLite APIs
   remain owned by the memory project.

## 8. Migration-benefit decision gate

Replacing a working baseline is justified only when the new path demonstrates
equal-or-better user-visible behaviour and lower operational risk. “The API is
similar” or “the index type is available” is not sufficient.

The migration gate must compare the same corpus, chunk manifest, query set,
provider vectors, hardware class, and warm/cold state:

| Dimension | Required result for migration | Evidence |
| --- | --- | --- |
| Correctness and source safety | No missing eligible IDs, stale ranges, duplicate chunks, or unverifiable previews; typed errors remain stable | Differential corpus report and fault tests |
| Lexical relevance | FTS/BM25 ranking agrees with the frozen golden reference, or every intentional difference has an approved report | Golden query set with rank correlation and top-k diff |
| Semantic quality | Recall and hybrid rank are no worse than the current exact/vector oracle for the target corpus | Exact-oracle comparison, not model intuition |
| Latency | p50/p95 query and indexing latency are no worse than baseline within the benchmark's declared noise budget | Repeated warm/cold benchmark runs |
| Memory and disk | Peak RSS, persistent bytes, and request amplification stay within the product budget | Resource report with limits |
| Startup and readiness | Exact/FTS startup remains usable without waiting for a model; any semantic readiness delay is bounded and observable | CLI/TUI/SDK startup traces |
| Durability | Every acknowledged mutation recovers to one coherent revision after injected crashes and partial tails | WAL/checkpoint fault matrix |
| Lifecycle | Close, cancellation, lock release, and provider shutdown leave no leaked tasks, handles, sockets, vectors, or temp files | Repeated session lifecycle test |
| Privacy and egress | New path makes no additional provider/network call and emits no source text/vector/credential in diagnostics | Redaction and egress audit |
| Platform support | Linux, Windows, and supported macOS pass the same required core gates. macOS 12 Monterey is unsupported and is not an open engine gate | Pinned artifact/runtime matrix |

The benchmark protocol must define a small statistical noise allowance before
running the comparison. A result outside that allowance blocks migration; it
does not get hidden by changing the query set or silently selecting a fallback.

## 9. Ordered remediation recorded for the `fbb1081` snapshot

This sequence is the `fbb1081` record, not a new engine plan. Asking to keep
collection FTS as a scan, to require a Monterey build, or to keep ANN facades
exact is not a current engine defect. Steps 1 and 2 are withdrawn as well:
they are not required closure. The snapshot sequence was:

1. That snapshot asked to correct revision, replay, and the read-only
   lifecycle. This is not a current defect. `WalRecord` carries a revision,
   and read-only create returns `permission_denied` before it creates the
   collection. The separate claim that checkpoint publishes one `snapshot.json`
   before `manifest.json` is also not a current defect.
2. That snapshot asked to bound deserialization and to apply schema WAL
   records. This is not a current defect. Recovery reads are bounded, and
   schema replay installs the schema. This step does not restore a
   Jieba/`zstd-sys` default or a Monterey gate.
3. That snapshot asked to make the FTS index authoritative or to label it as a
   reference scan. Collection FTS is not only a full scan in the current engine.
4. That snapshot asked for a Monterey build. macOS 12 Monterey is unsupported
   and is not an open engine gate.
5. That snapshot asked to keep ANN facades exact. HNSW, IVF, and DiskANN do
   not delegate to a flat index.
6. That snapshot asked to build the Code adapter in shadow mode. Publication of
   the public result stays owned by Code.
7. That snapshot asked to run the migration-benefit matrix before removing the
   old Code path. That removal is not an engine gate.

Formatting, strict Clippy, deterministic unit/integration tests, crash
recovery, cross-platform smoke, and host end-to-end evidence are release gates,
not optional cleanup. That sentence described the `fbb1081` snapshot. It does
not describe the current engine contract at the top of this file.

## 10. Source references

- [`docs/retrieval-platform-architecture.md`](retrieval-platform-architecture.md)
  — cross-project ownership and trust boundaries.
- [`docs/retrieval-platform-roadmap.md`](retrieval-platform-roadmap.md) —
  phase dependencies and exit gates.
- [`crates/vec/ARCHITECTURE.md`](../crates/vec/ARCHITECTURE.md) — engine-level
  intended layout and contracts.
- [`crates/vec/README.md`](../crates/vec/README.md) — current prototype and
  platform statement.
- [`crates/code/manual/WORKSPACE_RETRIEVAL_OPERATIONS.md`](../crates/code/manual/WORKSPACE_RETRIEVAL_OPERATIONS.md)
  — current Code workspace retrieval operations.
