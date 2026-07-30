import { describe, expect, test } from 'bun:test';
import { architectureProjectCount, architectureProjects, systemArchitectureProject } from '.';

describe('homepage architecture atlas', () => {
  test('covers every project in the repository map', () => {
    expect(architectureProjectCount).toBe(34);
    expect(new Set(architectureProjects.map((project) => project.id)).size).toBe(34);
  });

  test('keeps every project diagram complete and localized', () => {
    for (const project of [systemArchitectureProject, ...architectureProjects]) {
      expect(project.nodes).toHaveLength(5);
      expect(project.role.cn.length).toBeGreaterThan(0);
      expect(project.role.en.length).toBeGreaterThan(0);

      const nodeIds = new Set(project.nodes.map((node) => node.id));
      expect(nodeIds.size).toBe(project.nodes.length);

      for (const node of project.nodes) {
        expect(node.detail.cn.length).toBeGreaterThan(0);
        expect(node.detail.en.length).toBeGreaterThan(0);
      }

      for (const [from, to] of project.links ?? []) {
        expect(nodeIds.has(from)).toBe(true);
        expect(nodeIds.has(to)).toBe(true);
      }
    }
  });

  test('preserves repository-map category totals', () => {
    const totals = architectureProjects.reduce<Record<string, number>>((counts, project) => {
      counts[project.category] = (counts[project.category] ?? 0) + 1;
      return counts;
    }, {});

    expect(totals).toEqual({ products: 14, runtime: 8, interfaces: 12 });
  });
});
