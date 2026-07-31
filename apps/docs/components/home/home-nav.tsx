"use client";

import { Github, Menu, MoveUpRight } from "lucide-react";
import type { MouseEvent as ReactMouseEvent } from "react";
import { localePath } from "../../lib/i18n";
import { A3SMark } from "./a3s-mark";
import { homeContent, type Lang } from "./home-content";
import { SiteLink } from "./site-link";

function closeMobileNav(event: ReactMouseEvent<HTMLDivElement>) {
  if (!(event.target instanceof Element) || !event.target.closest("a")) return;

  const details = event.currentTarget.closest("details");
  if (details) details.open = false;
}

export function HomeNav({ lang }: { lang: Lang }) {
  const tr = homeContent[lang].nav;
  const homeHref = localePath("/", lang);
  const languageHref = localePath("/", lang === "cn" ? "en" : "cn");

  const anchorLinks = [
    { label: tr.aiNative, href: "#ai-native" },
    { label: tr.cloudLifecycle, href: "#cloud-lifecycle" },
    { label: tr.architecture, href: "#architecture" },
  ];

  return (
    <header className="a3s-home-nav">
      <nav className="a3s-home-nav__inner" aria-label="Primary navigation">
        <SiteLink
          className="a3s-home-brand"
          href={homeHref}
          aria-label="A3S home"
        >
          <A3SMark className="a3s-home-brand__mark" />
          <span>A3S</span>
          <small>LAB</small>
        </SiteLink>

        <div className="a3s-home-nav__links">
          {anchorLinks.map((item) => (
            <a href={item.href} key={item.href}>
              {item.label}
            </a>
          ))}
          <SiteLink href={localePath("/blog", lang)}>{tr.blog}</SiteLink>
        </div>

        <div className="a3s-home-nav__actions">
          <SiteLink
            className="a3s-home-nav__language"
            href={languageHref}
            hrefLang={lang === "cn" ? "en" : "zh-Hans"}
          >
            {tr.language}
          </SiteLink>
          <SiteLink
            className="a3s-home-nav__github"
            href="https://github.com/A3S-Lab/a3s"
            target="_blank"
            rel="noopener noreferrer"
            aria-label="A3S on GitHub"
          >
            <Github aria-hidden="true" />
          </SiteLink>
          <SiteLink
            className="a3s-home-nav__docs"
            href={localePath("/docs", lang)}
          >
            {tr.docs}
            <MoveUpRight aria-hidden="true" />
          </SiteLink>
        </div>

        <details className="a3s-home-nav__mobile">
          <summary aria-haspopup="menu" aria-label={tr.menu} role="button">
            <Menu aria-hidden="true" />
            <span className="sr-only">{tr.menu}</span>
          </summary>
          <div className="a3s-home-nav__mobile-panel" onClick={closeMobileNav}>
            {anchorLinks.map((item) => (
              <a href={item.href} key={item.href}>
                {item.label}
              </a>
            ))}
            <SiteLink href={localePath("/docs", lang)}>{tr.docs}</SiteLink>
            <SiteLink href={localePath("/blog", lang)}>{tr.blog}</SiteLink>
            <SiteLink href={languageHref}>{tr.language}</SiteLink>
            <SiteLink
              href="https://github.com/A3S-Lab/a3s"
              target="_blank"
              rel="noopener noreferrer"
            >
              GitHub
            </SiteLink>
          </div>
        </details>
      </nav>
    </header>
  );
}
