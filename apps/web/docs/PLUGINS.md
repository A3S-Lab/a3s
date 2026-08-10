# A3S Web Plugin System

## Product model

A3S Web treats an installed A3S Use package as the equivalent of a VS Code
extension package. Work is the first and default Activity Bar entry, and the
non-executable Knowledge product follows it. An enabled package can add a
workbench view through the
`contributes.activity_bar` contribution point; vertical products such as
Research and Finance are not hardcoded shell entries.

The package remains the unit of identity, trust, installation, upgrade,
disable, and removal. A contribution adds navigation and a non-callable HTML
view only. Native actions continue to use the package's CLI, standard MCP, and
Skill surfaces.

Knowledge packages are a separate product contract. A knowledge package installs
data into the personal knowledge library and never contributes executable UI,
CLI, MCP, or Skill authority. Its discovery and local lifecycle are documented
in [A3S Web Knowledge](KNOWLEDGE.md).

The model combines two useful references without copying either runtime:

- VS Code supplies the package identity, declarative contribution point,
  Marketplace lifecycle, and enable/disable model.
- [Paciolan remote-component](https://github.com/Paciolan/remote-component)
  supplies the small loader boundary: an explicit content source, separate
  loading/error/ready states, caller-controlled fallback UI, and rejection of
  stale asynchronous loads.

The equivalent of `remote-component` dependency injection is the versioned,
bounded `a3s.activity.v3` capability protocol. After the verified document's
first load, the host creates a `MessageChannel` and transfers one endpoint in
`host.init`. A plugin receives only that dedicated port and declared host facts,
never the host React runtime, ambient window messages, or a `require` function.
The equivalent of its custom fetcher is server-owned: the Activity catalog
publishes one exact Registry generation/revision `documentUrl`, and the browser
adopts that URL without fetching executable package bytes through management
JSON.

```acl
contributes {
  activity_bar "research" {
    title       = "科研"
    description = "Prepare reviewable, evidence-backed research tasks across disciplines."
    icon        = "flask-conical"
    entry       = "web/activity.html"
    styles      = ["web/activity.css"]
    scripts     = ["web/activity.js"]
    skill       = "a3s-use-science"
    order       = 120
  }
}
```

The stable host key is `<route>:<activity-id>`. The referenced Skill must be
declared by the same package. The host ignores any Skill name supplied by
plugin messages.

## Asset contract

Activity HTML and its explicitly declared CSS/JavaScript resources are validated
twice: first by A3S Use while projecting the installed package, then
independently by A3S Web before they become Web API content. Every asset must
be:

- inside the immutable package root;
- a regular file rather than a link or special file;
- UTF-8 with the declared `text/html`, `text/css`, or `text/javascript` media
  type and no larger than 2 MiB;
- bound to the registry snapshot by lowercase SHA-256;
- associated with the same package and Skill in both the capability snapshot
  and Activity catalog.

The server removes undeclared external stylesheet and script references, then
inlines only verified package resources under the response CSP. The enabled
catalog item must publish exactly
`/api/v1/plugins/activities/<encoded-key>/document?generation=<generation>&revision=<revision>`.
The browser rejects a missing URL, absolute URL, raw or different key, stale
generation or revision, extra query parameter, or a URL on a disabled item.
Loading, ready, and error states remain separate. A Registry identity change
creates a new iframe and closes the prior communication capability.

A3S does not adopt remote URL evaluation, runtime JavaScript compilation, or
`new Function`. Plugin UI always comes from an installed, reviewed package.

## Browser isolation

The server reparses package HTML and prepends a restrictive Content Security
Policy before any package node. Network connections, objects, nested frames,
workers, forms, base URLs, and navigation are denied. Images, fonts, and media
are limited to embedded `data:` or `blob:` content as applicable.

The view renders with:

```html
<iframe
  sandbox="allow-scripts"
  referrerpolicy="no-referrer"
  src="/api/v1/plugins/activities/.../document?generation=...&revision=..."
></iframe>
```

`allow-same-origin` is intentionally absent, so the plugin receives a unique
opaque origin and cannot access host DOM or storage. The parent ignores ambient
`window` messages. On the first iframe load it transfers a fresh `MessagePort`;
the view must report ready within 10 seconds. A second load from the same iframe
is treated as a self-navigation violation: the host closes the port and removes
the iframe. Unmount, retry, disable, uninstall, and Registry generation or
revision changes also close the old port. Every port message is checked against
the current key, generation, revision, and document URL before parsing.

The protocol supports:

- `host.init`: verified package identity, contribution key, Registry generation
  and revision, resolved theme, locale, and the transferred port;
- `activity.ready`: view startup completion;
- `activity.error`: a bounded user-visible runtime error;
- `context.propose`: a bounded title, summary, prompt, and up to 12 display
  fields, plus the optional `usePackageSkill` routing decision;
- `state.get`, `state.set`, `state.delete`, and `state.clear`: correlated,
  serialized access to host-owned durable state; and
- `state.result` or `state.error`: the host response carrying the same bounded
  `requestId`.

There is no generic execute message.

State requests use a machine-safe `requestId` of at most 64 UTF-8 bytes. Keys
are at most 128 bytes; a value is at most 16 KiB; each surface stores at most 64
entries and 256 KiB. The host submits requests in MessagePort order, so a
completed `set` is visible to the following `get`. It checks the document
identity before and after every operation. A stale generation receives no port
reply and is drained; missing, converging, capacity, and storage failures return
bounded errors.

The iframe never receives filesystem access and does not use `localStorage` or
same-origin storage. Code persists state under the canonical User scope,
lifecycle package ID, and UI surface ID. The server acquires an exact published
package-generation lease before touching the generation-neutral namespace, so
a stale iframe cannot start a write after retirement begins. Restart, disable,
rollback, replacement retirement, and an upgrade retaining the same surface
preserve state. True uninstall or removal of that surface clears it, including
a corrupt snapshot. State retention and failed-N+1 selection are separate
contracts; Code Web now composes both.

## Candidate readiness before cutover

An install or upgrade that contributes UI does not publish N+1 immediately.
The server first exposes a process-local, path-free candidate catalog and an
exact candidate document URL. Code Web loads that document in a hidden
`sandbox="allow-scripts"` iframe without same-origin authority and transfers a
new dedicated `MessagePort` with only readiness-mode `host.init` identity.
The candidate receives no state, context, Tool, MCP, Flow, or other backend
bindings. The host accepts only bounded `activity.ready` or `activity.error`
messages on that port.

Load failure, a second navigation, a malformed/error message, or the readiness
deadline fails closed. A3S Use rolls the candidate lifecycle back while the
selected N document remains callable; package receipts, Registry generation,
and lifecycle roots do not retain N+1 residue. That reviewed plan becomes
terminal and cannot republish its candidate. A fresh reviewed plan may retry
the same N+1 lifecycle generation, and a successful ready proof produces one
Registry cutover. CLI, TUI, and native hosts still use static integrity checks
only until they inject an equivalent renderer. Reviewed UI backend bindings
and general-purpose native UI hosting remain release work.

## Context handoff

`context.propose` always opens a host-owned review dialog. The user sees the
summary, fields, exact prompt, and host-verified Skill decision before anything
enters the current Work draft. In the current v3 schema, an omitted
`usePackageSkill` value defaults to `true`; when it is `false`, accepting the
review appends only the prompt and does not select a Skill. A plugin cannot name
an arbitrary Skill: the host can attach
only the Skill declared by the same installed package. Dismissing the proposal
has no side effect, and plugin HTML cannot submit a task directly. The host
adds the source key, generation, revision, and document URL to every proposal;
review and acceptance fail closed if any part of that identity is no longer
current.

The Research contribution uses this distinction deliberately. Its workbench
organizes a task as a project and follows a question → evidence → analysis →
artifact → review loop. Life-science sources may request the verified
`a3s-use-science` Skill. Other disciplines use A3S's current general research
capabilities and do not receive the life-science Skill. Every brief asks for a
reviewable research package with a provenance note covering sources, methods or
code, execution records, key parameters, artifact relationships, and unfinished
verification items.

The product organization takes inspiration from Claude Science's project and
artifact-centered workbench and Open Science's plan → execute → produce →
preview flow. A3S does not copy either shell or grant their runtime authority:
the contribution remains an isolated A3S Web plugin surface, while Work owns
execution, files, editable artifacts, logs, and final review.

## Marketplace lifecycle

The Marketplace reads two explicit package-source classes:

- optional `release-bundle` packages carried by the verified A3S Use release;
- configured remote registries verified through TUF.

Release bundles are not built-in capabilities: they remain absent from the
runtime until the user installs them, and they can be disabled or removed like
any other extension. A3S Use exposes only validated bundle metadata, while the
umbrella plan binds the exact expanded-package SHA-256 and A3S Use checks it
again immediately before activation. Unconfigured or failed registries remain
visible with their verification state but contribute no installable packages.

The catalog keeps discovery and trust inspection separate. The Plugins view
provides one complete catalog plus an installed-only view, with text search and
Stable, Beta, or Nightly channel filters. It does not invent recommendations in
the browser. The Sources view shows a plain verification state and keeps TUF
metadata under Technical information. Selecting install or upgrade from a
package card opens confirmation; it never mutates the installation directly.

Every install, upgrade, or uninstall is two phase:

1. the Web API invokes the current `a3s` executable with `--dry-run`;
2. the user reviews the exact plan and its SHA-256 digest;
3. explicit confirmation invokes the same operation with `--plan-digest`;
4. a changed plan, release bundle, registry, target, or package fails closed.

Enable and disable operations use the A3S Use extension lifecycle and trigger a
registry refresh. The Activity Bar polls the immutable registry revision, so
install, upgrade, disable, and uninstall converge without reloading A3S Web.

The repository-level lifecycle proof is:

```sh
just marketplace-science-e2e
```

It builds the real `a3s-use` binary and packaged `a3s-use-science` release, then
runs both supported source paths: an A3S Use release bundle with no configured
registry and an ephemeral signed TUF repository. The checks exercise Web API
install plan/apply, direct Activity opening, verified
HTML/CSS/JavaScript delivery, reviewed workbench-to-Work handoff, packaged
`science doctor`, all 13 namespaced Science MCP tools, uninstall plan/apply,
`release-bundle`/`registry-tuf` receipt provenance, and package-directory
cleanup. The test is local and does not claim that a package has been published
to the production registry.

The browser composition proof uses the real production Web build and the
static CLI fixture:

```sh
a3s-test check tests/e2e/activity-host.acl --json
a3s-test run tests/e2e/activity-host.acl --json
```

It waits for the host-owned ready state, sends a proposal through the
transferred port, verifies the host review dialog, and captures sandbox,
accessibility, console, page-error, and screenshot evidence. The adjacent
`activity-document.acl` suite verifies the server document boundary directly.

Pre-cutover candidate behavior has separate coverage. The Web host tests prove
exact identity, the script-only sandbox, readiness-only messages, authority
isolation, navigation failure, and protocol failure. The generic real A3S Use
Marketplace E2E then proves N remains callable after a failed N+1 proof, no
receipt or lifecycle root leaks, the rolled-back plan cannot republish, a fresh
plan succeeds at the same lifecycle generation, and the final Registry
generation advances once. Browser QA uses a candidate that emits
`activity.error` if either its state or context probe receives any host reply;
the captured ready decision therefore proves the candidate port stayed
authority-free.

## Local Web API

The loopback Web service exposes:

- `GET /api/v1/plugins/activities`
- `GET /api/v1/plugins/activities/{key}`
- `GET /api/v1/plugins/activities/{key}/document?generation={generation}&revision={revision}`
- `POST /api/v1/plugins/activities/{key}/state?generation={generation}&revision={revision}`
- `GET /api/v1/plugins/marketplace`
- `POST /api/v1/plugins/operations/plan`
- `POST /api/v1/plugins/operations/apply`
- `POST /api/v1/plugins/packages/enabled`

These routes do not weaken the existing loopback deployment boundary. Browser
clients never receive A3S OS tokens, trust-root keys, package filesystem paths,
or authority to bypass plan review. The `{key}` JSON response is management
data; plugin execution uses only the exact `/document` URL.
