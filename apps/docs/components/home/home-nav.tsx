import { Github, Menu, MoveUpRight } from 'lucide-react';
import Link from 'next/link';
import { A3SMark } from '@/components/home/a3s-mark';
import { homeContent, type Lang } from '@/components/home/home-content';

export function HomeNav({ lang }: { lang: Lang }) {
  const tr = homeContent[lang].nav;
  const base = lang === 'cn' ? '/cn' : '';
  const languageHref = lang === 'cn' ? '/' : '/cn';

  const anchorLinks = [
    { label: tr.products, href: '#products' },
    { label: tr.architecture, href: '#architecture' },
    { label: tr.principles, href: '#principles' },
  ];

  return (
    <header className="a3s-home-nav">
      <nav className="a3s-home-nav__inner" aria-label="Primary navigation">
        <Link className="a3s-home-brand" href={base || '/'} aria-label="A3S home">
          <A3SMark className="a3s-home-brand__mark" />
          <span>A3S</span>
          <small>LAB</small>
        </Link>

        <div className="a3s-home-nav__links">
          {anchorLinks.map((item) => (
            <a href={item.href} key={item.href}>
              {item.label}
            </a>
          ))}
          <Link href={`${base}/blog`}>{tr.blog}</Link>
        </div>

        <div className="a3s-home-nav__actions">
          <Link className="a3s-home-nav__language" href={languageHref} hrefLang={lang === 'cn' ? 'en' : 'zh-Hans'}>
            {tr.language}
          </Link>
          <Link className="a3s-home-nav__github" href="https://github.com/A3S-Lab/a3s" target="_blank" rel="noopener noreferrer" aria-label="A3S on GitHub">
            <Github aria-hidden="true" />
          </Link>
          <Link className="a3s-home-nav__docs" href={`${base}/docs`}>
            {tr.docs}
            <MoveUpRight aria-hidden="true" />
          </Link>
        </div>

        <details className="a3s-home-nav__mobile">
          <summary aria-label={tr.menu}>
            <Menu aria-hidden="true" />
          </summary>
          <div className="a3s-home-nav__mobile-panel">
            {anchorLinks.map((item) => (
              <a href={item.href} key={item.href}>
                {item.label}
              </a>
            ))}
            <Link href={`${base}/docs`}>{tr.docs}</Link>
            <Link href={`${base}/blog`}>{tr.blog}</Link>
            <Link href={languageHref}>{tr.language}</Link>
            <Link href="https://github.com/A3S-Lab/a3s" target="_blank" rel="noopener noreferrer">
              GitHub
            </Link>
          </div>
        </details>
      </nav>
    </header>
  );
}
