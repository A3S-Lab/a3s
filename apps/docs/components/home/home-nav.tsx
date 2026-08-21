import { GithubLogo, List } from '@phosphor-icons/react/dist/ssr';
import { withBase } from '@rspress/core/runtime';
import { A3SMark } from '@/components/home/a3s-mark';
import { homeContent, type Lang } from '@/components/home/home-content';

export function HomeNav({ lang }: { lang: Lang }) {
  const tr = homeContent[lang].nav;
  const homeHref = withBase('/');
  const rootHref = lang === 'en' ? homeHref.replace(/en\/$/, '') : homeHref;
  const languageHref = lang === 'cn' ? `${rootHref}en/` : rootHref;
  const powerHref = lang === 'cn' ? `${rootHref}power/` : `${rootHref}power/en/`;

  const anchorLinks = [
    { label: tr.ecosystem, href: '#ecosystem' },
    { label: tr.power, href: powerHref },
    { label: tr.principles, href: '#principles' },
  ];

  return (
    <header className="a3s-home-nav">
      <nav className="a3s-home-nav__inner" aria-label="Primary navigation">
        <a className="a3s-home-brand" href={homeHref} aria-label="A3S home">
          <A3SMark className="a3s-home-brand__mark" />
          <span>A3S</span>
          <small>LAB</small>
        </a>

        <div className="a3s-home-nav__links">
          {anchorLinks.map((item) => (
            <a href={item.href} key={item.href}>
              {item.label}
            </a>
          ))}
        </div>

        <div className="a3s-home-nav__actions">
          <a className="a3s-home-nav__language" href={languageHref} hrefLang={lang === 'cn' ? 'en' : 'zh-Hans'}>
            {tr.language}
          </a>
          <a className="a3s-home-nav__github" href="https://github.com/A3S-Lab/a3s" target="_blank" rel="noopener noreferrer" aria-label="A3S on GitHub">
            <GithubLogo aria-hidden="true" weight="fill" />
          </a>
        </div>

        <details className="a3s-home-nav__mobile">
          <summary aria-label={tr.menu}>
            <List aria-hidden="true" />
          </summary>
          <div className="a3s-home-nav__mobile-panel">
            {anchorLinks.map((item) => (
              <a href={item.href} key={item.href}>
                {item.label}
              </a>
            ))}
            <a href={languageHref}>{tr.language}</a>
            <a href="https://github.com/A3S-Lab/a3s" target="_blank" rel="noopener noreferrer">
              GitHub
            </a>
          </div>
        </details>
      </nav>
    </header>
  );
}
