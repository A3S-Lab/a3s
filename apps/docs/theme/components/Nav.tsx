import { useLang, withBase } from '@rspress/core/runtime';

function Mark() {
  return (
    <svg aria-hidden="true" viewBox="0 0 48 48">
      <path d="M25.2 4.8A19.2 19.2 0 1 0 42.1 33.2" />
      <path d="M15.2 31.8 24 12.7l8.8 19.1M18.8 24.2h10.4" />
      <path d="M6.8 31.6c6.7-4.8 11.6-4.5 17-.8 5.4 3.7 10.5 2.4 17.7-4.6" />
    </svg>
  );
}

function GitHubIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24">
      <path d="M12 .8a11.5 11.5 0 0 0-3.64 22.42c.58.1.79-.25.79-.56v-2.02c-3.22.7-3.9-1.37-3.9-1.37-.53-1.34-1.29-1.7-1.29-1.7-1.05-.72.08-.7.08-.7 1.16.08 1.78 1.2 1.78 1.2 1.03 1.77 2.7 1.26 3.36.96.1-.75.4-1.26.73-1.55-2.57-.29-5.27-1.28-5.27-5.68 0-1.26.45-2.28 1.19-3.09-.12-.29-.52-1.46.11-3.05 0 0 .97-.31 3.16 1.18a10.9 10.9 0 0 1 5.76 0c2.2-1.49 3.16-1.18 3.16-1.18.63 1.59.23 2.76.11 3.05.74.81 1.19 1.83 1.19 3.09 0 4.41-2.71 5.38-5.29 5.67.42.36.78 1.06.78 2.14v3.05c0 .31.21.67.8.56A11.5 11.5 0 0 0 12 .8Z" />
    </svg>
  );
}

export function Nav() {
  const locale = useLang() === 'en' ? 'en' : 'zh';
  const homeHref = withBase(locale === 'en' ? '/en/' : '/');
  const otherLocaleHref = withBase(locale === 'en' ? '/' : '/en/');

  return (
    <header className="a3s-cli-nav">
      <div className="a3s-cli-nav__inner">
        <a className="a3s-cli-brand" href={homeHref}>
          <Mark />
          <span>A3S</span>
          <small>CLI</small>
        </a>
        <nav
          className="a3s-cli-nav__links"
          aria-label={locale === 'zh' ? '主导航' : 'Primary navigation'}
        >
          <a href={`${homeHref}#commands`}>
            {locale === 'zh' ? '命令' : 'Commands'}
          </a>
          <a href={`${homeHref}#components`}>
            {locale === 'zh' ? '组件' : 'Components'}
          </a>
          <a href={`${homeHref}#install`}>
            {locale === 'zh' ? '安装' : 'Install'}
          </a>
        </nav>
        <div className="a3s-cli-nav__actions">
          <a
            className="a3s-cli-nav__language"
            href={otherLocaleHref}
            hrefLang={locale === 'zh' ? 'en' : 'zh-CN'}
          >
            {locale === 'zh' ? 'EN' : '中文'}
          </a>
          <a
            className="a3s-cli-nav__github"
            href="https://github.com/A3S-Lab/a3s"
            aria-label="A3S on GitHub"
          >
            <GitHubIcon />
          </a>
        </div>
      </div>
    </header>
  );
}
