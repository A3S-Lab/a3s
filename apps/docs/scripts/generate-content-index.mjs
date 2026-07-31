import { readdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { format } from "prettier";

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const docsRoot = path.join(websiteRoot, "docs");
const outputPath = path.join(websiteRoot, "theme/generated/content-index.json");

function parseScalar(value) {
  const trimmed = value.trim();
  if (trimmed.startsWith('"') && trimmed.endsWith('"')) {
    return JSON.parse(trimmed);
  }
  if (trimmed.startsWith("'") && trimmed.endsWith("'")) {
    return trimmed.slice(1, -1);
  }
  return trimmed;
}

function parseFrontmatter(source, filePath) {
  const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!match) throw new Error(`${filePath} has no frontmatter`);

  const data = {};
  for (const line of match[1].split(/\r?\n/)) {
    const property = line.match(/^([A-Za-z][A-Za-z0-9_-]*):\s*(.*)$/);
    if (!property) continue;
    const [, key, rawValue] = property;
    if (rawValue.startsWith("[") && rawValue.endsWith("]")) {
      data[key] = rawValue
        .slice(1, -1)
        .split(",")
        .map((item) => parseScalar(item))
        .filter(Boolean);
    } else {
      data[key] = parseScalar(rawValue);
    }
  }

  if (!data.title || !data.description) {
    throw new Error(`${filePath} must define title and description`);
  }
  return data;
}

async function readEntries(language, section) {
  const directory = path.join(docsRoot, language, section);
  const files = (await readdir(directory))
    .filter((file) => file.endsWith(".mdx") && file !== "index.mdx")
    .sort();
  const entries = await Promise.all(
    files.map(async (file) => {
      const filePath = path.join(directory, file);
      const frontmatter = parseFrontmatter(
        await readFile(filePath, "utf8"),
        filePath,
      );
      return {
        slug: file.replace(/\.mdx$/, ""),
        title: frontmatter.title,
        description: frontmatter.description,
        ...(frontmatter.date ? { date: frontmatter.date } : {}),
        ...(frontmatter.author ? { author: frontmatter.author } : {}),
        ...(frontmatter.tags ? { tags: frontmatter.tags } : {}),
      };
    }),
  );

  return entries.sort((left, right) =>
    String(right.date).localeCompare(String(left.date)),
  );
}

const contentIndex = {
  blog: {
    zh: await readEntries("zh", "blog"),
    en: await readEntries("en", "blog"),
  },
};

const serializedIndex = await format(JSON.stringify(contentIndex), {
  filepath: outputPath,
});
await writeFile(outputPath, serializedIndex);
console.log(
  `Generated content index for ${contentIndex.blog.zh.length} blog posts per language.`,
);
