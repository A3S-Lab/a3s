import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const docsRoot = path.join(websiteRoot, "docs");

async function collectMarkdownFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectMarkdownFiles(absolutePath)));
    } else if (/\.(md|mdx)$/.test(entry.name)) {
      files.push(absolutePath);
    }
  }

  return files;
}

const failures = [];
const files = await collectMarkdownFiles(docsRoot);

for (const file of files) {
  const source = await readFile(file, "utf8");
  const relativePath = path.relative(websiteRoot, file);

  if (source.includes("fumadocs")) {
    failures.push(`${relativePath}: still references Fumadocs`);
  }
  if (/from ['"]next(?:\/|['"])/.test(source)) {
    failures.push(`${relativePath}: still imports Next.js`);
  }
  if (source.includes("](/cn/")) {
    failures.push(`${relativePath}: still uses the legacy /cn prefix`);
  }
}

if (failures.length > 0) {
  throw new Error(`Content checks failed:\n${failures.join("\n")}`);
}

console.log(`Content checks passed across ${files.length} Markdown pages.`);
