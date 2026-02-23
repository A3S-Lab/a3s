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
}
