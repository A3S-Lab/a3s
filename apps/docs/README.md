# A3S Site and Blog

This application builds the A3S ecosystem homepage and bilingual engineering
blog with Rspress. It does not publish product documentation or tutorials.

## Routes

| Route | Content |
| --- | --- |
| `/` | Chinese ecosystem homepage |
| `/blog/` | Chinese engineering blog |
| `/en/` | English ecosystem homepage |
| `/en/blog/` | English engineering blog |

The production build runs Rspress once per language and assembles both outputs
under `out/`. Chinese remains the unprefixed default language.

## Project layout

```text
apps/docs/
├── components/home/          # Homepage UI and ecosystem data
├── public/brand/             # A3S OS brand assets
├── public/ecosystem-sites/   # Captured project-site previews
├── scripts/                  # Build, validation, and screenshot tasks
├── site/cn/                  # Chinese homepage and blog MDX
├── site/en/                  # English homepage and blog MDX
├── theme/                    # Rspress theme extension and blog styles
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

## Writing blog posts

Every post has one Chinese and one English MDX file with the same slug:

```text
site/cn/blog/<slug>.mdx
site/en/blog/<slug>.mdx
```

Keep `title`, `description`, `date`, `author`, and `tags` in frontmatter. Add the
post to both blog index pages and to the locale article lists in
`rspress.config.ts`.

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

The Form playground is a build preview rather than a live project site. Point
the capture at a local or preview deployment when refreshing it:

```bash
A3S_FORM_PREVIEW_URL=http://127.0.0.1:4173/playground/ \
  bun run capture:sites --site=form
```

The Pages workflow checks out a pinned Form revision, builds the playground,
and serves it only for the screenshot step. Update `FORM_REVISION` in
`.github/workflows/site.yml` when intentionally refreshing the preview against
a newer Form commit.

The capture task keeps an existing committed image when a remote site is
temporarily unavailable. Add or change destinations in
`components/home/project-sites.ts`.
