'use client';

import {
  ArrowLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowUpRight,
  Search,
} from 'lucide-react';
import Link from 'next/link';
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
} from 'react';
import {
  architectureNodeCount,
  architectureProjectCount,
  architectureProjects,
  systemArchitectureProject,
  type ArchitectureEdge,
  type ArchitectureNodeKind,
  type ArchitectureProject,
} from '@/components/home/architecture';
import {
  filterArchitectureProjects,
  replacementArchitectureProject,
  type ArchitectureAtlasCategory,
} from '@/components/home/architecture/architecture-filter';
import type { Lang } from '@/components/home/home-content';
import { localePath } from '@/lib/i18n';

const categoryOrder: readonly ArchitectureAtlasCategory[] = [
  'all',
  'products',
  'runtime',
  'interfaces',
];

const projectNumbers = new Map(
  architectureProjects.map((project, index) => [
    project.id,
    String(index + 1).padStart(2, '0'),
  ]),
);

const architectureEdgeCount = architectureProjects.reduce(
  (count, project) => count + project.edges.length,
  0,
);

const atlasCopy = {
  cn: {
    categories: {
      all: '全部',
      products: '产品与应用',
      runtime: '运行时与数据',
      interfaces: '服务与工具',
    },
    projectIndex: '项目',
    search: '搜索项目或组件',
    noResults: '没有找到匹配项',
    openProject: '打开项目',
    structure: '项目结构',
    diagramCanvas: '架构图，可横向滚动查看完整结构',
    source: '架构依据',
    nodes: '节点',
    relations: '关系',
    selectedNode: '当前节点',
    connected: '直接连接',
    noConnections: '没有直接连接',
    nodeHint: '移动到节点上可高亮它的直接依赖；点击关系可跳到另一端。',
    footer: `${architectureProjectCount} 个项目 · ${architectureNodeCount} 个结构节点 · ${architectureEdgeCount} 条明确关系`,
    kinds: {
      entry: '入口',
      process: '处理',
      control: '控制',
      service: '服务',
      adapter: '适配',
      runtime: '执行',
      store: '存储',
      security: '校验 / 安全',
      output: '输出',
    },
  },
  en: {
    categories: {
      all: 'All',
      products: 'Products & apps',
      runtime: 'Runtime & data',
      interfaces: 'Services & tools',
    },
    projectIndex: 'Projects',
    search: 'Search projects or components',
    noResults: 'No matching project or component',
    openProject: 'Open project',
    structure: 'Project structure',
    diagramCanvas:
      'architecture diagram; scroll horizontally to inspect the full structure',
    source: 'Architecture sources',
    nodes: 'nodes',
    relations: 'relations',
    selectedNode: 'Selected node',
    connected: 'Direct connections',
    noConnections: 'No direct connections',
    nodeHint:
      'Move over a node to highlight direct dependencies. Select a relation to jump to its other end.',
    footer: `${architectureProjectCount} projects · ${architectureNodeCount} structure nodes · ${architectureEdgeCount} explicit relations`,
    kinds: {
      entry: 'entry',
      process: 'process',
      control: 'control',
      service: 'service',
      adapter: 'adapter',
      runtime: 'runtime',
      store: 'store',
      security: 'validation / security',
      output: 'output',
    },
  },
} as const;

function projectHref(project: ArchitectureProject, lang: Lang): string {
  return project.href.startsWith('/')
    ? localePath(project.href, lang)
    : project.href;
}

function otherEnd(architectureEdge: ArchitectureEdge, nodeId: string): string {
  return architectureEdge.from === nodeId
    ? architectureEdge.to
    : architectureEdge.from;
}

export function ArchitectureAtlas({ lang }: { lang: Lang }) {
  const copy = atlasCopy[lang];
  const [activeCategory, setActiveCategory] =
    useState<ArchitectureAtlasCategory>('all');
  const [activeProjectId, setActiveProjectId] = useState(
    systemArchitectureProject.id,
  );
  const [activeNodeId, setActiveNodeId] = useState(
    systemArchitectureProject.nodes[0].id,
  );
  const [query, setQuery] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    function focusSearch(event: KeyboardEvent) {
      const target = event.target;
      const editing =
        target instanceof HTMLElement &&
        (target.isContentEditable ||
          ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName));

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
      : (architectureProjects.find(
          (project) => project.id === activeProjectId,
        ) ?? systemArchitectureProject);
  const activeNode =
    activeProject.nodes.find((node) => node.id === activeNodeId) ??
    activeProject.nodes[0];
  const activeNodeIndex = activeProject.nodes.findIndex(
    (node) => node.id === activeNode.id,
  );
  const activeEdges = activeProject.edges.filter(
    (architectureEdge) =>
      architectureEdge.from === activeNode.id ||
      architectureEdge.to === activeNode.id,
  );
  const neighborIds = new Set(
    activeEdges.map((architectureEdge) =>
      otherEnd(architectureEdge, activeNode.id),
    ),
  );

  const visibleProjects = useMemo(
    () => filterArchitectureProjects(activeCategory, query),
    [activeCategory, query],
  );

  const visibleSubprojectCount = visibleProjects.filter(
    (project) => project.id !== systemArchitectureProject.id,
  ).length;
  const visibleCountLabel =
    visibleSubprojectCount === 0 &&
    visibleProjects.some(
      (project) => project.id === systemArchitectureProject.id,
    )
      ? 'SYS'
      : `${String(visibleSubprojectCount).padStart(2, '0')} / ${architectureProjectCount}`;

  function selectProject(project: ArchitectureProject) {
    setActiveProjectId(project.id);
    setActiveNodeId(project.nodes[0].id);
  }

  function updateQuery(nextQuery: string) {
    const nextCategory = nextQuery.trim() ? 'all' : activeCategory;
    const nextProjects = filterArchitectureProjects(nextCategory, nextQuery);
    const replacement = replacementArchitectureProject(
      nextProjects,
      activeProjectId,
      nextQuery,
    );

    setQuery(nextQuery);
    if (nextCategory !== activeCategory) setActiveCategory(nextCategory);
    if (replacement) selectProject(replacement);
  }

  function selectCategory(category: ArchitectureAtlasCategory) {
    setActiveCategory(category);
    setQuery('');
    const project =
      category === 'all'
        ? systemArchitectureProject
        : (architectureProjects.find(
            (candidate) => candidate.category === category,
          ) ?? systemArchitectureProject);
    selectProject(project);
  }

  const externalProject = !activeProject.href.startsWith('/');

  return (
    <div className="a3s-atlas">
      <div className="a3s-atlas__toolbar">
        <div
          className="a3s-atlas__categories"
          role="tablist"
          aria-label={copy.projectIndex}
        >
          {categoryOrder.map((category) => {
            const count =
              category === 'all'
                ? architectureProjectCount
                : architectureProjects.filter(
                    (project) => project.category === category,
                  ).length;

            return (
              <button
                aria-controls="a3s-atlas-projects"
                aria-selected={activeCategory === category}
                className={
                  activeCategory === category ? 'is-active' : undefined
                }
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
          <Search aria-hidden="true" />
          <span className="sr-only">{copy.search}</span>
          <input
            aria-keyshortcuts="/"
            onChange={(event) => updateQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape') {
                updateQuery('');
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
          <div
            className="a3s-atlas__project-list"
            id="a3s-atlas-projects"
            role="tabpanel"
          >
            {visibleProjects.map((project) => {
              const active = activeProject.id === project.id;
              const number =
                project.id === systemArchitectureProject.id
                  ? 'SYS'
                  : projectNumbers.get(project.id);

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

        <section
          className="a3s-atlas__diagram"
          aria-label={`${activeProject.name} ${copy.structure}`}
        >
          <header className="a3s-atlas__diagram-head">
            <div>
              <span>{copy.structure}</span>
              <h3>{activeProject.name}</h3>
              <p>{activeProject.role[lang]}</p>
              <div className="a3s-atlas__evidence" aria-label={copy.source}>
                <small>{copy.source}</small>
                {activeProject.evidence.map((source) => (
                  <a
                    href={source.href}
                    key={source.href}
                    rel="noopener noreferrer"
                    target="_blank"
                  >
                    {source.label}
                    <ArrowUpRight aria-hidden="true" />
                  </a>
                ))}
              </div>
            </div>
            <div className="a3s-atlas__diagram-meta">
              <span>
                <b>{activeProject.nodes.length}</b>
                {copy.nodes}
              </span>
              <span>
                <b>{activeProject.edges.length}</b>
                {copy.relations}
              </span>
              <Link
                href={projectHref(activeProject, lang)}
                rel={externalProject ? 'noopener noreferrer' : undefined}
                target={externalProject ? '_blank' : undefined}
              >
                {copy.openProject}
                <ArrowUpRight aria-hidden="true" />
              </Link>
            </div>
          </header>

          <div
            aria-label={`${activeProject.name} ${copy.diagramCanvas}`}
            className="a3s-atlas__stage-scroll"
            role="region"
            tabIndex={0}
          >
            <div className="a3s-atlas__stage">
              {(activeProject.groups ?? []).map((architectureGroup) => {
                const style = {
                  '--group-x': `${architectureGroup.x}%`,
                  '--group-y': `${architectureGroup.y}%`,
                  '--group-width': `${architectureGroup.width}%`,
                  '--group-height': `${architectureGroup.height}%`,
                } as CSSProperties;

                return (
                  <div
                    aria-hidden="true"
                    className="a3s-atlas__group"
                    key={architectureGroup.id}
                    style={style}
                  >
                    <span>{architectureGroup.label[lang]}</span>
                  </div>
                );
              })}

              <svg
                aria-hidden="true"
                className="a3s-atlas__wires"
                preserveAspectRatio="none"
                viewBox="0 0 100 100"
              >
                <defs>
                  <marker
                    id="a3s-atlas-arrow"
                    markerHeight="5"
                    markerWidth="5"
                    orient="auto"
                    refX="4"
                    refY="2.5"
                  >
                    <path d="M0,0 L5,2.5 L0,5 Z" />
                  </marker>
                  <marker
                    id="a3s-atlas-arrow-start"
                    markerHeight="5"
                    markerWidth="5"
                    orient="auto"
                    refX="1"
                    refY="2.5"
                  >
                    <path d="M5,0 L0,2.5 L5,5 Z" />
                  </marker>
                </defs>
                {activeProject.edges.map((architectureEdge, index) => {
                  const from = activeProject.nodes.find(
                    (item) => item.id === architectureEdge.from,
                  );
                  const to = activeProject.nodes.find(
                    (item) => item.id === architectureEdge.to,
                  );
                  if (!from || !to) return null;
                  const active =
                    architectureEdge.from === activeNode.id ||
                    architectureEdge.to === activeNode.id;
                  return (
                    <g
                      className={
                        active ? 'a3s-atlas__edge is-active' : 'a3s-atlas__edge'
                      }
                      data-kind={architectureEdge.kind}
                      key={`${architectureEdge.from}-${architectureEdge.to}-${index}`}
                    >
                      <line
                        markerEnd="url(#a3s-atlas-arrow)"
                        markerStart={
                          architectureEdge.bidirectional
                            ? 'url(#a3s-atlas-arrow-start)'
                            : undefined
                        }
                        vectorEffect="non-scaling-stroke"
                        x1={from.position.x}
                        x2={to.position.x}
                        y1={from.position.y}
                        y2={to.position.y}
                      />
                    </g>
                  );
                })}
              </svg>

              {activeEdges.map((architectureEdge, index) => {
                const from = activeProject.nodes.find(
                  (item) => item.id === architectureEdge.from,
                );
                const to = activeProject.nodes.find(
                  (item) => item.id === architectureEdge.to,
                );
                if (!from || !to) return null;
                const style = {
                  '--edge-x': `${(from.position.x + to.position.x) / 2}%`,
                  '--edge-y': `${(from.position.y + to.position.y) / 2}%`,
                } as CSSProperties;

                return (
                  <span
                    aria-hidden="true"
                    className="a3s-atlas__edge-label"
                    data-kind={architectureEdge.kind}
                    key={`${architectureEdge.from}-${architectureEdge.to}-label-${index}`}
                    style={style}
                  >
                    {architectureEdge.label[lang]}
                  </span>
                );
              })}

              {activeProject.nodes.map((architectureNode, index) => {
                const style = {
                  '--node-x': `${architectureNode.position.x}%`,
                  '--node-y': `${architectureNode.position.y}%`,
                } as CSSProperties;
                const active = activeNode.id === architectureNode.id;
                const neighbor = neighborIds.has(architectureNode.id);
                const className = [
                  'a3s-atlas__node',
                  active ? 'is-active' : '',
                  neighbor ? 'is-neighbor' : '',
                ]
                  .filter(Boolean)
                  .join(' ');

                return (
                  <button
                    aria-label={`${architectureNode.label}: ${architectureNode.detail[lang]}`}
                    aria-pressed={active}
                    className={className}
                    data-kind={architectureNode.kind}
                    key={architectureNode.id}
                    onClick={() => setActiveNodeId(architectureNode.id)}
                    onFocus={() => setActiveNodeId(architectureNode.id)}
                    onMouseEnter={() => setActiveNodeId(architectureNode.id)}
                    style={style}
                    type="button"
                  >
                    <span>{String(index + 1).padStart(2, '0')}</span>
                    <b title={architectureNode.label}>
                      {architectureNode.label}
                    </b>
                    <small>
                      {
                        copy.kinds[
                          architectureNode.kind as ArchitectureNodeKind
                        ]
                      }
                    </small>
                  </button>
                );
              })}
            </div>
          </div>

          <div className="a3s-atlas__node-detail" data-kind={activeNode.kind}>
            <div className="a3s-atlas__node-copy">
              <span>
                {copy.selectedNode} /{' '}
                {String(activeNodeIndex + 1).padStart(2, '0')}
              </span>
              <b>{copy.kinds[activeNode.kind as ArchitectureNodeKind]}</b>
              <h4>{activeNode.label}</h4>
              <p>{activeNode.detail[lang]}</p>
              <small>{copy.nodeHint}</small>
            </div>

            <div className="a3s-atlas__connections">
              <span>{copy.connected}</span>
              {activeEdges.length > 0 ? (
                activeEdges.map((architectureEdge, index) => {
                  const targetId = otherEnd(architectureEdge, activeNode.id);
                  const target = activeProject.nodes.find(
                    (item) => item.id === targetId,
                  );
                  const outgoing = architectureEdge.from === activeNode.id;

                  return (
                    <button
                      key={`${architectureEdge.from}-${architectureEdge.to}-${index}`}
                      onClick={() => setActiveNodeId(targetId)}
                      type="button"
                    >
                      {architectureEdge.bidirectional ? (
                        <ArrowLeftRight aria-hidden="true" />
                      ) : outgoing ? (
                        <ArrowRight aria-hidden="true" />
                      ) : (
                        <ArrowLeft aria-hidden="true" />
                      )}
                      <span>
                        <small>{architectureEdge.label[lang]}</small>
                        <b>{target?.label}</b>
                      </span>
                    </button>
                  );
                })
              ) : (
                <p>{copy.noConnections}</p>
              )}
            </div>
          </div>
        </section>
      </div>

      <div className="a3s-atlas__footer">
        <span>{copy.footer}</span>
        <code>README · SOURCE TREE · ARCHITECTURE DOCS</code>
      </div>
    </div>
  );
}
