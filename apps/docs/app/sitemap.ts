import type { MetadataRoute } from 'next';
import { source } from '@/lib/source';
import { blogPostSlug, getBlogPosts } from '@/lib/blog';
import { tutorialsSource } from '@/lib/tutorials';
import { localizedUrl, type Locale } from '@/lib/i18n';

export const dynamic = 'force-static';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date().toISOString();

  const staticPages: MetadataRoute.Sitemap = [
    { url: siteUrl, lastModified: now, changeFrequency: 'weekly', priority: 1.0 },
    { url: `${siteUrl}/en`, lastModified: now, changeFrequency: 'weekly', priority: 0.9 },
    { url: `${siteUrl}/blog`, lastModified: now, changeFrequency: 'weekly', priority: 0.8 },
    { url: `${siteUrl}/en/blog`, lastModified: now, changeFrequency: 'weekly', priority: 0.7 },
    { url: `${siteUrl}/tutorials`, lastModified: now, changeFrequency: 'weekly', priority: 0.7 },
    { url: `${siteUrl}/en/tutorials`, lastModified: now, changeFrequency: 'weekly', priority: 0.6 },
  ];

  const localizedPages = (locale: Locale): MetadataRoute.Sitemap => {
    const blogPages: MetadataRoute.Sitemap = getBlogPosts(locale).map((post) => ({
      url: localizedUrl(siteUrl, `/blog/${blogPostSlug(post.info.path)}`, locale),
      lastModified: new Date(post.date).toISOString(),
      changeFrequency: 'monthly' as const,
      priority: locale === 'cn' ? 0.7 : 0.6,
    }));

    const docsPages: MetadataRoute.Sitemap = source.getPages(locale).map((page) => ({
      url: localizedUrl(
        siteUrl,
        `/docs${page.slugs.length > 0 ? `/${page.slugs.join('/')}` : ''}`,
        locale,
      ),
      lastModified: now,
      changeFrequency: 'weekly' as const,
      priority: locale === 'cn' ? 0.6 : 0.5,
    }));

    const tutorialPages: MetadataRoute.Sitemap = tutorialsSource.getPages(locale).map((page) => ({
      url: localizedUrl(siteUrl, `/tutorials/${page.slugs.join('/')}`, locale),
      lastModified: now,
      changeFrequency: 'monthly' as const,
      priority: locale === 'cn' ? 0.6 : 0.5,
    }));

    return [...blogPages, ...docsPages, ...tutorialPages];
  };

  return [...staticPages, ...localizedPages('cn'), ...localizedPages('en')];
}
