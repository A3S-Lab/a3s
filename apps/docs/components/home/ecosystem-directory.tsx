'use client';

import {
  ArrowUpRight,
  Browsers,
  Database,
  GithubLogo,
  MagnifyingGlass,
  Package,
  SquaresFour,
  Stack,
  type Icon,
} from '@phosphor-icons/react';
import { withBase } from '@rspress/core/runtime';
import { useMemo, useState } from 'react';
import {
  architectureProjects,
  type ArchitectureCategory,
  type ArchitectureProject,
} from '@/components/home/architecture';
import {
  deliveryStages,
  getDeliveryStageCopy,
  getProjectDeliveryStatus,
  statusVerifiedAt,
} from '@/components/home/ecosystem-status';
import type { Lang } from '@/components/home/home-content';
import { getProjectRepositoryHref } from '@/components/home/project-links';
import { featuredProjectSites, type FeaturedProjectSite } from '@/components/home/project-sites';

type DirectoryFilter = 'all' | ArchitectureCategory;

const categoryIcons: Record<ArchitectureCategory, Icon> = {
  products: Browsers,
  runtime: Database,
  interfaces: Stack,
};

const copy = {
  cn: {
    liveEyebrow: '公开项目页面 / 8',
    liveTitle: '看看已经可以访问的项目页面。',
    liveDescription: '这些入口包括产品页面和交互式 Playground。预览截图取自线上页面或当前构建。',
    live: '在线',
    buildPreview: '构建预览',
    openSite: '打开页面',
    openRepository: '查看代码',
    directoryEyebrow: '完整项目目录 / 36',
    directoryTitle: '36 个项目，一处查清。',
    directoryDescription: '按名称或职责搜索，也可以按层级筛选。每个条目都列出职责、主要能力、交付阶段、当前版本或通道，以及代码入口。',
    search: '搜索项目名称、职责或能力',
    result: '个项目',
    noResults: '没有项目符合当前筛选条件。',
    reset: '清除筛选',
    openGuide: '查看项目',
    repository: '代码',
    deliveryStatus: '交付阶段',
    stageGuide: '交付阶段说明',
    statusMethod: `这些阶段描述项目现在怎么用，不统计功能完成率。版本和阶段已于 ${statusVerifiedAt} 根据公开 Release、README 和 Roadmap 复核。`,
    categories: {
      all: '全部',
      products: '产品与应用',
      runtime: '运行与数据',
      interfaces: '服务与接口',
    },
  },
  en: {
    liveEyebrow: '8 PROJECT SITES',
    liveTitle: 'Open the projects that already have a public page.',
    liveDescription: 'These links go to product pages and interactive playgrounds. Previews come from the live page or its current build.',
    live: 'ONLINE',
    buildPreview: 'BUILD PREVIEW',
    openSite: 'Open site',
    openRepository: 'View source',
    directoryEyebrow: 'COMPLETE PROJECT DIRECTORY / 36',
    directoryTitle: 'One directory for all 36 projects.',
    directoryDescription: 'Search by name or responsibility, or filter by layer. Each entry shows its role, core capabilities, delivery stage, current version or channel, and source.',
    search: 'Search projects, responsibilities, or capabilities',
    result: 'projects',
    noResults: 'No projects match the current filters.',
    reset: 'Clear filters',
    openGuide: 'View project',
    repository: 'Source',
    deliveryStatus: 'Delivery stage',
    stageGuide: 'Delivery stage guide',
    statusMethod: `These stages describe how a project can be used today; they are not a feature-completion score. Versions and stages were checked against public releases, READMEs, and roadmaps on ${statusVerifiedAt}.`,
    categories: {
      all: 'All projects',
      products: 'Products & apps',
      runtime: 'Runtime & data',
      interfaces: 'Services & interfaces',
    },
  },
} as const;

function localizedHref(href: string, lang: Lang) {
  void lang;
  return href.startsWith('/') ? withBase(href) : href;
}

function sharedSiteHref(href: string, lang: Lang) {
  if (!href.startsWith('/')) return href;

  const localizedBase = withBase('/');
  const deploymentBase = lang === 'en' ? localizedBase.replace(/en\/$/, '') : localizedBase;
  return `${deploymentBase}${href.slice(1)}`;
}

function externalLinkProps(href: string) {
  return href.startsWith('http')
    ? { target: '_blank' as const, rel: 'noopener noreferrer' }
    : {};
}

function DeliveryStageGuide({ lang }: { lang: Lang }) {
  const tr = copy[lang];

  return (
    <div className="a3s-delivery-guide">
      <p>{tr.statusMethod}</p>
      <ul aria-label={tr.stageGuide}>
        {deliveryStages.map((stage) => {
          const stageDetails = getDeliveryStageCopy(stage, lang);

          return (
            <li data-stage={stage} key={stage}>
              <strong><i aria-hidden="true" />{stageDetails.label}</strong>
              <span>{stageDetails.description}</span>
            </li>
          );
        })}
      </ul>
    </div>
  );
}

function ProjectSitePreview({ project, site }: { project: ArchitectureProject; site: FeaturedProjectSite }) {
  return (
    <div className="a3s-site-preview" data-site={project.id} aria-hidden="true">
      <div className="a3s-site-preview__chrome">
        <span><i /><i /><i /></span>
        <code>{site.displayUrl}</code>
        <b />
      </div>
      <div className="a3s-site-preview__image">
        <img alt="" height="800" loading="lazy" src={withBase(site.screenshot)} width="1280" />
      </div>
    </div>
  );
}

function FeaturedSiteCard({ project, site, lang }: { project: ArchitectureProject; site: FeaturedProjectSite; lang: Lang }) {
  const tr = copy[lang];
  const href = sharedSiteHref(site.href, lang);

  return (
    <article className="a3s-site-card" data-preview-mode={site.mode} data-site={project.id}>
      <ProjectSitePreview project={project} site={site} />
      <div className="a3s-site-card__body">
        <div className="a3s-site-card__heading">
          <div>
            <span><i /> {site.mode === 'live' ? tr.live : tr.buildPreview}</span>
            <h3>A3S {project.name}</h3>
          </div>
          <SquaresFour aria-hidden="true" weight="duotone" />
        </div>
        <p>{project.role[lang]}</p>
        <a href={href} {...externalLinkProps(href)}>
          {site.destination === 'site' ? tr.openSite : tr.openRepository}
          <ArrowUpRight aria-hidden="true" weight="bold" />
        </a>
      </div>
    </article>
  );
}

function ProjectCard({ project, index, lang }: { project: ArchitectureProject; index: number; lang: Lang }) {
  const tr = copy[lang];
  const Icon = categoryIcons[project.category];
  const delivery = getProjectDeliveryStatus(project.id, lang);
  const featuredSite = featuredProjectSites.find((site) => (
    site.id === project.id && site.destination === 'site'
  ));
  const projectHref = featuredSite
    ? sharedSiteHref(featuredSite.href, lang)
    : localizedHref(project.href, lang);
  const repositoryHref = getProjectRepositoryHref(project.id);
  const hasDistinctRepository = repositoryHref !== projectHref;

  return (
    <article className="a3s-directory-card" data-category={project.category}>
      <div className="a3s-directory-card__topline">
        <span>{String(index + 1).padStart(2, '0')}</span>
        <span>{tr.categories[project.category]}</span>
        <Icon aria-hidden="true" weight="duotone" />
      </div>
      <div className="a3s-directory-card__title">
        <span>{project.name.slice(0, 2).toUpperCase()}</span>
        <h3>{project.name}</h3>
      </div>
      <p>{project.role[lang]}</p>
      <ul aria-label={`${project.name} capabilities`}>
        {project.nodes.slice(0, 3).map((node) => <li key={node.id}>{node.label}</li>)}
      </ul>
      <div className="a3s-project-delivery" data-stage={delivery.stage}>
        <span>{tr.deliveryStatus}</span>
        <div>
          <strong><i aria-hidden="true" />{delivery.label}</strong>
          <code>{delivery.release}</code>
        </div>
      </div>
      <div className="a3s-directory-card__actions">
        <a href={projectHref} {...externalLinkProps(projectHref)}>
          {tr.openGuide}
          <ArrowUpRight aria-hidden="true" />
        </a>
        {hasDistinctRepository ? (
          <a href={repositoryHref} target="_blank" rel="noopener noreferrer" aria-label={`${project.name} ${tr.repository}`}>
            <GithubLogo aria-hidden="true" weight="fill" />
            {tr.repository}
          </a>
        ) : null}
      </div>
    </article>
  );
}

export function EcosystemDirectory({ lang }: { lang: Lang }) {
  const tr = copy[lang];
  const [filter, setFilter] = useState<DirectoryFilter>('all');
  const [query, setQuery] = useState('');
  const featuredProjects = featuredProjectSites.map((site) => {
    const project = architectureProjects.find((candidate) => candidate.id === site.id);
    if (!project) throw new Error(`Featured ecosystem project is missing: ${site.id}`);
    return { project, site };
  });
  const filteredProjects = useMemo(() => {
    const normalizedQuery = query.trim().toLocaleLowerCase();

    return architectureProjects.filter((project) => {
      if (filter !== 'all' && project.category !== filter) return false;
      if (!normalizedQuery) return true;

      const searchable = [
        project.name,
        project.id,
        project.role.cn,
        project.role.en,
        ...project.nodes.flatMap((node) => [node.label, node.detail.cn, node.detail.en]),
      ].join(' ').toLocaleLowerCase();
      return searchable.includes(normalizedQuery);
    });
  }, [filter, query]);

  return (
    <div className="a3s-ecosystem-directory">
      <section className="a3s-live-sites" aria-labelledby="a3s-live-sites-title">
        <div className="a3s-directory-intro">
          <div>
            <span className="a3s-section-eyebrow">{tr.liveEyebrow}</span>
            <h3 id="a3s-live-sites-title">{tr.liveTitle}</h3>
          </div>
          <p>{tr.liveDescription}</p>
        </div>
        <div className="a3s-live-sites__grid">
          {featuredProjects.map(({ project, site }) => <FeaturedSiteCard key={project.id} project={project} site={site} lang={lang} />)}
        </div>
      </section>

      <section className="a3s-project-directory" aria-labelledby="a3s-project-directory-title">
        <div className="a3s-directory-intro">
          <div>
            <span className="a3s-section-eyebrow">{tr.directoryEyebrow}</span>
            <h3 id="a3s-project-directory-title">{tr.directoryTitle}</h3>
          </div>
          <p>{tr.directoryDescription}</p>
        </div>

        <div className="a3s-directory-toolbar">
          <div className="a3s-directory-filters" aria-label={tr.directoryTitle}>
            {(Object.keys(tr.categories) as DirectoryFilter[]).map((category) => {
              const count = category === 'all'
                ? architectureProjects.length
                : architectureProjects.filter((project) => project.category === category).length;

              return (
                <button
                  aria-pressed={filter === category}
                  className={filter === category ? 'is-active' : undefined}
                  key={category}
                  onClick={() => setFilter(category)}
                  type="button"
                >
                  <span>{tr.categories[category]}</span>
                  <b>{String(count).padStart(2, '0')}</b>
                </button>
              );
            })}
          </div>
          <label className="a3s-directory-search">
            <MagnifyingGlass aria-hidden="true" />
            <input aria-label={tr.search} onChange={(event) => setQuery(event.target.value)} placeholder={tr.search} type="search" value={query} />
            {query ? <button aria-label={tr.reset} onClick={() => setQuery('')} type="button">×</button> : null}
          </label>
        </div>

        <div className="a3s-directory-status" aria-live="polite">
          <span><i /> {String(filteredProjects.length).padStart(2, '0')} {tr.result}</span>
          <code>A3S / ECOSYSTEM.INDEX</code>
        </div>
        <DeliveryStageGuide lang={lang} />

        {filteredProjects.length > 0 ? (
          <div className="a3s-directory-grid">
            {filteredProjects.map((project) => (
              <ProjectCard
                index={architectureProjects.findIndex((candidate) => candidate.id === project.id)}
                key={project.id}
                lang={lang}
                project={project}
              />
            ))}
          </div>
        ) : (
          <div className="a3s-directory-empty">
            <Package aria-hidden="true" weight="duotone" />
            <p>{tr.noResults}</p>
            <button onClick={() => { setFilter('all'); setQuery(''); }} type="button">{tr.reset}</button>
          </div>
        )}
      </section>
    </div>
  );
}
