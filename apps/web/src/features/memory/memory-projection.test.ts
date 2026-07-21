import { describe, expect, it } from 'vitest';
import { memoryTestData } from './memory-test-data';
import {
  countActiveMemoryFilters,
  filterMemoryEntries,
  projectMemoryGraph,
  type MemoryFilters,
} from './memory-projection';

const emptyFilters: MemoryFilters = {
  query: '',
  types: [],
  sources: [],
  tiers: [],
  signals: [],
  lifecycle: [],
  timeRange: 'all',
};

describe('memory projection', () => {
  it('searches content, tags, metadata, sources, entity names, and aliases', () => {
    const data = memoryTestData();
    expect(filterMemoryEntries(data, { ...emptyFilters, query: 'focused cargo' }).map((entry) => entry.id)).toEqual([
      'procedure-1',
    ]);
    expect(filterMemoryEntries(data, { ...emptyFilters, query: 'src/lib.rs' }).map((entry) => entry.id)).toEqual([
      'semantic-1',
    ]);
    expect(filterMemoryEntries(data, { ...emptyFilters, query: 'OpenAI Codex' }).map((entry) => entry.id)).toEqual([
      'semantic-1',
    ]);
    expect(filterMemoryEntries(data, { ...emptyFilters, query: 'workflow' }).map((entry) => entry.id)).toEqual([
      'procedure-1',
    ]);
    expect(filterMemoryEntries(data, { ...emptyFilters, query: 'experiment' }).map((entry) => entry.id)).toEqual([
      'episodic-old',
    ]);
    expect(filterMemoryEntries(data, { ...emptyFilters, query: 'supersedes' }).map((entry) => entry.id)).toEqual([
      'procedure-1',
    ]);
  });

  it('combines type, tier, lifecycle, retention, source, and time filters', () => {
    const data = memoryTestData();
    expect(
      filterMemoryEntries(
        data,
        {
          ...emptyFilters,
          types: ['procedural'],
          sources: ['workflow'],
          tiers: ['long'],
          signals: ['protected'],
          lifecycle: ['consolidated', 'conflicts'],
          timeRange: '90d',
        },
        Date.parse('2026-07-20T10:00:00Z')
      ).map((entry) => entry.id)
    ).toEqual(['procedure-1']);
    expect(
      filterMemoryEntries(
        data,
        { ...emptyFilters, signals: ['candidate'], timeRange: '90d' },
        Date.parse('2026-07-20T10:00:00Z')
      )
    ).toHaveLength(0);
  });

  it('projects only the selected result neighborhood and preserves graph relations', () => {
    const data = memoryTestData();
    const entries = data.entries.filter((entry) => entry.id === 'procedure-1');
    const graph = projectMemoryGraph(data, entries, 'complete');

    expect(graph.nodes.map((node) => node.id)).toEqual(
      expect.arrayContaining(['event:procedure-1', 'tool:cargo', 'tag:tests'])
    );
    expect(graph.nodes.some((node) => node.id === 'event:semantic-1')).toBe(false);
    expect(graph.edges).toHaveLength(3);
    expect(graph.truncated).toBe(false);
  });

  it('reports complete node and edge totals when the balanced event cap is active', () => {
    const template = memoryTestData();
    const entries = Array.from({ length: 40 }, (_, index) => ({
      ...template.entries[0],
      id: `memory-${index}`,
    }));
    const events = entries.map((entry, index) => ({
      ...template.graph.events[0],
      id: `event:${entry.id}`,
      memoryId: entry.id,
      entityIds: [`entity-${index}`],
    }));
    const entities = entries.map((entry, index) => ({
      ...template.graph.entities[0],
      id: `entity-${index}`,
      name: `Entity ${index}`,
      memoryIds: [entry.id],
    }));
    const relations = entries.map((entry, index) => ({
      id: index,
      from: `event:${entry.id}`,
      to: `entity-${index}`,
      kind: 'mentions',
      memoryId: entry.id,
      weight: 1,
    }));
    const data = {
      ...template,
      entries,
      stats: { ...template.stats, entries: entries.length },
      graph: {
        ...template.graph,
        events,
        entities,
        relations,
        facets: {},
      },
    };

    const graph = projectMemoryGraph(data, entries, 'balanced');

    expect(graph.nodes).toHaveLength(72);
    expect(graph.edges).toHaveLength(36);
    expect(graph.totalNodes).toBe(80);
    expect(graph.totalEdges).toBe(40);
    expect(graph.truncated).toBe(true);
  });

  it('counts each active filter so the reset action remains truthful', () => {
    expect(
      countActiveMemoryFilters({
        ...emptyFilters,
        query: 'rust',
        types: ['semantic'],
        sources: ['preference'],
        tiers: ['short'],
        lifecycle: ['llm'],
        timeRange: '30d',
      })
    ).toBe(6);
  });
});
