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

const [english, chinese] = await Promise.all([read('index.html'), read('cn.html')]);

if (process.env.NEXT_PUBLIC_SITE_URL) {
  const sitemap = await read('sitemap.xml');
  assert(
    sitemap.includes(`<loc>${process.env.NEXT_PUBLIC_SITE_URL}</loc>`),
    `Sitemap is missing the public site URL: ${process.env.NEXT_PUBLIC_SITE_URL}`,
  );
}

for (const marker of [
  'One command.',
  'id="products"',
  'id="architecture"',
  'id="principles"',
  'a3s-canvas-backdrop',
  'a3s code',
]) {
  assert(english.includes(marker), `English homepage is missing: ${marker}`);
}

for (const marker of ['一个命令。', '每一道 Agent 边界', '从零启动一个可治理 Agent。']) {
  assert(chinese.includes(marker), `Chinese homepage is missing: ${marker}`);
}

assert(!english.includes('/og.png'), 'Homepage still references the removed /og.png asset');

const files = await collectFiles(output);
const cssFiles = files.filter((file) => file.endsWith('.css'));
assert(cssFiles.length > 0, 'Static export contains no CSS assets');

const css = (await Promise.all(cssFiles.map((file) => readFile(file, 'utf8')))).join('\n');
for (const selector of ['.a3s-canvas-backdrop', '.a3s-home-nav', '.a3s-system-panel', '.a3s-product-card', '.a3s-stack-layer']) {
  assert(css.includes(selector), `Production CSS is missing: ${selector}`);
}

assert(
  files.some((file) => path.basename(file) === 'opengraph-image'),
  'Static export is missing the generated Open Graph image',
);

console.log(`Validated bilingual A3S homepage across ${files.length} exported files.`);
