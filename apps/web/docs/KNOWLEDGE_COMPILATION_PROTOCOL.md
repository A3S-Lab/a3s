# Knowledge Compilation Protocol

## Purpose and boundary

This protocol connects A3S Web's durable knowledge-compilation queue to an
independent compiler. A3S owns source-package creation, change detection,
scheduling, state, and atomic publication. The compiler owns extraction,
normalization, and generation of conformant OKF Markdown.

The compiler may process Office, PDF, image, archive, or other source formats,
but those capabilities are intentionally outside A3S Web. Search never reads
raw `sources/` and never claims a staging directory.

The current `contractVersion` is `"1"`.

## Lifecycle

```text
source package created
        |
        v
source_ready -- manual/smart-auto --> queued -- claim --> running
                                                        |       |
                                                        |       +--> failed / retry
                                                        v
                                               staged wiki/**/*.md
                                                        |
                                                  success result
                                                        |
                                                        v
                                               atomic wiki/ promotion
```

Creation and compilation are independent mutations. A newly created source
package has no searchable output. The host keeps queue state durable across
page refreshes and process restarts.

## Claiming work

The compiler polls the loopback Web API:

```http
POST /api/v1/knowledge/compilations/claim
Content-Type: application/json

{}
```

The optional `workspace` request field admits that workspace to the host's
monitored set. Selection still considers every monitored workspace so the host
can enforce one global running job. Manual jobs are selected before retries,
which are selected before smart-automatic jobs; jobs with the same priority are
ordered by creation time and stable identifiers.

When no work is available, `data.claimed` is `false` and all path and job fields
are absent or null. A successful claim resembles:

```json
{
  "claimed": true,
  "contractVersion": "1",
  "workspaceRoot": "/workspace",
  "knowledgeBasePath": "/workspace/.a3s/kb/bases/project",
  "sourcePath": "/workspace/.a3s/kb/bases/project/sources",
  "previousWikiPath": "/workspace/.a3s/kb/bases/project/wiki",
  "outputPath": "/workspace/.a3s/kb/bases/project/.a3s/compilation-output/job-1/wiki",
  "job": {
    "id": "job-1",
    "knowledgeBaseId": "project",
    "trigger": "manual",
    "phase": "running",
    "sourceDigest": "sha256:...",
    "createdAt": "2026-07-26T00:00:00Z",
    "startedAt": "2026-07-26T00:00:01Z",
    "completedAt": null,
    "outputPath": "/workspace/.a3s/kb/bases/project/.a3s/compilation-output/job-1",
    "error": null
  }
}
```

All API values are wrapped in the standard A3S success envelope. The compiler
must reject an unsupported `contractVersion` instead of guessing compatible
semantics.

## Compiler obligations

For one claim, the compiler must:

1. Read inputs only from `sourcePath`. `previousWikiPath` is optional reference
   material, never an output destination.
2. Treat `job.sourceDigest` as the identity of the claimed generation. Changes
   after the claim are scheduled as another generation by the host.
3. Write the candidate tree only under `outputPath`. Do not mutate the managed
   `wiki/`, manifests, snapshots, or job records directly.
4. Emit at least one regular lowercase-`.md` file beneath `outputPath` and keep
   every emitted path inside that directory. Symlinks are not a publication
   mechanism.
5. Produce UTF-8 Markdown with closed YAML frontmatter and the required OKF
   `type` field when the output is intended for A3S search.
6. Send exactly one terminal result. Do not retry a result mutation after an
   ambiguous transport failure without first reading job state.

The host currently validates that the staged `wiki/` is a real directory and
contains at least one Markdown file before promotion. The search index performs
the stricter per-document OKF conformance validation.

## Reporting success

After the complete output tree is durable, report:

```http
POST /api/v1/knowledge/bases/project/compilations/job-1/result
Content-Type: application/json

{
  "workspace": "/workspace",
  "outcome": "succeeded",
  "compilerVersion": "my-compiler/1.4.0"
}
```

The host verifies the claimed job is still active, validates staged output,
renames the previous `wiki/` to a temporary backup, promotes the staged tree,
and removes the backup only after promotion succeeds. If promotion fails, it
restores the previous tree. A successful result records the source digest and
compiler version. If a newer source generation arrived while the job ran, the
base returns to `source_ready` and keeps that newer generation pending.

## Reporting failure

Report bounded diagnostic text and whether another attempt is likely to work
without changing the source package:

```http
POST /api/v1/knowledge/bases/project/compilations/job-1/result
Content-Type: application/json

{
  "workspace": "/workspace",
  "outcome": "failed",
  "transient": true,
  "error": "temporary model process unavailable",
  "compilerVersion": "my-compiler/1.4.0"
}
```

Errors are stored with a 1,000-character bound. A transient failure is retried
after 5 minutes, 30 minutes, and 2 hours; the fourth failure remains failed.
This retry policy applies whether the original job was manual or automatic.
Permanent input, policy, or format errors must set `transient` to `false`.
Failure always preserves the last successful `wiki/`.

## Cancellation and concurrency

The host can cancel queued or running work through:

```http
POST /api/v1/knowledge/bases/{id}/compilations/{jobId}/cancel
Content-Type: application/json

{ "workspace": "/workspace" }
```

Cancellation changes host state; a compiler must also stop its local process
when it observes cancellation or a rejected terminal result. The staged output
is never searchable.

Only one job may be in `running` state across the host's monitored workspaces.
A second claim returns `claimed: false` while any job is running. Compilers
should use bounded polling with backoff and must not start parallel work for the
same claim.

## Source-change policy

Smart-automatic compilation is opt-in per knowledge base. The host waits for a
5-second file-stability window, 30 seconds of source-set quiet, and a 10-minute
minimum interval between automatic jobs. Pure mtime changes do not produce a
new source digest.

Automatic work pauses for review when deletion or total-change thresholds
indicate a likely unmount, sync reset, or accidental bulk operation. Manual
compilation clears the pause after the user reviews the source state. The
compiler does not implement its own filesystem watcher or scheduling policy.

## Search publication boundary

Only the managed, successfully promoted `wiki/**/*.md` tree is eligible for
indexing. These locations are always excluded:

- `sources/**`;
- `.a3s/compilation-output/**`;
- a source-backed base with no successful `wiki/` tree;
- failed output that was not promoted.

This boundary guarantees that creation is cheap and reversible, failed builds
cannot corrupt the searchable generation, and the independent compiler can
evolve without coupling raw-format extraction to the search service.
