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
    <section
      className='memory-graph'
      data-has-selection={Boolean(selectedNodeId) || undefined}
      aria-label='记忆知识图谱'
    >
      <Suspense
        fallback={
          <output className='memory-graph-loading'>
            <span />
            正在准备 3D 记忆图谱…
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

      <aside className='memory-graph-legend' aria-label='图谱图例'>
        <span>
          <i data-tone='event' />
          记忆
        </span>
        <span>
          <i data-tone='source' />
          来源
        </span>
        <span>
          <i data-tone='tool' />
          工具
        </span>
        <span>
          <i data-tone='file' />
          文件
        </span>
        <span>
          <i data-tone='tag' />
          标签
        </span>
        <span>
          <i data-tone='other' />
          其他实体
        </span>
      </aside>

      <fieldset className='memory-graph-controls'>
        <legend>3D 图谱视图控制</legend>
        <button type='button' aria-label='放大图谱' onClick={() => graphRef.current?.zoomIn()}>
          <Plus size={15} />
        </button>
        <button type='button' aria-label='缩小图谱' onClick={() => graphRef.current?.zoomOut()}>
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
          type='button'
          className={browserOpen ? 'active' : undefined}
          aria-label='浏览图谱节点'
          aria-expanded={browserOpen}
          aria-controls='memory-graph-node-browser'
          onClick={() => setBrowserOpen((open) => !open)}
        >
          <ListTree size={15} />
        </button>
      </fieldset>

      {browserOpen && (
        <aside id='memory-graph-node-browser' className='memory-graph-node-browser' aria-label='图谱节点浏览器'>
          <header>
            <div>
              <strong>浏览节点</strong>
              <span>{renderedGraph.nodes.length} 个可见节点</span>
            </div>
            <button type='button' aria-label='关闭节点浏览器' onClick={() => setBrowserOpen(false)}>
              <X size={15} />
            </button>
          </header>
          <label>
            <Search size={14} />
            <input
              type='search'
              value={nodeQuery}
              aria-label='筛选图谱节点'
              placeholder='搜索节点名称'
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
                  selectNode(node);
                  graphRef.current?.focusNode(node.id);
                }}
              >
                <i />
                <span>
                  <strong>{node.label}</strong>
                  <small>{node.nodeType === 'event' ? '记忆' : entityKindLabel(node.tone)}</small>
                </span>
              </button>
            ))}
            {browserNodes.length === 0 && <p>没有匹配的节点。</p>}
          </div>
        </aside>
      )}

      <div className='memory-graph-footnote'>
        <span>拖拽旋转 · 滚轮缩放 · 点击节点聚焦</span>
        <span className={renderedGraph.truncated ? 'attention' : undefined}>
          已渲染 {renderedGraph.nodes.length}/{graph.totalNodes} 个节点 · {renderedGraph.edges.length}/
          {graph.totalEdges} 条关系
        </span>
      </div>
    </section>
  );
}

function nodeAriaLabel(node: MemoryVisualNode): string {
  if (node.nodeType === 'event') return `记忆：${node.label}`;
  return `${entityKindLabel(node.tone)}实体：${node.label}，关联 ${node.relatedMemoryIds.length} 条记忆`;
}
