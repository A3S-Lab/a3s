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
import { getProjectProgress } from '@/components/home/ecosystem-progress';
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
    liveEyebrow: '项目网站 / 实时入口',
    liveTitle: '先看产品页面，再看代码。',
    liveDescription: '这里展示已经上线的独立网站、交互式 Playground 和博客入口。截图来自真实页面。',
    live: 'LIVE',
    openSite: '访问网站',
    directoryEyebrow: '完整项目目录 / 35',
    directoryTitle: '35 个项目，各自负责什么。',
    directoryDescription: '按层级筛选或直接搜索。每张卡列出项目职责、主要能力、开发阶段和代码入口。',
    search: '搜索项目、职责或能力',
    result: '个项目',
    noResults: '没有匹配的项目。',
    reset: '查看全部项目',
    openGuide: '打开项目',
    repository: 'GitHub',
    progress: '开发进度',
    progressMethod: '进度按公开交付阶段映射：开发中 40% · 实验 60% · 预览 80% · 已发布 100%，不代表功能数量完成率。',
    categories: {
      all: '全部',
      products: '产品与应用',
      runtime: '运行与数据',
      interfaces: '服务与接口',
    },
  },
  en: {
    liveEyebrow: 'PROJECT SITES / LIVE DESTINATIONS',
    liveTitle: 'See the product. Open the code when needed.',
    liveDescription: 'These are live project sites, interactive playgrounds, and the engineering blog. Every image is captured from the real page.',
    live: 'LIVE',
    openSite: 'Visit site',
    directoryEyebrow: 'COMPLETE PROJECT DIRECTORY / 35',
    directoryTitle: 'What each of the 35 projects owns.',
    directoryDescription: 'Filter by layer or search directly. Each card lists ownership, core capabilities, delivery stage, and source code.',
    search: 'Search projects, responsibilities, or capabilities',
    result: 'projects',
    noResults: 'No projects match this search.',
    reset: 'View every project',
    openGuide: 'Open project',
    repository: 'GitHub',
    progress: 'Development progress',
    progressMethod: 'Bars map public delivery stages: Building 40% · Experimental 60% · Preview 80% · Released 100%. They do not claim feature-count completion.',
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

function externalLinkProps(href: string) {
  return href.startsWith('http')
    ? { target: '_blank' as const, rel: 'noopener noreferrer' }
    : {};
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
  const href = localizedHref(site.href, lang);

  return (
    <article className="a3s-site-card" data-site={project.id}>
      <ProjectSitePreview project={project} site={site} />
      <div className="a3s-site-card__body">
        <div className="a3s-site-card__heading">
          <div>
            <span><i /> {tr.live}</span>
            <h3>A3S {project.name}</h3>
          </div>
          <SquaresFour aria-hidden="true" weight="duotone" />
        </div>
        <p>{project.role[lang]}</p>
        <a href={href} {...externalLinkProps(href)}>
          {tr.openSite}
          <ArrowUpRight aria-hidden="true" weight="bold" />
        </a>
      </div>
    </article>
  );
}

function ProjectCard({ project, index, lang }: { project: ArchitectureProject; index: number; lang: Lang }) {
  const tr = copy[lang];
  const Icon = categoryIcons[project.category];
  const progress = getProjectProgress(project.id, lang);
  const projectHref = localizedHref(project.href, lang);
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
      <div className="a3s-project-progress" data-stage={progress.stage}>
        <div>
          <span>{tr.progress} · {progress.label}</span>
          <b>{progress.value}%</b>
        </div>
        <span
          aria-label={`${tr.progress}: ${progress.label}, ${progress.value}%`}
          aria-valuemax={100}
          aria-valuemin={0}
          aria-valuenow={progress.value}
          role="progressbar"
        >
          <i style={{ '--project-progress': `${progress.value}%` } as React.CSSProperties} />
        </span>
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
            <span className="sr-only">{tr.search}</span>
            <input onChange={(event) => setQuery(event.target.value)} placeholder={tr.search} type="search" value={query} />
            {query ? <button aria-label={tr.reset} onClick={() => setQuery('')} type="button">×</button> : null}
          </label>
        </div>

        <div className="a3s-directory-status" aria-live="polite">
          <span><i /> {String(filteredProjects.length).padStart(2, '0')} {tr.result}</span>
          <code>A3S / ECOSYSTEM.INDEX</code>
        </div>
        <p className="a3s-progress-method">{tr.progressMethod}</p>

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
