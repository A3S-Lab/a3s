# Local Retrieval Platform Architecture Review

Status: review of an explicitly recorded checkout snapshot. The component
revisions below were tested from their owning repositories and are not implied
to be the root repository's current gitlinks. This is an engineering review
and a migration decision record; it is not a release qualification.

Initial review date: 2026-08-29

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

VEC-P1-05 is **partially closed**. Process and collection runtime configuration
is now truthful; future index/query parameters, schema segment sizing, and
schema-evolution concurrency controls still need explicit `NotSupported`
behavior or implemented consumers. VEC-P1-04 and that remaining P1-05 work are
the open engine-contract findings.

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

VEC-P1-07 is **closed** at this revision. VEC-P1-04/05, real ANN/indexed FTS,
the durability fault matrix, concurrency, migration benefit, and the supported-
platform matrix remain open. This remains a pre-migration prototype.

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
| VEC-P0-07 | **Partially closed** | The default build is portable by dependency inspection and arm64 execution. The explicit Jieba feature still needs native-toolchain packaging evidence, and both configurations still need x86_64 macOS 12 build/runtime qualification. |
| VEC-P1-02 | **Closed for the current exact query surface** | Every current numeric dense schema type and metric has negative dimension evidence; dense/sparse/FTS type routing and explicit binary/sparse unsupported paths are covered. Future index implementations must reuse this contract and add differential evidence. |
| VEC-P1-03 | **Closed** | JSON has a documented schema-aware adapter policy, all supported scalar/non-binary array variants have canonicalization evidence, and incompatible/overflow/binary cases fail before persistence. |
| Strict format/Clippy debt | **Closed at `10412b3`** | Both strict Clippy variants, rustfmt, rustdoc, and all three feature-matrix test runs pass. |
| Insufficient integration coverage | **Improved, open** | Contract and durability suites now provide 15 public integration tests; concurrency, differential FTS/vector, fault injection, index recall, and platform suites remain open. |

VEC-P1-04/05/07, real ANN/indexed FTS, the full durability fault matrix,
concurrency, migration benefit, and supported-platform evidence remain open.
Code's current retrieval implementation remains the golden reference and no
old path is removed.

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

Finding disposition at this revision:

| Finding | Follow-up state | Remaining evidence |
| --- | --- | --- |
| VEC-P0-01 | **Mitigated, open** | False private ANN facades are gone, but real ANN plus recall/latency/parameter-sensitivity evidence remains P3 work. |
| VEC-P0-02 | **Open** | FTS remains an explicitly documented exact corpus scan; generation-tagged postings and golden parity evidence are absent. |
| VEC-P0-03 | **Partially closed** | Immutable generations and orphan-publication recovery are tested; every fsync/rename/prune boundary still needs deterministic fault injection. |
| VEC-P0-04 | **Closed for format 2** | WAL revisions/operation identities replay monotonically across every DML and schema sequence in the current suite. |
| VEC-P0-05 | **Closed on the review host** | Read-only create, missing-lock open, normal open, and close behavior have integration coverage. Cross-platform lock evidence remains in the platform gate. |
| VEC-P0-06 | **Partially closed** | Byte budgets and oversized/corrupt input tests exist; per-document/field budgets and recovery fuzzing remain open. |
| VEC-P0-07 | **Open** | The default Jieba dependency chain and Intel macOS 12 qualification are unchanged. |
| VEC-P1-01 | **Closed for format 2** | Schema plus backfilled documents replay and checkpoint at one revision. A compact schema-delta encoding is optional future work, not current authority. |
| VEC-P1-06 | **Closed as documentation drift** | Vec architecture now describes the checked-in modules/tests and labels future index/planner modules as gated work. Broader API/query/concurrency coverage remains a release gate. |
| Strict format/Clippy debt | **Closed at `78840ce`** | The two strict Clippy variants and rustfmt pass. Three noisy API-annotation lints are explicitly deferred in Cargo lint policy while the prototype API changes; substantive `all`/`pedantic` lints remain enabled. |

All other findings remain open. In particular, this revision does not validate
query dimension errors, the JSON type escape hatch, quantization semantics,
configuration wiring, the public `zvec_core` escape hatch, real ANN/indexed
FTS, concurrency, migration benefit, or Intel macOS 12. Code's current
retrieval implementation remains the golden reference and no old path is
removed.

## Executive decision

The current `a3s-vec` revision is a useful API and portability prototype, but
it is not complete and it has not passed the quality bar required to replace
the Code workspace retrieval path. The existing Code and TUI baselines are
the available behavioral reference at this point; they are not a qualification
of the new engine.

The review therefore makes four decisions:

1. Keep Code's current workspace BM25 and `a3s-memory` vector path as the
   read-only golden reference. Do not delete SQLite, SQLite-vector, or Code
   BM25 integration in this revision.
2. Treat `a3s-vec` as pre-migration until the P0/P1 correctness, recovery,
   resource, and API-contract findings below are closed with reproducible
   evidence.
3. Keep semantic Embedding and reranking outside the storage engine. Providers
   must be explicitly admitted and injected by the host; storage must not
   download models, access the network, or discover workspace files.
4. Do not advertise Intel macOS 12 support for the new engine or semantic
   providers until an actual `x86_64-apple-darwin` Monterey artifact and
   runtime report exists.

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

The following commands were used from the owning crate workspaces:

| Command | Observed result | Meaning |
| --- | --- | --- |
| `cargo test --manifest-path crates/vec/Cargo.toml --no-default-features` | **5 passed**, 0 failed | Only the five current unit tests pass; this is not full feature coverage |
| `cargo test --manifest-path crates/vec/Cargo.toml` | **5 passed**, 0 failed | Default `jieba` feature also builds on arm64; this does not qualify Intel Monterey |
| `cargo test --manifest-path crates/vec/Cargo.toml --all-features` | **5 passed**, 0 failed | Async plus Jieba feature combination builds on arm64; coverage remains five unit tests |
| `cargo check --manifest-path crates/vec/Cargo.toml --release` | **Passed**, 49 warnings | Release compilation works on arm64; warnings include unused/dead code |
| `cargo fmt --manifest-path crates/vec/Cargo.toml -- --check` | **Failed** | The checked-in Vec source is not rustfmt-clean |
| `cargo clippy --manifest-path crates/vec/Cargo.toml --all-targets -- -D warnings` | **Failed**: 533 diagnostics | Strict lint quality gate is open |
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

That chain is important for the portability claim. The current architecture
requires a portable correctness path, but the default feature set still
causes a native C compilation dependency. Neither the default feature nor the
no-default-feature path was built on Intel macOS 12 during this review.

## 4. Findings

Severity describes the risk to a safe migration, not the amount of code
required to fix it.

### P0 — must close before any Code shadow migration

| ID | Observation | Consequence | Required closure evidence |
| --- | --- | --- | --- |
| VEC-P0-01 | HNSW, IVF, and DiskANN types delegate `build` and `search` directly to `FlatIndex` (`crates/vec/src/index/hnsw.rs`, `ivf.rs`, `diskann.rs`). Parameters such as `m`, `ef`, `n_list`, and degree are not used by query execution. | The public index names overstate capability. A benchmark or release note could incorrectly claim ANN behaviour or performance. | Implement a real index or mark the type explicitly as an exact facade. Add recall/latency tests against the flat oracle and verify parameter sensitivity. |
| VEC-P0-02 | The collection FTS route re-tokenizes every document and recomputes document frequency during each query (`crates/vec/src/collection.rs`, `execute_fts`/`bm25`). The standalone postings helper is not the collection query authority. | Query cost is a full corpus scan and can diverge from the advertised indexed FTS semantics. | Build and publish generation-tagged postings/statistics; compare indexed results with a brute-force oracle on golden corpora. |
| VEC-P0-03 | Checkpoint writes and renames `snapshot.json`, then writes `manifest.json` (`crates/vec/src/storage/mod.rs`). A crash between those publications leaves the old manifest checksum paired with the new snapshot. | Reopen can fail with a checksum mismatch after a power-loss window, even though a complete snapshot exists. | Use generation-specific immutable files and publish one manifest commit point; inject crashes at every rename/fsync boundary and prove deterministic recovery. |
| VEC-P0-04 | WAL replay returns records but does not carry or apply a record revision. `StorageHandle::open` initializes the in-memory revision from the manifest before replay. | After restart, the document state and revision can disagree; stale derived indexes or clients can observe an invalid generation. | Persist a monotonic revision/operation identity in each frame, advance it during replay, and test insert/update/delete/schema sequences across restart. |
| VEC-P0-05 | `StorageHandle::create` creates directories, lock files, and a snapshot even when `read_only` is true. `Collection::close` always calls `flush`, while a read-only handle cannot checkpoint. | Read-only mode mutates the filesystem and may fail during normal close. This violates both API and lifecycle expectations. | Separate create/open modes, prohibit writes before touching storage, make read-only close side-effect-free, and test lock/close/error paths. |
| VEC-P0-06 | WAL and snapshot reads deserialize unbounded byte buffers (`read_to_end`, whole JSON vectors/documents) without a configured allocation budget. | A corrupted or hostile local file can force unbounded memory growth before a typed error is returned. | Add frame, document, field, and total-recovery limits; fuzz malformed lengths and verify `resource_exhausted` outcomes. |
| VEC-P0-07 | The default dependency path reaches `zstd-sys` and `cc` through the optional Jieba tokenizer, while the architecture promises no mandatory C/C++ dependency. | The stated Intel/macOS portability contract is not mechanically true for the default build and has no Monterey artifact evidence. | Make the portable default genuinely portable or document the native requirement; build and run both feature sets on x86_64 macOS 12. |

### P1 — must close before opt-in semantic `vgrep`

| ID | Observation | Consequence | Required closure evidence |
| --- | --- | --- | --- |
| VEC-P1-01 | `WalRecord::Schema` replay only checks the collection name and does not apply the schema. | A schema change acknowledged before a crash may disappear or leave documents interpreted under the wrong schema. | Replay every schema operation and validate schema digest/revision transitions. |
| VEC-P1-02 | Dense query scoring can skip a dimension-mismatched vector and return an empty result instead of a typed query error. | Callers cannot distinguish “no match” from an invalid query/index state. | Validate query dimension at the boundary and add error-contract tests for every metric and vector type. |
| VEC-P1-03 | `matches_field_type` treats any `FieldValue::Json` as compatible with every schema field. | JSON can bypass scalar type and nullability guarantees. | Define an explicit JSON schema/type policy and reject incompatible values. |
| VEC-P1-04 | Binary/int4/fp16 and related codecs currently perform approximate conversions; they do not yet establish complete quantized storage/search semantics. | API names can imply zvec-compatible quantization when only a conversion helper exists. | Add encoded-format round trips, error bounds, metric tests, and exact re-score guarantees. |
| VEC-P1-05 | `IoBackend`, buffer, segment, and several index configuration fields are recorded but are not connected to a measurable execution path. | Configuration is misleading and makes resource planning impossible. | Either wire each option into execution or remove it from the public contract until implemented. |
| VEC-P1-06 | The architecture promises `tests/api_compat.rs`, `crud_and_query.rs`, `durability.rs`, `indexes.rs`, and `concurrency.rs`; those integration tests and the documented `api/`, `planner/`, `codec/`, and `testkit/` layout are absent in the pinned crate. | The stated verification and module ownership cannot be reproduced from the repository. | Add the tests/layout or revise the architecture document to match the implementation, then pin the evidence. |
| VEC-P1-07 | `pub mod core { pub use zvec_core::*; }` exposes the external kernel through the public API. | Replacing the kernel becomes a breaking change and leaks an unreviewed dependency surface. | Keep the kernel private and expose only stable A3S types, or record an explicit compatibility exception. |

### P2 — quality and release debt

- The strict format and Clippy gates are open (`cargo fmt --check` fails and
  Clippy reports 533 errors under `-D warnings`).
- The Vec crate has five inline unit tests and no durable CRUD, corruption,
  concurrency, index-recall, or cross-platform integration suite in the pinned
  checkout.
- Existing Code performance evidence describes session-local/small-corpus
  behaviour. It does not establish that a durable `a3s-vec` projection would
  improve latency, memory, startup, or request amplification.
- FTS/filter paths parse and scan repeatedly. This is a measurable optimization
  opportunity only after correctness and bounded-resource contracts are fixed.

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

## 6. Intel macOS 12 conclusion

No Intel macOS 12 release claim is justified by this snapshot.

What is known:

- The review host is Apple Silicon macOS 26.3, not Intel Monterey.
- The crate has a portable no-default-feature build that passed five unit
  tests on that host.
- The default Jieba feature reaches a C compiler through `zstd-sys`.
- There is no recorded x86_64 Monterey artifact, installer smoke run, runtime
  benchmark, CPU-feature check, or offline semantic-provider test.

The minimum honest support matrix is therefore:

| Capability | Intel macOS 12 status |
| --- | --- |
| Existing Code exact/FTS baseline | Must be qualified independently by the Code/CLI release pipeline |
| `a3s-vec` portable core | **Unverified** until x86_64 build and runtime smoke pass |
| `vgrep --rg` | **Not shipped** in the pinned CLI |
| `vgrep --fts` | **Not shipped** and engine path unverified |
| Local semantic Embedding | **Unadvertised** until a compatible Intel artifact/runtime report exists |
| Explicitly authorized remote semantic route | Possible only under host egress policy; not a local-engine qualification |

If product policy requires full semantic support on Intel, the release gate is
blocked. Exact search must remain truthful and usable rather than silently
falling back to stale semantic results.

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
| Platform support | Linux, Windows, macOS arm64, and Intel macOS 12 pass the same required core gates | Pinned artifact/runtime matrix |

The benchmark protocol must define a small statistical noise allowance before
running the comparison. A result outside that allowance blocks migration; it
does not get hidden by changing the query set or silently selecting a fallback.

## 9. Ordered remediation and release gates

The next implementation sequence is:

1. Correct storage publication/revision/replay and read-only lifecycle (the
   P0 durability set).
2. Add bounded deserialization, explicit dimension/type errors, and schema WAL
   application.
3. Make the FTS index authoritative or label it as a reference scan; add the
   brute-force evaluator and integration/fault tests promised by the design.
4. Resolve the default-feature native dependency and produce x86_64 Monterey
   build/runtime evidence.
5. Keep ANN facades exact and explicitly named until real HNSW/IVF/DiskANN
   implementations pass recall and resource gates.
6. Build the Code adapter in shadow mode, then add one governed `vgrep` route
   and shared CLI/TUI/SDK result DTOs.
7. Run the migration-benefit matrix. Only after it passes may the roadmap's
   P7 removal of duplicate workspace SQLite/BM25/vector paths begin.

Formatting, strict Clippy, deterministic unit/integration tests, crash
recovery, cross-platform smoke, and host end-to-end evidence are release gates,
not optional cleanup. Until all are green, the honest status is “prototype and
baseline under review,” not “latest a3s-code retrieval engine released.”

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
