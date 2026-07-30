import { DocsContent, docsMetadata } from '@/components/docs-content';
import { source } from '@/lib/source';
import type { Metadata } from 'next';

interface PageProps {
  params: Promise<{ slug?: string[] }>;
}

export default async function Page(props: PageProps) {
  const { slug } = await props.params;
  return <DocsContent locale="cn" slug={slug} />;
}

export function generateStaticParams() {
  return source.getPages('cn').map((page) => ({
    slug: page.slugs.length > 0 ? page.slugs : undefined,
  }));
}

export async function generateMetadata(props: PageProps): Promise<Metadata> {
  const { slug } = await props.params;
  return docsMetadata({ locale: 'cn', slug });
}
