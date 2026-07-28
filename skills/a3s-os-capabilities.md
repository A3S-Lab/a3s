---
name: a3s-os-capabilities
description: "A3S OS progressive API — the way to answer ANY question about the signed-in A3S OS platform: your platform account/identity, what the platform can do, and its data / resources / state (LLM/OCR, assets, packages, runtime, knowledge, observability, …). One action-dispatched endpoint, broad-to-narrow: list -> search -> describe -> execute. Use this — NOT the local shell (whoami/paths) — for anything about the OS platform. Available only when signed in."
kind: instruction
allowed-tools: "bash(*), read(*)"
---

# A3S OS progressive API

Discover and call the A3S OS platform capabilities you are authorized for,
*progressively* (broad → narrow), through **one** endpoint. This is the whole
platform's capability surface — not just one domain: AI capabilities (LLM/OCR
config), assets, packages, registry, runtime, resources, knowledge bases,
observability, marketplace, and more. Security is one domain among these.

Use this skill for **any** question about the OS platform — when the user says
"OS" they mean this signed-in platform, not the local operating system. That
includes your platform **account / identity** ("what's my OS account?"), what the
platform can do, and its data/state ("what LLM/OCR is configured", "OCR this
PDF", "list my assets", "search platform operations for X"). Do NOT answer these
from the local shell or filesystem (`whoami`, paths, env) — those describe this
machine, not the OS platform. Start with `list` to find the right module/op.

Single endpoint (`action`-dispatched, always `POST` with a JSON body):

```
POST {{BASE_URL}}/api/v1/kernel/capabilities
```

Flow (broad-then-narrow) — like a CLI, expand on demand instead of
loading every manual into context:

```
list  →  search  →  describe  →  execute
(modules) (find op) (op schema)  (run it)
 git --help  apropos  rebase --help  rebase -i
```

## Authentication

The endpoint and Bearer token are **already in your shell environment** (exported
when you signed in) — use them directly; do NOT read `~/.a3s/os-auth.json` or any
config file on each call:

- `$A3S_OS_BASE_URL` — the platform base URL (`{{BASE_URL}}`)
- `$A3S_OS_TOKEN` — the Bearer token

Everything is permission-filtered by that token; you only ever see/run what the
signed-in user may access.

## Request

```json
{ "action": "list | search | describe | execute",
  "module":    "<module name> (describe / execute)",
  "query":     "<keywords> (search)",
  "operation": "<operation name> (execute = the op to run; describe = return just that one op's full schema)",
  "params":    { } }
```

| action | needs | returns |
| --- | --- | --- |
| `list` | — | every module you can access (name, description, path, operationCount) |
| `search` | `query` | matching operations across modules |
| `describe` | `module` (+ optional `operation`) | the module's sub-modules + its operations; or, with `operation`, just that ONE operation's full input/output schema |
| `execute` | `module`, `operation`, `params` (+ `"shaped": true` to get the view) | the operation result (`data`); with `"shaped": true`, also an optional `view` object (a console deep link + suggested popup size) and `ui` agent-ui directive |

## Rules

- **Stay narrow — never dump the whole catalog.** Walk it like a CLI, one rung at
  a time, fetching only what the user's question needs: `list` (modules only) →
  pick the relevant module → `describe` it to see its **sub-modules** and
  operation counts (drill into a sub-module, don't enumerate every operation) →
  `search`/`describe <module> <operation>` for the ONE operation you'll run →
  `execute`. Show the user only the operation(s) that answer them, not every
  interface.
- **Keep output tight — extract, don't dump.** For `list`/`search`/`describe`,
  pipe through `jq` to pull only the fields you need (e.g.
  `... | jq -r '.data.modules[].name'` for a module list, `... | jq '.data | keys'`
  to peek a shape) so the result is a few relevant lines, not a raw JSON blob.
  **Exception — `execute`:** send `"shaped": true` and pipe the response WHOLE (do
  not narrow it); it is already compact and its `.view` field is what gives the
  user the Open view button — narrowing it away loses the view affordance. In your reply,
  summarize in a few lines either way — do NOT paste the whole response back.
- Never guess `module`, `operation`, field names, or enums. `list` / `search` /
  `describe` first, then build `params` from the returned schema. `describe` with
  an `operation` gives that op's exact schema — the rung right before `execute`.
- On an `execute` schema error, re-`describe` that operation and fix `params`
  instead of inventing fields.
- Prefer read/`GET`-style operations for discovery; write operations (create /
  update / delete) run with the user's real platform permissions — confirm intent
  before mutating platform state.
- **Always report the call trace.** Every response envelope carries a `requestId`
  and a `timestamp`. After summarizing the result, output them on their own line so
  the call is traceable, exactly: `↳ requestId <requestId> · <timestamp>`. Keep
  `.requestId` and `.timestamp` in any `jq` projection (don't narrow them away).
- **Let the host surface the `view` link — you don't print it.** An `execute`
  response includes a `view` object — `{ "url": "…?embed=1", "width": N, "height":
  N }` — a focused console page sized for a popup, **only when you send
  `"shaped": true` in the execute request body.** Two things must happen:
  1. **Send `"shaped": true`** on every `execute` call, and **do NOT `jq`-narrow
     the execute response** — pipe it whole (it is already compact). Without the
     flag, or if you strip `.view`, no view is produced and the user gets no link.
  2. **Do NOT print any `Open view` line yourself.** The host inspects the execute
     output and, whenever a `.view` is present, automatically renders a one-click
     `Open view` button that opens the **Progressive UI** popup (the user's current OS
     login is injected — no re-login). It is **user-triggered**: the popup is NOT
     opened automatically; the user clicks that line (or runs `/view`). Never
     print the raw URL. Printing the link yourself only duplicates the host's.

  So a typical reply ends with just the trace line:
  ```
  ↳ requestId 52178323-b614-42e4-af60-4b0b91ad8355 · 2026-07-01T01:21:07.905Z
  ```
- The `ui` field (`protocol: "agent-ui"`) is a host-rendered remote component —
  note that it exists if present, but don't try to render it yourself.

## Learned shortcuts — shorten the chain on repeat tasks

The `list → search → describe → execute` walk is for *discovering* an operation.
Once you've resolved one, remember it so the next similar task skips discovery.

- **Cache:** `~/.a3s/os-learned.md` (per-user). At the **start** of an OS task,
  read it — `cat ~/.a3s/os-learned.md 2>/dev/null`. If it already maps a task like
  the user's to a `module`/`operation`, **skip `list`/`search`**: go straight to
  `describe` that operation (to confirm its current schema) → `execute`. That turns
  a 4-step walk into 1–2 steps.
- **After** you successfully `execute` a NEWLY-resolved operation, append one terse
  line so the next run is faster:
  ```bash
  echo '- <short task intent> → {module}/{operation} (params: <key params>)' >> ~/.a3s/os-learned.md
  ```
  Don't duplicate an existing entry; don't cache failed, ambiguous, or one-off calls.
- The cache is a **hint, not gospel**: if `describe` shows the schema changed or
  `execute` errors with the cached operation, fall back to `list`/`search` to
  re-discover and fix the stale entry.

## Examples

```bash
# Endpoint + token come from the env exported at login — no config read needed.
API="$A3S_OS_BASE_URL/api/v1/kernel/capabilities"
post() { curl -s -X POST "$API" -H "Authorization: Bearer $A3S_OS_TOKEN" -H 'Content-Type: application/json' -d "$1"; }

post '{"action":"list"}'                                              # 1. what modules exist
post '{"action":"search","query":"ocr"}'                             # 2. find operations
post '{"action":"describe","module":"kernel","operation":"runOcr"}'  # 3. exact schema

# execute: send "shaped":true and DON'T narrow — the whole (compact) response
# carries `.view`, which the host turns into the Open view button.
post '{"action":"execute","module":"assets","operation":"listAssets","shaped":true}'
```

```json
// 4a. list the system's configured LLM/OCR capabilities (masked projection)
{ "action": "execute", "module": "kernel", "operation": "listAiCaps", "shaped": true }
```

```json
// 4b. run OCR through the platform's configured backend (you never see its URL/key)
{ "action": "execute", "module": "kernel", "operation": "runOcr", "shaped": true,
  "params": { "url": "https://…/spec.pdf", "mimeType": "application/pdf", "modelType": "document", "outputFormat": "markdown" } }
```
