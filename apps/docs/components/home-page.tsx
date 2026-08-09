import {
  ArrowRight,
  Browsers,
  GithubLogo,
  SquaresFour,
  Stack,
} from '@phosphor-icons/react/dist/ssr';
import { withBase } from '@rspress/core/runtime';
import { A3SMark } from '@/components/home/a3s-mark';
import { EcosystemDirectory } from '@/components/home/ecosystem-directory';
import { GlobalWorkspaceScene } from '@/components/home/global-workspace-scene';
import { HomeNav } from '@/components/home/home-nav';
import { homeContent, type Lang } from '@/components/home/home-content';
import { InstallCommandTerminal } from '@/components/home/install-command-terminal';

const signalIcons = [SquaresFour, Browsers, Stack, GithubLogo];

export default function HomePage({ lang = 'cn' }: { lang?: Lang }) {
  const tr = homeContent[lang];

  return (
    <main className="a3s-site">
      <a className="a3s-skip-link" href="#main-content">
        {lang === 'cn' ? '跳到主要内容' : 'Skip to main content'}
      </a>
      <HomeNav lang={lang} />

      <section className="a3s-hero" id="main-content" aria-labelledby="a3s-hero-title">
        <div className="a3s-hero__ambient" aria-hidden="true" />
        <div className="a3s-hero__grid">
          <div className="a3s-hero__copy">
            <div className="a3s-eyebrow">{tr.hero.eyebrow}</div>
            <h1
              aria-label={`${tr.hero.lineOne} ${tr.hero.lineTwo} ${tr.hero.accent}`}
              id="a3s-hero-title"
            >
              <span>{tr.hero.lineOne}</span>
              <span>{tr.hero.lineTwo}{' '}<em>{tr.hero.accent}</em></span>
            </h1>
            <p>{tr.hero.description}</p>
            <div className="a3s-hero__actions">
              <a className="a3s-button a3s-button--primary" href="#ecosystem">
                {tr.hero.primaryAction}
                <ArrowRight aria-hidden="true" weight="bold" />
              </a>
              <a className="a3s-button a3s-button--ghost" href={withBase('/blog/')}>
                {tr.hero.secondaryAction}
              </a>
            </div>
          </div>
          <GlobalWorkspaceScene lang={lang} />
        </div>
      </section>

      <aside className="a3s-signal-strip" aria-label={lang === 'cn' ? 'A3S 生态概况' : 'A3S ecosystem overview'}>
        <div>
          {tr.signal.map((item, index) => {
            const Icon = signalIcons[index];
            return <span key={item}><Icon aria-hidden="true" weight="duotone" />{item}</span>;
          })}
        </div>
      </aside>

      <section className="a3s-ecosystem" id="ecosystem" aria-labelledby="a3s-ecosystem-title">
        <div className="a3s-section a3s-ecosystem__inner">
          <div className="a3s-section-heading">
            <div>
              <span className="a3s-section-eyebrow">{tr.ecosystem.eyebrow}</span>
              <h2 id="a3s-ecosystem-title">{tr.ecosystem.title}</h2>
            </div>
            <p>{tr.ecosystem.description}</p>
          </div>
          <EcosystemDirectory lang={lang} />
        </div>
      </section>

      <section className="a3s-section a3s-principles" id="principles" aria-labelledby="a3s-principles-title">
        <div className="a3s-section-heading">
          <div>
            <span className="a3s-section-eyebrow">{tr.principles.eyebrow}</span>
            <h2 id="a3s-principles-title">{tr.principles.title}</h2>
          </div>
          <p>{tr.principles.description}</p>
        </div>
        <div className="a3s-principle-grid">
          {tr.principles.items.map((item, index) => (
            <article className={index === 0 ? 'a3s-principle-card is-featured' : 'a3s-principle-card'} key={item.index}>
              <div><span>{item.index}</span><i /></div>
              <h3>{item.title}</h3>
              <p>{item.description}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="a3s-section a3s-quickstart" id="quickstart" aria-labelledby="a3s-quickstart-title">
        <div className="a3s-quickstart__copy">
          <span className="a3s-section-eyebrow">{tr.quickstart.eyebrow}</span>
          <h2 id="a3s-quickstart-title">{tr.quickstart.title}</h2>
          <p>{tr.quickstart.description}</p>
          <a href="https://github.com/A3S-Lab/a3s#quick-start" target="_blank" rel="noopener noreferrer">{tr.quickstart.docs}<ArrowRight aria-hidden="true" /></a>
          <small>{tr.quickstart.note}</small>
        </div>
        <InstallCommandTerminal lang={lang} />
      </section>

      <section className="a3s-cta" aria-labelledby="a3s-cta-title">
        <div className="a3s-cta__mark" aria-hidden="true"><A3SMark /></div>
        <div className="a3s-cta__copy">
          <span className="a3s-section-eyebrow">{tr.cta.eyebrow}</span>
          <h2 id="a3s-cta-title">{tr.cta.title}</h2>
          <p>{tr.cta.description}</p>
          <div>
            <a className="a3s-button a3s-button--light" href={withBase('/blog/')}>{tr.cta.primary}<ArrowRight aria-hidden="true" /></a>
            <a className="a3s-button a3s-button--outline" href="https://github.com/A3S-Lab/a3s" target="_blank" rel="noopener noreferrer">
              <GithubLogo aria-hidden="true" weight="fill" />{tr.cta.secondary}
            </a>
          </div>
        </div>
      </section>

      <footer className="a3s-footer">
        <div className="a3s-footer__inner">
          <div className="a3s-footer__brand">
            <a href={withBase('/')}><A3SMark /><span>A3S</span></a>
            <p>{tr.footer.description}</p>
          </div>
          <div className="a3s-footer__column">
            <b>{tr.footer.resources}</b>
            <a href="#ecosystem">{tr.footer.ecosystem}</a>
            <a href={withBase('/blog/')}>{tr.footer.blog}</a>
          </div>
          <div className="a3s-footer__column">
            <b>{tr.footer.community}</b>
            <a href="https://github.com/A3S-Lab" target="_blank" rel="noopener noreferrer">{tr.footer.github}</a>
            <a href="https://discord.gg/XVg6Hu6H" target="_blank" rel="noopener noreferrer">{tr.footer.discord}</a>
          </div>
        </div>
        <div className="a3s-footer__base">
          <span>© {new Date().getFullYear()} A3S Lab</span>
          <span>{tr.footer.license}</span>
          <span>RUST / ASYNC / OPEN</span>
        </div>
      </footer>
    </main>
  );
}
