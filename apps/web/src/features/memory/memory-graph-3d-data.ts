import type { MemoryGraphProjection, MemoryVisualEdge, MemoryVisualNode } from './memory-projection';

export const MEMORY_GRAPH_NODE_RENDER_LIMIT = 600;
export const MEMORY_GRAPH_EDGE_RENDER_LIMIT = 4_000;
const MEMORY_GRAPH_SEED_LIMIT_PER_TYPE = 16;
const MEMORY_GRAPH_SELECTED_NEIGHBOUR_LIMIT = 160;

interface NodeDegree {
  count: number;
  weight: number;
}

interface GraphNeighbour {
  id: string;
  weight: number;
}

export function activeMemoryGraphNodeId(
  nodes: readonly MemoryVisualNode[],
  selectedNodeId?: string,
  hoveredNodeId?: string
): string | undefined {
  const nodeIds = new Set(nodes.map((node) => node.id));
  if (selectedNodeId && nodeIds.has(selectedNodeId)) return selectedNodeId;
  if (hoveredNodeId && nodeIds.has(hoveredNodeId)) return hoveredNodeId;
  return undefined;
}

export function connectedMemoryGraphNodeIds(edges: readonly MemoryVisualEdge[], activeNodeId?: string): Set<string> {
  const connected = new Set<string>();
  if (!activeNodeId) return connected;
  connected.add(activeNodeId);
  for (const edge of edges) {
    if (edge.from === activeNodeId) connected.add(edge.to);
    if (edge.to === activeNodeId) connected.add(edge.from);
  }
  return connected;
}

export function selectedMemoryGraphNodeId(
  graph: MemoryGraphProjection,
  selectedMemoryId?: string,
  selectedEntityId?: string
): string | undefined {
  return graph.nodes.find(
    (node) =>
      (selectedMemoryId !== undefined && node.memoryId === selectedMemoryId) ||
      (selectedEntityId !== undefined && node.entityId === selectedEntityId)
  )?.id;
}

/**
 * Keeps the 3D force simulation bounded without changing the complete totals
 * reported by the memory service. The current selection and its immediate
 * neighbourhood always outrank unrelated hubs.
 */
export function limitMemoryGraphFor3D(graph: MemoryGraphProjection, selectedNodeId?: string): MemoryGraphProjection {
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  const degree = new Map<string, NodeDegree>();
  const adjacency = new Map<string, GraphNeighbour[]>();
  const neighbours = new Set<string>();

  for (const node of graph.nodes) {
    degree.set(node.id, { count: 0, weight: 0 });
    adjacency.set(node.id, []);
  }
  for (const edge of graph.edges) {
    if (!nodeIds.has(edge.from) || !nodeIds.has(edge.to)) continue;
    increaseDegree(degree, edge.from, edge.weight);
    increaseDegree(degree, edge.to, edge.weight);
    adjacency.get(edge.from)?.push({ id: edge.to, weight: edge.weight });
    adjacency.get(edge.to)?.push({ id: edge.from, weight: edge.weight });
    if (edge.from === selectedNodeId) neighbours.add(edge.to);
    if (edge.to === selectedNodeId) neighbours.add(edge.from);
  }
  for (const adjacent of adjacency.values()) {
    adjacent.sort(
      (left, right) =>
        right.weight - left.weight ||
        (degree.get(right.id)?.count ?? 0) - (degree.get(left.id)?.count ?? 0) ||
        left.id.localeCompare(right.id)
    );
  }

  let nodes = graph.nodes;
  if (nodes.length > MEMORY_GRAPH_NODE_RENDER_LIMIT) {
    nodes = selectCohesiveNodes(graph.nodes, selectedNodeId, neighbours, degree, adjacency);
  }

  const keptNodeIds = new Set(nodes.map((node) => node.id));
  let edges = graph.edges.filter((edge) => keptNodeIds.has(edge.from) && keptNodeIds.has(edge.to));
  if (edges.length > MEMORY_GRAPH_EDGE_RENDER_LIMIT) {
    edges = selectCoveredEdges(edges, selectedNodeId, neighbours, degree);
  }

  return {
    nodes: nodes.map((node) => ({ ...node, relatedMemoryIds: [...node.relatedMemoryIds] })),
    edges: edges.map((edge) => ({ ...edge })),
    totalNodes: graph.totalNodes,
    totalEdges: graph.totalEdges,
    truncated: graph.truncated || nodes.length < graph.nodes.length || edges.length < graph.edges.length,
  };
}

function selectCohesiveNodes(
  nodes: MemoryVisualNode[],
  selectedNodeId: string | undefined,
  selectedNeighbours: Set<string>,
  degree: Map<string, NodeDegree>,
  adjacency: Map<string, GraphNeighbour[]>
): MemoryVisualNode[] {
  const nodeById = new Map(nodes.map((node) => [node.id, node]));
  const ranked = [...nodes].sort((left, right) =>
    compareNodes(left, right, selectedNodeId, selectedNeighbours, degree)
  );
  const seeds = [
    ...ranked.filter((node) => node.nodeType === 'event').slice(0, MEMORY_GRAPH_SEED_LIMIT_PER_TYPE),
    ...ranked.filter((node) => node.nodeType === 'entity').slice(0, MEMORY_GRAPH_SEED_LIMIT_PER_TYPE),
  ].sort((left, right) => compareNodes(left, right, selectedNodeId, selectedNeighbours, degree));
  const keptIds = new Set<string>();
  const result: MemoryVisualNode[] = [];
  const addNode = (id: string): boolean => {
    if (keptIds.has(id) || result.length >= MEMORY_GRAPH_NODE_RENDER_LIMIT) return false;
    const node = nodeById.get(id);
    if (!node) return false;
    keptIds.add(id);
    result.push(node);
    return true;
  };

  if (selectedNodeId) {
    addNode(selectedNodeId);
    for (const neighbour of adjacency.get(selectedNodeId)?.slice(0, MEMORY_GRAPH_SELECTED_NEIGHBOUR_LIMIT) ?? []) {
      addNode(neighbour.id);
    }
  }
  for (const seed of seeds) addNode(seed.id);

  const seedOffsets = new Map(seeds.map((seed) => [seed.id, 0]));
  while (result.length < MEMORY_GRAPH_NODE_RENDER_LIMIT) {
    let progressed = false;
    for (const seed of seeds) {
      const adjacent = adjacency.get(seed.id) ?? [];
      let offset = seedOffsets.get(seed.id) ?? 0;
      while (offset < adjacent.length && keptIds.has(adjacent[offset].id)) offset += 1;
      seedOffsets.set(seed.id, offset + 1);
      if (offset < adjacent.length && addNode(adjacent[offset].id)) progressed = true;
      if (result.length >= MEMORY_GRAPH_NODE_RENDER_LIMIT) break;
    }
    if (!progressed) break;
  }

  // Expand from the retained neighbourhood when direct seed neighbours do not
  // fill the cap. Every added node remains attached to the visible subgraph.
  for (let index = 0; index < result.length && result.length < MEMORY_GRAPH_NODE_RENDER_LIMIT; index += 1) {
    for (const neighbour of adjacency.get(result[index].id) ?? []) {
      addNode(neighbour.id);
      if (result.length >= MEMORY_GRAPH_NODE_RENDER_LIMIT) break;
    }
  }

  // Graphs with genuinely isolated nodes still get a deterministic fallback.
  for (const node of ranked) {
    addNode(node.id);
    if (result.length >= MEMORY_GRAPH_NODE_RENDER_LIMIT) break;
  }
  return result;
}

function selectCoveredEdges(
  edges: MemoryVisualEdge[],
  selectedNodeId: string | undefined,
  neighbours: Set<string>,
  degree: Map<string, NodeDegree>
): MemoryVisualEdge[] {
  const ranked = [...edges].sort((left, right) => compareEdges(left, right, selectedNodeId, neighbours, degree));
  const selected: MemoryVisualEdge[] = [];
  const selectedIds = new Set<number>();
  const coveredNodes = new Set<string>();

  // Reserve one strong incident relation for every node before filling the
  // remaining edge budget. This avoids turning retained nodes into a starfield.
  for (const edge of ranked) {
    if (coveredNodes.has(edge.from) && coveredNodes.has(edge.to)) continue;
    selected.push(edge);
    selectedIds.add(edge.id);
    coveredNodes.add(edge.from);
    coveredNodes.add(edge.to);
    if (selected.length >= MEMORY_GRAPH_EDGE_RENDER_LIMIT) return selected;
  }
  for (const edge of ranked) {
    if (selectedIds.has(edge.id)) continue;
    selected.push(edge);
    if (selected.length >= MEMORY_GRAPH_EDGE_RENDER_LIMIT) break;
  }
  return selected;
}

function increaseDegree(degree: Map<string, NodeDegree>, id: string, weight: number): void {
  const value = degree.get(id);
  if (!value) return;
  value.count += 1;
  value.weight += weight;
}

function compareNodes(
  left: MemoryVisualNode,
  right: MemoryVisualNode,
  selectedNodeId: string | undefined,
  neighbours: Set<string>,
  degree: Map<string, NodeDegree>
): number {
  const selectionRank =
    nodeSelectionRank(right.id, selectedNodeId, neighbours) - nodeSelectionRank(left.id, selectedNodeId, neighbours);
  if (selectionRank) return selectionRank;
  const leftDegree = degree.get(left.id) ?? { count: 0, weight: 0 };
  const rightDegree = degree.get(right.id) ?? { count: 0, weight: 0 };
  return (
    rightDegree.count - leftDegree.count ||
    rightDegree.weight - leftDegree.weight ||
    right.size - left.size ||
    left.label.localeCompare(right.label) ||
    left.id.localeCompare(right.id)
  );
}

function compareEdges(
  left: MemoryVisualEdge,
  right: MemoryVisualEdge,
  selectedNodeId: string | undefined,
  neighbours: Set<string>,
  degree: Map<string, NodeDegree>
): number {
  const leftSelected = left.from === selectedNodeId || left.to === selectedNodeId;
  const rightSelected = right.from === selectedNodeId || right.to === selectedNodeId;
  if (leftSelected !== rightSelected) return Number(rightSelected) - Number(leftSelected);
  const leftNeighbourhood = neighbours.has(left.from) && neighbours.has(left.to);
  const rightNeighbourhood = neighbours.has(right.from) && neighbours.has(right.to);
  if (leftNeighbourhood !== rightNeighbourhood) return Number(rightNeighbourhood) - Number(leftNeighbourhood);
  const leftDegree = (degree.get(left.from)?.count ?? 0) + (degree.get(left.to)?.count ?? 0);
  const rightDegree = (degree.get(right.from)?.count ?? 0) + (degree.get(right.to)?.count ?? 0);
  return right.weight - left.weight || rightDegree - leftDegree || left.id - right.id;
}

function nodeSelectionRank(id: string, selectedNodeId: string | undefined, neighbours: Set<string>): number {
  if (id === selectedNodeId) return 2;
  return neighbours.has(id) ? 1 : 0;
}
