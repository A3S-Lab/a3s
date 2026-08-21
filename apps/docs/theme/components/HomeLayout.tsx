import HomePage from '@/components/home-page';
import type { Lang } from '@/components/home/home-content';

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
          ? 'A3S 包含 34 个职责明确的项目，覆盖编码、浏览器操作、持久执行、数据、评测与基础设施。'
          : 'A3S includes 34 focused projects for coding, browser work, durable execution, data, evaluation, and infrastructure.'}
      </p>
      <h2>{lang === 'cn' ? '项目目录' : 'Project directory'}</h2>
      <p>{lang === 'cn' ? '每个项目都列出职责、交付阶段、当前版本或通道和代码入口。' : 'Every project lists its responsibility, delivery stage, current version or channel, and source.'}</p>
    </main>
  );
}

export function HomeLayout() {
  const lang: Lang = __A3S_SITE_LOCALE__;

  if (import.meta.env.SSG_MD) return <MarkdownHome lang={lang} />;

  return <HomePage lang={lang} />;
}
