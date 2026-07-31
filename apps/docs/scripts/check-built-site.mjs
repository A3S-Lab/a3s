import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const outputRoot = path.join(websiteRoot, "out");
const base = process.env.DOCS_BASE ?? "/a3s/";
const requiredFiles = [
  "index.html",
  "en/index.html",
  "docs/index.html",
  "en/docs/index.html",
  "docs/installation.html",
  "en/docs/installation.html",
  "docs/configuration.html",
  "en/docs/configuration.html",
  "docs/commands.html",
  "en/docs/commands.html",
  "docs/code.html",
  "en/docs/code.html",
  "docs/web.html",
  "en/docs/web.html",
  "docs/components.html",
  "en/docs/components.html",
  "docs/updates.html",
  "en/docs/updates.html",
  "docs/architecture.html",
  "en/docs/architecture.html",
  "blog/index.html",
  "en/blog/index.html",
  "llms.txt",
  "llms-full.txt",
  "en/llms.txt",
  "en/llms-full.txt",
  "a3s-mark.svg",
  "favicon.svg",
  "social-card.svg",
  "robots.txt",
  "sitemap.xml",
];

function assert(condition, message) {
  if (!condition) throw new Error(message);
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

for (const file of requiredFiles) {
  await access(path.join(outputRoot, file));
}

for (const file of [
  "tutorials/index.html",
  "en/tutorials/index.html",
  "docs/cli/index.html",
  "en/docs/cli/index.html",
  "docs/cloud/index.html",
  "en/docs/cloud/index.html",
]) {
  try {
    await access(path.join(outputRoot, file));
    throw new Error(`Excluded route was still generated: ${file}`);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") {
      continue;
    }
    throw error;
  }
}

const [chinese, english, chineseDocs, englishDocs, blog, englishBlog] =
  await Promise.all([
    readFile(path.join(outputRoot, "index.html"), "utf8"),
    readFile(path.join(outputRoot, "en/index.html"), "utf8"),
    readFile(path.join(outputRoot, "docs/index.html"), "utf8"),
    readFile(path.join(outputRoot, "en/docs/index.html"), "utf8"),
    readFile(path.join(outputRoot, "blog/index.html"), "utf8"),
    readFile(path.join(outputRoot, "en/blog/index.html"), "utf8"),
  ]);

for (const marker of [
  "A3S 是",
  "AI Native 的",
  "智能体",
  "操作系统",
  'id="ai-native"',
  'id="cloud-lifecycle"',
  'id="architecture"',
  'data-canvas-field="hero"',
  'data-canvas-field="lifecycle"',
  'data-canvas-field="terminal"',
  "渐进式 API",
  "POST /api/v1/kernel/capabilities",
  "search → describe → execute",
  "提交部署意图",
  "读取有边界的日志",
  "H0.3",
  "Gateway · Runtime · Cloud",
]) {
  assert(chinese.includes(marker), `Chinese homepage is missing: ${marker}`);
}

for (const marker of [
  "A3S is",
  "the AI Native",
  "operating system",
  "for agents",
  'id="ai-native"',
  'id="cloud-lifecycle"',
  'id="architecture"',
  'data-canvas-field="hero"',
  'data-canvas-field="lifecycle"',
  'data-canvas-field="terminal"',
  "Progressive API",
  "POST /api/v1/kernel/capabilities",
  "search → describe → execute",
  "Commit deployment intent",
  "Read bounded logs",
  "H0.3",
  "Gateway · Runtime · Cloud",
]) {
  assert(english.includes(marker), `English homepage is missing: ${marker}`);
}

for (const command of [
  "a3s config paths",
  "a3s model current",
  "a3s code exec",
  "a3s web -d",
  "a3s doctor",
]) {
  assert(chinese.includes(command), `Chinese CLI demo is missing: ${command}`);
  assert(english.includes(command), `English CLI demo is missing: ${command}`);
}

assert(chinese.includes("Web"), "Chinese homepage has no Web product");
assert(english.includes("Web"), "English homepage has no Web product");
assert(
  !chinese.includes("Web + Work"),
  "Chinese homepage still says Web + Work",
);
assert(
  !english.includes("Web + Work"),
  "English homepage still says Web + Work",
);

assert(
  chineseDocs.includes("A3S CLI"),
  "Chinese CLI documentation index is missing",
);
assert(
  englishDocs.includes("A3S CLI"),
  "English CLI documentation index is missing",
);
assert(
  blog.includes("工程记录"),
  "Chinese blog index is missing its introduction",
);
assert(
  englishBlog.includes("Engineering notes"),
  "English blog index is missing its introduction",
);

const files = await collectFiles(outputRoot);
const htmlFiles = files.filter((file) => file.endsWith(".html"));
const brokenReferences = [];
const referencePattern = /(?:href|src)="([^"]+)"/g;

for (const htmlFile of htmlFiles) {
  const html = await readFile(htmlFile, "utf8");
  for (const [, rawReference] of html.matchAll(referencePattern)) {
    if (
      rawReference.startsWith("#") ||
      rawReference.startsWith("data:") ||
      rawReference.startsWith("mailto:") ||
      /^[a-z]+:\/\//i.test(rawReference)
    ) {
      continue;
    }

    if (rawReference.startsWith("/") && !rawReference.startsWith(base)) {
      brokenReferences.push(
        `${path.relative(outputRoot, htmlFile)} -> ${rawReference} (outside ${base})`,
      );
      continue;
    }

    if (!rawReference.startsWith(base)) continue;

    const withoutBase = rawReference
      .slice(base.length)
      .split(/[?#]/, 1)[0]
      .replace(/\/+/g, "/");
    const candidates =
      withoutBase === "" || withoutBase.endsWith("/")
        ? [path.join(outputRoot, withoutBase, "index.html")]
        : [
            path.join(outputRoot, withoutBase),
            path.join(outputRoot, `${withoutBase}.html`),
            path.join(outputRoot, withoutBase, "index.html"),
          ];

    let found = false;
    for (const candidate of candidates) {
      try {
        await access(candidate);
        found = true;
        break;
      } catch {
        // Try the next static-output shape.
      }
    }
    if (!found) {
      brokenReferences.push(
        `${path.relative(outputRoot, htmlFile)} -> ${rawReference}`,
      );
    }
  }
}

if (brokenReferences.length > 0) {
  throw new Error(
    `Built-site reference check failed:\n${brokenReferences
      .map((reference) => `  - ${reference}`)
      .join("\n")}`,
  );
}

const css = (
  await Promise.all(
    files
      .filter((file) => file.endsWith(".css"))
      .map((file) => readFile(file, "utf8")),
  )
).join("\n");

for (const selector of [
  ".a3s-home-nav",
  ".a3s-cli-terminal__screen",
  ".a3s-canvas-field",
  ".a3s-cloud-terminal__screen",
  ".a3s-cloud-terminal__systems",
  ".a3s-type-char",
  ".a3s-native__flow",
  ".a3s-native__stack",
  ".a3s-atlas__node",
  ".a3s-doc-code",
  ".a3s-content-index",
]) {
  assert(css.includes(selector), `Production CSS is missing: ${selector}`);
}

for (const project of [
  "Code",
  "Desktop",
  "Office",
  "Parser",
  "Test",
  "Cloud",
  "Runtime",
  "Observer",
  "Homebrew Tap",
]) {
  assert(chinese.includes(project), `Chinese atlas is missing: ${project}`);
  assert(english.includes(project), `English atlas is missing: ${project}`);
}

for (const marker of [
  "A3S Code",
  "Gateway",
  "Runtime",
  "Cloud",
  "Observer",
  "Sentry",
]) {
  assert(chinese.includes(marker), `Chinese lifecycle is missing: ${marker}`);
  assert(english.includes(marker), `English lifecycle is missing: ${marker}`);
}

console.log(
  `Validated ${htmlFiles.length} Rspress pages and ${files.length} exported files.`,
);
