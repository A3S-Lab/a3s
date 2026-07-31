import { readdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const docsRoot = path.join(websiteRoot, "docs");
const languages = ["zh", "en"];

async function collectContentFiles(directory, prefix = "") {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const relativePath = path.posix.join(prefix, entry.name);
    const absolutePath = path.join(directory, entry.name);

    if (entry.isDirectory()) {
      files.push(...(await collectContentFiles(absolutePath, relativePath)));
    } else if (/\.(md|mdx)$/.test(entry.name)) {
      files.push(relativePath);
    }
  }

  return files.sort();
}

const filesByLanguage = new Map();
for (const language of languages) {
  filesByLanguage.set(
    language,
    new Set(await collectContentFiles(path.join(docsRoot, language))),
  );
}

const allFiles = new Set(
  [...filesByLanguage.values()].flatMap((files) => [...files]),
);
const missing = [];

for (const file of [...allFiles].sort()) {
  for (const language of languages) {
    if (!filesByLanguage.get(language)?.has(file)) {
      missing.push(`${language}/${file}`);
    }
  }
}

if (missing.length > 0) {
  throw new Error(
    `Language parity check failed. Missing files:\n${missing
      .map((file) => `  - ${file}`)
      .join("\n")}`,
  );
}

console.log(
  `Language parity verified: ${allFiles.size} pages in ${languages.join(", ")}.`,
);
