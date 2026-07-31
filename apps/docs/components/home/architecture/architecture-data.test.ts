import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { describe, test } from 'node:test';
import {
  architectureProjectCount,
  architectureProjects,
  systemArchitectureProject,
} from '.';

const repositoryReadme = readFileSync(
  new URL('../../../../../README.md', import.meta.url),
  'utf8',
);
const repositoryMap = repositoryReadme
  .split('## Repository map')[1]
  ?.split('## Development')[0];
const repositoryProjectNames = [
  ...(repositoryMap ?? '').matchAll(
    /^\| \[(?:A3S )?([^\]]+)\]\([^)]+\) \|/gm,
  ),
].map((match) =>
  match[1] === 'CLI website' ? 'Documentation' : match[1],
);

describe('homepage architecture atlas', () => {
  test('covers every project in the repository map', () => {
    assert.equal(architectureProjectCount, 37);
    assert.equal(
      new Set(architectureProjects.map((project) => project.id)).size,
      architectureProjectCount,
    );
    assert.deepEqual(
      architectureProjects.map((project) => project.name).sort(),
      repositoryProjectNames.sort(),
    );
  });

  test('uses project-specific topologies instead of one fixed template', () => {
    const nodeCounts = new Set(
      architectureProjects.map((project) => project.nodes.length),
    );
    const signatures = architectureProjects.map((project) =>
      project.nodes
        .map((node) => node.label)
        .sort()
        .join('|'),
    );

    assert.ok(nodeCounts.size >= 6);
    assert.equal(new Set(signatures).size, architectureProjects.length);
    assert.equal(
      new Set(architectureProjects.map((project) => project.role.cn)).size,
      architectureProjects.length,
    );
    assert.equal(
      new Set(architectureProjects.map((project) => project.role.en)).size,
      architectureProjects.length,
    );
  });

  test('keeps diagram node hit areas from overlapping', () => {
    const nodeWidthPercent = 13;
    const nodeHeightPercent = (54 / 590) * 100;

    for (const project of [
      systemArchitectureProject,
      ...architectureProjects,
    ]) {
      for (const [index, node] of project.nodes.entries()) {
        for (const otherNode of project.nodes.slice(index + 1)) {
          const horizontalGap = Math.abs(
            node.position.x - otherNode.position.x,
          );
          const verticalGap = Math.abs(node.position.y - otherNode.position.y);

          assert.ok(
            horizontalGap >= nodeWidthPercent ||
              verticalGap >= nodeHeightPercent,
            `${project.name}: ${node.label} overlaps ${otherNode.label}`,
          );
        }
      }
    }
  });

  test('keeps every diagram linked, localized, positioned, and sourced', () => {
    for (const project of [
      systemArchitectureProject,
      ...architectureProjects,
    ]) {
      assert.ok(project.nodes.length >= 7);
      assert.ok(project.edges.length >= project.nodes.length - 1);
      assert.ok(project.role.cn.length > 0);
      assert.ok(project.role.en.length > 0);
      assert.ok(project.evidence.length > 0);

      for (const source of project.evidence) {
        assert.ok(source.label.length > 0);
        assert.match(source.href, /^https:\/\//);
      }

      const nodeIds = new Set(project.nodes.map((node) => node.id));
      assert.equal(nodeIds.size, project.nodes.length);

      const adjacency = new Map(
        project.nodes.map((node) => [node.id, new Set<string>()]),
      );
      for (const node of project.nodes) {
        assert.ok(node.detail.cn.length > 0);
        assert.ok(node.detail.en.length > 0);
        assert.ok(node.position.x >= 5 && node.position.x <= 95);
        assert.ok(node.position.y >= 5 && node.position.y <= 95);
      }

      for (const architectureEdge of project.edges) {
        assert.equal(nodeIds.has(architectureEdge.from), true);
        assert.equal(nodeIds.has(architectureEdge.to), true);
        assert.ok(architectureEdge.label.cn.length > 0);
        assert.ok(architectureEdge.label.en.length > 0);
        adjacency.get(architectureEdge.from)?.add(architectureEdge.to);
        adjacency.get(architectureEdge.to)?.add(architectureEdge.from);
      }

      for (const node of project.nodes) {
        assert.ok(
          (adjacency.get(node.id)?.size ?? 0) > 0,
          `${project.name}.${node.id} has no architecture relation`,
        );
      }

      for (const architectureGroup of project.groups ?? []) {
        assert.ok(architectureGroup.label.cn.length > 0);
        assert.ok(architectureGroup.label.en.length > 0);
        assert.ok(architectureGroup.x >= 0 && architectureGroup.y >= 0);
        assert.ok(architectureGroup.width > 0 && architectureGroup.height > 0);
        assert.ok(architectureGroup.x + architectureGroup.width <= 100);
        assert.ok(architectureGroup.y + architectureGroup.height <= 100);
      }
    }
  });

  test('preserves repository-map category totals', () => {
    const totals = architectureProjects.reduce<Record<string, number>>(
      (counts, project) => {
        counts[project.category] = (counts[project.category] ?? 0) + 1;
        return counts;
      },
      {},
    );

    assert.deepEqual(totals, { products: 17, runtime: 8, interfaces: 12 });
  });
});
