export const locales = ['cn', 'en'] as const;
export type Locale = (typeof locales)[number];

export const defaultLocale: Locale = 'cn';

export function isLocale(value: string): value is Locale {
  return locales.includes(value as Locale);
}

export function resolveLocale(value?: string): Locale {
  return value && isLocale(value) ? value : defaultLocale;
}

export function localeFromPathname(pathname: string): Locale {
  const match = pathname.match(/^\/(cn|en)(?:\/|$)/);
  return match && isLocale(match[1]) ? match[1] : defaultLocale;
}

export function stripLocalePrefix(pathname: string): string {
  const stripped = pathname.replace(/^\/(cn|en)(?=\/|$)/, '');
  if (!stripped || stripped === '/') return '/';
  return stripped.startsWith('/') ? stripped : `/${stripped}`;
}

/**
 * Localize an internal absolute path while keeping the Chinese default locale
 * unprefixed. Existing `/cn` links are normalized as legacy aliases.
 */
export function localePath(href: string, locale: Locale): string {
  if (!href.startsWith('/') || href.startsWith('//')) return href;

  const suffixIndex = href.search(/[?#]/);
  const pathname = suffixIndex === -1 ? href : href.slice(0, suffixIndex);
  const suffix = suffixIndex === -1 ? '' : href.slice(suffixIndex);
  const normalized = stripLocalePrefix(pathname).replace(/\/$/, '') || '/';
  const prefix = locale === defaultLocale ? '' : `/${locale}`;

  return `${prefix}${normalized === '/' ? '' : normalized}${suffix}` || '/';
}

/**
 * Point custom-site links at the canonical directory URL emitted by Rspress.
 * Static hosts serve these routes from `index.html`; using the trailing slash
 * also keeps the browser pathname identical to the pathname used during SSG.
 */
export function canonicalSitePath(href: string): string {
  if (!href.startsWith('/') || href.startsWith('//')) return href;

  const suffixIndex = href.search(/[?#]/);
  const pathname = suffixIndex === -1 ? href : href.slice(0, suffixIndex);
  const suffix = suffixIndex === -1 ? '' : href.slice(suffixIndex);
  const hasFileExtension = /\/[^/]+\.[^/]+$/.test(pathname);

  if (pathname === '/' || pathname.endsWith('/') || hasFileExtension) {
    return href;
  }

  return `${pathname}/${suffix}`;
}

export function htmlLanguage(locale: Locale): string {
  return locale === 'cn' ? 'zh-CN' : 'en';
}

export function openGraphLocale(locale: Locale): string {
  return locale === 'cn' ? 'zh_CN' : 'en_US';
}

export function localizedUrl(siteUrl: string, pathname: string, locale: Locale): string {
  return `${siteUrl.replace(/\/$/, '')}${localePath(pathname, locale)}`;
}
