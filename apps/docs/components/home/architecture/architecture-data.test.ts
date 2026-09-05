import assert from 'node:assert/strict';
import { describe, test } from 'node:test';
import { architectureProjects } from '.';
import { linkedProjectIds } from '../project-links';

describe('homepage ecosystem project data', () => {
  test('covers every project in the repository map', () => {
    assert.equal(architectureProjects.length, 35);
    assert.equal(new Set(architectureProjects.map((project) => project.id)).size, 35);
    assert.deepEqual(
      [...linkedProjectIds].sort(),
      architectureProjects.map((project) => project.id).sort(),
    );
  });

  test('does not link the ecosystem directory to removed docs or tutorials', () => {
    for (const project of architectureProjects) {
      assert.equal(project.href.includes('/docs'), false, `${project.name} still links to docs`);
      assert.equal(project.href.includes('/tutorials'), false, `${project.name} still links to tutorials`);
    }
  });

  test('keeps every project capability summary complete and localized', () => {
    for (const project of architectureProjects) {
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

    assert.deepEqual(totals, { products: 14, runtime: 8, interfaces: 13 });
  });
});
