import type { Metadata } from 'next';
import { TutorialContent, tutorialMetadata } from '@/components/tutorial-content';
import { tutorialsSource } from '@/lib/tutorials';

interface PageProps {
  params: Promise<{ slug?: string[] }>;
}

export default async function Page({ params }: PageProps) {
  const { slug } = await params;
  return <TutorialContent locale="cn" slug={slug} />;
}

export function generateStaticParams() {
  return [
    { slug: undefined },
    ...tutorialsSource.getPages('cn').map((page) => ({
      slug: page.slugs.length > 0 ? page.slugs : undefined,
    })),
  ];
}

export async function generateMetadata({ params }: PageProps): Promise<Metadata> {
  const { slug } = await params;
  return tutorialMetadata({ locale: 'cn', slug });
}
