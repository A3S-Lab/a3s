import {
  useLang,
  useLocation,
  usePage,
  useVersion,
} from '@rspress/core/runtime';
import { Link } from '@rspress/core/theme-original';
import { useRef } from 'react';
import documentation from '../../documentation.json';

interface SelectorItem {
  current: boolean;
  href: string;
  hrefLang?: string;
  id: string;
  label: string;
}

function Mark() {
  return (
    <svg aria-hidden="true" viewBox="0 0 48 48">
      <path d="M25.2 4.8A19.2 19.2 0 1 0 42.1 33.2" />
      <path d="M15.2 31.8 24 12.7l8.8 19.1M18.8 24.2h10.4" />
      <path d="M6.8 31.6c6.7-4.8 11.6-4.5 17-.8 5.4 3.7 10.5 2.4 17.7-4.6" />
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M12 .8a11.5 11.5 0 0 0-3.64 22.42c.58.1.79-.25.79-.56v-2.02c-3.22.7-3.9-1.37-3.9-1.37-.53-1.34-1.29-1.7-1.29-1.7-1.05-.72.08-.7.08-.7 1.16.08 1.78 1.2 1.78 1.2 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.57-.29-5.27-1.28-5.27-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.16 1.18a10.9 10.9 0 0 1 5.76 0c2.2-1.49 3.16-1.18 3.16-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.41-2.71 5.38-5.29 5.67.42.36.78 1.06.78 2.14v3.05c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .8Z" />
    </svg>
  );
}

function ChevronIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 12 12">
      <path d="m2.5 4.25 3.5 3.5 3.5-3.5" />
    </svg>
  );
}

function routeHref(locale: string, version: string, pureRoutePath: string) {
  const segments = [];

  if (version !== documentation.defaultVersion) segments.push(version);
  if (locale !== documentation.defaultLocale) segments.push(locale);

  const routeSegments = pureRoutePath.split('/').filter(Boolean);
  segments.push(...routeSegments);

  return segments.length === 0
    ? '/'
    : `/${segments.join('/')}${routeSegments.length === 0 ? '/' : ''}`;
}

function stripRouteContext(routePath: string, locale: string, version: string) {
  const segments = routePath.split('/').filter(Boolean);

  if (version !== documentation.defaultVersion && segments[0] === version) {
    segments.shift();
  }
  if (locale !== documentation.defaultLocale && segments[0] === locale) {
    segments.shift();
  }

  return segments.length === 0 ? '/' : `/${segments.join('/')}`;
}

function versionLabel(
  version: (typeof documentation.versions)[number],
  locale: string,
) {
  const labels: Record<string, string> = version.labels;
  return labels[locale] ?? version.id;
}

function Selector({
  items,
  label,
  menuLabel,
}: {
  items: SelectorItem[];
  label: string;
  menuLabel: string;
}) {
  const detailsRef = useRef<HTMLDetailsElement>(null);

  return (
    <details
      ref={detailsRef}
      className="a3s-cli-nav__selector"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) {
          event.currentTarget.open = false;
        }
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') {
          event.currentTarget.open = false;
          event.currentTarget.querySelector('summary')?.focus();
        }
      }}
    >
      <summary aria-label={menuLabel}>
        <span>{label}</span>
        <ChevronIcon />
      </summary>
      <ul aria-label={menuLabel}>
        {items.map((item) => (
          <li key={item.id}>
            <Link
              aria-current={item.current ? 'page' : undefined}
              href={item.href}
              hrefLang={item.hrefLang}
              lang={item.hrefLang}
              onClick={() => {
                if (detailsRef.current) detailsRef.current.open = false;
              }}
              rel={item.hrefLang ? 'alternate' : undefined}
            >
              <span>{item.label}</span>
              {item.current ? <span aria-hidden="true">✓</span> : null}
            </Link>
          </li>
        ))}
      </ul>
    </details>
  );
}

export function Nav() {
  const requestedLocale = useLang();
  const locale = documentation.locales.some(
    ({ lang }) => lang === requestedLocale,
  )
    ? requestedLocale
    : documentation.defaultLocale;
  const version = useVersion() || documentation.defaultVersion;
  const { page } = usePage();
  const { hash, search } = useLocation();
  const productHomeHref = routeHref(locale, documentation.defaultVersion, '/');
  const currentLocale = documentation.locales.find(
    ({ lang }) => lang === locale,
  );
  const currentVersion = documentation.versions.find(
    ({ id }) => id === version,
  );
  const currentRoutePath =
    typeof page.routePath === 'string' ? page.routePath : '/';
  const pureRoutePath =
    page.pageType === '404'
      ? '/'
      : stripRouteContext(currentRoutePath, locale, version);
  const routeSuffix = `${search}${hash}`;
  const languageItems = documentation.locales.map((item) => ({
    current: item.lang === locale,
    href: `${routeHref(item.lang, version, pureRoutePath)}${routeSuffix}`,
    hrefLang: item.htmlLang,
    id: item.lang,
    label: item.label,
  }));
  const versionItems = documentation.versions.map((item) => ({
    current: item.id === version,
    href: `${routeHref(locale, item.id, pureRoutePath)}${routeSuffix}`,
    id: item.id,
    label: versionLabel(item, locale),
  }));

  return (
    <header className="a3s-cli-nav">
      <div className="a3s-cli-nav__inner">
        <Link className="a3s-cli-brand" href={productHomeHref}>
          <Mark />
          <span>A3S</span>
          <small>CLI</small>
        </Link>
        <nav
          className="a3s-cli-nav__links"
          aria-label={locale === 'zh' ? '主导航' : 'Primary navigation'}
        >
          <Link href={`${productHomeHref}#commands`}>
            {locale === 'zh' ? '命令' : 'Commands'}
          </Link>
          <Link href={`${productHomeHref}#components`}>
            {locale === 'zh' ? '组件' : 'Components'}
          </Link>
          <Link href={`${productHomeHref}#capabilities`}>
            {locale === 'zh' ? 'Code 能力' : 'Code capabilities'}
          </Link>
          <Link href={`${productHomeHref}#install`}>
            {locale === 'zh' ? '安装' : 'Install'}
          </Link>
        </nav>
        <div className="a3s-cli-nav__actions">
          <Selector
            items={versionItems}
            label={
              currentVersion ? versionLabel(currentVersion, locale) : version
            }
            menuLabel={
              locale === 'zh' ? '切换文档版本' : 'Switch documentation version'
            }
          />
          <Selector
            items={languageItems}
            label={currentLocale?.shortLabel ?? locale}
            menuLabel={locale === 'zh' ? '切换语言' : 'Switch language'}
          />
          <a
            className="a3s-cli-nav__github"
            href="https://github.com/A3S-Lab/a3s"
            aria-label="A3S on GitHub"
          >
            <GitHubIcon />
          </a>
        </div>
      </div>
    </header>
  );
}
