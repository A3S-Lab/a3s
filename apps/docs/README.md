# A3S Site

This application builds the bilingual A3S ecosystem site with Rspress. It
publishes the ecosystem homepage, the cross-platform A3S Desktop download
page, and links to the public sites owned by individual A3S products. It does
not publish product reference documentation, tutorials, articles, or a blog.

The [A3S Flow product site](https://a3s-lab.github.io/Flow/) owns reusable
authoring components, React and Vue integrations, CLI and Skill guidance, and
the node reference. The ecosystem site links to that product surface instead
of shipping another workflow editor or product-specific top navigation.

## Routes

| Route | Content |
| --- | --- |
| `/` | Chinese ecosystem homepage |
| `/en/` | English ecosystem homepage |
| `/download/` | Chinese A3S Desktop downloads |
| `/en/download/` | English A3S Desktop downloads |

The production build runs Rspress once per language and assembles both outputs
under `out/`. Chinese remains the unprefixed default language.

The latest download actions use stable `releases/latest/download` links from
`A3S-Lab/Desktop`. The release-history entries use tag-pinned asset URLs and
checked bilingual notes from `components/download/desktop-release-history.ts`.
Update that file when a Desktop release is published. Asset names are owned by
the Desktop repository release workflow and must change in both repositories
if packaging names change.

## Project layout

```text
apps/docs/
├── components/home/                 # Shared homepage and download-page styles
├── components/download/             # Desktop release links and localized copy
├── public/brand/                    # A3S OS brand assets
├── public/ecosystem-sites/          # Captured project-site previews
├── scripts/                         # Build, validation, and screenshot tasks
├── site/cn/                         # Chinese route entrypoints
├── site/en/                         # English route entrypoints
├── theme/                           # Rspress theme extension
└── rspress.config.ts
```

## Development

```bash
bun install
bun run dev
```

The default development server uses Chinese content. Start the English build
with:

```bash
bun run dev:en
```

Run all local checks with:

```bash
bun run check
```

Individual commands are also available:

```bash
bun run test
bun run typecheck
bun run build
bun run check:site
```

Set `SITE_URL` when validating a deployment under a non-root path, for example:

```bash
SITE_URL=https://a3s-lab.github.io/a3s/ bun run build
```

## Project-site screenshots

Homepage previews use committed 1280 x 800 screenshots from real project sites.
Refresh all previews with:

```bash
bun run capture:sites
```

Capture one project while iterating with:

```bash
bun run capture:sites --site=cloud
```

Each project defines its own animation settle time in
`components/home/project-sites.ts`. The capture waits for fonts and visible
images, lets the hero reach that frame, then freezes CSS, SVG, video, and GIF
motion before writing the PNG.

Projects with language-aware pages can define a capture language and localized
preview overrides. The capture task switches the remote page through its
`data-language-toggle` control and verifies the resulting `data-language`
state, so each homepage locale receives the matching committed screenshot.

The capture task keeps an existing committed image when a remote site is
temporarily unavailable. Add or change destinations in
`components/home/project-sites.ts`.

## Ecosystem status data

The project directory keeps its delivery stages and current versions or
channels in `components/home/ecosystem-status.ts`. Stages describe the current
usage boundary; they are categories, not feature-completion percentages. The
homepage renders the definitions directly instead of converting stages into a
numeric rail. Before changing an entry, check the current project version and
public release together with its README and roadmap, then update the shared
verification date.
