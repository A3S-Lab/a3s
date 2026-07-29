import { access, readdir, readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const outputRoot = path.join(websiteRoot, 'doc_build');
const base = process.env.SITE_BASE ?? '/a3s/';

const requiredFiles = [
  'index.html',
  'en/index.html',
  '404.html',
  'llms.txt',
  'en/llms.txt',
  'a3s-mark.svg',
  'favicon.svg',
  'social-card.svg',
  'robots.txt',
  'sitemap.xml',
];

async function collectFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(absolutePath)));
    } else {
      files.push(absolutePath);
    }
  }

  return files;
}

async function resolvesToBuiltFile(relativeReference) {
  const decodedReference = decodeURIComponent(relativeReference);
  const candidates =
    decodedReference === '' || decodedReference.endsWith('/')
      ? [path.join(decodedReference, 'index.html')]
      : [
          decodedReference,
          decodedReference + '.html',
          path.join(decodedReference, 'index.html'),
        ];

  for (const candidate of candidates) {
    const outputPath = path.resolve(outputRoot, candidate);
    if (
      outputPath !== outputRoot &&
      !outputPath.startsWith(outputRoot + path.sep)
    ) {
      continue;
    }

    try {
      if ((await stat(outputPath)).isFile()) return true;
    } catch {
      // Try the next supported output form.
    }
  }

  return false;
}

for (const file of requiredFiles) {
  await access(path.join(outputRoot, file));
}

const [chinese, english, sitemap] = await Promise.all([
  readFile(path.join(outputRoot, 'index.html'), 'utf8'),
  readFile(path.join(outputRoot, 'en/index.html'), 'utf8'),
  readFile(path.join(outputRoot, 'sitemap.xml'), 'utf8'),
]);

for (const marker of [
  '用一个 CLI 安装和运行 A3S',
  'a3s code',
  'a3s doctor',
  'A3S CLI',
]) {
  if (!chinese.includes(marker)) {
    throw new Error('Chinese homepage is missing: ' + marker);
  }
}

for (const marker of [
  'Install and run A3S from one CLI',
  'One entry point, independent products',
  'a3s upgrade --all --yes',
]) {
  if (!english.includes(marker)) {
    throw new Error('English homepage is missing: ' + marker);
  }
}

for (const forbidden of ['/docs/', '/blog/', '/tutorials/']) {
  if (chinese.includes(forbidden) || english.includes(forbidden)) {
    throw new Error('Homepage contains removed route: ' + forbidden);
  }
  if (sitemap.includes(forbidden)) {
    throw new Error('Sitemap contains removed route: ' + forbidden);
  }
}

if (!chinese.includes('https://a3s-lab.github.io/a3s/')) {
  throw new Error('Chinese homepage has no canonical production URL.');
}
if (!english.includes('https://a3s-lab.github.io/a3s/en/')) {
  throw new Error('English homepage has no canonical production URL.');
}

const files = await collectFiles(outputRoot);
const cssFiles = files.filter((file) => file.endsWith('.css'));
const css = (
  await Promise.all(cssFiles.map((file) => readFile(file, 'utf8')))
).join('\n');

for (const selector of [
  '.a3s-cli-nav',
  '.a3s-cli-home',
  '.cli-command-row',
  '.cli-product-grid',
]) {
  if (!css.includes(selector)) {
    throw new Error('Production CSS is missing: ' + selector);
  }
}

const htmlFiles = files.filter((file) => file.endsWith('.html'));
const referencePattern = /(?:href|src)="([^"]+)"/g;
const brokenReferences = [];

for (const htmlFile of htmlFiles) {
  const html = await readFile(htmlFile, 'utf8');

  for (const [, rawReference] of html.matchAll(referencePattern)) {
    if (
      rawReference.startsWith('#') ||
      rawReference.startsWith('data:') ||
      rawReference.startsWith('mailto:') ||
      /^[a-z]+:\/\//i.test(rawReference)
    ) {
      continue;
    }

    if (rawReference.startsWith('/') && !rawReference.startsWith(base)) {
      brokenReferences.push(
        path.relative(outputRoot, htmlFile) +
          ' -> ' +
          rawReference +
          ' (outside ' +
          base +
          ')',
      );
      continue;
    }

    if (!rawReference.startsWith(base)) continue;

    const withoutBase = rawReference
      .slice(base.length)
      .split(/[?#]/, 1)[0]
      .replace(/\/+/g, '/');
    if (!(await resolvesToBuiltFile(withoutBase))) {
      brokenReferences.push(
        path.relative(outputRoot, htmlFile) + ' -> ' + rawReference,
      );
    }
  }
}

if (brokenReferences.length > 0) {
  throw new Error(
    'Built-site reference check failed:\n' +
      brokenReferences.map((reference) => '  - ' + reference).join('\n'),
  );
}

console.log(
  'Validated the bilingual CLI site across ' +
    htmlFiles.length +
    ' HTML pages.',
);
