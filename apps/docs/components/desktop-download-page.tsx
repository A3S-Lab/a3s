import {
  AppleLogo,
  ArrowRight,
  DownloadSimple,
  GithubLogo,
  LinuxLogo,
  ShieldCheck,
  TerminalWindow,
  WindowsLogo,
} from '@phosphor-icons/react/dist/ssr';
import { withBase } from '@rspress/core/runtime';
import {
  desktopChecksumsUrl,
  desktopDownloadContent,
  desktopLatestReleaseUrl,
  desktopReleaseAssets,
  desktopReleasesUrl,
  desktopRepositoryUrl,
} from '@/components/download/download-content';
import { A3SMark } from '@/components/home/a3s-mark';
import { HomeNav } from '@/components/home/home-nav';
import type { Lang } from '@/components/home/home-content';

const platformIcons = {
  macos: AppleLogo,
  windows: WindowsLogo,
  linux: LinuxLogo,
} as const;

export default function DesktopDownloadPage({ lang = 'cn' }: { lang?: Lang }) {
  const tr = desktopDownloadContent[lang];

  return (
    <main className="a3s-site a3s-download-site">
      <a className="a3s-skip-link" href="#desktop-downloads">{tr.skip}</a>
      <HomeNav lang={lang} page="download" />

      <section className="a3s-download-hero" aria-labelledby="a3s-download-title">
        <div className="a3s-download-hero__inner">
          <div className="a3s-download-hero__copy">
            <div className="a3s-download-hero__product" aria-hidden="true">
              <A3SMark />
              <span>A3S</span>
              <small>DESKTOP</small>
            </div>
            <h1 id="a3s-download-title">{tr.title}</h1>
            <p>{tr.description}</p>
            <div className="a3s-download-hero__actions">
              <a
                className="a3s-button a3s-button--primary"
                href={desktopLatestReleaseUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                {tr.releaseAction}<ArrowRight aria-hidden="true" weight="bold" />
              </a>
              <a
                className="a3s-button a3s-button--secondary"
                href={desktopRepositoryUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                <GithubLogo aria-hidden="true" weight="fill" />{tr.sourceAction}
              </a>
            </div>
            <p className="a3s-download-hero__availability">{tr.availability}</p>
          </div>

          <div
            className="a3s-release-panel"
            id="desktop-downloads"
            aria-labelledby="a3s-release-panel-title"
          >
            <header>
              <div>
                <h2 id="a3s-release-panel-title">{tr.downloadHeading}</h2>
                <p>{tr.downloadDescription}</p>
              </div>
              <DownloadSimple aria-hidden="true" weight="duotone" />
            </header>

            <div className="a3s-release-panel__platforms">
              {desktopReleaseAssets.map((asset) => {
                const Icon = platformIcons[asset.id];
                return (
                  <a
                    href={asset.href}
                    key={asset.id}
                    aria-label={`${tr.downloadAction} A3S ${asset.name}`}
                  >
                    <Icon aria-hidden="true" weight="duotone" />
                    <span>
                      <strong>{asset.name}</strong>
                      <small>{asset.fileName}</small>
                    </span>
                    <b>{asset.format}</b>
                    <DownloadSimple aria-hidden="true" weight="bold" />
                  </a>
                );
              })}
            </div>

            <footer>
              <a href={desktopChecksumsUrl}>{tr.checksumAction}</a>
              <a
                href={desktopLatestReleaseUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                {tr.releaseNotesAction}
              </a>
            </footer>
          </div>
        </div>
      </section>

      <section
        className="a3s-download-setup a3s-section"
        aria-labelledby="a3s-download-setup-title"
      >
        <header>
          <h2 id="a3s-download-setup-title">{tr.requirementsTitle}</h2>
          <p>{tr.requirementsDescription}</p>
        </header>
        <ol>
          {tr.requirements.map(([title, description], index) => (
            <li key={title}>
              <span>{String(index + 1).padStart(2, '0')}</span>
              <div>
                <h3>{title}</h3>
                <p>{description}</p>
              </div>
            </li>
          ))}
        </ol>
        <a
          className="a3s-download-setup__link"
          href="https://github.com/A3S-Lab/a3s#quick-start"
          target="_blank"
          rel="noopener noreferrer"
        >
          <TerminalWindow aria-hidden="true" weight="duotone" />
          {tr.installCli}
          <ArrowRight aria-hidden="true" />
        </a>
      </section>

      <section className="a3s-download-trust" aria-labelledby="a3s-download-trust-title">
        <div className="a3s-download-trust__inner">
          <ShieldCheck aria-hidden="true" weight="duotone" />
          <div>
            <h2 id="a3s-download-trust-title">{tr.trustTitle}</h2>
            <p>{tr.trustDescription}</p>
          </div>
          <ul>
            {tr.trustPoints.map((point) => <li key={point}>{point}</li>)}
          </ul>
        </div>
      </section>

      <footer className="a3s-download-footer">
        <div>
          <a className="a3s-download-footer__brand" href={withBase('/')}>
            <A3SMark />
            <span>A3S</span>
          </a>
          <p>{tr.footerDescription}</p>
        </div>
        <nav aria-label={lang === 'cn' ? '下载页页脚' : 'Download page footer'}>
          <a href={withBase('/')}>{tr.footerHome}</a>
          <a href={desktopReleasesUrl} target="_blank" rel="noopener noreferrer">
            {tr.footerReleases}
          </a>
          <a href={desktopRepositoryUrl} target="_blank" rel="noopener noreferrer">
            {tr.footerSource}
          </a>
        </nav>
        <small>© {new Date().getFullYear()} A3S Lab · MIT</small>
      </footer>
    </main>
  );
}
