import { access, readdir, readFile, stat } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
);
const outputRoot = path.join(websiteRoot, 'doc_build');
const base = process.env.SITE_BASE ?? '/a3s/';
const documentation = JSON.parse(
  await readFile(path.join(websiteRoot, 'documentation.json'), 'utf8'),
);
const stableVersion = documentation.versions.find(
  ({ channel }) => channel === 'stable',
);

if (!stableVersion) {
  throw new Error('At least one stable documentation version is required.');
}

function routePath(version, locale, page = '') {
  const segments = [];
  if (version !== documentation.defaultVersion) segments.push(version);
  if (locale !== documentation.defaultLocale) segments.push(locale);
  if (page) segments.push(page);
  return segments.join('/');
}

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

for (const version of documentation.versions) {
  for (const locale of documentation.locales) {
    const prefix = routePath(version.id, locale.lang);
    if (prefix) requiredFiles.push(`${prefix}/index.html`);
    requiredFiles.push(prefix ? `${prefix}/llms.txt` : 'llms.txt');
  }
}

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

async function readBuiltRoute(relativeReference) {
  const candidates = [
    relativeReference + '.html',
    path.join(relativeReference, 'index.html'),
  ];

  for (const candidate of candidates) {
    const outputPath = path.resolve(outputRoot, candidate);
    try {
      if ((await stat(outputPath)).isFile()) {
        return readFile(outputPath, 'utf8');
      }
    } catch {
      // Try the other supported output form.
    }
  }

  throw new Error('Missing built route: ' + relativeReference);
}

for (const file of requiredFiles) {
  await access(path.join(outputRoot, file));
}

const requiredRoutes = documentation.versions.flatMap((version) =>
  documentation.locales.flatMap((locale) =>
    ['code/capabilities', 'code/code-intelligence'].map((page) =>
      routePath(version.id, locale.lang, page),
    ),
  ),
);

for (const route of requiredRoutes) {
  if (!(await resolvesToBuiltFile(route))) {
    throw new Error('Missing built route: ' + route);
  }
}

const [
  chinese,
  english,
  sitemap,
  chineseCapabilities,
  chineseIntelligence,
  englishCapabilities,
  englishIntelligence,
  stableChinese,
  stableEnglish,
  stableChineseCapabilities,
  stableEnglishCapabilities,
] = await Promise.all([
  readFile(path.join(outputRoot, 'index.html'), 'utf8'),
  readFile(path.join(outputRoot, 'en/index.html'), 'utf8'),
  readFile(path.join(outputRoot, 'sitemap.xml'), 'utf8'),
  readBuiltRoute('code/capabilities'),
  readBuiltRoute('code/code-intelligence'),
  readBuiltRoute('en/code/capabilities'),
  readBuiltRoute('en/code/code-intelligence'),
  readBuiltRoute(stableVersion.id),
  readBuiltRoute(`${stableVersion.id}/en`),
  readBuiltRoute(`${stableVersion.id}/code/capabilities`),
  readBuiltRoute(`${stableVersion.id}/en/code/capabilities`),
]);

for (const [locale, html] of [
  ['Chinese', chinese],
  ['English', english],
]) {
  const openingScripts = html.match(/<script(?:\s|>)/g)?.length ?? 0;
  const closingScripts = html.match(/<\/script>/g)?.length ?? 0;
  if (openingScripts !== closingScripts) {
    throw new Error(
      `${locale} homepage has ${openingScripts} script openings but ${closingScripts} closings.`,
    );
  }

  if (!html.includes('data-canvas-ui="grid"')) {
    throw new Error(`${locale} homepage is missing the Canvas UI Grid.`);
  }

  if (!html.includes('data-cli-terminal-showcase="true"')) {
    throw new Error(`${locale} homepage is missing the CLI terminal playback.`);
  }

  for (const [scenario, command] of [
    ['code', 'a3s code'],
    ['web', 'a3s web'],
    ['box', 'a3s box ps'],
    ['use', 'a3s use capabilities --json'],
    ['doctor', 'a3s doctor'],
    ['upgrade', 'a3s upgrade'],
  ]) {
    const marker = `data-command="${command}" data-terminal-scenario="${scenario}"`;
    if (!html.includes(marker)) {
      throw new Error(
        `${locale} homepage is missing the ${scenario} terminal scenario.`,
      );
    }
  }

  if (!html.includes('content="#090a0d"')) {
    throw new Error(`${locale} homepage is missing the dark theme color.`);
  }
}

for (const marker of [
  '用一个 CLI 安装和运行 A3S',
  'a3s code',
  'a3s doctor',
  'A3S CLI',
  'A3S CLI 命令执行演示',
  'Code 终端工作区已准备就绪',
  'Code、Web 和 Research 随 CLI 提供',
  'Work 是 #home 默认工作台',
  '编码 Agent 的能力和边界',
  '按语义理解代码',
  '/ctx 为什么 auth refresh 会失败',
]) {
  if (!chinese.includes(marker)) {
    throw new Error('Chinese homepage is missing: ' + marker);
  }
}

for (const [name, html, marker] of [
  ['Chinese stable landing page', stableChinese, '文档快照'],
  ['English stable landing page', stableEnglish, 'documentation snapshot'],
]) {
  if (!html.includes(marker)) {
    throw new Error(`${name} is missing: ${marker}`);
  }
}

for (const [name, html, lang] of [
  ['Chinese homepage', chinese, 'zh'],
  ['English homepage', english, 'en'],
  ['Chinese stable landing page', stableChinese, 'zh'],
  ['English stable landing page', stableEnglish, 'en'],
]) {
  if (!html.includes(`<html lang="${lang}">`)) {
    throw new Error(`${name} has the wrong HTML language.`);
  }
}

for (const marker of [
  'Install and run A3S from one CLI',
  'One entry point, independent products',
  'A3S CLI command playback',
  'The Code terminal workspace is ready',
  'a3s upgrade --all --yes',
  'Code, Web, and Research ship with the CLI',
  'Work is the default #home workbench',
  'Release pending',
  'A coding agent should expose its operations',
  'Ask the codebase by meaning',
  '/ctx why did auth refresh fail',
]) {
  if (!english.includes(marker)) {
    throw new Error('English homepage is missing: ' + marker);
  }
}

for (const [name, html, markers] of [
  [
    'Chinese capability page',
    chineseCapabilities,
    [
      '渐进式工具 API',
      'runtime',
      '/ctx save &lt;n&gt;',
      '6000 bytes',
      'id="progressive-tool-api"',
      'id="runtime-tool"',
      'id="cross-session-context"',
    ],
  ],
  [
    'English capability page',
    englishCapabilities,
    [
      'Progressive tool API',
      'runtime',
      '/ctx save &lt;n&gt;',
      '6000 bytes',
      'id="progressive-tool-api"',
      'id="runtime-tool"',
      'id="cross-session-context"',
    ],
  ],
  [
    'Chinese Code Intelligence page',
    chineseIntelligence,
    ['code_symbols', 'code_navigation', 'UTF-16', ':implementations'],
  ],
  [
    'English Code Intelligence page',
    englishIntelligence,
    ['code_symbols', 'code_navigation', 'UTF-16', ':implementations'],
  ],
]) {
  for (const marker of markers) {
    if (!html.includes(marker)) {
      throw new Error(`${name} is missing: ${marker}`);
    }
  }
}

for (const [name, html, markers] of [
  [
    'Latest Chinese capability navigation',
    chineseCapabilities,
    [
      'aria-label="切换文档版本"',
      'aria-label="切换语言"',
      'href="/a3s/en/code/capabilities"',
      `href="/a3s/${stableVersion.id}/code/capabilities"`,
    ],
  ],
  [
    'Latest English capability navigation',
    englishCapabilities,
    [
      'aria-label="Switch documentation version"',
      'aria-label="Switch language"',
      'href="/a3s/code/capabilities"',
      `href="/a3s/${stableVersion.id}/en/code/capabilities"`,
    ],
  ],
  [
    'Stable Chinese capability navigation',
    stableChineseCapabilities,
    [
      'href="/a3s/code/capabilities"',
      `href="/a3s/${stableVersion.id}/en/code/capabilities"`,
    ],
  ],
  [
    'Stable English capability navigation',
    stableEnglishCapabilities,
    [
      'href="/a3s/en/code/capabilities"',
      `href="/a3s/${stableVersion.id}/code/capabilities"`,
    ],
  ],
]) {
  for (const marker of markers) {
    if (!html.includes(marker)) {
      throw new Error(`${name} is missing: ${marker}`);
    }
  }
}

for (const [name, html, routePrefix] of [
  ['Latest Chinese capability page', chineseCapabilities, ''],
  ['Latest English capability page', englishCapabilities, ''],
  [
    'Stable Chinese capability page',
    stableChineseCapabilities,
    `${stableVersion.id}/`,
  ],
  [
    'Stable English capability page',
    stableEnglishCapabilities,
    `${stableVersion.id}/`,
  ],
]) {
  for (const alternate of [
    `hreflang="zh" href="https://a3s-lab.github.io/a3s/${routePrefix}code/capabilities.html"`,
    `hreflang="en" href="https://a3s-lab.github.io/a3s/${routePrefix}en/code/capabilities.html"`,
  ]) {
    if (!html.includes(alternate)) {
      throw new Error(`${name} is missing alternate metadata: ${alternate}`);
    }
  }
}

const expectedSitemapRoutes = documentation.versions.flatMap((version) =>
  documentation.locales.flatMap((locale) =>
    ['', 'code/capabilities', 'code/code-intelligence'].map((page) => {
      const route = routePath(version.id, locale.lang, page);
      return `https://a3s-lab.github.io/a3s/${route}`;
    }),
  ),
);

for (const route of expectedSitemapRoutes) {
  if (!sitemap.includes(route)) {
    throw new Error('Sitemap is missing: ' + route);
  }
}

for (const staleName of [
  'Web + Work',
  'The CLI has three jobs',
  'CLI 只做三件事',
]) {
  if (chinese.includes(staleName) || english.includes(staleName)) {
    throw new Error('Homepage contains stale product copy: ' + staleName);
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
  '.a3s-cli-nav__selector',
  '.a3s-cli-home',
  '.cli-command-row',
  '.cli-product-grid',
  '.cli-canvas-grid',
  '.code-capabilities',
  '.code-context-demo',
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

  if (html.includes('FastCtx')) {
    throw new Error(
      'A3S Code documentation contains a forbidden product reference: ' +
        path.relative(outputRoot, htmlFile),
    );
  }

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
  'Validated the localized, versioned CLI site across ' +
    htmlFiles.length +
    ' HTML pages.',
);
