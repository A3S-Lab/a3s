import { readFile, readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const repositoryRoot = path.resolve(websiteRoot, "..", "..");
const docsRoot = path.join(websiteRoot, "docs");
const homeComponentsRoot = path.join(websiteRoot, "components", "home");

async function collectFilesMatching(directory, pattern) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFilesMatching(absolutePath, pattern)));
    } else if (pattern.test(entry.name)) {
      files.push(absolutePath);
    }
  }

  return files;
}

const failures = [];
const files = await collectFilesMatching(docsRoot, /\.(md|mdx)$/);
const brandingSources = [
  path.join(repositoryRoot, "README.md"),
  ...files,
  ...(await collectFilesMatching(homeComponentsRoot, /\.(ts|tsx)$/)),
];

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

const forbiddenWebBranding = [
  [/Web\s*\/\s*Work\b/i, "uses the obsolete Web / Work product name"],
  [/Web\s*\+\s*Work\b/i, "uses the obsolete Web + Work product name"],
  [/Web\s+is\s+Work\b/i, "explains Web as Work instead of naming Web directly"],
  [/Web\s+就是\s+Work/i, "explains Web as Work instead of naming Web directly"],
  [/Work\s+is\s+Web\b/i, "explains Work as Web instead of naming Web directly"],
  [/Work(?:Product|Copilot)\b/, "exposes an obsolete Work-prefixed Web label"],
];

for (const file of brandingSources) {
  const source = await readFile(file, "utf8");
  const relativePath = path.relative(repositoryRoot, file);

  for (const [pattern, message] of forbiddenWebBranding) {
    if (pattern.test(source)) failures.push(`${relativePath}: ${message}`);
  }
}

if (failures.length > 0) {
  throw new Error(`Content checks failed:\n${failures.join("\n")}`);
}

console.log(`Content checks passed across ${files.length} Markdown pages.`);
