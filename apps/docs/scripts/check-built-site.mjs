import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const output = path.join(root, 'out');

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function read(relativePath) {
  return readFile(path.join(output, relativePath), 'utf8');
}

async function collectFiles(directory) {
  const entries = await readdir(directory);
  const files = [];

  for (const entry of entries) {
    const target = path.join(directory, entry);
    if ((await stat(target)).isDirectory()) {
      files.push(...(await collectFiles(target)));
    } else {
      files.push(target);
    }
  }

  return files;
}

const [
  chinese,
  english,
  legacyChinese,
  chineseDocs,
  englishDocs,
  chineseVersion,
  englishVersion,
] = await Promise.all([
  read('index.html'),
  read('en.html'),
  read('cn.html'),
  read('docs.html'),
  read('en/docs.html'),
  read('docs/cloud/v0.1.0.html'),
  read('en/docs/cloud/v0.1.0.html'),
]);

if (process.env.NEXT_PUBLIC_SITE_URL) {
  const sitemap = await read('sitemap.xml');
  assert(
    sitemap.includes(`<loc>${process.env.NEXT_PUBLIC_SITE_URL}</loc>`),
    `Sitemap is missing the public site URL: ${process.env.NEXT_PUBLIC_SITE_URL}`,
  );
}

for (const marker of [
  'Run Code locally.',
  'id="products"',
  'id="architecture"',
  'id="principles"',
  'shared runtime',
  'a3s code',
]) {
  assert(english.includes(marker), `English homepage is missing: ${marker}`);
}

for (const marker of [
  '先在本地运行。',
  '浏览器、文档、评测与云节点。',
  '逐个查看每个项目到底由什么组成。',
]) {
  assert(chinese.includes(marker), `Chinese homepage is missing: ${marker}`);
  assert(
    legacyChinese.includes(marker),
    `Legacy Chinese homepage is missing: ${marker}`,
  );
}

assert(
  chineseDocs.includes('A3S 文档'),
  'The unprefixed documentation is not Chinese',
);
assert(
  englishDocs.includes('A3S Docs'),
  'The /en documentation is not English',
);
assert(
  chineseDocs.includes('选择语言'),
  'Chinese documentation has no language switch',
);
assert(
  englishDocs.includes('Choose a language'),
  'English documentation has no language switch',
);
assert(
  chineseVersion.includes('文档版本'),
  'Chinese versioned docs have no version switch',
);
assert(
  englishVersion.includes('Documentation version'),
  'English versioned docs have no version switch',
);
assert(
  chineseVersion.includes('v0.1.0'),
  'Chinese version selector is missing v0.1.0',
);
assert(
  englishVersion.includes('v0.1.0'),
  'English version selector is missing v0.1.0',
);
assert(
  chineseVersion.includes('/docs/cloud/v0.1.0/recovery'),
  'Chinese snapshot links escape the selected version',
);
assert(
  englishVersion.includes('/en/docs/cloud/v0.1.0/recovery'),
  'English snapshot links escape the selected version',
);
assert(
  !chineseVersion.includes('href="/docs/cloud/recovery"'),
  'Chinese snapshot contains an unversioned recovery link',
);

const canonicalSite = (
  process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev'
).replace(/\/$/, '');
for (const [name, homepage] of [
  ['Chinese', chinese],
  ['English', english],
  ['legacy Chinese', legacyChinese],
]) {
  assert(
    homepage.includes(`${canonicalSite}/opengraph-image`),
    `${name} homepage is missing the social preview image`,
  );
  assert(
    homepage.includes('summary_large_image'),
    `${name} homepage is missing the large Twitter card`,
  );
}

assert(
  chineseDocs.includes(`href="${canonicalSite}/docs"`),
  'Chinese documentation canonical URL is not unprefixed',
);
assert(
  englishDocs.includes(`href="${canonicalSite}/en/docs"`),
  'English documentation canonical URL is missing the /en prefix',
);

assert(
  !english.includes('/og.png'),
  'Homepage still references the removed /og.png asset',
);

const files = await collectFiles(output);
const cssFiles = files.filter((file) => file.endsWith('.css'));
assert(cssFiles.length > 0, 'Static export contains no CSS assets');

const css = (
  await Promise.all(cssFiles.map((file) => readFile(file, 'utf8')))
).join('\n');
for (const selector of [
  '.a3s-home-nav',
  '.a3s-system-orbit',
  '.a3s-product-card',
  '.a3s-atlas__node',
  '.a3s-atlas__stage-scroll',
]) {
  assert(css.includes(selector), `Production CSS is missing: ${selector}`);
}

for (const project of [
  'A3S',
  'Code',
  'Desktop',
  'Office',
  'Parser',
  'Test',
  'Cloud',
  'Runtime',
  'Observer',
  'Homebrew Tap',
]) {
  assert(
    chinese.includes(project),
    `Chinese homepage architecture atlas is missing: ${project}`,
  );
  assert(
    english.includes(project),
    `English homepage architecture atlas is missing: ${project}`,
  );
}

assert(
  files.some((file) => path.basename(file) === 'opengraph-image'),
  'Static export is missing the generated Open Graph image',
);

console.log(
  `Validated bilingual A3S homepage across ${files.length} exported files.`,
);
