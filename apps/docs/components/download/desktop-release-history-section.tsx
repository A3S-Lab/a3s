import {
  ArrowUpRight,
  CaretDown,
  DownloadSimple,
} from '@phosphor-icons/react/dist/ssr';
import {
  desktopDownloadContent,
  desktopReleasesUrl,
} from '@/components/download/download-content';
import { desktopReleaseHistory } from '@/components/download/desktop-release-history';
import type { Lang } from '@/components/home/home-content';

const releaseDateFormatters = {
  cn: new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: 'long',
    day: 'numeric',
    timeZone: 'UTC',
  }),
  en: new Intl.DateTimeFormat('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    timeZone: 'UTC',
  }),
} as const;

function formatReleaseDate(date: string, lang: Lang) {
  return releaseDateFormatters[lang].format(new Date(`${date}T00:00:00Z`));
}

export function DesktopReleaseHistory({ lang }: { lang: Lang }) {
  const tr = desktopDownloadContent[lang];

  return (
    <section
      className="a3s-download-history"
      id="desktop-release-history"
      aria-labelledby="a3s-download-history-title"
    >
      <div className="a3s-download-history__inner">
        <header className="a3s-download-history__header">
          <div>
            <h2 id="a3s-download-history-title">{tr.historyTitle}</h2>
            <p>{tr.historyDescription}</p>
          </div>
          <a href={desktopReleasesUrl} target="_blank" rel="noopener noreferrer">
            {tr.allReleasesAction}
            <ArrowUpRight aria-hidden="true" weight="bold" />
          </a>
        </header>

        <div className="a3s-release-history-list">
          {desktopReleaseHistory.map((release, index) => (
            <details key={release.version} open={index === 0}>
              <summary>
                <span className="a3s-release-history-list__version">
                  <strong>{release.version}</strong>
                  {index === 0 ? <small>{tr.latestLabel}</small> : null}
                </span>
                <time dateTime={release.publishedAt}>
                  {formatReleaseDate(release.publishedAt, lang)}
                </time>
                <CaretDown aria-hidden="true" weight="bold" />
              </summary>

              <div className="a3s-release-history-list__body">
                <div className="a3s-release-history-list__notes">
                  <h3>{tr.releaseChangesTitle}</h3>
                  <ul>
                    {release.notes[lang].map((note) => <li key={note}>{note}</li>)}
                  </ul>
                  <a href={release.releaseUrl} target="_blank" rel="noopener noreferrer">
                    {tr.releasePageAction}
                    <ArrowUpRight aria-hidden="true" weight="bold" />
                  </a>
                </div>

                <div className="a3s-release-history-list__downloads">
                  <h3>{tr.releaseDownloadsTitle}</h3>
                  <nav
                    aria-label={lang === 'cn'
                      ? `${release.version} 版本下载`
                      : `${release.version} downloads`}
                  >
                    {release.assets.map((asset) => (
                      <a href={asset.href} key={asset.id}>
                        <span>
                          <strong>{asset.name}</strong>
                          <small>{asset.format}</small>
                        </span>
                        <DownloadSimple aria-hidden="true" weight="bold" />
                      </a>
                    ))}
                  </nav>
                  <a className="a3s-release-history-list__checksum" href={release.checksumUrl}>
                    {tr.checksumAction}
                  </a>
                </div>
              </div>
            </details>
          ))}
        </div>
      </div>
    </section>
  );
}
