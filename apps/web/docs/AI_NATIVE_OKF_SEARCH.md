# AI-Native OKF Search

> Status: implementation plan. This document defines the search boundary before
> backend or UI implementation begins.

## Product decision

A3S will build one local, persistent search service for compiled knowledge:

```text
independent knowledge compiler
        |
        v
OKF Markdown + YAML frontmatter
        |
        v
embedded Tantivy index
        |
        +--> Work and Knowledge search UI
        +--> agent search tool
        +--> bounded automatic context provider
```

The first release indexes only normalized OKF Markdown. It does not parse,
extract, OCR, or convert Office, PDF, image, email, archive, or web content.
Those formats belong to independent knowledge compilers. Search starts after a
compiler has emitted conformant OKF.

This boundary keeps ingestion replaceable and makes the index deterministic:
the same OKF bundle must produce the same searchable documents regardless of
the source format that produced it.

## Open-source research

The design uses two projects in different roles.

| Project | Role in A3S | Decision |
| --- | --- | --- |
| [QMD](https://github.com/tobi/qmd) | Product and evaluation reference | Adopt its agent-oriented structured output, lexical/vector separation, Markdown-aware chunk boundaries, multi-query retrieval, line-numbered reads, explainable scoring, and relevance fixtures. Do not embed its Node.js and local GGUF runtime in the Rust host. QMD extracts a title from a heading or filename but does not expose the complete OKF frontmatter schema as typed search fields. |
| [Tantivy](https://github.com/quickwit-oss/tantivy) | Embedded lexical index | Use as the first implementation. It is an MIT-licensed Rust library with BM25, phrases, fielded queries, incremental and multithreaded indexing, memory-mapped directories, compressed stores, and a documented startup time below 10 ms. |
| [tantivy-jieba](https://github.com/jiegec/tantivy-jieba) | Chinese tokenization | Register for mixed Chinese and English title, description, heading, tag, and body fields. Keep exact keyword fields for filters and identifiers. |
| [LanceDB](https://github.com/lancedb/lancedb) | Optional future semantic backend | Reconsider only after the lexical baseline has a measured relevance gap that embeddings fix. Its vector and full-text combination is useful, but its Arrow/Lance runtime and optimization lifecycle are unnecessary for the first local keyword release. |
| [SQLite FTS5](https://www.sqlite.org/fts5.html) | Fallback prototype | Retain as a low-dependency comparison in benchmarks. It has strong phrase, prefix, Boolean, snippet, and incremental-update support, but Chinese tokenization and weighted structured fields require more integration work than Tantivy. |

Service-oriented engines such as Meilisearch, Quickwit, and Typesense are not
the desktop default. A local A3S session should not need a second daemon, port,
authentication boundary, or task queue to search its own compiled knowledge.

## Source contract

### Included roots

The first implementation discovers conformant Markdown under:

- `.a3s/kb/wiki/**/*.md`;
- `.a3s/kb/bases/*/wiki/**/*.md`;
- the `wiki/` subtree of an explicitly selected local OKF package.

Raw `sources/` trees are excluded. They are compiler inputs, not searchable
authority. Human-authored notes can participate after they are compiled or
validated into an OKF `wiki/` tree.

### Publication eligibility

For a source-backed personal knowledge base, directory presence is not enough
to establish search authority. The indexer admits only the managed
`wiki/**/*.md` generation that the host has atomically promoted after a
successful knowledge-compilation result. Knowledge-base creation alone creates
no searchable documents. Compiler output under
`.a3s/compilation-output/**`, queued or running output, and failed output are
always excluded.

If a newer compilation fails, the last successfully promoted `wiki/` remains
eligible and searchable. The new generation becomes visible only after the
host's atomic promotion completes. Legacy, marketplace, imported, and directly
authored knowledge bases may already own a valid `wiki/` tree; they still pass
the same per-document OKF conformance checks.

The queue and worker handoff are defined separately in
[Knowledge Compilation Protocol](KNOWLEDGE_COMPILATION_PROTOCOL.md). Search
does not extract source formats, claim compiler jobs, or infer success from a
staging directory.

### Conformance

Every indexed file must:

- be valid UTF-8 Markdown;
- start with closed YAML frontmatter;
- contain exactly one non-empty scalar `type`;
- resolve to a canonical path inside the selected workspace or package;
- stay within configured file and parsed-node limits.

Optional OKF fields are indexed when present:

```yaml
---
type: Architecture Decision
title: Why microVM, not container
description: Rationale for the runtime isolation choice.
resource: crates/box/src/runtime.rs
tags: [architecture, security]
timestamp: 2026-06-30T12:00:00Z
source: compiled
sources:
  - crates/box/src/runtime.rs
source_digest: sha256:...
---
```

Malformed documents do not silently enter an unstructured fallback index. The
status surface reports the path and a bounded diagnostic so the compiler or
author can repair the source.

## Search document model

One Markdown file becomes one document record and one or more section records.
Search ranks sections; grouping reconstructs a document-level result.

### Stable identity

- `document_id`: hash of knowledge-base identity plus bundle-relative OKF path;
- `section_id`: hash of `document_id`, heading path, and start line;
- `content_digest`: hash of normalized frontmatter plus Markdown body;
- file path remains the OKF concept identity and is always returned.

A rename is a delete plus insert because the OKF path is identity. A content
edit retains the document identity and replaces its section records.

### Indexed fields

| Field | Shape | Search behavior |
| --- | --- | --- |
| `knowledge_base_id` | exact string | required isolation filter |
| `path` | stored exact string plus tokenized path | citation, exact filter, filename and path lookup |
| `title` | tokenized text | strongest free-text field |
| `description` | tokenized text | concise intent match |
| `type` | exact and tokenized text | typed filter and strong boost |
| `tags` | repeated exact and tokenized text | typed filter and strong boost |
| `resource` | exact and tokenized path or URL | source lookup |
| `source` | exact string | provenance filter |
| `sources` | repeated exact and tokenized path | provenance lookup |
| `timestamp` | date fast field | explicit sorting or bounded freshness boost |
| `heading` | tokenized text | section intent match |
| `heading_path` | stored string | result orientation |
| `body` | tokenized text with positions | BM25, phrase search, and snippets |
| `links` | repeated exact paths | backlinks and later graph-aware ranking |
| `start_line`, `end_line` | stored integers | precise citation and bounded read |
| `content_digest` | exact string | incremental update and diagnostics |

Initial boosts are configuration constants covered by relevance fixtures:

```text
title 4.0
tags 3.0
type 2.5
heading 2.5
description 2.0
resource and sources 1.5
path 1.25
body 1.0
```

Exact phrase and exact keyword matches may add deterministic bonuses. Timestamp
must not reorder evergreen knowledge unless the request explicitly asks for
recent material.

## Markdown-aware sectioning

The parser preserves source line ranges while walking CommonMark blocks.

- Prefer heading boundaries from `#` through `######`.
- Carry the full heading ancestry into every section.
- Keep fenced code blocks, tables, and short lists intact.
- Keep a section intact while it remains under the configured token ceiling.
- Split an oversized section at paragraph or list boundaries.
- Use a small overlap only for an oversized split, never between normal
  adjacent sections.
- Never include YAML frontmatter in the visible body snippet.

The initial target is 400–800 model tokens per section. The exact target is an
evaluation parameter, not a permanent API promise.

## Chinese and identifier search

Human-language fields use the Jieba tokenizer with lowercase normalization.
Exact keyword fields remain untokenized for `type`, tags, paths, provenance,
and knowledge-base identity.

An additional n-gram field covers identifiers that normal word segmentation
handles poorly, including:

- `WorkspaceBackendCache`;
- `a3s.evolution.signal.v1`;
- `source_digest`;
- file paths and package names;
- mixed Chinese, Latin, and numeric terms.

The query planner searches normal language and identifier fields together.
Chinese correctness must be evaluated with Chinese fixtures rather than inferred
from English benchmark results.

## AI-native retrieval contract

AI-native means the search surface is designed for bounded reasoning and
evidence use. It does not mean every query must load an embedding or reranking
model.

### Request

The shared service accepts:

```typescript
interface OkfSearchRequest {
  workspaceRoot: string;
  queries: Array<{
    text: string;
    weight?: number;
    mode?: 'terms' | 'phrase' | 'advanced';
  }>;
  filters?: {
    knowledgeBaseIds?: string[];
    types?: string[];
    tags?: string[];
    source?: 'compiled' | 'user';
    pathPrefix?: string;
    resourcePrefix?: string;
  };
  limit?: number;
  candidateLimit?: number;
  maxSnippetCharacters?: number;
  explain?: boolean;
}
```

The original query receives the highest default weight. An agent can submit
synonyms or alternate terminology in the same call. Results from multiple
queries are combined with reciprocal-rank fusion before final deterministic
field bonuses.

`advanced` syntax is opt-in. Ordinary user text is parsed as terms so
punctuation cannot unexpectedly become an expensive or invalid query.

### Result

Every result is usable as an evidence citation without reading the entire
document:

```typescript
interface OkfSearchHit {
  score: number;
  documentId: string;
  sectionId: string;
  knowledgeBaseId: string;
  path: string;
  title: string;
  description?: string;
  type: string;
  tags: string[];
  resource?: string;
  sources: string[];
  headingPath: string[];
  startLine: number;
  endLine: number;
  snippet: string;
  matchedQueries: number[];
  provenance: {
    source?: 'compiled' | 'user';
    sourceDigest?: string;
    contentDigest: string;
  };
  explanation?: {
    lexicalScore: number;
    fieldMatches: string[];
    fusionContribution: number;
    bonuses: string[];
  };
}
```

The service bounds hit count, candidate count, query length, filters, snippet
size, and total response bytes. It never returns a complete knowledge base in
one response.

## Runtime integration

### Ownership

The reusable contract belongs in A3S Core, next to workspace services and
context providers. The Web controller must not own the index.

```text
WorkspaceServices
  +-- file manifest and change stream
  +-- code intelligence
  +-- optional typed OkfSearch service
```

The Tantivy implementation is feature-gated so library embedders do not pay for
it unless their host enables local OKF search. `WorkspaceServices` receives a
typed search object, never a raw engine name or index path.

Code Web's existing canonical-workspace cache owns one service instance and one
writer per workspace. Work, Knowledge, agent sessions, and API requests reuse
that instance.

### Agent surfaces

The first implementation exposes:

- `knowledge_search`: structured multi-query search with typed filters and
  bounded snippets;
- the existing `read` tool for retrieving the cited OKF line range;
- `OkfContextProvider`: optional automatic top-k context injection with a
  relevance threshold and strict token cap.

The context provider emits `okf://<knowledge-base>/<path>#L<start>-L<end>`
provenance. It does not inject low-confidence hits merely to fill the token
budget.

The existing `grep` tool remains the exact, real-time raw-workspace search
surface. OKF search does not replace code grep and code grep does not serve as
the persistent knowledge index.

## Persistent index lifecycle

### Storage

Indexes live outside the workspace under the platform cache directory:

```text
<cache>/a3s/okf-search/<canonical-workspace-hash>/schema-v1/
```

This avoids repository noise and prevents index files from triggering the
workspace watcher. The cache directory uses owner-only permissions where the
platform supports them.

### Startup

1. Canonicalize and validate the workspace root.
2. Acquire the process-local service entry and index writer.
3. Open the current schema version.
4. Compare the durable file manifest with the current workspace manifest.
5. Re-index only missing or digest-mismatched conformant OKF files.
6. Publish readiness while any remaining initial work continues in the
   background.

Corruption or a schema mismatch builds a fresh sibling index and swaps it in
only after a successful commit. The previous readable index remains available
during rebuild.

### Incremental updates

The service subscribes to the existing A3S Core workspace manifest change
stream. It does not create another recursive watcher.

- create or change: parse, validate, delete old sections by document ID, insert
  new sections;
- delete: delete all sections by document ID;
- rename: delete the old path identity and insert the new identity;
- burst: debounce and batch commits;
- lagged change channel: reconcile against the manifest instead of guessing
  which events were missed.

An atomic compiler promotion is observed as one committed generation change.
The indexer must not subscribe to or traverse `.a3s/compilation-output`, and it
must never publish a partially written generation while the compiler is still
running.

A successful commit reloads the reader. Failed parsing retains the last valid
indexed version only when the path and content digest prove it is the same
concept; otherwise the stale record is removed and the diagnostic is visible.

### Status model

```typescript
type OkfIndexPhase = 'unavailable' | 'opening' | 'building' | 'ready' | 'degraded' | 'failed';

interface OkfIndexStatus {
  phase: OkfIndexPhase;
  schemaVersion: number;
  indexedDocuments: number;
  indexedSections: number;
  invalidDocuments: number;
  pendingDocuments: number;
  lastCommittedAt?: string;
  lastError?: string;
}
```

Search can serve the last committed index during `building` or `degraded`.
`failed` means no readable committed index exists.

## Local Web API

The Web host exposes the shared service through the standard A3S response
envelope:

- `POST /api/v1/knowledge/search`;
- `GET /api/v1/knowledge/search/status?rootPath=...`;
- `POST /api/v1/knowledge/search/rebuild`.

Rebuild is an explicit mutation. Ordinary search never triggers an unbounded
synchronous rebuild. A request may start bounded lazy initialization, return
the current status, and let the UI retry as readiness advances.

All roots are canonicalized and confined to workspaces already admitted by the
host. Clients cannot choose an arbitrary cache path.

## Work and Knowledge UX

Filename navigation and knowledge retrieval stay visually distinct.

### Work

- An empty search field continues to filter and navigate files.
- Inline scope controls distinguish `Current folder`, `All file names`, and
  `Knowledge content`.
- Knowledge results use a dedicated list presentation with title, heading,
  snippet, OKF type, tags, and bundle-relative location.
- Selecting a result opens the OKF document at its cited section.
- A secondary source action opens `resource` when it resolves inside the
  workspace.
- Multi-selection can add the cited sections to the AI Assistant as bounded
  context without sending the entire files.
- Indexing and degraded states appear inline; they do not use blocking dialogs.

The first UI integration must not label name-only traversal as full-text search.

### Knowledge

- Reuse the same query, filters, ranking, status, and result components.
- Add explicit type and tag filters.
- Preserve the knowledge-base boundary in every result and deep link.
- Surface invalid OKF diagnostics near the affected base rather than as a
  global modal.

## Delivery phases

### Phase 0: lexical foundation

- Add the typed service and result contracts.
- Parse and validate OKF frontmatter and CommonMark sections.
- Implement the Tantivy schema, Jieba and identifier tokenizers, persistent
  cache, first build, incremental updates, and schema recovery.
- Add status, search, and rebuild APIs.
- Add Chinese and English correctness fixtures plus latency benchmarks.

Exit: persistent BM25 search returns precise line-cited OKF sections and updates
after a file edit without rescanning all content.

### Phase 1: AI and product integration

- Add `knowledge_search` and the bounded context provider.
- Add multi-query fusion and optional score explanations.
- Integrate Work and Knowledge result UX, filters, keyboard navigation, Quick
  Look, and AI context selection.
- Replace any UI copy that currently implies filename traversal is full-text.

Exit: the human UI and the agent consume the same ranked service and return the
same citations for the same request.

### Phase 2: measured hybrid retrieval

- Build relevance fixtures with expected documents and sections.
- Compare BM25, lexical multi-query fusion, vector retrieval, and hybrid
  retrieval using recall@k, MRR, and nDCG.
- Add an optional vector backend and reranker only if it materially improves
  declared query classes within desktop latency and memory budgets.

Embedding generation remains asynchronous and never blocks lexical search.
Lexical results remain available when models, accelerators, or vector indexes
are unavailable.

## Release gates

### Correctness

- exact title, type, tag, resource, phrase, Chinese term, mixed identifier, and
  path searches;
- deterministic section line ranges and snippets;
- create, modify, delete, rename, lagged-event reconciliation, and interrupted
  rebuild cases;
- invalid frontmatter, invalid UTF-8, oversized documents, broken links, and
  out-of-workspace path rejection;
- identical result contract across Web API, agent tool, and context provider.

### Performance

Measure at 1,000, 10,000, and 100,000 sections on supported desktop platforms.
Initial targets for a warm 10,000-section index are:

- p50 search below 20 ms;
- p95 search below 75 ms;
- cold service open below 200 ms;
- edited OKF searchable within 500 ms p95;
- no query-time recursive directory traversal or complete-file corpus reads.

Targets are release gates only after the benchmark fixture and hardware class
are recorded. They are not inferred from upstream project claims.

### Resource and privacy

- bounded writer memory and response size;
- cache size and document counts visible in status;
- no network dependency in the lexical path;
- no query or document body in telemetry;
- no Office/PDF extraction dependency;
- owner-only cache permissions where supported;
- clean shutdown with committed index state and no orphaned writer lock.

## Explicit non-goals

- Office, PDF, image, archive, email, or web extraction;
- OCR;
- editing or compiling OKF;
- distributed or team-shared search;
- a second local search daemon;
- silent model download;
- replacing code intelligence, filename navigation, or exact grep;
- graph visualization or a general knowledge query language.
