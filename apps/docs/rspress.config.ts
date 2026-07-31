import * as path from "node:path";
import { defineConfig } from "@rspress/core";
import type { Theme } from "@code-hike/lighter";
import type { RawCode } from "codehike/code";
import { remarkCodeHike } from "codehike/mdx";
import codeHikeTheme from "./codehike-theme.json";

const base = process.env.DOCS_BASE ?? "/a3s/";
const siteOrigin = process.env.DOCS_ORIGIN ?? "https://a3s-lab.github.io";
const publicSite = `${siteOrigin.replace(/\/$/, "")}${base.replace(/\/$/, "")}`;

export default defineConfig({
  root: path.join(__dirname, "docs"),
  base,
  siteOrigin,
  title: "A3S",
  description:
    "A3S is an operating system for agents: sessions, tools, state, workflows, execution, and policy.",
  route: {
    exclude: ["**/docs/*/**", "**/tutorials/**"],
  },
  lang: "zh",
  icon: "/favicon.svg",
  logo: `${base}a3s-mark.svg`,
  logoText: "A3S",
  outDir: "out",
  llms: true,
  markdown: {
    remarkPlugins: [
      [
        remarkCodeHike,
        {
          components: { code: "A3SCodeBlock" },
          ignoreCode: (codeblock: RawCode) => !codeblock.lang,
          syntaxHighlighting: {
            theme: codeHikeTheme as Theme,
          },
        },
      ],
    ],
    globalComponents: [
      path.join(__dirname, "theme/components/A3SCodeBlock.tsx"),
      path.join(__dirname, "theme/components/BlogIndex.tsx"),
      path.join(__dirname, "theme/components/BlogMeta.tsx"),
      path.join(__dirname, "theme/components/Callout.tsx"),
      path.join(__dirname, "theme/components/Tab.tsx"),
      path.join(__dirname, "theme/components/Tabs.tsx"),
      path.join(__dirname, "theme/components/TypeTable.tsx"),
    ],
  },
  locales: [
    {
      lang: "zh",
      label: "简体中文",
      title: "A3S",
      description:
        "A3S 是智能体操作系统，为会话、工具、状态、工作流、执行环境和权限提供统一接口。",
    },
    {
      lang: "en",
      label: "English",
      title: "A3S",
      description:
        "A3S is an operating system for agents, with common contracts for sessions, tools, state, workflows, execution, and policy.",
    },
  ],
  head: [
    ["meta", { name: "theme-color", content: "#000000" }],
    ["meta", { property: "og:type", content: "website" }],
    ["meta", { property: "og:site_name", content: "A3S" }],
    [
      "meta",
      {
        property: "og:image",
        content: `${publicSite}/social-card.svg`,
      },
    ],
    ["meta", { name: "twitter:card", content: "summary_large_image" }],
    ["link", { rel: "preconnect", href: "https://fonts.googleapis.com" }],
    [
      "link",
      {
        rel: "preconnect",
        href: "https://fonts.gstatic.com",
        crossorigin: "anonymous",
      },
    ],
    [
      "link",
      {
        rel: "stylesheet",
        href: "https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;700&display=swap",
      },
    ],
    [
      "script",
      {
        async: "true",
        src: "https://www.googletagmanager.com/gtag/js?id=G-VWVNY8DWPG",
      },
    ],
    ["script", { src: `${base}analytics.js` }],
    (route) => [
      "link",
      {
        rel: "canonical",
        href: `${publicSite}${route.routePath}`,
      },
    ],
  ],
  themeConfig: {
    darkMode: "force-dark",
    search: true,
    localeRedirect: "never",
    enableContentAnimation: true,
    editLink: {
      docRepoBaseUrl: "https://github.com/A3S-Lab/a3s/tree/main/apps/docs/docs",
    },
    lastUpdated: {
      author: true,
    },
    llmsUI: {
      placement: "outline",
      viewOptions: ["markdownLink", "chatgpt", "claude"],
    },
    socialLinks: [
      {
        icon: "github",
        mode: "link",
        content: "https://github.com/A3S-Lab/a3s",
      },
    ],
  },
});
