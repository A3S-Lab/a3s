import { readdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const docsRoot = path.join(websiteRoot, 'docs');
const manifestPath = path.join(websiteRoot, 'documentation.json');

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function collectMarkdownFiles(directory, prefix = '') {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const relativePath = path.posix.join(prefix, entry.name);
    if (entry.isDirectory()) {
      files.push(
        ...(await collectMarkdownFiles(
          path.join(directory, entry.name),
          relativePath,
        )),
      );
    } else if (/\.mdx?$/.test(entry.name)) {
      files.push(relativePath);
    }
  }

  return files.sort();
}

const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
const localeIds = manifest.locales.map(({ lang }) => lang);
const versionIds = manifest.versions.map(({ id }) => id);
const routeSegmentPattern = /^[a-z0-9][a-z0-9._-]*$/;

assert(
  localeIds.length >= 2,
  'Documentation must declare at least two locales.',
);
assert(
  localeIds.includes(manifest.defaultLocale),
  'The default locale must be present in the locale list.',
);
assert(
  manifest.defaultLocale === 'zh',
  'Simplified Chinese must remain the default locale.',
);
assert(
  versionIds.length >= 2,
  'Documentation must expose at least two versions.',
);
assert(
  versionIds.includes(manifest.defaultVersion),
  'The default version must be present in the version list.',
);
assert(
  new Set(localeIds).size === localeIds.length,
  'Documentation locale identifiers must be unique.',
);
assert(
  new Set(versionIds).size === versionIds.length,
  'Documentation version identifiers must be unique.',
);
assert(
  localeIds.every((locale) => routeSegmentPattern.test(locale)),
  'Documentation locale identifiers must be valid route segments.',
);
assert(
  versionIds.every((version) => routeSegmentPattern.test(version)),
  'Documentation version identifiers must be valid route segments.',
);
assert(
  manifest.locales.every(
    ({ label, shortLabel, htmlLang }) => label && shortLabel && htmlLang,
  ),
  'Every documentation locale needs full, short, and HTML labels.',
);

const currentVersions = manifest.versions.filter(
  ({ channel }) => channel === 'current',
);
assert(
  currentVersions.length === 1 &&
    currentVersions[0].id === manifest.defaultVersion,
  'The default version must be the only current documentation channel.',
);

const versionDirectories = (await readdir(docsRoot, { withFileTypes: true }))
  .filter((entry) => entry.isDirectory() && entry.name !== 'public')
  .map((entry) => entry.name)
  .sort();
assert(
  JSON.stringify(versionDirectories) === JSON.stringify([...versionIds].sort()),
  'Documentation version directories must match documentation.json.',
);

for (const version of manifest.versions) {
  assert(
    version.labels && localeIds.every((locale) => version.labels[locale]),
    `Documentation version ${version.id} needs a label for every locale.`,
  );
  if (version.channel === 'stable') {
    assert(
      version.sourceRef && version.releasedAt,
      `Stable documentation version ${version.id} needs a source ref and release date.`,
    );
  }

  const localeDirectories = (
    await readdir(path.join(docsRoot, version.id), { withFileTypes: true })
  )
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort();
  assert(
    JSON.stringify(localeDirectories) === JSON.stringify([...localeIds].sort()),
    `Documentation version ${version.id} locale directories must match documentation.json.`,
  );

  const localeFiles = await Promise.all(
    localeIds.map(async (locale) => ({
      locale,
      files: await collectMarkdownFiles(
        path.join(docsRoot, version.id, locale),
      ),
    })),
  );
  const referenceFiles = localeFiles[0].files;
  assert(
    referenceFiles.includes('index.mdx'),
    `Documentation version ${version.id} needs a landing page.`,
  );

  for (const { locale, files } of localeFiles.slice(1)) {
    assert(
      JSON.stringify(files) === JSON.stringify(referenceFiles),
      `Documentation version ${version.id} is missing route parity for ${locale}.`,
    );
  }
}

console.log(
  `Validated ${versionIds.length} documentation versions across ${localeIds.length} locales.`,
);
