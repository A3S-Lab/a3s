import type {
  MemoryEntry,
  MemoryForgetSignal,
  MemoryGraphEntity,
  MemoryGraphEvent,
  MemoryOverview,
  MemoryTier,
} from '../../types/api';
import { entityNameLabel, memoryTagLabel } from './memory-format';
import type { MemoryGraphScope, MemoryLifecycleFilter, MemoryTimeRange } from './memory-state';

export interface MemoryFilters {
  query: string;
  types: string[];
  sources: string[];
  tiers: MemoryTier[];
  signals: MemoryForgetSignal[];
  lifecycle: MemoryLifecycleFilter[];
  timeRange: MemoryTimeRange;
}

export interface MemoryVisualNode {
  id: string;
  nodeType: 'event' | 'entity';
  label: string;
  x: number;
  y: number;
  size: number;
  tone: string;
  memoryId?: string;
  entityId?: string;
  relatedMemoryIds: string[];
}

export interface MemoryVisualEdge {
  id: number;
  from: string;
  to: string;
  kind: string;
  weight: number;
  memoryId: string;
}

export interface MemoryGraphProjection {
  nodes: MemoryVisualNode[];
  edges: MemoryVisualEdge[];
  totalNodes: number;
  totalEdges: number;
  truncated: boolean;
}

const BALANCED_EVENT_LIMIT = 36;
const BALANCED_ENTITY_LIMIT = 64;
const BALANCED_RELATION_LIMIT = 360;

export function filterMemoryEntries(data: MemoryOverview, filters: MemoryFilters, now = Date.now()): MemoryEntry[] {
  const eventByMemory = new Map(data.graph.events.map((event) => [event.memoryId, event]));
  const entitiesByMemory = new Map<string, string[]>();
  for (const entity of data.graph.entities) {
    for (const memoryId of entity.memoryIds) {
      const names = entitiesByMemory.get(memoryId) ?? [];
      names.push(entity.name, ...entity.aliases);
      entitiesByMemory.set(memoryId, names);
    }
  }
  const query = filters.query.trim().toLocaleLowerCase();
  const rangeMs = timeRangeMs(filters.timeRange);

  return data.entries.filter((entry) => {
    const facet = data.graph.facets[entry.id];
    const event = eventByMemory.get(entry.id);
    if (filters.types.length && !filters.types.includes(entry.memoryType)) return false;
    if (filters.sources.length && (!event || !filters.sources.includes(event.source))) return false;
    if (filters.tiers.length && (!facet || !filters.tiers.includes(facet.tier))) return false;
    if (filters.signals.length && (!facet || !filters.signals.includes(facet.forget))) return false;
    if (
      filters.lifecycle.some((filter) => {
        if (!facet) return true;
        if (filter === 'llm') return !facet.llmExtracted;
        if (filter === 'consolidated') return !facet.consolidated;
        return !facet.conflicts;
      })
    )
      return false;
    if (rangeMs !== null) {
      const timestamp = Date.parse(entry.timestamp);
      if (!Number.isFinite(timestamp) || now - timestamp > rangeMs) return false;
    }
    if (!query) return true;
    const metadata = Object.entries(entry.metadata ?? {})
      .flat()
      .join(' ');
    const haystack = [
      entry.content,
      entry.preview,
      entry.memoryType,
      entry.tags.join(' '),
      metadata,
      event?.source ?? '',
      ...(entitiesByMemory.get(entry.id) ?? []),
    ]
      .join(' ')
      .toLocaleLowerCase();
    return haystack.includes(query);
  });
}

export function countActiveMemoryFilters(filters: MemoryFilters): number {
  return (
    (filters.query.trim() ? 1 : 0) +
    filters.types.length +
    filters.sources.length +
    filters.tiers.length +
    filters.signals.length +
    filters.lifecycle.length +
    (filters.timeRange === 'all' ? 0 : 1)
  );
}

export function projectMemoryGraph(
  data: MemoryOverview,
  entries: MemoryEntry[],
  scope: MemoryGraphScope,
  selectedMemoryId?: string,
  selectedEntityId?: string
): MemoryGraphProjection {
  const entryById = new Map(data.entries.map((entry) => [entry.id, entry]));
  const visibleMemoryIds = new Set(entries.map((entry) => entry.id));
  const allEvents = data.graph.events.filter((event) => visibleMemoryIds.has(event.memoryId));
  allEvents.sort((left, right) => {
    if (left.memoryId === selectedMemoryId) return -1;
    if (right.memoryId === selectedMemoryId) return 1;
    const importance =
      (entryById.get(right.memoryId)?.importance ?? 0) - (entryById.get(left.memoryId)?.importance ?? 0);
    return importance || Date.parse(right.timestamp) - Date.parse(left.timestamp);
  });

  const events = scope === 'balanced' ? allEvents.slice(0, BALANCED_EVENT_LIMIT) : allEvents;
  const eventMemoryIds = new Set(events.map((event) => event.memoryId));
  const allEventMemoryIds = new Set(allEvents.map((event) => event.memoryId));
  const allLinkedEntityIds = new Set(allEvents.flatMap((event) => event.entityIds));
  const shownLinkedEntityIds = new Set(events.flatMap((event) => event.entityIds));
  if (selectedEntityId) {
    allLinkedEntityIds.add(selectedEntityId);
    shownLinkedEntityIds.add(selectedEntityId);
  }
  const allEntities = data.graph.entities.filter(
    (entity) =>
      (entity.kind !== 'tag' || memoryTagLabel(entity.name) !== null) &&
      allLinkedEntityIds.has(entity.id) &&
      entity.memoryIds.some((memoryId) => visibleMemoryIds.has(memoryId))
  );
  allEntities.sort((left, right) => {
    if (left.id === selectedEntityId) return -1;
    if (right.id === selectedEntityId) return 1;
    return right.mentions - left.mentions || right.importance - left.importance || left.name.localeCompare(right.name);
  });
  const linkedEntities = allEntities.filter((entity) => shownLinkedEntityIds.has(entity.id));
  const entities = scope === 'balanced' ? linkedEntities.slice(0, BALANCED_ENTITY_LIMIT) : linkedEntities;
  const nodes = [...layoutEventNodes(events), ...layoutEntityNodes(entities)];
  const nodeIds = new Set(nodes.map((node) => node.id));
  const allNodeIds = new Set([...allEvents.map((event) => event.id), ...allEntities.map((entity) => entity.id)]);
  const allEdges = data.graph.relations.filter(
    (relation) =>
      allEventMemoryIds.has(relation.memoryId) && allNodeIds.has(relation.from) && allNodeIds.has(relation.to)
  );
  let edges = allEdges
    .filter(
      (relation) => eventMemoryIds.has(relation.memoryId) && nodeIds.has(relation.from) && nodeIds.has(relation.to)
    )
    .sort((left, right) => {
      const leftSelected =
        left.memoryId === selectedMemoryId || left.from === selectedEntityId || left.to === selectedEntityId;
      const rightSelected =
        right.memoryId === selectedMemoryId || right.from === selectedEntityId || right.to === selectedEntityId;
      return Number(rightSelected) - Number(leftSelected) || right.weight - left.weight;
    })
    .map((relation) => ({ ...relation }));
  if (scope === 'balanced') edges = edges.slice(0, BALANCED_RELATION_LIMIT);

  return {
    nodes,
    edges,
    totalNodes: allEvents.length + allEntities.length,
    totalEdges: allEdges.length,
    truncated:
      events.length < allEvents.length || entities.length < allEntities.length || edges.length < allEdges.length,
  };
}

export function memoryTypeCounts(entries: MemoryEntry[]): Array<[string, number]> {
  const counts = new Map<string, number>();
  for (const entry of entries) counts.set(entry.memoryType, (counts.get(entry.memoryType) ?? 0) + 1);
  return [...counts].sort((left, right) => right[1] - left[1] || left[0].localeCompare(right[0]));
}

export function eventForMemory(data: MemoryOverview, memoryId: string): MemoryGraphEvent | undefined {
  return data.graph.events.find((event) => event.memoryId === memoryId);
}

export function entityForId(data: MemoryOverview, entityId: string): MemoryGraphEntity | undefined {
  return data.graph.entities.find((entity) => entity.id === entityId);
}

function layoutEventNodes(events: MemoryGraphEvent[]): MemoryVisualNode[] {
  const centerX = 600;
  const centerY = 360;
  const goldenAngle = Math.PI * (3 - Math.sqrt(5));
  return events.map((event, index) => {
    const progress = events.length <= 1 ? 0 : Math.sqrt(index / (events.length - 1));
    const radius = events.length <= 1 ? 0 : 54 + progress * 185;
    const angle = index * goldenAngle - Math.PI / 2;
    return {
      id: event.id,
      nodeType: 'event',
      label: event.label,
      x: centerX + Math.cos(angle) * radius * 1.15,
      y: centerY + Math.sin(angle) * radius * 0.82,
      size: 8 + event.retentionScore * 5,
      tone: event.tier,
      memoryId: event.memoryId,
      relatedMemoryIds: [event.memoryId],
    };
  });
}

function layoutEntityNodes(entities: MemoryGraphEntity[]): MemoryVisualNode[] {
  const centerX = 600;
  const centerY = 360;
  const ordered = [...entities].sort(
    (left, right) =>
      left.kind.localeCompare(right.kind) || right.mentions - left.mentions || left.name.localeCompare(right.name)
  );
  return ordered.map((entity, index) => {
    const angle = (index / Math.max(ordered.length, 1)) * Math.PI * 2 - Math.PI / 2;
    const ring = index % 3;
    const radiusX = 385 + ring * 48;
    const radiusY = 248 + ring * 30;
    return {
      id: entity.id,
      nodeType: 'entity',
      label: entityNameLabel(entity.kind, entity.name),
      x: centerX + Math.cos(angle) * radiusX,
      y: centerY + Math.sin(angle) * radiusY,
      size: Math.min(13, 5.5 + Math.log2(entity.mentions + 1) * 2),
      tone: entity.kind,
      entityId: entity.id,
      relatedMemoryIds: entity.memoryIds,
    };
  });
}

function timeRangeMs(range: MemoryTimeRange): number | null {
  if (range === 'all') return null;
  const days = Number.parseInt(range, 10);
  return days * 24 * 60 * 60 * 1000;
}
