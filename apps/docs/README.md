# A3S CLI website and documentation

The public A3S site combines the product page for the root-owned `a3s`
command-line interface with localized, versioned product documentation. It
intentionally contains no tutorial or blog routes.

## Architecture

The site follows the same implementation model as the A3S Code website:

- **Rspress** owns static generation, locale and version routing, metadata, and
  the Pages base path.
- **`documentation.json`** is the single source of truth for supported locales,
  the default locale, published documentation versions, and the rolling
  version.
- **Localized MDX entry files** select the custom home layout without carrying
  product copy.
- **A custom theme** owns navigation, CLI product content, installation
  switching, same-page locale and version switching, and responsive styles.
- **Canvas UI Grid** is vendored as source and provides the pointer-responsive
  canvas treatment around the hero's CLI playback. The playback demonstrates
  representative command outcomes, pauses off-screen, and respects reduced
  motion. The Canvas UI license is recorded in `THIRD_PARTY_NOTICES.md`.
- **Static public assets** provide the favicon, social card, and robots policy.
  The official Rspress sitemap plugin derives the sitemap from generated routes
  so locale and version entries cannot drift.

The default route is Simplified Chinese. English is served from `/en/`.
Language switching keeps the current documentation version and page. Version
switching keeps the current language and page.

## Documentation versions

The rolling documentation lives under `docs/latest/<locale>/`. Because
`latest` is the default version, Rspress omits that segment from public URLs.
For example, the Chinese current documentation is served from `/`, while its
English counterpart is served from `/en/`.

Stable snapshots live under `docs/<version>/<locale>/` and retain their version
segment in public URLs. The first snapshot is `v0.11.1`, served from
`/v0.11.1/` and `/v0.11.1/en/`.

To publish another stable documentation version:

1. Add its metadata to `documentation.json`.
2. Copy the complete `latest` locale trees into `docs/<version>/`.
3. Replace the snapshot landing pages with release-specific descriptions.
4. Keep identical relative MDX paths across all locales.
5. Run the complete verification suite. Stable snapshots receive only accuracy
   and security corrections after publication.

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
npm run check:content
npm run lint
npm run build
npm run check:site
```

`check:content` verifies the manifest and route parity for every
version/locale pair. `check:site` verifies generated routes, cross-version and
cross-language links, sitemap coverage, assets, and internal references.
