/**
 * Workspace utilities for managing agent default workspaces and session workspaces
 */
import { invoke } from "@tauri-apps/api/core";
import { homeDir } from "@tauri-apps/api/path";
import { writeTextFile, mkdir } from "@tauri-apps/plugin-fs";
import settingsModel from "@/models/settings.model";
import personaModel from "@/models/persona.model";

/**
 * Get default workspace root path
 */
async function getDefaultWorkspaceRoot(): Promise<string> {
	const home = await homeDir();
	return `${home}/.a3s/workspace`;
}

/**
 * Get agent workspace path: {workspaceRoot}/agents/{agentId}
 */
export async function getAgentWorkspacePath(agentId: string): Promise<string> {
	let workspaceRoot = settingsModel.state.agentDefaults.workspaceRoot;
	if (!workspaceRoot) {
		workspaceRoot = await getDefaultWorkspaceRoot();
	}
	return `${workspaceRoot}/agents/${agentId}`;
}

/**
 * Get session workspace path: {workspaceRoot}/sessions/{personaId}-YYYYMMDD-HHmmss
 * Special case: super-admin gets access to user's home directory
 */
export async function getSessionWorkspacePath(
	personaId: string,
): Promise<string | null> {
	// Super admin gets full system access via home directory
	if (personaId === "super-admin") {
		try {
			// Get user's home directory for full system access
			const home = await homeDir();
			return home;
		} catch (err) {
			console.error("Failed to get home directory for super-admin:", err);
			// Fallback: use root workspace
			const workspaceRoot = settingsModel.state.agentDefaults.workspaceRoot;
			return workspaceRoot || null;
		}
	}

	const workspaceRoot = settingsModel.state.agentDefaults.workspaceRoot;
	if (!workspaceRoot) return null;

	const now = new Date();
	const pad = (n: number) => String(n).padStart(2, "0");
	const date = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
	const time = `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
	const folderName = `${personaId}-${date}-${time}`;

	return `${workspaceRoot}/sessions/${folderName}`;
}

/**
 * Initialize default workspace for an agent
 * Creates directory structure: {workspaceRoot}/agents/{agentId}/{skills,flows,tasks,knowledge}
 */
export async function initializeAgentWorkspace(
	agentId: string,
): Promise<string | null> {
	try {
		const agentWorkspace = await getAgentWorkspacePath(agentId);
		await createWorkspaceStructure(agentWorkspace);

		// Save to persona model
		personaModel.setPersonaWorkspace(agentId, agentWorkspace);

		return agentWorkspace;
	} catch (err) {
		console.error("Failed to initialize agent workspace:", err);
		return null;
	}
}

/**
 * Create workspace directory structure with subfolders
 */
async function createWorkspaceStructure(workspacePath: string): Promise<void> {
	const subfolders = ["skills", "flows", "tasks", "knowledge"];

	// Create main workspace directory
	await mkdir(workspacePath, { recursive: true });

	// Create subfolders
	for (const subfolder of subfolders) {
		const subfolderPath = `${workspacePath}/${subfolder}`;
		await mkdir(subfolderPath, { recursive: true });
	}

	console.log(`Created workspace structure at: ${workspacePath}`);
}

/**
 * Get or initialize agent workspace
 * Returns existing workspace if already configured, otherwise creates new one
 */
export async function getOrInitializeAgentWorkspace(
	agentId: string,
): Promise<string | null> {
	// Check if workspace already exists
	const existingWorkspace = personaModel.getPersonaWorkspace(agentId);
	if (existingWorkspace) {
		return existingWorkspace;
	}

	// Initialize new workspace
	return await initializeAgentWorkspace(agentId);
}

/**
 * Initialize default skills and flows for a persona
 * Writes default skills and flows to the workspace directory
 */
export async function initializePersonaDefaults(
	sessionId: string,
	personaId: string,
): Promise<void> {
	try {
		// Get persona configuration
		const { getPersonaById } = await import("@/lib/builtin-personas");
		const persona = getPersonaById(personaId);

		if (!persona) {
			console.warn(`Persona ${personaId} not found`);
			return;
		}

		// Get or create agent workspace
		const workspace = await getOrInitializeAgentWorkspace(personaId);
		if (!workspace) {
			console.warn("No workspace available for persona defaults");
			return;
		}

		// Write default skills
		if (persona.defaultSkills && persona.defaultSkills.length > 0) {
			const skillsDir = `${workspace}/skills`;
			for (const skillName of persona.defaultSkills) {
				const skillPath = `${skillsDir}/${skillName}.md`;
				const skillContent = `---
name: ${skillName}
description: Built-in skill for ${persona.name}
enabled: true
---

# ${skillName}

This is a built-in skill automatically configured for ${persona.name}.
`;
				await writeTextFile(skillPath, skillContent);
			}
			console.log(
				`Initialized ${persona.defaultSkills.length} default skills for ${personaId}`,
			);
		}

		// Write default flows
		if (persona.defaultFlows && persona.defaultFlows.length > 0) {
			const flowsDir = `${workspace}/flows`;
			for (const flow of persona.defaultFlows) {
				const flowPath = `${flowsDir}/${flow.id}.json`;
				const flowContent = JSON.stringify(flow, null, 2);
				await writeTextFile(flowPath, flowContent);
			}
			console.log(
				`Initialized ${persona.defaultFlows.length} default flows for ${personaId}`,
			);
		}
	} catch (err) {
		console.error("Failed to initialize persona defaults:", err);
	}
}

/**
 * Initialize default skills and flows for a marketplace agent
 * Writes default skills and flows to the workspace directory
 */
export async function initializeMarketplaceAgentDefaults(
	sessionId: string,
	agent: {
		id: string;
		name: string;
		skills?: string[];
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
		}>;
	},
): Promise<void> {
	try {
		console.log(
			`[initializeMarketplaceAgentDefaults] Starting for agent: ${agent.id}, session: ${sessionId}`,
		);

		// Use agent.id as the workspace identifier
		const workspace = await getOrInitializeAgentWorkspace(agent.id);
		if (!workspace) {
			console.warn("No workspace available for marketplace agent defaults");
			return;
		}

		console.log(
			`[initializeMarketplaceAgentDefaults] Workspace path: ${workspace}`,
		);

		// Write default skills
		if (agent.skills && agent.skills.length > 0) {
			const skillsDir = `${workspace}/skills`;
			for (const skillName of agent.skills) {
				const skillPath = `${skillsDir}/${skillName}.md`;
				const skillContent = `---
name: ${skillName}
description: Built-in skill for ${agent.name}
enabled: true
---

# ${skillName}

This is a built-in skill automatically configured for ${agent.name}.
`;
				await writeTextFile(skillPath, skillContent);
			}
			console.log(
				`Initialized ${agent.skills.length} default skills for ${agent.id}`,
			);
		}

		// Write default flows
		if (agent.flows && agent.flows.length > 0) {
			const flowsDir = `${workspace}/flows`;
			for (const flow of agent.flows) {
				const flowPath = `${flowsDir}/${flow.id}.json`;
				const flowContent = JSON.stringify(flow, null, 2);
				await writeTextFile(flowPath, flowContent);
			}
			console.log(
				`Initialized ${agent.flows.length} default flows for ${agent.id}`,
			);
		}
	} catch (err) {
		console.error("Failed to initialize marketplace agent defaults:", err);
	}
}
