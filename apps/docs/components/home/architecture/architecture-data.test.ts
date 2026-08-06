import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, test } from "node:test";
import { architectureProjectCount, architectureProjects } from ".";
import { homeContent, lifecycleProjects } from "../home-content";

const repositoryReadme = readFileSync(
  new URL("../../../../../README.md", import.meta.url),
  "utf8",
);
const repositoryMap = repositoryReadme
  .split("## Repository map")[1]
  ?.split("## Development")[0];
const repositoryProjectNames = [
  ...(repositoryMap ?? "").matchAll(/\[(?:A3S )?([^\]]+)\]\([^)]+\)/g),
].map((match) => (match[1] === "CLI website" ? "Documentation" : match[1]));

describe("homepage architecture atlas", () => {
  test("covers the website project set with unique source repositories", () => {
    assert.equal(architectureProjectCount, 35);
    assert.equal(
      new Set(architectureProjects.map((project) => project.id)).size,
      architectureProjectCount,
    );
    for (const project of architectureProjects) {
      assert.ok(
        repositoryProjectNames.includes(project.name),
        `${project.name} is missing from the repository map`,
      );
    }
  });

  test("introduces every architecture project once across the lifecycle", () => {
    const lifecycleIds = Object.values(lifecycleProjects).flat();
    const englishIds = homeContent.en.aiNative.steps.flatMap(
      (step) => step.projects,
    );
    const chineseIds = homeContent.cn.aiNative.steps.flatMap(
      (step) => step.projects,
    );

    assert.equal(new Set(lifecycleIds).size, lifecycleIds.length);
    assert.deepEqual(englishIds, lifecycleIds);
    assert.deepEqual(chineseIds, lifecycleIds);
    assert.deepEqual(
      [...lifecycleIds].sort(),
      architectureProjects.map((project) => project.id).sort(),
    );
  });

  test("keeps the localized CLI terminal on the real command surface", () => {
    const expectedCommands = [
      "a3s config paths",
      "a3s model current",
      'a3s code exec "Check the API boundary"',
      "a3s web -d",
      "a3s doctor",
    ];
    const englishCommands = homeContent.en.hero.terminal.commands;
    const chineseCommands = homeContent.cn.hero.terminal.commands;

    assert.deepEqual(
      englishCommands.map((command) => command.command),
      expectedCommands,
    );
    assert.deepEqual(
      chineseCommands.map((command) => command.command),
      expectedCommands,
    );
    assert.deepEqual(
      englishCommands.map((command) => command.id),
      chineseCommands.map((command) => command.id),
    );
    for (const command of [...englishCommands, ...chineseCommands]) {
      assert.ok(command.summary.length > 0);
      assert.ok(command.output.length > 0);
    }
  });

  test("keeps the Cloud lifecycle terminal on real progressive and Cloud contracts", () => {
    const english = homeContent.en.cloudLifecycle;
    const chinese = homeContent.cn.cloudLifecycle;
    const expectedStageIds = [
      "develop",
      "discover",
      "deploy",
      "converge",
      "serve",
      "observe",
      "recover",
    ];

    assert.deepEqual(
      english.stages.map((stage) => stage.id),
      expectedStageIds,
    );
    assert.deepEqual(
      chinese.stages.map((stage) => stage.id),
      expectedStageIds,
    );

    for (const content of [english, chinese]) {
      assert.equal(
        content.contract[0]?.value,
        "POST /api/v1/kernel/capabilities",
      );
      assert.equal(content.contract[1]?.value, "search → describe → execute");
      assert.deepEqual(
        content.systems.map((system) => system.id),
        ["code", "os", "cloud", "runtime", "gateway"],
      );

      const commands = content.stages.map((stage) => stage.command);
      assert.ok(commands.includes("a3s code"));
      assert.deepEqual(
        content.stages.map((stage) => stage.prompt),
        ["$", "›", "›", "→", "›", "›", "›"],
      );

      const trace = content.stages
        .flatMap((stage) => stage.lines.map((line) => line.text))
        .join("\n");
      for (const marker of [
        '"action":"search"',
        '"action":"describe"',
        '"action":"execute"',
        "AgentBuildController_triggerAgentBuild",
        "TriggerAgentBuildRequestDto",
        "a3s_cloud_workloads_get",
        "a3s_cloud_routes_get",
        "a3s_cloud_workload_logs_get",
        "a3s_cloud_workloads_rollback",
        "workloadId · sourceRevisionId · idempotencyKey",
      ]) {
        assert.ok(trace.includes(marker), `Cloud trace is missing: ${marker}`);
      }

      for (const stage of content.stages) {
        assert.ok(stage.lines.length >= 5);
        assert.ok(stage.systems.length > 0);
        assert.ok(stage.result.length > 0);
      }
    }
  });

  test("uses project-specific topologies instead of one fixed template", () => {
    const nodeCounts = new Set(
      architectureProjects.map((project) => project.nodes.length),
    );
    const signatures = architectureProjects.map((project) =>
      project.nodes
        .map((node) => node.label)
        .sort()
        .join("|"),
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

  test("keeps diagram node hit areas from overlapping", () => {
    const nodeWidthPercent = 13;
    const nodeHeightPercent = (54 / 590) * 100;

    for (const project of architectureProjects) {
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

  test("keeps every diagram linked, localized, positioned, and sourced", () => {
    for (const project of architectureProjects) {
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

  test("preserves repository-map category totals", () => {
    const totals = architectureProjects.reduce<Record<string, number>>(
      (counts, project) => {
        counts[project.category] = (counts[project.category] ?? 0) + 1;
        return counts;
      },
      {},
    );

    assert.deepEqual(totals, { products: 16, runtime: 8, interfaces: 11 });
  });
});
