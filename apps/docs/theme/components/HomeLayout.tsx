import HomePage from '@/components/home-page';
import type { Lang } from '@/components/home/home-content';

function MarkdownHome({ lang }: { lang: Lang }) {
  return (
    <main>
      <h1>{lang === 'cn' ? '为 AI Native 组织构建的生态系统' : 'The ecosystem for AI Native organizations'}</h1>
      <p>
        {lang === 'cn'
          ? 'A3S 由 36 个项目组成，覆盖 Agent 产品、运行时、数据、协议与基础设施。'
          : 'A3S consists of 36 projects across agent products, runtimes, data, protocols, and infrastructure.'}
      </p>
      <h2>{lang === 'cn' ? '项目目录' : 'Project directory'}</h2>
      <p>{lang === 'cn' ? '每个项目都提供职责、开发阶段、网站与仓库入口。' : 'Every project lists its responsibility, delivery stage, site, and repository.'}</p>
      <h2>{lang === 'cn' ? '工程博客' : 'Engineering blog'}</h2>
      <p>{lang === 'cn' ? '博客记录 A3S 的架构、运行时与工程实践。' : 'The blog covers A3S architecture, runtimes, and engineering practice.'}</p>
    </main>
  );
}

export function HomeLayout() {
  const lang: Lang = __A3S_SITE_LOCALE__;

  if (import.meta.env.SSG_MD) return <MarkdownHome lang={lang} />;

  return <HomePage lang={lang} />;
}
