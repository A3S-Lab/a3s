import type { MetadataRoute } from 'next';
import { source } from '@/lib/source';
import { blog } from '@/lib/blog';

const siteUrl = process.env.NEXT_PUBLIC_SITE_URL ?? 'https://a3s.dev';

function postSlug(path: string) {
  return path.replace(/^\//, '').replace(/\.mdx$/, '');
}

export default function sitemap(): MetadataRoute.Sitemap {
  const now = new Date().toISOString();

  const staticPages: MetadataRoute.Sitemap = [
    { url: siteUrl, lastModified: now, changeFrequency: 'weekly', priority: 1.0 },
    { url: `${siteUrl}/blog`, lastModified: now, changeFrequency: 'weekly', priority: 0.8 },
  ];

  const blogPages: MetadataRoute.Sitemap = blog.map((post) => ({
    url: `${siteUrl}/blog/${postSlug(post.info.path)}`,
    lastModified: new Date(post.date).toISOString(),
    changeFrequency: 'monthly' as const,
    priority: 0.7,
  }));

  const enDocs: MetadataRoute.Sitemap = source.getPages('en').map((page) => ({
    url:
      page.slugs.length > 0
        ? `${siteUrl}/docs/${page.slugs.join('/')}`
        : `${siteUrl}/docs`,
    lastModified: now,
    changeFrequency: 'weekly' as const,
    priority: 0.6,
  }));

  const cnDocs: MetadataRoute.Sitemap = source.getPages('cn').map((page) => ({
    url:
      page.slugs.length > 0
        ? `${siteUrl}/cn/docs/${page.slugs.join('/')}`
        : `${siteUrl}/cn/docs`,
    lastModified: now,
    changeFrequency: 'weekly' as const,
    priority: 0.5,
  }));

  return [...staticPages, ...blogPages, ...enDocs, ...cnDocs];
}
