# A3S Site

This application builds the bilingual A3S ecosystem homepage and publishes the
pinned A3S Power website in the same Rspress-based Pages artifact. Other
product documentation, tutorials, and articles remain with their owners.

## Routes

| Route | Content |
| --- | --- |
| `/` | Chinese ecosystem homepage |
| `/en/` | English ecosystem homepage |
| `/power/` | Chinese A3S Power website and current documentation |
| `/power/en/` | English A3S Power website and current documentation |
| `/power/v0.9.0/` | Versioned A3S Power documentation |

The production build runs the ecosystem Rspress app once per language, builds
the exact Power site locked by the `crates/power` gitlink, and assembles every
route under `out/`. Chinese remains the unprefixed default language.

## Project layout

```text
apps/docs/
├── components/home/          # Homepage UI and ecosystem data
├── public/brand/             # A3S OS brand assets
├── public/ecosystem-sites/   # Captured project-site previews
├── scripts/                  # Build, validation, and screenshot tasks
├── site/cn/                  # Chinese homepage entrypoint
├── site/en/                  # English homepage entrypoint
├── theme/                    # Rspress theme extension
└── rspress.config.ts
```

Power pages stay owned by `crates/power/site`; the main site consumes that
locked source directly instead of copying its Markdown or theme.

## Development

```bash
git -C ../.. submodule update --init crates/power
bun install
npm ci --prefix ../../crates/power/site
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
bun run typecheck:power
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
