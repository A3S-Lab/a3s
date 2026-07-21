# A3S Web Plugin System

## Product model

A3S Web treats an installed A3S Use package as the equivalent of a VS Code
extension package. Code is the first and default Activity Bar entry. An enabled
package can add a workbench view through the `contributes.activity_bar`
contribution point; Work and Science are not hardcoded shell entries.

The package remains the unit of identity, trust, installation, upgrade,
disable, and removal. A contribution adds navigation and a non-callable HTML
view only. Native actions continue to use the package's CLI, standard MCP, and
Skill surfaces.

The model combines two useful references without copying either runtime:

- VS Code supplies the package identity, declarative contribution point,
  Marketplace lifecycle, and enable/disable model.
- [Paciolan remote-component](https://github.com/Paciolan/remote-component)
  supplies the small loader boundary: an explicit content source, separate
  loading/error/ready states, caller-controlled fallback UI, and rejection of
  stale asynchronous loads.

The equivalent of `remote-component` dependency injection is the versioned,
bounded `host.init`/`postMessage` protocol. A plugin receives only declared host
facts and proposal capabilities, never the host React runtime or an ambient
`require` function. The equivalent of its custom fetcher is owned by A3S Code:
it resolves content only from the installed package snapshot and verifies the
registry revision and digest before rendering.

```acl
contributes {
  activity_bar "research" {
    title       = "Science"
    description = "Prepare evidence-backed life-science research tasks."
    icon        = "flask-conical"
    entry       = "web/activity.html"
    skill       = "a3s-use-science"
    order       = 120
  }
}
```

The stable host key is `<route>:<activity-id>`. The referenced Skill must be
declared by the same package. The host ignores any Skill name supplied by
plugin messages.

## Asset contract

Activity HTML is validated twice: first by A3S Use while projecting the
installed package, then independently by A3S Code before it becomes Web API
content. The asset must be:

- inside the immutable package root;
- a regular file rather than a link or special file;
- UTF-8 `text/html` no larger than 2 MiB;
- bound to the registry snapshot by lowercase SHA-256;
- associated with the same package and Skill in both catalog and content
  responses.

A changed registry revision or asset digest invalidates cached content. Requests
carry sequence IDs and abort signals so a stale response cannot replace a newer
selection. Loading, error, and rendered states remain separate, and late loads
are discarded.

A3S does not adopt remote URL evaluation, runtime JavaScript compilation, or
`new Function`. Plugin UI always comes from an installed, reviewed package.

## Browser isolation

The host reparses package HTML and prepends a restrictive Content Security
Policy before any package node. Network connections, objects, nested frames,
workers, forms, base URLs, and navigation are denied. Images, fonts, and media
are limited to embedded `data:` or `blob:` content as applicable.

The view renders with:

```html
<iframe sandbox="allow-scripts" referrerpolicy="no-referrer"></iframe>
```

`allow-same-origin` is intentionally absent, so the plugin receives a unique
opaque origin and cannot access host DOM or storage. The host accepts messages
only when `event.source` is the active iframe and the payload passes the
bounded `a3s.activity.v1` schema.

The protocol supports:

- `host.init`: verified package identity, contribution key, resolved theme,
  and locale;
- `activity.ready`: view startup completion;
- `activity.error`: a bounded user-visible runtime error;
- `context.propose`: a bounded title, summary, prompt, and up to 12 display
  fields.

There is no generic execute message.

## Context handoff

`context.propose` always opens a host-owned review dialog. The user sees the
summary, fields, exact prompt, and host-verified Skill before anything enters
Code. Accepting the review appends the context to the Code composer and selects
the same-package Skill; dismissing it has no side effect. Plugin HTML cannot
submit a task directly.

## Marketplace lifecycle

The Marketplace reads only configured TUF registries. Unconfigured or failed
registries remain visible with their verification state but contribute no
installable packages.

Every install, upgrade, or uninstall is two phase:

1. the Web API invokes the current `a3s` executable with `--dry-run`;
2. the user reviews the exact plan and its SHA-256 digest;
3. explicit confirmation invokes the same operation with `--plan-digest`;
4. a changed plan, registry, target, or package fails closed.

Enable and disable operations use the A3S Use extension lifecycle and trigger a
registry refresh. The Activity Bar polls the immutable registry revision, so
install, upgrade, disable, and uninstall converge without reloading A3S Web.

## Local Web API

The loopback Web service exposes:

- `GET /api/v1/plugins/activities`
- `GET /api/v1/plugins/activities/{key}`
- `GET /api/v1/plugins/marketplace`
- `POST /api/v1/plugins/operations/plan`
- `POST /api/v1/plugins/operations/apply`
- `POST /api/v1/plugins/packages/enabled`

These routes do not weaken the existing loopback deployment boundary. Browser
clients never receive A3S OS tokens, trust-root keys, package filesystem paths,
or authority to bypass plan review.
