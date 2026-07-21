import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { MemoryGraphProjection } from '../memory-projection';
import { MemoryGraph } from './memory-graph';

afterEach(cleanup);

describe('MemoryGraph', () => {
  it('provides a keyboard-accessible node browser alongside the 3D scene', async () => {
    const onSelectMemory = vi.fn();
    const onSelectEntity = vi.fn();
    render(
      <MemoryGraph
        graph={graphProjection(1)}
        onSelectMemory={onSelectMemory}
        onSelectEntity={onSelectEntity}
        onClearSelection={vi.fn()}
      />
    );

    expect(await screen.findByTestId('memory-graph-3d-scene')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '浏览图谱节点' }));

    const memoryNode = screen.getByRole('button', { name: '记忆：Memory one' });
    fireEvent.click(memoryNode);
    expect(onSelectMemory).toHaveBeenCalledWith('memory-1');

    const entityNode = screen.getByRole('button', { name: /标签实体：Entity one/ });
    fireEvent.click(entityNode);
    expect(onSelectEntity).toHaveBeenCalledWith('tag:entity-1');
  });

  it('filters the accessible node browser without changing graph data', () => {
    render(
      <MemoryGraph
        graph={graphProjection(1201)}
        onSelectMemory={vi.fn()}
        onSelectEntity={vi.fn()}
        onClearSelection={vi.fn()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '浏览图谱节点' }));
    expect(screen.getByRole('button', { name: '记忆：Memory one' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /标签实体：Entity one/ })).toBeInTheDocument();

    fireEvent.change(screen.getByRole('searchbox', { name: '筛选图谱节点' }), {
      target: { value: 'entity' },
    });
    expect(screen.queryByRole('button', { name: '记忆：Memory one' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /标签实体：Entity one/ })).toBeInTheDocument();
  });

  it('reports both shown and complete node and relation totals', () => {
    const graph = { ...graphProjection(1), totalNodes: 8, totalEdges: 12, truncated: true };
    render(<MemoryGraph graph={graph} onSelectMemory={vi.fn()} onSelectEntity={vi.fn()} onClearSelection={vi.fn()} />);

    expect(screen.getByText('已渲染 2/8 个节点 · 1/12 条关系')).toBeInTheDocument();
  });

  it('clears selection before manually reframing the graph', () => {
    const onClearSelection = vi.fn();
    render(
      <MemoryGraph
        graph={graphProjection(1)}
        onSelectMemory={vi.fn()}
        onSelectEntity={vi.fn()}
        onClearSelection={onClearSelection}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '重新取景' }));
    expect(onClearSelection).toHaveBeenCalledOnce();
  });
});

function graphProjection(edgeCount: number): MemoryGraphProjection {
  return {
    nodes: [
      {
        id: 'event:memory-1',
        nodeType: 'event',
        label: 'Memory one',
        x: 500,
        y: 350,
        size: 10,
        tone: 'short',
        memoryId: 'memory-1',
        relatedMemoryIds: ['memory-1'],
      },
      {
        id: 'tag:entity-1',
        nodeType: 'entity',
        label: 'Entity one',
        x: 700,
        y: 350,
        size: 8,
        tone: 'tag',
        entityId: 'tag:entity-1',
        relatedMemoryIds: ['memory-1'],
      },
    ],
    edges: Array.from({ length: edgeCount }, (_, id) => ({
      id,
      from: 'event:memory-1',
      to: 'tag:entity-1',
      kind: 'tagged',
      weight: 0.8,
      memoryId: 'memory-1',
    })),
    totalNodes: 2,
    totalEdges: edgeCount,
    truncated: false,
  };
}
