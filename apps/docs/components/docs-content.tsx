import {
  DocsBody,
  DocsDescription,
  DocsPage,
  DocsTitle,
} from 'fumadocs-ui/layouts/docs/page';
import { notFound } from 'next/navigation';
import type { Metadata } from 'next';
import { source } from '@/lib/source';
import { getMDXComponents } from '@/mdx-components';
import { createLocalizedRelativeMdxLink } from '@/lib/localized-mdx-link';
import {
  localizedUrl,
  openGraphLocale,
  type Locale,
} from '@/lib/i18n';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

interface DocsContentProps {
  locale: Locale;
  slug?: string[];
}

export function DocsContent({ locale, slug }: DocsContentProps) {
  const page = source.getPage(slug, locale);
  if (!page) notFound();

  const MDX = page.data.body;
  const LocalizedLink = createLocalizedRelativeMdxLink(source, page, locale);

  return (
    <DocsPage toc={page.data.toc} full={page.data.full}>
      <DocsTitle>{page.data.title}</DocsTitle>
      <DocsDescription>{page.data.description}</DocsDescription>
      <DocsBody className="dark:prose-invert">
        <MDX
          components={getMDXComponents({
            a: LocalizedLink,
          })}
        />
      </DocsBody>
    </DocsPage>
  );
}

export function docsMetadata({ locale, slug }: DocsContentProps): Metadata {
  const page = source.getPage(slug, locale);
  if (!page) notFound();

  const pathname = `/docs${page.slugs.length > 0 ? `/${page.slugs.join('/')}` : ''}`;

  return {
    title: page.data.title,
    description: page.data.description,
    alternates: {
      canonical: localizedUrl(siteUrl, pathname, locale),
      languages: {
        'zh-CN': localizedUrl(siteUrl, pathname, 'cn'),
        en: localizedUrl(siteUrl, pathname, 'en'),
      },
    },
    openGraph: {
      title: page.data.title,
      description: page.data.description,
      type: 'article',
      locale: openGraphLocale(locale),
    },
  };
}
