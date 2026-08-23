# A3S Site

This application builds the bilingual A3S ecosystem homepage and an
interactive Workflow Designer Playground with Rspress. It does not publish
product reference documentation, tutorials, or articles.

## Routes

| Route | Content |
| --- | --- |
| `/` | Chinese ecosystem homepage |
| `/en/` | English ecosystem homepage |
| `/playground/workflow-designer/` | Chinese Workflow Designer Playground |
| `/en/playground/workflow-designer/` | English Workflow Designer Playground |

The production build runs Rspress once per language and assembles both outputs
under `out/`. Chinese remains the unprefixed default language.

## Project layout

```text
apps/docs/
├── components/home/                 # Homepage UI and ecosystem data
├── components/workflow-playground/  # Interactive workflow editor
├── public/brand/                    # A3S OS brand assets
├── public/ecosystem-sites/          # Captured project-site previews
├── scripts/                         # Build, validation, and screenshot tasks
├── site/cn/                         # Chinese homepage entrypoint
├── site/en/                         # English homepage entrypoint
├── tests/e2e/                       # Deterministic browser regression suites
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

With the development server running on port `4173`, validate or run the
deterministic browser flow with A3S Test:

```bash
a3s-test check tests/e2e/workflow-playground.acl --json
a3s-test run tests/e2e/workflow-playground.acl --json
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
