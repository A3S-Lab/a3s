import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const output = path.join(root, 'out');
const flowHref = 'https://a3s-lab.github.io/Flow/';
const powerHref = 'https://a3s-lab.github.io/Power/';
const requiredImages = [
  'brand/a3s-os-logo.png',
  'ecosystem-sites/cloud.png',
  'ecosystem-sites/code.png',
  'ecosystem-sites/flow.png',
  'ecosystem-sites/flow-en.png',
  'ecosystem-sites/office.png',
  'ecosystem-sites/use.png',
  'ecosystem-sites/ui.png',
  'ecosystem-sites/gateway.png',
  'ecosystem-sites/gateway-en.png',
  'ecosystem-sites/power.png',
  'ecosystem-sites/box.png',
];

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
    if ((await stat(target)).isDirectory()) files.push(...await collectFiles(target));
    else files.push(target);
  }

  return files;
}

const routeFiles = [
  'index.html',
  'en/index.html',
  'download/index.html',
  'en/download/index.html',
];

const routeEntries = await Promise.all(
  routeFiles.map(async (relativePath) => [relativePath, await read(relativePath)]),
);
const routeHtml = new Map(routeEntries);
const chineseHome = routeHtml.get('index.html');
const englishHome = routeHtml.get('en/index.html');
const chineseDownload = routeHtml.get('download/index.html');
const englishDownload = routeHtml.get('en/download/index.html');

assert(chineseHome, 'Chinese homepage is missing');
assert(englishHome, 'English homepage is missing');
assert(chineseDownload, 'Chinese Desktop download page is missing');
assert(englishDownload, 'English Desktop download page is missing');
assert(
  chineseHome.includes('/ecosystem-sites/gateway.png'),
  'Chinese homepage does not use the Chinese Gateway preview',
);
assert(
  englishHome.includes('/ecosystem-sites/gateway-en.png'),
  'English homepage does not use the English Gateway preview',
);
assert(
  chineseHome.includes('/ecosystem-sites/flow.png'),
  'Chinese homepage does not use the Chinese Flow preview',
);
assert(
  englishHome.includes('/ecosystem-sites/flow-en.png'),
  'English homepage does not use the English Flow preview',
);

const releaseBase = 'https://github.com/A3S-Lab/Desktop/releases/latest/download';
for (const [downloadPage, locale] of [[chineseDownload, 'Chinese'], [englishDownload, 'English']]) {
  for (const asset of ['A3S-macos.zip', 'A3S-windows.zip', 'A3S-linux.tar.gz', 'SHA256SUMS.txt']) {
    assert(
      downloadPage.includes(`${releaseBase}/${asset}`),
      `${locale} download page is missing ${asset}`,
    );
  }
  assert(
    downloadPage.includes('https://github.com/A3S-Lab/Desktop/releases'),
    `${locale} download page is missing release history`,
  );
  assert(
    downloadPage.includes('https://github.com/A3S-Lab/Desktop'),
    `${locale} download page is missing source`,
  );
}

for (const marker of [
  '为AI Native组织构建的AI操作系统生态',
  '分布在全球的成员和 Agent 共用一个 Workspace',
  'Shared Workspace',
  '每个AI Native组织都需要构建专属的AI操作系统',
  '交付阶段',
  '不统计功能完成率',
  '34 个职责明确的项目',
  'v0.12.4',
]) {
  assert(chineseHome.includes(marker), `Chinese homepage is missing: ${marker}`);
}

for (const marker of [
  'An AI operating system ecosystem for AI Native organizations.',
  'globally distributed humans and agents',
  'Shared Workspace',
  'Every AI Native organization needs to build its own AI operating system.',
  'Delivery stage',
  'not a feature-completion score',
  '34 focused projects',
  'v0.12.4',
]) {
  assert(englishHome.includes(marker), `English homepage is missing: ${marker}`);
}

for (const [homepage, locale] of [[chineseHome, 'Chinese'], [englishHome, 'English']]) {
  const projectStages = homepage.match(/class="a3s-project-delivery"/g) ?? [];
  const flowLinkCount = homepage.split(`href="${flowHref}"`).length - 1;
  const powerLinkCount = homepage.split(`href="${powerHref}"`).length - 1;
  assert(projectStages.length === 34, `${locale} homepage has ${projectStages.length} project stages instead of 34`);
  assert(!homepage.includes('a3s-project-progress'), `${locale} homepage still uses progress UI for categorical delivery stages`);
  assert(!homepage.includes('role="progressbar"'), `${locale} homepage still presents delivery stages as completion percentages`);
  assert(homepage.includes('/brand/a3s-os-logo.png'), `${locale} homepage does not use the A3S OS logo`);
  assert(flowLinkCount >= 3, `${locale} homepage does not route A3S Flow entries to its product site`);
  assert(powerLinkCount >= 2, `${locale} homepage does not route both A3S Power entries to its website`);
  assert(!homepage.includes('/form/'), `${locale} homepage still links to the removed Form site`);
  assert(!homepage.includes('A3S Form'), `${locale} homepage still includes the removed Form project`);
  assert(homepage.includes('a3s-lab.github.io/Box'), `${locale} homepage does not feature the A3S Box site`);
  assert(!homepage.includes('id="architecture"'), `${locale} homepage still renders the architecture diagram`);
  assert(!homepage.includes('href="#architecture"'), `${locale} homepage still links to the architecture diagram`);
  assert(homepage.includes('a3s-global-workspace'), `${locale} homepage is missing the global workspace scene`);
  assert(
    homepage.includes('Human') && homepage.includes('Agent'),
    `${locale} homepage does not identify Human and Agent collaborators`,
  );
  assert(homepage.includes('Edge + Cloud'), `${locale} homepage does not identify the edge-cloud layer`);
  assert(!homepage.includes('a3s-cli-terminal'), `${locale} homepage still renders the CLI hero terminal`);

  for (const installer of [
    'https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh',
    'https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1',
    'brew install a3s-lab/tap/a3s',
  ]) {
    assert(homepage.includes(installer), `${locale} homepage is missing the installer: ${installer}`);
  }
}

const files = await collectFiles(output);
const relativeFiles = files.map((file) => path.relative(output, file).split(path.sep).join('/'));

assert(
  !relativeFiles.some((file) => /(^|\/)blog(\/|$)/.test(file)),
  'Static build still contains a blog route',
);

assert(
  !relativeFiles.some((file) => file.startsWith('form/')),
  'Static build still contains the removed Form site',
);

assert(
  !relativeFiles.some((file) => /(^|\/)playground(\/|$)/.test(file)),
  'Static build still contains a standalone Playground route',
);

for (const image of requiredImages) {
  assert(relativeFiles.includes(image), `Static build is missing image: ${image}`);
}

assert(
  !relativeFiles.some((file) => /(^|\/)(docs|tutorials)(\/|$)/.test(file)),
  'Static build still contains a docs or tutorials route',
);

const html = routeEntries.map(([, content]) => content).join('\n');
const disallowedReferenceName = String.fromCharCode(100, 105, 102, 121);
assert(!html.toLowerCase().includes(disallowedReferenceName), 'Public routes contain a disallowed reference-product name');
const localHrefs = [...html.matchAll(/href="([^"]+)"/g)]
  .map((match) => match[1])
  .filter((href) => !/^https?:\/\//.test(href));
assert(
  !localHrefs.some((href) => /(^|\/)(docs|tutorials)(\/|$|[?#])/.test(href)),
  'Static routes still link to docs or tutorials',
);
assert(
  !localHrefs.some((href) => /(^|\/)blog(\/|$|[?#])/.test(href)),
  'Static routes still link to the removed blog',
);
assert(!/fumadocs/i.test(html), 'Static routes still contain a Fumadocs reference');

const cssFiles = files.filter((file) => file.endsWith('.css'));
assert(cssFiles.length > 0, 'Static build contains no CSS assets');
const css = (await Promise.all(cssFiles.map((file) => readFile(file, 'utf8')))).join('\n');
for (const selector of [
  '.a3s-home-nav',
  '.a3s-global-workspace',
  '.a3s-site-preview__image',
  '.a3s-directory-card',
  '.a3s-delivery-guide',
  '.a3s-project-delivery',
]) {
  assert(css.includes(selector), `Production CSS is missing: ${selector}`);
}
assert(!css.includes('.a3s-cli-terminal'), 'Production CSS still contains the replaced CLI hero terminal');
assert(!css.includes('.a3s-canvas-backdrop'), 'Production CSS still contains the replaced canvas backdrop');
assert(!css.includes('.a3s-atlas'), 'Production CSS still contains architecture diagram styles');
assert(!css.includes('.a3s-ecosystem-visual'), 'Production CSS still contains the replaced ecosystem hero visual');
assert(!css.includes('.a3s-project-progress'), 'Production CSS still contains the misleading project progress component');
assert(!css.includes('--project-progress'), 'Production CSS still contains numeric project progress scaling');

if (process.env.SITE_URL) {
  const canonicalSite = process.env.SITE_URL.replace(/\/$/, '');
  assert(chineseHome.includes(`href="${canonicalSite}/"`), 'Chinese canonical URL is incorrect');
  assert(englishHome.includes(`href="${canonicalSite}/en/"`), 'English canonical URL is incorrect');
}

console.log(`Validated 2 homepages, 2 Desktop download pages, and ${files.length} exported files.`);
