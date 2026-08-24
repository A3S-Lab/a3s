import {
  AppleLogo,
  ArrowRight,
  DownloadSimple,
  LinuxLogo,
  WindowsLogo,
} from '@phosphor-icons/react/dist/ssr';
import {
  desktopChecksumsUrl,
  desktopDownloadContent,
  desktopLatestReleaseUrl,
  desktopReleaseAssets,
  desktopRepositoryUrl,
} from '@/components/download/download-content';
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

      <section
        className="a3s-download"
        id="desktop-downloads"
        aria-labelledby="a3s-download-title"
      >
        <div className="a3s-download__inner">
          <header className="a3s-download__header">
            <h1 id="a3s-download-title">{tr.title}</h1>
            <p>{tr.description}</p>
          </header>

          <div
            className="a3s-download__platforms"
            aria-label={lang === 'cn' ? '选择下载平台' : 'Choose a download platform'}
          >
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
                    <small>{asset.fileName} · {asset.format}</small>
                  </span>
                  <b>
                    {tr.downloadAction}
                    <DownloadSimple aria-hidden="true" weight="bold" />
                  </b>
                </a>
              );
            })}
          </div>

          <div className="a3s-download__meta">
            <p>{tr.previewNote}</p>
            <nav aria-label={lang === 'cn' ? '下载资源' : 'Download resources'}>
              <a href={desktopChecksumsUrl}>{tr.checksumAction}</a>
              <a
                href={desktopLatestReleaseUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                {tr.releaseNotesAction}
              </a>
              <a
                href={desktopRepositoryUrl}
                target="_blank"
                rel="noopener noreferrer"
              >
                {tr.sourceAction}
              </a>
            </nav>
          </div>

          <aside className="a3s-download__cli">
            <div>
              <strong>{tr.cliTitle}</strong>
              <p>{tr.cliDescription}</p>
            </div>
            <a
              href="https://github.com/A3S-Lab/a3s#quick-start"
              target="_blank"
              rel="noopener noreferrer"
            >
              {tr.installCli}
              <ArrowRight aria-hidden="true" weight="bold" />
            </a>
          </aside>
        </div>
      </section>
    </main>
  );
}
