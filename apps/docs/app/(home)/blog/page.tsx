import { blog } from '@/lib/blog';
import Link from 'next/link';

export default async function BlogPage() {
  const posts = [...blog].sort(
    (a, b) => new Date(b.data.date).getTime() - new Date(a.data.date).getTime()
  );

  return (
    <main className="term-font" style={{ background: 'var(--term-bg)', color: 'var(--term-green)', minHeight: '100vh' }}>
      <div className="crt-overlay" />

      <nav className="w-full max-w-4xl mx-auto px-4 sm:px-6 pt-4 pb-2 flex items-center justify-between text-xs" style={{ borderBottom: '1px solid var(--term-muted)' }}>
        <Link href="/" className="term-glow font-bold tracking-wider uppercase hover-glitch">
          &gt; A3S_
        </Link>
        <div className="flex gap-4">
          <Link href="/docs/code" style={{ color: 'var(--term-muted)' }} className="hover-glitch">[docs]</Link>
          <Link href="/blog" style={{ color: 'var(--term-green)' }} className="hover-glitch">[blog]</Link>
          <Link href="https://github.com/A3S-Lab" style={{ color: 'var(--term-muted)' }} className="hover-glitch">[github]</Link>
        </div>
      </nav>

      <section className="w-full max-w-4xl mx-auto px-4 sm:px-6 pt-12 pb-20">
        <div className="mb-2 text-xs" style={{ color: 'var(--term-muted)' }}>
          <span style={{ color: 'var(--term-amber)' }}>root@a3s</span>:~$ ls -la /blog/
        </div>
        <h1 className="text-2xl font-bold term-glow uppercase mb-8">Blog</h1>

        <div className="space-y-4">
          {posts.map((post) => {
            const slug = post.info.path.replace(/^\//, '').replace(/\.mdx$/, '');
            return (
            <Link key={slug} href={`/blog/${slug}`} className="term-pane block hover-glitch group">
              <div className="term-pane-header">
                <span>┌─ {new Date(post.date).toISOString().slice(0, 10)} ─┐</span>
              </div>
              <div className="p-4">
                <h2 className="text-sm font-bold term-glow mb-2">{post.title}</h2>
                <p className="text-xs leading-relaxed" style={{ color: 'var(--term-green)', opacity: 0.7 }}>
                  {post.description}
                </p>
                {post.tags && (
                  <div className="mt-3 flex flex-wrap gap-2">
                    {post.tags.map((tag) => (
                      <span key={tag} className="text-[0.6rem] px-1.5 py-0.5" style={{ border: '1px solid var(--term-muted)', color: 'var(--term-muted)' }}>
                        #{tag}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            </Link>
            );
          })}
        </div>
      </section>
    </main>
  );
}
