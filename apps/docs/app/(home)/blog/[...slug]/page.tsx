import type { Metadata } from 'next';
import { BlogPost, blogPostMetadata } from '@/components/blog-post';
import { blogPostSlug, getBlogPosts } from '@/lib/blog';

interface PageProps {
  params: Promise<{ slug: string[] }>;
}

export function generateStaticParams() {
  return getBlogPosts('cn').map((post) => ({
    slug: blogPostSlug(post.info.path).split('/'),
  }));
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { slug } = await params;
  return blogPostMetadata({ locale: 'cn', slug: slug.join('/') });
}

export default async function Page({ params }: PageProps) {
  const { slug } = await params;
  return <BlogPost locale="cn" slug={slug.join('/')} />;
}
