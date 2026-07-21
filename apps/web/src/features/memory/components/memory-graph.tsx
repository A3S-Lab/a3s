import { ListTree, LocateFixed, Minus, Plus, Search, X } from 'lucide-react';
import { lazy, Suspense, useMemo, useRef, useState } from 'react';
import { entityKindLabel } from '../memory-format';
import { limitMemoryGraphFor3D, selectedMemoryGraphNodeId } from '../memory-graph-3d-data';
import type { MemoryGraphProjection, MemoryVisualNode } from '../memory-projection';
import type { MemoryGraph3DHandle } from './memory-graph-3d';

const MemoryGraph3D = lazy(() => import('./memory-graph-3d'));

export function MemoryGraph({
  graph,
  selectedMemoryId,
  selectedEntityId,
  onSelectMemory,
  onSelectEntity,
  onClearSelection,
}: {
  graph: MemoryGraphProjection;
  selectedMemoryId?: string;
  selectedEntityId?: string;
  onSelectMemory: (id: string) => void;
  onSelectEntity: (id: string) => void;
  onClearSelection: () => void;
}) {
  const graphRef = useRef<MemoryGraph3DHandle>(null);
  const browserToggleRef = useRef<HTMLButtonElement>(null);
  const [browserOpen, setBrowserOpen] = useState(false);
  const [nodeQuery, setNodeQuery] = useState('');
  const selectedNodeId = useMemo(
    () => selectedMemoryGraphNodeId(graph, selectedMemoryId, selectedEntityId),
    [graph, selectedEntityId, selectedMemoryId]
  );
  const renderedGraph = useMemo(() => limitMemoryGraphFor3D(graph, selectedNodeId), [graph, selectedNodeId]);
  const browserNodes = useMemo(() => {
    const query = nodeQuery.trim().toLocaleLowerCase();
    if (!query) return renderedGraph.nodes;
    return renderedGraph.nodes.filter((node) =>
      [node.label, node.tone, node.memoryId, node.entityId].some((value) => value?.toLocaleLowerCase().includes(query))
    );
  }, [nodeQuery, renderedGraph.nodes]);

  const selectNode = (node: MemoryVisualNode) => {
    if (node.memoryId) onSelectMemory(node.memoryId);
    else if (node.entityId) onSelectEntity(node.entityId);
  };

  return (
    <section className='memory-graph' data-has-selection={Boolean(selectedNodeId) || undefined} aria-label='记忆关联图'>
      <Suspense
        fallback={
          <output className='memory-graph-loading'>
            <span />
            正在加载关联图…
          </output>
        }
      >
        <MemoryGraph3D
          ref={graphRef}
          graph={renderedGraph}
          selectedNodeId={selectedNodeId}
          onSelectNode={selectNode}
          onClearSelection={onClearSelection}
        />
      </Suspense>

      <fieldset className='memory-graph-controls'>
        <legend>关系图控制</legend>
        <button type='button' aria-label='放大关系图' onClick={() => graphRef.current?.zoomIn()}>
          <Plus size={15} />
        </button>
        <button type='button' aria-label='缩小关系图' onClick={() => graphRef.current?.zoomOut()}>
          <Minus size={15} />
        </button>
        <button
          type='button'
          aria-label='重新取景'
          onClick={() => {
            onClearSelection();
            graphRef.current?.resetView();
          }}
        >
          <LocateFixed size={15} />
        </button>
        <button
          id='memory-graph-browser-toggle'
          ref={browserToggleRef}
          type='button'
          className={browserOpen ? 'active' : undefined}
          aria-label='浏览图中内容'
          aria-expanded={browserOpen}
          aria-controls='memory-graph-node-browser'
          onClick={() => setBrowserOpen((open) => !open)}
        >
          <ListTree size={15} />
        </button>
      </fieldset>

      {browserOpen && (
        <aside id='memory-graph-node-browser' className='memory-graph-node-browser' aria-label='图中内容'>
          <header>
            <div>
              <strong>图中内容</strong>
              <span>{renderedGraph.nodes.length} 项</span>
            </div>
            <button type='button' aria-label='关闭列表' onClick={() => setBrowserOpen(false)}>
              <X size={15} />
            </button>
          </header>
          <label>
            <Search size={14} />
            <input
              type='search'
              value={nodeQuery}
              aria-label='搜索图中内容'
              placeholder='搜索'
              onChange={(event) => setNodeQuery(event.target.value)}
            />
          </label>
          <div className='memory-graph-node-list'>
            {browserNodes.map((node) => (
              <button
                type='button'
                key={node.id}
                data-tone={node.nodeType === 'event' ? 'event' : node.tone}
                aria-pressed={node.id === selectedNodeId}
                aria-label={nodeAriaLabel(node)}
                onClick={() => {
                  browserToggleRef.current?.focus();
                  setBrowserOpen(false);
                  selectNode(node);
                }}
              >
                <i />
                <span>
                  <strong>{node.label}</strong>
                  <small>{node.nodeType === 'event' ? '记忆' : entityKindLabel(node.tone)}</small>
                </span>
              </button>
            ))}
            {browserNodes.length === 0 && <p>没有匹配内容。</p>}
          </div>
        </aside>
      )}
    </section>
  );
}

function nodeAriaLabel(node: MemoryVisualNode): string {
  if (node.nodeType === 'event') return `记忆：${node.label}`;
  return `${entityKindLabel(node.tone)}：${node.label}，关联 ${node.relatedMemoryIds.length} 条记忆`;
}
