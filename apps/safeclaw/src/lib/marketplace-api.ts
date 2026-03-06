/**
 * Marketplace API client.
 * Integrates with A3S Box for agent/skill installation.
 */
import { apiFetch, jsonBody } from "@/lib/http";
import * as boxApi from "@/lib/box-api";
import boxModel from "@/models/box.model";

export interface MarketplaceAgent {
	id: string;
	name: string;
	description: string;
	category: string;
	author: string;
	avatar: string;
	downloads: number;
	rating: number;
	tags: string[];
	installed: boolean;
	// Box integration fields
	image: string; // OCI image reference (e.g., "ghcr.io/a3s-lab/agent-code-reviewer:latest")
	containerId?: string; // Box container ID if installed
}

export interface MarketplaceSkill {
	id: string;
	name: string;
	description: string;
	category: string;
	author: string;
	icon: string;
	downloads: number;
	rating: number;
	tags: string[];
	installed: boolean;
	version: string;
	// Box integration fields
	image?: string; // OCI image if skill runs as container
	pluginPath?: string; // Plugin path if skill is a plugin
	containerId?: string; // Box container ID if installed
}

// ── Agents ───────────────────────────────────────────────────────

/**
 * List available agents from marketplace
 */
export async function listMarketplaceAgents(): Promise<MarketplaceAgent[]> {
	// TODO: Replace with actual marketplace API
	return apiFetch("/marketplace/agents");
}

/**
 * Install agent by pulling image and creating container
 */
export async function installAgent(agent: MarketplaceAgent): Promise<string> {
	// 1. Pull image
	await boxApi.pullImage(agent.image);
	await boxModel.fetchImages();

	// 2. Create container
	const container = await apiFetch(
		"/box/containers/create",
		jsonBody("POST", {
			image: agent.image,
			name: `agent-${agent.id}`,
			labels: {
				"a3s.marketplace.type": "agent",
				"a3s.marketplace.id": agent.id,
				"a3s.marketplace.name": agent.name,
			},
			// Auto-start container
			autoStart: true,
		}),
	);

	await boxModel.fetchBoxes();
	return container.id;
}

/**
 * Uninstall agent by stopping and removing container
 */
export async function uninstallAgent(
	containerId: string,
	force = false,
): Promise<void> {
	// Stop container first
	try {
		await boxApi.stopBox(containerId);
	} catch {
		// Ignore if already stopped
	}

	// Remove container
	await boxApi.removeBox(containerId, force);
	await boxModel.fetchBoxes();
}

/**
 * Get installed agents from Box containers
 */
export async function getInstalledAgents(): Promise<
	Array<{ agentId: string; containerId: string; status: string }>
> {
	const boxes = await boxApi.listBoxes(true);
	return boxes
		.filter((box) => box.labels?.["a3s.marketplace.type"] === "agent")
		.map((box) => ({
			agentId: box.labels?.["a3s.marketplace.id"] || "",
			containerId: box.id,
			status: box.status,
		}));
}

// ── Skills ───────────────────────────────────────────────────────

/**
 * List available skills from marketplace
 */
export async function listMarketplaceSkills(): Promise<MarketplaceSkill[]> {
	// TODO: Replace with actual marketplace API
	return apiFetch("/marketplace/skills");
}

/**
 * Install skill (container-based or plugin-based)
 */
export async function installSkill(skill: MarketplaceSkill): Promise<string> {
	if (skill.image) {
		// Container-based skill
		await boxApi.pullImage(skill.image);
		await boxModel.fetchImages();

		const container = await apiFetch(
			"/box/containers/create",
			jsonBody("POST", {
				image: skill.image,
				name: `skill-${skill.id}`,
				labels: {
					"a3s.marketplace.type": "skill",
					"a3s.marketplace.id": skill.id,
					"a3s.marketplace.name": skill.name,
					"a3s.marketplace.version": skill.version,
				},
				autoStart: true,
			}),
		);

		await boxModel.fetchBoxes();
		return container.id;
	}

	if (skill.pluginPath) {
		// Plugin-based skill
		await apiFetch(
			"/marketplace/skills/install",
			jsonBody("POST", {
				skillId: skill.id,
				pluginPath: skill.pluginPath,
			}),
		);
		return skill.id;
	}

	throw new Error("Skill has no installation method");
}

/**
 * Uninstall skill
 */
export async function uninstallSkill(
	skill: MarketplaceSkill,
	force = false,
): Promise<void> {
	if (skill.containerId) {
		// Container-based skill
		try {
			await boxApi.stopBox(skill.containerId);
		} catch {
			// Ignore if already stopped
		}
		await boxApi.removeBox(skill.containerId, force);
		await boxModel.fetchBoxes();
	} else {
		// Plugin-based skill
		await apiFetch(
			`/marketplace/skills/${skill.id}/uninstall`,
			jsonBody("POST", {}),
		);
	}
}

/**
 * Get installed skills from Box containers
 */
export async function getInstalledSkills(): Promise<
	Array<{ skillId: string; containerId: string; status: string }>
> {
	const boxes = await boxApi.listBoxes(true);
	return boxes
		.filter((box) => box.labels?.["a3s.marketplace.type"] === "skill")
		.map((box) => ({
			skillId: box.labels?.["a3s.marketplace.id"] || "",
			containerId: box.id,
			status: box.status,
		}));
}

// ── Sync ─────────────────────────────────────────────────────────

/**
 * Sync marketplace items with Box containers
 * Updates installed status based on running containers
 */
export async function syncMarketplaceWithBox(
	agents: MarketplaceAgent[],
	skills: MarketplaceSkill[],
): Promise<{
	agents: MarketplaceAgent[];
	skills: MarketplaceSkill[];
}> {
	const [installedAgents, installedSkills] = await Promise.all([
		getInstalledAgents(),
		getInstalledSkills(),
	]);

	// Update agents
	const syncedAgents = agents.map((agent) => {
		const installed = installedAgents.find((a) => a.agentId === agent.id);
		return {
			...agent,
			installed: !!installed,
			containerId: installed?.containerId,
		};
	});

	// Update skills
	const syncedSkills = skills.map((skill) => {
		const installed = installedSkills.find((s) => s.skillId === skill.id);
		return {
			...skill,
			installed: !!installed,
			containerId: installed?.containerId,
		};
	});

	return { agents: syncedAgents, skills: syncedSkills };
}
