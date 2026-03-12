import type { AvatarFullConfig } from "react-nice-avatar";

export interface AgentPersona {
	id: string;
	name: string;
	description: string;
	avatar: AvatarFullConfig;
	systemPrompt: string;
	defaultModel?: string;
	defaultPermissionMode?: string;
	builtin?: boolean;
	/** If true, cannot be deleted by user */
	undeletable?: boolean;
	/** Category tags for marketplace filtering */
	tags?: string[];
	/** Default workspace path for this agent (shared across all sessions) */
	defaultWorkspace?: string;
	/** Default skills to enable for this persona */
	defaultSkills?: string[];
	/** Default workflow configurations */
	defaultFlows?: Array<{
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
}
