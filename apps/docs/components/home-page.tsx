import {
  ArrowRight,
  ArrowUpRight,
  Blocks,
  Box as BoxIcon,
  Braces,
  CheckCircle2,
  CircleDot,
  Gauge,
  Github,
  Layers3,
  PanelsTopLeft,
  ScanSearch,
  ShieldCheck,
  Terminal,
  type LucideIcon,
} from 'lucide-react';
import Link from 'next/link';
import { A3SMark } from '@/components/home/a3s-mark';
import { CanvasHero } from '@/components/home/canvas-hero';
import { CopyCommand } from '@/components/home/copy-command';
import { HomeNav } from '@/components/home/home-nav';
import { homeContent, type Lang, type ProductCopy, type ProductId } from '@/components/home/home-content';

const productIcons: Record<ProductId, LucideIcon> = {
  code: Braces,
  web: PanelsTopLeft,
  research: ScanSearch,
  use: Blocks,
  box: BoxIcon,
  bench: Gauge,
};

const signalIcons = [Terminal, ShieldCheck, Layers3, CheckCircle2];

function ProductCard({ product, action }: { product: ProductCopy; action: string }) {
  const Icon = productIcons[product.id];

  return (
    <Link
      className="a3s-product-card"
      data-product={product.id}
      href={product.href}
      target={product.external ? '_blank' : undefined}
      rel={product.external ? 'noopener noreferrer' : undefined}
    >
      <div className="a3s-product-card__topline">
        <span>{product.index}</span>
        <span>{product.eyebrow}</span>
        <ArrowUpRight aria-hidden="true" />
      </div>
      <div className="a3s-product-card__icon">
        <Icon aria-hidden="true" />
      </div>
      <h3>{product.name}</h3>
      <p>{product.description}</p>
      <div className="a3s-product-card__footer">
        <code>
          <span>$</span> {product.command}
        </code>
        <span className="a3s-product-card__action">{action}</span>
      </div>
    </Link>
  );
}

export default function HomePage({ lang = 'en' }: { lang?: Lang }) {
  const tr = homeContent[lang];
  const base = lang === 'cn' ? '/cn' : '';
  const products = tr.products.items as readonly ProductCopy[];

  return (
    <main className="a3s-site">
      <HomeNav lang={lang} />

      <CanvasHero>
        <section className="a3s-hero" aria-labelledby="a3s-hero-title">
          <div className="a3s-hero__ambient" aria-hidden="true" />
          <div className="a3s-hero__grid">
            <div className="a3s-hero__copy">
              <div className="a3s-eyebrow">
                <span />
                {tr.hero.eyebrow}
              </div>
              <h1 id="a3s-hero-title">
                <span>{tr.hero.lineOne}</span>
                <span>{tr.hero.lineTwo}</span>
                <em>{tr.hero.accent}</em>
              </h1>
              <p>{tr.hero.description}</p>
              <div className="a3s-hero__actions">
                <Link className="a3s-button a3s-button--primary" href={`${base}/docs`}>
                  {tr.hero.primaryAction}
                  <ArrowRight aria-hidden="true" />
                </Link>
                <a className="a3s-button a3s-button--ghost" href="#architecture">
                  <CircleDot aria-hidden="true" />
                  {tr.hero.secondaryAction}
                </a>
              </div>
              <div className="a3s-hero__command" aria-label="A3S launch command">
                <span>$</span>
                <code>a3s code</code>
                <i aria-hidden="true" />
              </div>
            </div>

            <div className="a3s-system-panel" aria-label="A3S governed execution topology">
              <div className="a3s-system-panel__chrome">
                <span><i /> <i /> <i /></span>
                <code>{tr.hero.terminalTitle}</code>
                <b>{tr.hero.terminalReady}</b>
              </div>
              <div className="a3s-system-panel__body">
                <div className="a3s-system-flow">
                  <div className="a3s-system-node a3s-system-node--intent">
                    <span>01</span>
                    <b>{tr.hero.intent}</b>
                    <small>prompt / goal / context</small>
                  </div>
                  <div className="a3s-system-connector"><i /><i /><i /></div>
                  <div className="a3s-system-node a3s-system-node--policy">
                    <span>02</span>
                    <b>{tr.hero.policy}</b>
                    <small>model · tools · permissions</small>
                  </div>
                  <div className="a3s-system-connector"><i /><i /><i /></div>
                  <div className="a3s-system-node a3s-system-node--runtime">
                    <span>03</span>
                    <b>{tr.hero.runtime}</b>
                    <small>process · container · MicroVM</small>
                  </div>
                </div>
                <div className="a3s-system-telemetry">
                  <div className="a3s-system-telemetry__head">
                    <span>LIVE TELEMETRY</span>
                    <span><i /> {tr.hero.status}</span>
                  </div>
                  {tr.hero.terminalRows.map(([key, value]) => (
                    <div className="a3s-system-telemetry__row" key={key}>
                      <code>{key}</code>
                      <span />
                      <b>{value}</b>
                    </div>
                  ))}
                </div>
              </div>
              <div className="a3s-system-panel__scan" aria-hidden="true" />
            </div>
          </div>
        </section>
      </CanvasHero>

      <section className="a3s-signal-strip" aria-label="A3S characteristics">
        <div>
          {tr.signal.map((item, index) => {
            const Icon = signalIcons[index];
            return (
              <span key={item}>
                <Icon aria-hidden="true" />
                {item}
              </span>
            );
          })}
        </div>
      </section>

      <section className="a3s-section a3s-products" id="products" aria-labelledby="a3s-products-title">
        <div className="a3s-section-heading">
          <div>
            <span className="a3s-section-eyebrow">{tr.products.eyebrow}</span>
            <h2 id="a3s-products-title">{tr.products.title}</h2>
          </div>
          <p>{tr.products.description}</p>
        </div>
        <div className="a3s-product-grid">
          {products.map((product) => (
            <ProductCard action={tr.products.action} key={product.id} product={product} />
          ))}
        </div>
      </section>

      <section className="a3s-architecture" id="architecture" aria-labelledby="a3s-architecture-title">
        <div className="a3s-section a3s-architecture__inner">
          <div className="a3s-architecture__copy">
            <span className="a3s-section-eyebrow">{tr.architecture.eyebrow}</span>
            <h2 id="a3s-architecture-title">{tr.architecture.title}</h2>
            <p>{tr.architecture.description}</p>
            <div className="a3s-architecture__legend">
              <span><i className="is-blue" /> policy</span>
              <span><i className="is-violet" /> contract</span>
              <span><i className="is-green" /> execution</span>
            </div>
          </div>

          <div className="a3s-stack-map">
            <div className="a3s-stack-map__rail a3s-stack-map__rail--top">
              <span>{tr.architecture.railTop}</span>
              <i />
            </div>
            {tr.architecture.layers.map((layer) => (
              <div className="a3s-stack-layer" key={layer.index}>
                <div className="a3s-stack-layer__label">
                  <span>{layer.index}</span>
                  <b>{layer.label}</b>
                </div>
                <div className="a3s-stack-layer__items">
                  {layer.items.map((item) => <span key={item}>{item}</span>)}
                </div>
              </div>
            ))}
            <div className="a3s-stack-map__rail a3s-stack-map__rail--bottom">
              <i />
              <span>{tr.architecture.railBottom}</span>
            </div>
          </div>
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
              <div>
                <span>{item.index}</span>
                <i />
              </div>
              <h3>{item.title}</h3>
              <p>{item.description}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="a3s-ecosystem" aria-labelledby="a3s-ecosystem-title">
        <div className="a3s-section a3s-ecosystem__inner">
          <div>
            <span className="a3s-section-eyebrow">{tr.ecosystem.eyebrow}</span>
            <h2 id="a3s-ecosystem-title">{tr.ecosystem.title}</h2>
          </div>
          <div className="a3s-module-field">
            {tr.ecosystem.modules.map((module, index) => (
              <span key={module} style={{ '--module-index': index } as React.CSSProperties}>
                <i /> {module}
              </span>
            ))}
          </div>
        </div>
      </section>

      <section className="a3s-section a3s-quickstart" id="quickstart" aria-labelledby="a3s-quickstart-title">
        <div className="a3s-quickstart__copy">
          <span className="a3s-section-eyebrow">{tr.quickstart.eyebrow}</span>
          <h2 id="a3s-quickstart-title">{tr.quickstart.title}</h2>
          <p>{tr.quickstart.description}</p>
          <Link href={`${base}/docs/cli`}>
            {tr.quickstart.docs}
            <ArrowRight aria-hidden="true" />
          </Link>
          <small>{tr.quickstart.note}</small>
        </div>
        <div className="a3s-terminal-card">
          <div className="a3s-terminal-card__bar">
            <span><i /><i /><i /></span>
            <code>~/workspace</code>
            <CopyCommand command={tr.quickstart.command} copyLabel={tr.quickstart.copy} copiedLabel={tr.quickstart.copied} />
          </div>
          <pre><code>{tr.quickstart.command}</code></pre>
          <div className="a3s-terminal-card__status">
            <span><i /> ready</span>
            <span>shell / zsh</span>
          </div>
        </div>
      </section>

      <section className="a3s-cta" aria-labelledby="a3s-cta-title">
        <div className="a3s-cta__grid" aria-hidden="true" />
        <div className="a3s-cta__mark" aria-hidden="true">
          <A3SMark />
        </div>
        <div className="a3s-cta__copy">
          <span className="a3s-section-eyebrow">{tr.cta.eyebrow}</span>
          <h2 id="a3s-cta-title">{tr.cta.title}</h2>
          <p>{tr.cta.description}</p>
          <div>
            <Link className="a3s-button a3s-button--light" href={`${base}/docs`}>
              {tr.cta.primary}
              <ArrowRight aria-hidden="true" />
            </Link>
            <Link className="a3s-button a3s-button--outline" href="https://github.com/A3S-Lab/a3s" target="_blank" rel="noopener noreferrer">
              <Github aria-hidden="true" />
              {tr.cta.secondary}
            </Link>
          </div>
        </div>
      </section>

      <footer className="a3s-footer">
        <div className="a3s-footer__inner">
          <div className="a3s-footer__brand">
            <Link href={base || '/'}>
              <A3SMark />
              <span>A3S</span>
            </Link>
            <p>{tr.footer.description}</p>
          </div>
          <div className="a3s-footer__column">
            <b>{tr.footer.resources}</b>
            <Link href={`${base}/docs`}>{tr.footer.docs}</Link>
            <Link href={`${base}/tutorials`}>{tr.footer.tutorials}</Link>
            <Link href={`${base}/blog`}>{tr.footer.blog}</Link>
          </div>
          <div className="a3s-footer__column">
            <b>{tr.footer.community}</b>
            <Link href="https://github.com/A3S-Lab" target="_blank" rel="noopener noreferrer">{tr.footer.github}</Link>
            <Link href="https://discord.gg/XVg6Hu6H" target="_blank" rel="noopener noreferrer">{tr.footer.discord}</Link>
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
