# A3S Work Live Preview

## Product contract

Live Preview is a persistent Work surface for inspecting artifacts while files,
code, and AI activity remain available. It replaces the blocking Quick Look
dialog with a resizable panel mounted beside the active Work scene.

Users can open the panel from:

- Space, the preview toolbar action, or an item context menu in local files.
- The Preview action in the Work code editor.
- A `?preview=<target>#home` deep link created by the A3S Code TUI.
- The panel address field, using an active-workspace path or loopback URL.

Closing the panel removes the preview target from the current URL. The last
desktop panel width is retained locally; narrow layouts use an overlay so the
underlying Work scene does not become unusable.

## Supported targets

| Target | Behavior |
| --- | --- |
| Directory with `index.html` | Serves a static site from that directory. |
| HTML file | Serves the file with its parent directory as the site root. |
| `localhost`, `*.localhost`, or loopback HTTP(S) URL | Embeds the running local application without proxying it. |
| PDF or common raster image | Reuses the read-only Work previewer. |
| DOCX, XLSX/XLS/ODS/CSV, or PPTX | Reuses the in-memory Office preview path and never autosaves a conversion. |
| Supported text or source file | Shows a bounded read-only text preview. |

Static sites and local applications provide desktop, tablet, mobile, and zoom
controls. Document controls remain owned by their format-specific previewer.

## Reload and lifecycle

Creating a preview returns a short-lived descriptor with an opaque session ID,
capabilities, source metadata, content URL, and optional watch root. Workspace
change events are filtered to the selected file or static-site root, debounced,
and then reload the mounted preview. Users can pause automatic reload without
closing the session, refresh manually, change the target inline, or open the
content URL in a separate window.

The browser stops the previous preview session when its descriptor is replaced
or the panel unmounts. The service bounds the registry to 32 sessions and
prunes expired sessions as new previews are registered.

The A3S Code TUI reuses a compatible managed Web instance for the workspace or
starts one on an ephemeral loopback port. It opens the deep link in a native
`a3s-webview` window when available and falls back to the system browser. The
TUI tracks only the native preview window: replacement, `/preview stop`, and
TUI exit close that exact child without stopping the shared Web service. A
browser fallback cannot be closed safely by the host and remains user-owned.

## Security boundary

- Filesystem targets are canonicalized and must remain inside the active
  workspace. Directory entries and requested assets are canonicalized again so
  traversal and symbolic-link escapes fail closed.
- Hidden path segments and common credential or private-key names are not
  served. Static assets are capped at 64 MiB; the shared previewers apply their
  stricter 2 MiB text and 50 MiB binary limits.
- Static HTML is returned with no-store, no-referrer, no-sniff, restrictive
  permissions, and CSP sandbox headers. The iframe omits `allow-same-origin`,
  giving workspace HTML an opaque origin without access to the A3S host page.
- URL previews accept only HTTP(S) localhost, `*.localhost`, or numeric
  loopback hosts. Credentials in a preview URL are rejected, and fragments are
  removed before registration.
- The native preview window receives no A3S OS access or refresh token. The
  random preview content URL grants access only to the registered bounded
  artifact for the session lifetime.

Live Preview is a local development surface, not a remote publishing server.
The Web listener remains loopback-first and must not be exposed directly to an
untrusted network.

## Acceptance checks

- Static HTML loads relative CSS and JavaScript and reloads after a relevant
  file change without a manual refresh.
- Local Vite-style URLs retain application navigation and HMR behavior.
- Text, image, PDF, DOCX, XLSX, and PPTX targets render through the shared panel.
- Traversal, hidden-file access, sensitive files, escaping symbolic links,
  oversized assets, public URLs, and paths outside the workspace are rejected.
- The TUI `/preview`, `/preview status`, `/preview stop`, IDE `p`, and editor
  `:preview` flows use the same deep link; `:preview` saves a dirty buffer first.
- Preview stop and TUI exit close the tracked native window while leaving a
  reused Web process running.
