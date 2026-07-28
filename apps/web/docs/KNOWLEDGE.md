# A3S Web Knowledge

## Product boundary

Knowledge is a built-in A3S Web product for local, user-owned knowledge bases.
It is not a marketplace and it does not install executable A3S Use packages.
The signed plugin Market remains a separate system destination for reviewed
Plugin packages and registry sources.

The Knowledge product uses the same sidebar dimensions, navigation rows,
framed command glyphs, header hierarchy, spacing, and interaction patterns as
the Work conversation sidebar. It appears directly below Work in the Activity
Bar.

## Local library

The default view is **My Knowledge Bases**. It lists only knowledge bases that
are available in the current workspace, with pinned bases first. Each card
shows its origin, source count, concept count, size, and local path.

Users can:

- create an empty local knowledge base;
- import an Obsidian Vault or another local folder;
- create one source-backed knowledge base from one or more files, folders, or a
  mixed Work selection;
- compile a source-backed base on demand or enable its per-base smart automatic
  policy;
- search and pin local knowledge bases;
- click a knowledge-base card to open its editor.

There is no Knowledge Marketplace tab, catalog-install action, or executable
permission grant in this product.

## Knowledge editor

Opening a knowledge base enters a dedicated, Obsidian-inspired editor with:

- a collapsible local directory tree;
- file filtering and directory refresh;
- editing for Markdown, MDX, text, JSON, YAML, CSV, and TSV files;
- a live Markdown reading view that can be hidden;
- explicit save and `Cmd/Ctrl+S` save.

The editor reads and writes the selected managed knowledge-base directory
through the existing workspace filesystem service. Binary files and hidden A3S,
Obsidian, Git, and operating-system metadata are not opened as text. The
knowledge-base summary is refreshed after a successful save.

## Source packages and knowledge compilation

Creating a knowledge base from Work and compiling it are deliberately separate
steps:

1. **Create knowledge base** validates the selected files and folders, removes
   duplicate and parent-covered selections, copies a deterministic source
   package into `sources/`, and records the source set and content digest. It
   does not create searchable `wiki/` output or start a compiler.
2. **Compile knowledge** queues that source generation for an independent
   compiler. A successful result atomically promotes the compiler's staged OKF
   Markdown into `wiki/`. A failed or cancelled result never replaces the last
   successful `wiki/` tree.

The Work composer previews the effective source-root count, file count, and
total bytes before creation. A source package accepts regular files and
directories, including an empty selected directory, while rejecting symlinks,
out-of-workspace paths, managed knowledge directories, more than 20,000 files,
or more than 1 GiB. Selecting both a directory and one of its descendants keeps
only the parent source root.

The default policy is `manual`. Each source-backed knowledge base can instead
enable `smart_auto`. Automatic compilation uses content digests rather than
mtime alone and applies all of these gates:

- each changed file must be stable for 5 seconds;
- the complete source set must be quiet for 30 seconds;
- automatic compilations for the same base are at least 10 minutes apart;
- transient failures retry after 5 minutes, 30 minutes, and 2 hours;
- manual jobs take priority over retries and smart-automatic jobs;
- only one compiler job runs across all workspaces monitored by the Web host.

A source change that arrives during compilation becomes a later generation; it
does not alter the claimed input in place. Automatic compilation pauses for
review after a suspiciously large change: more than 100 deletions, at least 20
deletions affecting over half the previous files, more than 1,000 total
changes, or at least 200 total changes affecting over half the previous files.
Routine edits in small knowledge bases do not trigger this safety pause, and a
manual compile remains available after review.

Compiler implementations use the versioned claim/result handoff documented in
[Knowledge Compilation Protocol](KNOWLEDGE_COMPILATION_PROTOCOL.md).

## External import

The inline Import Knowledge Base composer accepts an Obsidian Vault or another
local folder. Users can open the native directory picker or enter an absolute
path manually when a graphical picker is unavailable.

Import is copy-based: the source directory remains unchanged, while its content
is copied into a new managed knowledge base under `sources/`. The importer:

- rejects missing paths, non-directories, symbolic-link roots, the workspace
  root, and recursive imports involving the managed knowledge directory;
- skips `.obsidian`, `.git`, `.a3s`, `.trash`, `.DS_Store`, and symbolic links;
- preserves nested notes and attachments;
- rejects an empty import after filtering;
- limits one import to 20,000 files or 1 GiB;
- stages all output and renames it into place only after the copy and manifests
  succeed.

Native directory selection uses `osascript` on macOS, PowerShell on Windows,
and Zenity or KDialog on Linux. Cancellation is returned as a normal result;
picker unavailability is shown as an actionable error so manual path entry
remains available.

## Personal storage contract

Managed bases live under the selected workspace:

```text
<workspace>/.a3s/kb/bases/<id>/
├── .a3s/
│   ├── asset.acl
│   ├── knowledge-base.acl
│   ├── source-set.acl              # source-backed bases only
│   ├── source-snapshot.json        # source-backed bases only
│   ├── compilation.acl             # source-backed bases only
│   └── compilations/               # durable job records
├── README.md
├── sources/
├── wiki/                            # absent until a successful compilation
└── eval/
```

The `wiki/` line is optional for a newly created source package. Search and
retrieval must not treat `sources/`, compiler staging directories, or an
uncompiled base as knowledge. Author-created, imported, legacy, and marketplace
bases can already contain an authored `wiki/` tree and do not acquire a source
compilation policy implicitly.

The UI consistently calls these assets knowledge bases. `okf` remains only a
technical protocol identifier in the storage manifest. The knowledge-base
manifest uses `a3s.knowledge-base.v1`; the asset manifest declares the
`knowledge` category and `okf` protocol. Both files are generated and parsed
with `a3s-acl`.

The existing legacy `<workspace>/.a3s/kb` tree remains visible as the pinned
Workspace Knowledge base when it contains sources or wiki content.

## Local Web API

The Knowledge experience uses these loopback API routes:

- `GET /api/v1/knowledge/bases`
- `POST /api/v1/knowledge/bases`
- `POST /api/v1/knowledge/bases/import`
- `POST /api/v1/knowledge/bases/from-selection/preview`
- `POST /api/v1/knowledge/bases/from-selection`
- `POST /api/v1/knowledge/bases/{id}/pinned`
- `POST /api/v1/knowledge/bases/{id}/compilations`
- `GET /api/v1/knowledge/bases/{id}/compilation`
- `PATCH /api/v1/knowledge/bases/{id}/compilation-policy`
- `GET /api/v1/knowledge/bases/{id}/source-changes`
- `POST /api/v1/knowledge/bases/{id}/compilations/{jobId}/cancel`
- `POST /api/v1/knowledge/compilations/claim`
- `POST /api/v1/knowledge/bases/{id}/compilations/{jobId}/result`
- `POST /api/v1/workspace/actions/pick-directory`
- `GET /api/v1/workspace/read-dir`
- `GET /api/v1/workspace/read`
- `POST /api/v1/workspace/write`

Responses use the standard A3S API envelope. Knowledge-base mutations return a
`changed` flag and the resulting projection so the UI can update immediately,
then reconcile through a background refresh.

## Validation

Run backend checks from the repository root:

```sh
cargo fmt --all -- --check
cargo test --bin a3s api::code_web::knowledge:: -- --nocapture
cargo test --test web_cli knowledge_marketplace_creates_and_installs_real_personal_bases -- --nocapture
cargo test --test web_cli knowledge_selection_creation_and_compilation_are_separate_api_steps -- --nocapture
```

Run frontend checks from `apps/web`:

```sh
bun run format:check
bun run lint:check
bun run typecheck
bun run test
bun run build
```

Browser acceptance covers the local library, opening a card into the directory
tree editor, editing and saving Markdown, inline create and import composers,
native directory selection, manual absolute-path fallback, mixed Work
selection, creation without compilation, explicit compilation, smart-automatic
policy changes, and visible queued/running/succeeded/failed/paused states. The
native operating-system dialog itself requires platform-level manual
acceptance.
