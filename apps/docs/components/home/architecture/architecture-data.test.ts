import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { architectureProjectCount, architectureProjects, systemArchitectureProject } from '.';

describe('homepage architecture atlas', () => {
  test('covers every project in the repository map', () => {
    assert.equal(architectureProjectCount, 34);
    assert.equal(new Set(architectureProjects.map((project) => project.id)).size, 34);
  });

  test('keeps every project diagram complete and localized', () => {
    for (const project of [systemArchitectureProject, ...architectureProjects]) {
      assert.equal(project.nodes.length, 5);
      assert.ok(project.role.cn.length > 0);
      assert.ok(project.role.en.length > 0);

      const nodeIds = new Set(project.nodes.map((node) => node.id));
      assert.equal(nodeIds.size, project.nodes.length);

      for (const node of project.nodes) {
        assert.ok(node.detail.cn.length > 0);
        assert.ok(node.detail.en.length > 0);
      }

      for (const [from, to] of project.links ?? []) {
        assert.equal(nodeIds.has(from), true);
        assert.equal(nodeIds.has(to), true);
      }
    }
  });

  test('preserves repository-map category totals', () => {
    const totals = architectureProjects.reduce<Record<string, number>>((counts, project) => {
      counts[project.category] = (counts[project.category] ?? 0) + 1;
      return counts;
    }, {});

    assert.deepEqual(totals, { products: 14, runtime: 8, interfaces: 12 });
  });
});
