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
	// Agent configuration
	systemPrompt?: string; // System prompt for the agent
	skills?: string[]; // Built-in skills to enable
	flows?: Array<{
		id: string;
		name: string;
		description: string;
		trigger?: string;
		steps: Array<{
			type: string;
			action: string;
			params?: Record<string, any>;
		}>;
	}>; // Default workflow configurations
	// Box integration fields (optional, for future container-based agents)
	image?: string; // OCI image reference (e.g., "ghcr.io/a3s-lab/agent-code-reviewer:latest")
	containerId?: string; // Box container ID if installed
	sessionId?: string; // Session ID if installed locally
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
 * Install agent by creating a local session with configuration
 */
export async function installAgent(agent: MarketplaceAgent): Promise<string> {
	console.log("[installAgent] Starting installation for:", agent.name);

	// Import dependencies
	const { agentApi } = await import("@/lib/agent-api");
	const { homeDir } = await import("@tauri-apps/api/path");
	const homePath = await homeDir();

	console.log("[installAgent] Home path:", homePath);

	// Check if agent is already installed (check for existing sessions with this persona_id)
	const existingSessions = await agentApi.listSessions();
	const alreadyInstalled = existingSessions?.some(
		(s: any) => s.persona_id === agent.id,
	);

	if (alreadyInstalled) {
		throw new Error(`智能体 "${agent.name}" 已经安装，无法重复安装`);
	}

	// Create a session for this agent with its configuration
	// Set persona_id to agent.id so skills/flows are stored in the correct workspace
	const session = await agentApi.createSession({
		cwd: homePath,
		persona_id: agent.id, // Use marketplace agent ID as persona ID
		system_prompt: agent.systemPrompt,
		skills: agent.skills,
		permission_mode: "permissive", // Allow agent to work freely
	});

	console.log("[installAgent] Session created:", session);

	if (!session?.session_id) {
		throw new Error("Failed to create session");
	}

	// Set session name to agent name
	await agentApi.updateSession(session.session_id, { name: agent.name });
	console.log("[installAgent] Session name updated");

	// Initialize marketplace agent defaults (skills and flows)
	try {
		const { initializeMarketplaceAgentDefaults } = await import(
			"@/lib/workspace-utils"
		);
		await initializeMarketplaceAgentDefaults(session.session_id, {
			id: agent.id,
			name: agent.name,
			skills: agent.skills,
			flows: agent.flows,
		});
		console.log("[installAgent] Defaults initialized");
	} catch (err) {
		console.error("[installAgent] Failed to initialize defaults:", err);
	}

	// Reload sessions and update UI state
	const agentModel = (await import("@/models/agent.model")).default;
	const personaModel = (await import("@/models/persona.model")).default;
	const { connectSession } = await import("@/hooks/use-agent-ws");

	// Register marketplace agent as a custom persona so it appears in the list
	// Use a consistent avatar based on agent category
	const avatarConfig = {
		sex: "man" as const,
		faceColor: "#F9C9B6",
		earSize: "small" as const,
		hairColor: "#000",
		hairStyle: "normal" as const,
		hatStyle: "none" as const,
		eyeStyle: "circle" as const,
		glassesStyle: "none" as const,
		noseStyle: "short" as const,
		mouthStyle: "smile" as const,
		shirtStyle: "polo" as const,
		shirtColor: "#4A90E2",
		bgColor: "#E8F4F8",
	};

	personaModel.addCustomPersona({
		id: agent.id,
		name: agent.name,
		description: agent.description,
		avatar: avatarConfig,
		systemPrompt: agent.systemPrompt || "",
		defaultSkills: agent.skills,
		defaultFlows: agent.flows,
	});
	console.log("[installAgent] Registered custom persona:", agent.id);

	// Reload all sessions from backend
	const sessions = await agentApi.listSessions();
	console.log("[installAgent] Loaded sessions:", sessions?.length);

	if (Array.isArray(sessions)) {
		// Set persona mappings and session names for all sessions
		for (const s of sessions) {
			if (s.persona_id) {
				personaModel.setSessionPersona(s.session_id, s.persona_id);
			}
			if (s.name && !agentModel.state.sessionNames[s.session_id]) {
				agentModel.setSessionName(s.session_id, s.name);
			}
		}

		// Update sessions list
		agentModel.setSdkSessions(sessions);
		console.log("[installAgent] Sessions updated in model");

		// Connect WebSocket for all active sessions
		for (const s of sessions) {
			if (s.state !== "exited") {
				connectSession(s.session_id);
			}
		}

		// Set the newly created session as current
		agentModel.setCurrentSession(session.session_id);
		console.log("[installAgent] Set current session:", session.session_id);
	}

	console.log("[installAgent] Installation complete");
	return session.session_id;
}

/**
 * Uninstall agent by deleting its session
 */
export async function uninstallAgent(
	sessionId: string,
	force = false,
): Promise<void> {
	const { agentApi } = await import("@/lib/agent-api");
	await agentApi.deleteSession(sessionId);

	// Reload sessions to update the UI
	const agentModel = (await import("@/models/agent.model")).default;
	const sessions = await agentApi.listSessions();
	if (Array.isArray(sessions)) {
		agentModel.setSdkSessions(sessions);
	}
}

/**
 * Get installed agents from sessions
 */
export async function getInstalledAgents(): Promise<
	Array<{ agentId: string; sessionId: string; status: string }>
> {
	const { agentApi } = await import("@/lib/agent-api");
	const sessions = await agentApi.listSessions();
	// For now, return empty array as we don't have agent metadata in sessions yet
	// TODO: Add agent metadata to sessions to track which marketplace agent they came from
	return [];
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
