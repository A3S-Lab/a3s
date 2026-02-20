import { blog } from '@/lib/blog';
import { getMDXComponents } from '@/mdx-components';
import { notFound } from 'next/navigation';
import Link from 'next/link';
import type { Metadata } from 'next';

interface PostProps {
  params: Promise<{ slug: string }>;
}

export default async function BlogPost({ params }: PostProps) {
  const { slug } = await params;
  const post = blog.find((p) => p.slugs[0] === slug);
  if (!post) notFound();

  const { body: MDX } = await post.load();

  return (
    <main className="term-font" style={{ background: 'var(--term-bg)', color: 'var(--term-green)', minHeight: '100vh' }}>
      <div className="crt-overlay" />

      <nav className="w-full max-w-4xl mx-auto px-4 sm:px-6 pt-4 pb-2 flex items-center justify-between text-xs" style={{ borderBottom: '1px solid var(--term-muted)' }}>
        <Link href="/" className="term-glow font-bold tracking-wider uppercase hover-glitch">
          &gt; A3S_
        </Link>
        <div className="flex gap-4">
          <Link href="/blog" style={{ color: 'var(--term-muted)' }} className="hover-glitch">[← blog]</Link>
          <Link href="/docs/code" style={{ color: 'var(--term-muted)' }} className="hover-glitch">[docs]</Link>
          <Link href="https://github.com/A3S-Lab" style={{ color: 'var(--term-muted)' }} className="hover-glitch">[github]</Link>
        </div>
      </nav>

      <article className="w-full max-w-4xl mx-auto px-4 sm:px-6 pt-10 pb-20">
        <div className="mb-6">
          <div className="text-xs mb-3" style={{ color: 'var(--term-muted)' }}>
            {new Date(post.data.date).toISOString().slice(0, 10)}
            {post.data.author && <span> · {post.data.author}</span>}
          </div>
          {post.data.tags && (
            <div className="flex flex-wrap gap-2 mb-6">
              {post.data.tags.map((tag) => (
                <span key={tag} className="text-[0.6rem] px-1.5 py-0.5" style={{ border: '1px solid var(--term-muted)', color: 'var(--term-muted)' }}>
                  #{tag}
                </span>
              ))}
            </div>
          )}
        </div>

        <div className="prose prose-invert prose-sm max-w-none
          prose-headings:text-[var(--term-green)] prose-headings:font-bold
          prose-h1:text-xl prose-h2:text-lg prose-h3:text-base
          prose-p:text-[var(--term-green)] prose-p:opacity-80
          prose-a:text-[var(--term-amber)] prose-a:no-underline hover:prose-a:underline
          prose-code:text-[var(--term-amber)] prose-code:bg-transparent
          prose-pre:bg-black/40 prose-pre:border prose-pre:border-[var(--term-muted)]
          prose-strong:text-[var(--term-green)]
          prose-blockquote:border-l-[var(--term-amber)] prose-blockquote:text-[var(--term-muted)]
          prose-table:text-xs prose-th:text-[var(--term-amber)] prose-td:text-[var(--term-green)]
          prose-hr:border-[var(--term-muted)]">
          <MDX components={getMDXComponents({})} />
        </div>
      </article>
    </main>
  );
}

export function generateStaticParams() {
  return blog.map((post) => ({ slug: post.slugs[0] }));
}

export async function generateMetadata({ params }: PostProps): Promise<Metadata> {
  const { slug } = await params;
  const post = blog.find((p) => p.slugs[0] === slug);
  if (!post) notFound();
  return {
    title: post.data.title,
    description: post.data.description,
  };
}
