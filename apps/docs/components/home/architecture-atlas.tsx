'use client';

import { ArrowUpRight, MagnifyingGlass, Network } from '@phosphor-icons/react';
import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';
import {
  architectureProjectCount,
  architectureProjects,
  systemArchitectureProject,
  type ArchitectureCategory,
  type ArchitectureProject,
  type ArchitectureTone,
} from '@/components/home/architecture';
import type { Lang } from '@/components/home/home-content';

type AtlasCategory = 'all' | ArchitectureCategory;

const nodePositions = [
  { x: 50, y: 12 },
  { x: 86, y: 36 },
  { x: 72, y: 82 },
  { x: 28, y: 82 },
  { x: 14, y: 36 },
] as const;

const categoryOrder: readonly AtlasCategory[] = ['all', 'products', 'runtime', 'interfaces'];

const projectNumbers = new Map(
  architectureProjects.map((project, index) => [project.id, String(index + 1).padStart(2, '0')]),
);

const atlasCopy = {
  cn: {
    categories: {
      all: '全栈总览',
      products: '产品与应用',
      runtime: '运行与数据',
      interfaces: '服务与接口',
    },
    projectIndex: '项目索引',
    systemLabel: '共享契约平面',
    search: '搜索 35 个项目',
    noResults: '没有匹配的项目',
    openProject: '打开项目',
    topology: '交互拓扑',
    nodeHint: '悬停、聚焦或点击节点，查看它在架构中的职责。',
    selectedNode: '当前节点',
    footer: '35 个项目 · 175 个架构节点 · 一套可检查的能力图谱',
    tones: {
      surface: '入口',
      core: '核心',
      contract: '契约',
      runtime: '执行',
      evidence: '证据',
    },
  },
  en: {
    categories: {
      all: 'System overview',
      products: 'Products & apps',
      runtime: 'Runtime & data',
      interfaces: 'Services & interfaces',
    },
    projectIndex: 'Project index',
    systemLabel: 'Shared contract plane',
    search: 'Search 35 projects',
    noResults: 'No matching projects',
    openProject: 'Open project',
    topology: 'Interactive topology',
    nodeHint: 'Hover, focus, or select a node to inspect its architectural responsibility.',
    selectedNode: 'Selected node',
    footer: '35 projects · 175 architecture nodes · one inspectable capability atlas',
    tones: {
      surface: 'surface',
      core: 'core',
      contract: 'contract',
      runtime: 'runtime',
      evidence: 'evidence',
    },
  },
} as const;

function projectLinks(project: ArchitectureProject): ReadonlyArray<readonly [string, string]> {
  if (project.links) return project.links;

  return project.nodes.slice(0, -1).map((node, index) => [node.id, project.nodes[index + 1].id] as const);
}

function projectHref(project: ArchitectureProject, lang: Lang): string {
  void lang;
  return project.href;
}

export function ArchitectureAtlas({ lang }: { lang: Lang }) {
  const copy = atlasCopy[lang];
  const [activeCategory, setActiveCategory] = useState<AtlasCategory>('all');
  const [activeProjectId, setActiveProjectId] = useState(systemArchitectureProject.id);
  const [activeNodeId, setActiveNodeId] = useState(systemArchitectureProject.nodes[0].id);
  const [query, setQuery] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      const target = event.target;
      const editing =
        target instanceof HTMLElement &&
        (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));

      if (event.key === '/' && !editing) {
        event.preventDefault();
        searchRef.current?.focus();
      }
    }

    window.addEventListener('keydown', focusSearch);
    return () => window.removeEventListener('keydown', focusSearch);
  }, []);

  const activeProject =
    activeProjectId === systemArchitectureProject.id
      ? systemArchitectureProject
      : architectureProjects.find((project) => project.id === activeProjectId) ?? systemArchitectureProject;
  const activeNode = activeProject.nodes.find((node) => node.id === activeNodeId) ?? activeProject.nodes[0];
  const activeNodeIndex = activeProject.nodes.findIndex((node) => node.id === activeNode.id);
  const links = projectLinks(activeProject);

  const visibleProjects = useMemo(() => {
    const projects: readonly ArchitectureProject[] =
      activeCategory === 'all'
        ? [systemArchitectureProject, ...architectureProjects]
        : architectureProjects.filter((project) => project.category === activeCategory);
    const normalizedQuery = query.trim().toLocaleLowerCase();

    if (!normalizedQuery) return projects;

    return projects.filter((project) => {
      const searchable = `${project.name} ${project.id} ${project.role.cn} ${project.role.en}`.toLocaleLowerCase();
      return searchable.includes(normalizedQuery);
    });
  }, [activeCategory, query]);
  const visibleSubprojectCount = visibleProjects.filter(
    (project) => project.id !== systemArchitectureProject.id,
  ).length;
  const visibleCountLabel =
    visibleSubprojectCount === 0 && visibleProjects.some((project) => project.id === systemArchitectureProject.id)
      ? 'SYS'
      : `${String(visibleSubprojectCount).padStart(2, '0')} / ${architectureProjectCount}`;

  function selectProject(project: ArchitectureProject) {
    setActiveProjectId(project.id);
    setActiveNodeId(project.nodes[0].id);
  }

  function selectCategory(category: AtlasCategory) {
    setActiveCategory(category);
    setQuery('');
    const project =
      category === 'all'
        ? systemArchitectureProject
        : architectureProjects.find((candidate) => candidate.category === category) ?? systemArchitectureProject;
    selectProject(project);
  }

  const externalProject = !activeProject.href.startsWith('/');

  return (
    <div className="a3s-atlas">
      <div className="a3s-atlas__toolbar">
        <div className="a3s-atlas__categories" role="tablist" aria-label={copy.projectIndex}>
          {categoryOrder.map((category) => {
            const count =
              category === 'all'
                ? architectureProjectCount
                : architectureProjects.filter((project) => project.category === category).length;

            return (
              <button
                aria-controls="a3s-atlas-projects"
                aria-selected={activeCategory === category}
                className={activeCategory === category ? 'is-active' : undefined}
                key={category}
                onClick={() => selectCategory(category)}
                role="tab"
                type="button"
              >
                <span>{copy.categories[category]}</span>
                <b>{String(count).padStart(2, '0')}</b>
              </button>
            );
          })}
        </div>

        <label className="a3s-atlas__search">
          <MagnifyingGlass aria-hidden="true" />
          <span className="sr-only">{copy.search}</span>
          <input
            aria-keyshortcuts="/"
            onChange={(event) => {
              setQuery(event.target.value);
              if (event.target.value.trim()) setActiveCategory('all');
            }}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                setQuery('');
                event.currentTarget.blur();
              }
            }}
            placeholder={copy.search}
            ref={searchRef}
            type="search"
            value={query}
          />
          <kbd>/</kbd>
        </label>
      </div>

      <div className="a3s-atlas__workspace">
        <aside className="a3s-atlas__rail" aria-label={copy.projectIndex}>
          <div className="a3s-atlas__rail-head">
            <span>{copy.projectIndex}</span>
            <code>{visibleCountLabel}</code>
          </div>
          <div className="a3s-atlas__project-list" id="a3s-atlas-projects" role="tabpanel">
            {visibleProjects.map((project) => {
              const active = activeProject.id === project.id;
              const number = project.id === systemArchitectureProject.id ? 'SYS' : projectNumbers.get(project.id);

              return (
                <button
                  aria-pressed={active}
                  className={active ? 'is-active' : undefined}
                  data-category={project.category}
                  key={project.id}
                  onClick={() => selectProject(project)}
                  type="button"
                >
                  <span>{number}</span>
                  <span>
                    <b>{project.name}</b>
                    <small>{project.role[lang]}</small>
                  </span>
                  <i aria-hidden="true" />
                </button>
              );
            })}
            {visibleProjects.length === 0 ? <p>{copy.noResults}</p> : null}
          </div>
        </aside>

        <section className="a3s-atlas__diagram" aria-label={`${activeProject.name} ${copy.topology}`}>
          <header className="a3s-atlas__diagram-head">
            <div>
              <span><i /> {copy.topology}</span>
              <h3>{activeProject.name}<em>.architecture</em></h3>
              <p>{activeProject.role[lang]}</p>
            </div>
            <a
              href={projectHref(activeProject, lang)}
              rel={externalProject ? 'noopener noreferrer' : undefined}
              target={externalProject ? '_blank' : undefined}
            >
              {copy.openProject}
              <ArrowUpRight aria-hidden="true" />
            </a>
          </header>

          <div className="a3s-atlas__stage">
            <div className="a3s-atlas__orbit a3s-atlas__orbit--outer" aria-hidden="true" />
            <div className="a3s-atlas__orbit a3s-atlas__orbit--inner" aria-hidden="true" />
            <svg aria-hidden="true" className="a3s-atlas__wires" preserveAspectRatio="none" viewBox="0 0 100 100">
              <defs>
                <marker id="a3s-atlas-arrow" markerHeight="5" markerWidth="5" orient="auto" refX="4" refY="2.5">
                  <path d="M0,0 L5,2.5 L0,5 Z" />
                </marker>
              </defs>
              {activeProject.nodes.map((node, index) => {
                const position = nodePositions[index];
                return (
                  <line
                    className={node.id === activeNode.id ? 'a3s-atlas__hub-wire is-active' : 'a3s-atlas__hub-wire'}
                    key={`hub-${node.id}`}
                    vectorEffect="non-scaling-stroke"
                    x1="50"
                    x2={position.x}
                    y1="50"
                    y2={position.y}
                  />
                );
              })}
              {links.map(([from, to]) => {
                const fromIndex = activeProject.nodes.findIndex((node) => node.id === from);
                const toIndex = activeProject.nodes.findIndex((node) => node.id === to);
                const fromPosition = nodePositions[fromIndex];
                const toPosition = nodePositions[toIndex];
                const active = from === activeNode.id || to === activeNode.id;

                return (
                  <line
                    className={active ? 'a3s-atlas__flow-wire is-active' : 'a3s-atlas__flow-wire'}
                    key={`${from}-${to}`}
                    markerEnd="url(#a3s-atlas-arrow)"
                    vectorEffect="non-scaling-stroke"
                    x1={fromPosition.x}
                    x2={toPosition.x}
                    y1={fromPosition.y}
                    y2={toPosition.y}
                  />
                );
              })}
            </svg>

            <div className="a3s-atlas__core">
              <span><Network aria-hidden="true" /></span>
              <b>{activeProject.name}</b>
              <small>{copy.systemLabel}</small>
            </div>

            {activeProject.nodes.map((node, index) => {
              const position = nodePositions[index];
              const style = { '--node-x': `${position.x}%`, '--node-y': `${position.y}%` } as CSSProperties;

              return (
                <button
                  aria-label={`${node.label}: ${node.detail[lang]}`}
                  aria-pressed={activeNode.id === node.id}
                  className={activeNode.id === node.id ? 'a3s-atlas__node is-active' : 'a3s-atlas__node'}
                  data-tone={node.tone}
                  key={node.id}
                  onClick={() => setActiveNodeId(node.id)}
                  onFocus={() => setActiveNodeId(node.id)}
                  onMouseEnter={() => setActiveNodeId(node.id)}
                  style={style}
                  type="button"
                >
                  <span>{String(index + 1).padStart(2, '0')}</span>
                  <i aria-hidden="true" />
                  <b>{node.label}</b>
                </button>
              );
            })}
          </div>

          <div className="a3s-atlas__node-detail" data-tone={activeNode.tone}>
            <div>
              <span>{copy.selectedNode} / {String(activeNodeIndex + 1).padStart(2, '0')}</span>
              <b>{copy.tones[activeNode.tone as ArchitectureTone]}</b>
            </div>
            <h4>{activeNode.label}</h4>
            <p>{activeNode.detail[lang]}</p>
            <small>{copy.nodeHint}</small>
          </div>
        </section>
      </div>

      <div className="a3s-atlas__footer">
        <span><i /> {copy.footer}</span>
        <code>ONE SHARED CONTRACT SURFACE</code>
      </div>
    </div>
  );
}
