import { blog as blogCollection } from 'fumadocs-mdx:collections/server';
import type { Locale } from '@/lib/i18n';

export const blog = blogCollection;

export function blogPostSlug(path: string): string {
  return path.replace(/^[^/]+\//, '').replace(/\.mdx$/, '');
}

export function getBlogPosts(locale: Locale) {
  return [...blog]
    .filter((post) => post.info.path.startsWith(`${locale}/`))
    .sort((left, right) => new Date(right.date).getTime() - new Date(left.date).getTime());
}

export function getBlogPost(locale: Locale, slug: string) {
  const path = `${locale}/${slug}.mdx`;
  return blog.find((post) => post.info.path === path);
}
