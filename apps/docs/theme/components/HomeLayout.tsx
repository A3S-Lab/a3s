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
          ? '为 Agent 时代构建 AI 原生操作系统，让贡献沉淀为晶体智力'
          : 'An AI-native operating system for fluid and crystal intelligence'}
      </h1>
      <p>
        {lang === 'cn'
          ? 'A3S OS 让流体智力在新问题中即时组合，并把经过验证的判断沉淀为团队可复用的晶体智力。'
          : 'A3S OS helps teams combine resources around new problems and turn verified judgment into reusable crystal intelligence.'}
      </p>
      <h2>{lang === 'cn' ? '为什么企业需要 A3S OS' : 'Why enterprises need A3S OS'}</h2>
      <p>
        {lang === 'cn'
          ? '企业需要一个带着上下文、证据、版本和归属持续生长的晶体智力大脑。'
          : 'Enterprises need a crystal intelligence brain that grows with context, evidence, versioning, and ownership.'}
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
