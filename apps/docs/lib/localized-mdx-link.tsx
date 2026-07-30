import Link from 'fumadocs-core/link';
import type { ComponentProps, FC } from 'react';
import { localePath, type Locale } from '@/lib/i18n';

interface LinkPage {
  path: string;
  slugs: string[];
}

interface PageLookup {
  getPage(slugs: string[] | undefined, locale?: string): { url: string } | undefined;
}

function relativeSlugs(page: LinkPage, href: string): string[] {
  const filename = page.path.split('/').at(-1) ?? '';
  const isIndexPage = /^index\.(?:md|mdx)$/.test(filename);
  const slugs = isIndexPage ? [...page.slugs] : page.slugs.slice(0, -1);
  const pathname = href.split(/[?#]/, 1)[0].replace(/^\.\//, '');

  for (const segment of pathname.split('/')) {
    if (!segment || segment === '.') continue;
    if (segment === '..') {
      slugs.pop();
      continue;
    }
    slugs.push(segment.replace(/\.(?:md|mdx)$/, ''));
  }

  return slugs;
}

export function resolveLocalizedMdxHref(
  source: PageLookup,
  page: LinkPage,
  href: string,
  locale: Locale,
): string {
  if (!href.startsWith('./') && !href.startsWith('../')) return localePath(href, locale);

  const suffixIndex = href.search(/[?#]/);
  const suffix = suffixIndex === -1 ? '' : href.slice(suffixIndex);
  const target = source.getPage(relativeSlugs(page, href), locale);
  return target ? `${localePath(target.url, locale)}${suffix}` : href;
}

export function createLocalizedMdxLink(locale: Locale): FC<ComponentProps<'a'>> {
  return function LocalizedMdxLink({ href, ...props }) {
    return <Link href={href ? localePath(href, locale) : href} {...props} />;
  };
}

export function createLocalizedRelativeMdxLink(
  source: PageLookup,
  page: LinkPage,
  locale: Locale,
): FC<ComponentProps<'a'>> {
  return function LocalizedRelativeMdxLink({ href, ...props }) {
    return (
      <Link
        href={href ? resolveLocalizedMdxHref(source, page, href, locale) : href}
        {...props}
      />
    );
  };
}
