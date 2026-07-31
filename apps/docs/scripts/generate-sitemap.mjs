import { readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const websiteRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const outputRoot = path.join(websiteRoot, "out");
const base = process.env.DOCS_BASE ?? "/a3s/";
const siteOrigin = process.env.DOCS_ORIGIN ?? "https://a3s-lab.github.io";
const publicSite = `${siteOrigin.replace(/\/$/, "")}${base.replace(/\/$/, "")}`;

async function collectHtmlFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectHtmlFiles(absolutePath)));
    } else if (entry.name.endsWith(".html")) {
      files.push(absolutePath);
    }
  }

  return files;
}

function routeFromFile(filePath) {
  const relativePath = path
    .relative(outputRoot, filePath)
    .split(path.sep)
    .join("/");
  if (relativePath === "index.html") return "/";
  if (relativePath.endsWith("/index.html")) {
    return `/${relativePath.slice(0, -"index.html".length)}`;
  }
  return `/${relativePath.replace(/\.html$/, "")}`;
}

const routes = (await collectHtmlFiles(outputRoot))
  .map(routeFromFile)
  .filter((route) => route !== "/404")
  .sort();
const urls = routes
  .map(
    (route) =>
      `  <url><loc>${publicSite}${route === "/" ? "/" : route}</loc></url>`,
  )
  .join("\n");
const sitemap = `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`;

await writeFile(path.join(outputRoot, "sitemap.xml"), sitemap);
console.log(`Generated sitemap.xml with ${routes.length} routes.`);
