import {
  AppleLogo,
  DownloadSimple,
  LinuxLogo,
  WindowsLogo,
} from "@phosphor-icons/react/dist/ssr";
import {
  desktopChecksumsUrl,
  desktopDownloadContent,
  desktopLatestReleaseUrl,
  desktopReleaseAssets,
  desktopRepositoryUrl,
} from "@/components/download/download-content";
import { A3SMark } from "@/components/home/a3s-mark";
import { HomeNav } from "@/components/home/home-nav";
import type { Lang } from "@/components/home/home-content";

const platformIcons = {
  macos: AppleLogo,
  windows: WindowsLogo,
  linux: LinuxLogo,
} as const;

export default function DesktopDownloadPage({ lang = "cn" }: { lang?: Lang }) {
  const tr = desktopDownloadContent[lang];

  return (
    <main className="a3s-site a3s-download-site">
      <a className="a3s-skip-link" href="#a3s-downloads">{tr.skip}</a>
      <HomeNav lang={lang} page="download" />

      <section
        className="a3s-download-main"
        id="a3s-downloads"
        aria-labelledby="a3s-download-title"
        tabIndex={-1}
      >
        <header className="a3s-download-intro">
          <div className="a3s-download-brand" aria-label="A3S">
            <A3SMark />
            <span>A3S</span>
          </div>
          <h1 id="a3s-download-title">{tr.title}</h1>
          <p>{tr.description}</p>
        </header>

        <div className="a3s-download-platforms">
          {desktopReleaseAssets.map((asset) => {
            const Icon = platformIcons[asset.id];
            return (
              <a
                href={asset.href}
                key={asset.id}
                aria-label={`${tr.downloadAction} A3S ${asset.name} ${asset.architecture}`}
              >
                <Icon aria-hidden="true" weight="duotone" />
                <span>
                  <strong>{asset.name}</strong>
                  <small>{asset.architecture} · {asset.format}</small>
                </span>
                <DownloadSimple aria-hidden="true" weight="bold" />
              </a>
            );
          })}
        </div>

        <footer className="a3s-download-meta">
          <p>{tr.availability}</p>
          <nav aria-label={lang === "cn" ? "下载资源" : "Download resources"}>
            <a
              href={desktopChecksumsUrl}
              target="_blank"
              rel="noopener noreferrer"
            >
              {tr.checksumAction}
            </a>
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
        </footer>
      </section>
    </main>
  );
}
