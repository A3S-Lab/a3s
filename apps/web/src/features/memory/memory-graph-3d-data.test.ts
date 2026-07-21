import { describe, expect, it } from 'vitest';
import {
  limitMemoryGraphFor3D,
  MEMORY_GRAPH_EDGE_RENDER_LIMIT,
  MEMORY_GRAPH_NODE_RENDER_LIMIT,
} from './memory-graph-3d-data';
import type { MemoryGraphProjection, MemoryVisualNode } from './memory-projection';

describe('3D memory graph limits', () => {
  it('keeps the selected node and its neighbourhood in a capped overview', () => {
    const nodes = Array.from({ length: MEMORY_GRAPH_NODE_RENDER_LIMIT + 10 }, (_, index) =>
      graphNode(`node-${index}`, index)
    );
    const edges = nodes.slice(1).map((node, index) => ({
      id: index,
      from: 'node-0',
      to: node.id,
      kind: 'mentions',
      weight: 1,
      memoryId: 'memory-0',
    }));
    edges.push({
      id: edges.length,
      from: 'node-609',
      to: 'node-608',
      kind: 'mentions',
      weight: 0.01,
      memoryId: 'memory-0',
    });
    const graph = graphProjection(nodes, edges);

    const limited = limitMemoryGraphFor3D(graph, 'node-609');

    expect(limited.nodes).toHaveLength(MEMORY_GRAPH_NODE_RENDER_LIMIT);
    expect(limited.nodes.map((node) => node.id)).toEqual(expect.arrayContaining(['node-609', 'node-608']));
    const linkedNodeIds = new Set(limited.edges.flatMap((edge) => [edge.from, edge.to]));
    expect(limited.nodes.every((node) => linkedNodeIds.has(node.id))).toBe(true);
    expect(limited.totalNodes).toBe(nodes.length);
    expect(limited.truncated).toBe(true);
  });

  it('prioritizes selected relations and reports complete totals when links are capped', () => {
    const nodes = [graphNode('selected', 0), graphNode('neighbour', 1), graphNode('other', 2)];
    const edges = Array.from({ length: MEMORY_GRAPH_EDGE_RENDER_LIMIT + 2 }, (_, index) => ({
      id: index,
      from: index === MEMORY_GRAPH_EDGE_RENDER_LIMIT + 1 ? 'selected' : 'other',
      to: index === MEMORY_GRAPH_EDGE_RENDER_LIMIT + 1 ? 'neighbour' : 'neighbour',
      kind: 'mentions',
      weight: index === MEMORY_GRAPH_EDGE_RENDER_LIMIT + 1 ? 0.01 : 1,
      memoryId: 'memory-0',
    }));
    const graph = graphProjection(nodes, edges);

    const limited = limitMemoryGraphFor3D(graph, 'selected');

    expect(limited.edges).toHaveLength(MEMORY_GRAPH_EDGE_RENDER_LIMIT);
    expect(limited.edges.some((edge) => edge.from === 'selected')).toBe(true);
    expect(limited.totalEdges).toBe(edges.length);
    expect(limited.truncated).toBe(true);
  });
});

function graphNode(id: string, index: number): MemoryVisualNode {
  return {
    id,
    nodeType: index % 2 ? 'entity' : 'event',
    label: id,
    x: 0,
    y: 0,
    size: 8,
    tone: index % 2 ? 'tag' : 'short',
    memoryId: index % 2 ? undefined : `memory-${index}`,
    entityId: index % 2 ? id : undefined,
    relatedMemoryIds: [`memory-${index}`],
  };
}

function graphProjection(nodes: MemoryVisualNode[], edges: MemoryGraphProjection['edges']): MemoryGraphProjection {
  return {
    nodes,
    edges,
    totalNodes: nodes.length,
    totalEdges: edges.length,
    truncated: false,
  };
}
