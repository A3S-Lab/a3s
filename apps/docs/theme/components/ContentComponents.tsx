import {
  Callout as RspressCallout,
  Tab,
  Tabs as RspressTabs,
  type TabsProps as RspressTabsProps,
} from "@rspress/core/theme";
import { useLang, usePageData, withBase } from "@rspress/core/runtime";
import type { ReactNode } from "react";
import contentIndexJson from "../generated/content-index.json";

type ContentLanguage = "zh" | "en";

interface ContentIndexEntry {
  slug: string;
  title: string;
  description: string;
  date?: string;
  author?: string;
  tags?: string[];
}

interface ContentIndex {
  blog: Record<ContentLanguage, ContentIndexEntry[]>;
}

interface LegacyTabsProps extends Omit<
  RspressTabsProps,
  "values" | "children"
> {
  items?: string[];
  children: ReactNode;
}

interface TypeTableEntry {
  type?: ReactNode;
  description?: ReactNode;
  default?: ReactNode;
  required?: boolean;
}

const contentIndex = contentIndexJson as ContentIndex;

export { Tab };
export const Callout = RspressCallout;

export function Tabs({ items, children, ...props }: LegacyTabsProps) {
  const values = items?.map((item) => ({ label: item, value: item }));
  return (
    <RspressTabs {...props} values={values}>
      {children}
    </RspressTabs>
  );
}

export function TypeTable({ type }: { type: Record<string, TypeTableEntry> }) {
  return (
    <div className="a3s-type-table">
      {Object.entries(type).map(([name, definition]) => (
        <div className="a3s-type-table__row" key={name}>
          <code>{name}</code>
          <div>
            {definition.type ? (
              <span className="a3s-type-table__type">{definition.type}</span>
            ) : null}
            {definition.required ? (
              <span className="a3s-type-table__required">required</span>
            ) : null}
            {definition.description ? <p>{definition.description}</p> : null}
            {definition.default !== undefined ? (
              <small>
                default: <code>{definition.default}</code>
              </small>
            ) : null}
          </div>
        </div>
      ))}
    </div>
  );
}

function languageKey(lang: string): ContentLanguage {
  return lang === "en" ? "en" : "zh";
}

function contentHref(lang: ContentLanguage, section: string, slug: string) {
  const prefix = lang === "en" ? "/en" : "";
  return withBase(`${prefix}/${section}/${slug}`);
}

function formatDate(value: string, lang: ContentLanguage) {
  return new Intl.DateTimeFormat(lang === "zh" ? "zh-CN" : "en-US", {
    year: "numeric",
    month: "short",
    day: "2-digit",
    timeZone: "UTC",
  }).format(new Date(`${value}T00:00:00Z`));
}

export function BlogIndex() {
  const lang = languageKey(useLang());
  const posts = contentIndex.blog[lang];
  const isChinese = lang === "zh";

  return (
    <div className="a3s-content-index">
      <div className="a3s-content-index__list">
        {posts.map((post) => (
          <a href={contentHref(lang, "blog", post.slug)} key={post.slug}>
            <div>
              <time dateTime={post.date}>
                {post.date ? formatDate(post.date, lang) : ""}
              </time>
              {post.tags?.slice(0, 3).map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </div>
            <h2>{post.title}</h2>
            <p>{post.description}</p>
            <small>{isChinese ? "阅读全文 →" : "Read article →"}</small>
          </a>
        ))}
      </div>
    </div>
  );
}

export function BlogMeta() {
  const lang = languageKey(useLang());
  const {
    page: { frontmatter },
  } = usePageData();
  const date =
    typeof frontmatter.date === "string" ? frontmatter.date : undefined;
  const author =
    typeof frontmatter.author === "string" ? frontmatter.author : undefined;
  const tags = Array.isArray(frontmatter.tags)
    ? frontmatter.tags.filter((tag): tag is string => typeof tag === "string")
    : [];

  return (
    <div className="a3s-blog-meta">
      {date ? <time dateTime={date}>{formatDate(date, lang)}</time> : null}
      {author ? <span>{author}</span> : null}
      {tags.map((tag) => (
        <span key={tag}>{tag}</span>
      ))}
    </div>
  );
}
