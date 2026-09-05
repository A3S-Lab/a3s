import HomePage from '@/components/home-page';
import DesktopDownloadPage from '@/components/desktop-download-page';
import { desktopDownloadContent } from '@/components/download/download-content';
import type { Lang } from '@/components/home/home-content';
import { usePageData } from '@rspress/core/runtime';

function MarkdownHome({ lang }: { lang: Lang }) {
  return (
    <main>
      <h1>
        {lang === 'cn'
          ? '为AI Native组织构建的AI操作系统生态'
          : 'The AI operating system ecosystem built for AI Native organizations'}
      </h1>
      <p>
        {lang === 'cn'
          ? 'A3S 包含 35 个职责明确的项目，覆盖编码、浏览器操作、持久执行、数据、评测与基础设施。'
          : 'A3S includes 35 focused projects for coding, browser work, durable execution, data, evaluation, and infrastructure.'}
      </p>
      <h2>{lang === 'cn' ? '项目目录' : 'Project directory'}</h2>
      <p>{lang === 'cn' ? '每个项目都列出职责、交付阶段、当前版本或通道和代码入口。' : 'Every project lists its responsibility, delivery stage, current version or channel, and source.'}</p>
    </main>
  );
}

function MarkdownDownload({ lang }: { lang: Lang }) {
  const tr = desktopDownloadContent[lang];

  return (
    <main>
      <h1>{tr.title}</h1>
      <p>{tr.description}</p>
    </main>
  );
}

export function HomeLayout() {
  const lang: Lang = __A3S_SITE_LOCALE__;
  const { page } = usePageData();
  const routePath = page.routePath.replace(/\/+$/, '') || '/';
  const isDownloadPage = routePath === '/download';

  if (import.meta.env.SSG_MD) {
    return isDownloadPage ? <MarkdownDownload lang={lang} /> : <MarkdownHome lang={lang} />;
  }

  return isDownloadPage ? <DesktopDownloadPage lang={lang} /> : <HomePage lang={lang} />;
}
