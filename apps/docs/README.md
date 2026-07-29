# A3S CLI website

The public A3S site is a bilingual product page for the root-owned `a3s`
command-line interface. It intentionally contains no documentation, tutorial,
or blog routes.

## Architecture

The site follows the same implementation model as the A3S Code website:

- **Rspress** owns static generation, locales, metadata, and the Pages base
  path.
- **Localized MDX entry files** select the custom home layout without carrying
  product copy.
- **A custom theme** owns navigation, CLI product content, installation
  switching, and responsive styles.
- **Canvas UI Grid** is vendored as source and provides the pointer-responsive
  canvas treatment in the hero command panel. Its license is recorded in
  `THIRD_PARTY_NOTICES.md`.
- **Static public assets** provide the favicon, social card, robots policy, and
  sitemap.

The default route is Simplified Chinese. English is served from `/en/`.

## Local development

```bash
npm ci
npm run dev
```

The production site is served from `/a3s/`. Override `SITE_BASE` and
`SITE_ORIGIN` only for another deployment target.

## Verification

```bash
npm run format:check
npm run lint
npm run build
npm run check:site
```
