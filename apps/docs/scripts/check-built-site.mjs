import { readFile, readdir, stat } from 'node:fs/promises';
import path from 'node:path';

const root = process.cwd();
const output = path.join(root, 'out');
const deploymentPath = process.env.SITE_URL
  ? new URL(process.env.SITE_URL).pathname.replace(/\/+$/, '')
  : '';
const formHref = `${deploymentPath}/form/`.replace(/\/+/g, '/');
const articleSlugs = [
  'programmable-agent-workflows',
  'domain-driven-design',
  'libkrun-libkrunfw-whpx',
  'http-402-generative-ui-agent-economy',
  'why-coding-agent-is-the-core',
  'why-ai-native-gateway',
  'a3s-power-technical-deep-dive',
  'a3s-box-technical-deep-dive',
];
const requiredImages = [
  'brand/a3s-os-logo.png',
  'ecosystem-sites/cloud.png',
  'ecosystem-sites/code.png',
  'ecosystem-sites/office.png',
  'ecosystem-sites/use.png',
  'ecosystem-sites/ui.png',
  'ecosystem-sites/gateway.png',
  'ecosystem-sites/form.png',
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
  'blog/index.html',
  'en/blog/index.html',
  ...articleSlugs.flatMap((slug) => [
    `blog/${slug}.html`,
    `en/blog/${slug}.html`,
  ]),
];

const routeEntries = await Promise.all(
  routeFiles.map(async (relativePath) => [relativePath, await read(relativePath)]),
);
const routeHtml = new Map(routeEntries);
const chineseHome = routeHtml.get('index.html');
const englishHome = routeHtml.get('en/index.html');
const chineseBlog = routeHtml.get('blog/index.html');
const englishBlog = routeHtml.get('en/blog/index.html');

assert(chineseHome, 'Chinese homepage is missing');
assert(englishHome, 'English homepage is missing');
assert(chineseBlog, 'Chinese blog index is missing');
assert(englishBlog, 'English blog index is missing');

for (const marker of [
  '用可检查的组件',
  '构建并运行',
  'Agent。',
  '分布在全球的成员和 Agent 共用一个 Workspace',
  'Shared Workspace',
  '先用 CLI 跑起来',
  '交付阶段',
  '不统计功能完成率',
  'v0.10.14',
]) {
  assert(chineseHome.includes(marker), `Chinese homepage is missing: ${marker}`);
}

for (const marker of [
  'Build and run agents',
  'with parts you can',
  'inspect.',
  'globally distributed humans and agents',
  'Shared Workspace',
  'Start with the CLI.',
  'Delivery stage',
  'not a feature-completion score',
  'v0.10.14',
]) {
  assert(englishHome.includes(marker), `English homepage is missing: ${marker}`);
}

for (const [homepage, locale] of [[chineseHome, 'Chinese'], [englishHome, 'English']]) {
  const projectStages = homepage.match(/class="a3s-project-delivery"/g) ?? [];
  const formLinkCount = homepage.split(`href="${formHref}"`).length - 1;
  assert(projectStages.length === 36, `${locale} homepage has ${projectStages.length} project stages instead of 36`);
  assert(!homepage.includes('a3s-project-progress'), `${locale} homepage still uses progress UI for categorical delivery stages`);
  assert(!homepage.includes('role="progressbar"'), `${locale} homepage still presents delivery stages as completion percentages`);
  assert(homepage.includes('/brand/a3s-os-logo.png'), `${locale} homepage does not use the A3S OS logo`);
  assert(formLinkCount >= 2, `${locale} homepage does not route both A3S Form entries to the published playground`);
  assert(homepage.includes('https://github.com/A3S-Lab/Form'), `${locale} homepage is missing the separate A3S Form repository link`);
  assert(homepage.includes('a3s-lab.github.io/Box'), `${locale} homepage does not feature the A3S Box site`);
  assert(!homepage.includes('ecosystem-sites/blog.png'), `${locale} homepage still features the Site & Blog preview`);
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

assert((chineseBlog.match(/class="a3s-blog-card/g) ?? []).length === 8, 'Chinese blog index does not list 8 posts');
assert((englishBlog.match(/class="a3s-blog-card/g) ?? []).length === 8, 'English blog index does not list 8 posts');

for (const slug of articleSlugs) {
  assert(chineseBlog.includes(`href="./${slug}"`), `Chinese blog link is not Pages-compatible: ${slug}`);
  assert(englishBlog.includes(`href="./${slug}"`), `English blog link is not Pages-compatible: ${slug}`);
}

const files = await collectFiles(output);
const relativeFiles = files.map((file) => path.relative(output, file).split(path.sep).join('/'));

if (process.env.REQUIRE_FORM_PREVIEW === '1') {
  const formIndex = await read('form/index.html');
  const formFiles = relativeFiles.filter((file) => file.startsWith('form/'));

  assert(formIndex.includes('<title>A3S Form · 表单设计器</title>'), 'Published Form playground has an unexpected title');
  assert(formIndex.includes('src="./static/js/'), 'Published Form playground must use relative JavaScript paths');
  assert(formIndex.includes('href="./static/css/'), 'Published Form playground must use relative CSS paths');
  assert(formFiles.some((file) => /^form\/static\/js\/[^/]+\.js$/.test(file)), 'Published Form playground contains no JavaScript bundle');
  assert(formFiles.some((file) => /^form\/static\/css\/[^/]+\.css$/.test(file)), 'Published Form playground contains no CSS bundle');
}

for (const image of requiredImages) {
  assert(relativeFiles.includes(image), `Static build is missing image: ${image}`);
}

assert(
  !relativeFiles.some((file) => /(^|\/)(docs|tutorials)(\/|$)/.test(file)),
  'Static build still contains a docs or tutorials route',
);

const html = routeEntries.map(([, content]) => content).join('\n');
const localHrefs = [...html.matchAll(/href="([^"]+)"/g)]
  .map((match) => match[1])
  .filter((href) => !/^https?:\/\//.test(href));
assert(
  !localHrefs.some((href) => /(^|\/)(docs|tutorials)(\/|$|[?#])/.test(href)),
  'Static routes still link to docs or tutorials',
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
  '.a3s-blog-grid',
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

  for (const slug of articleSlugs) {
    assert(
      routeHtml.get(`blog/${slug}.html`).includes(`href="${canonicalSite}/blog/${slug}"`),
      `Chinese article canonical URL is not Pages-compatible: ${slug}`,
    );
    assert(
      routeHtml.get(`en/blog/${slug}.html`).includes(`href="${canonicalSite}/en/blog/${slug}"`),
      `English article canonical URL is not Pages-compatible: ${slug}`,
    );
  }
}

console.log(`Validated 2 homepages, 2 blog indexes, 16 article routes, and ${files.length} exported files.`);
