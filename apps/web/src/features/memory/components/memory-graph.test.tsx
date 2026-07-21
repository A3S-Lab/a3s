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
    fireEvent.click(screen.getByRole('button', { name: '浏览图中内容' }));

    const memoryNode = screen.getByRole('button', { name: '记忆：Memory one' });
    fireEvent.click(memoryNode);
    expect(onSelectMemory).toHaveBeenCalledWith('memory-1');
    expect(screen.queryByRole('searchbox', { name: '搜索图中内容' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '浏览图中内容' }));
    const entityNode = screen.getByRole('button', { name: /标签：Entity one/ });
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

    fireEvent.click(screen.getByRole('button', { name: '浏览图中内容' }));
    expect(screen.getByRole('button', { name: '记忆：Memory one' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /标签：Entity one/ })).toBeInTheDocument();

    fireEvent.change(screen.getByRole('searchbox', { name: '搜索图中内容' }), {
      target: { value: 'entity' },
    });
    expect(screen.queryByRole('button', { name: '记忆：Memory one' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /标签：Entity one/ })).toBeInTheDocument();
  });

  it('states the visible range when the 3D view contains only part of the graph', () => {
    const graph = graphProjection(1);
    graph.totalNodes = 12;
    graph.truncated = true;
    render(<MemoryGraph graph={graph} onSelectMemory={vi.fn()} onSelectEntity={vi.fn()} onClearSelection={vi.fn()} />);

    expect(screen.getByText('图中 2 / 12 项')).toBeInTheDocument();
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
