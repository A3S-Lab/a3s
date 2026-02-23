// Agent session state (mirrors Rust AgentSessionState)
export interface AgentSessionState {
	session_id: string;
	model: string;
	cwd: string;
	tools: string[];
	permission_mode: string;
	claude_code_version: string;
	mcp_servers: { name: string; status: string }[];
	agents: string[];
	slash_commands: string[];
	skills: string[];
	total_cost_usd: number;
	num_turns: number;
	context_used_percent: number;
	is_compacting: boolean;
	total_lines_added: number;
	total_lines_removed: number;
	persona_id?: string | null;
	// Token usage (optional — populated when backend sends usage data)
	input_tokens?: number;
	output_tokens?: number;
	cache_read_tokens?: number;
	cache_write_tokens?: number;
}

// Agent process info (mirrors Rust AgentProcessInfo)
export interface AgentProcessInfo {
	session_id: string;
	pid?: number;
	state: "starting" | "connected" | "running" | "exited";
	exit_code?: number | null;
	model?: string;
	permission_mode?: string;
	cwd: string;
	created_at: number;
	cli_session_id?: string;
	archived: boolean;
	name?: string;
	persona_id?: string | null;
}

// Persona info (mirrors Rust PersonaInfo)
export interface PersonaInfo {
	id: string;
	name: string;
	description: string;
	tags: string[];
	version?: string | null;
}

// Content blocks from Claude Code CLI
export type ContentBlock =
	| { type: "text"; text: string }
	| {
			type: "tool_use";
			id: string;
			name: string;
			input: Record<string, unknown>;
	  }
	| {
			type: "tool_result";
			tool_use_id: string;
			content: string | ContentBlock[];
			is_error?: boolean;
	  }
	| { type: "thinking"; thinking: string; budget_tokens?: number };

// Chat message displayed in the UI
export interface AgentChatMessage {
	id: string;
	role: "user" | "assistant" | "system";
	content: string;
	contentBlocks?: ContentBlock[];
	images?: { media_type: string; data: string }[];
	timestamp: number;
	parentToolUseId?: string | null;
	model?: string;
	stopReason?: string | null;
	/** Where this message originated from (app, dingtalk, feishu, wecom) */
	source?: string;
}

// Incoming agent-to-agent message notification
export interface AgentMessage {
	message_id: string;
	from_session_id: string;
	topic: string;
	content: string;
	auto_execute: boolean;
}

// Permission request from CLI
export interface PermissionRequest {
	request_id: string;
	tool_name: string;
	input: Record<string, unknown>;
	permission_suggestions?: unknown[];
	description?: string;
	tool_use_id?: string;
	agent_id?: string;
	timestamp: number;
}

// Server -> Browser messages
export type BrowserIncomingMessage =
	| { type: "session_init"; session: AgentSessionState }
	| { type: "session_update"; session: Partial<AgentSessionState> }
	| {
			type: "assistant";
			message: {
				id: string;
				role: string;
				model: string;
				content: ContentBlock[];
				stop_reason: string | null;
			};
			parent_tool_use_id: string | null;
	  }
	| {
			type: "stream_event";
			event: Record<string, unknown>;
			parent_tool_use_id: string | null;
	  }
	| { type: "result"; data: Record<string, unknown> }
	| { type: "permission_request"; request: PermissionRequest }
	| { type: "permission_cancelled"; request_id: string }
	| {
			type: "tool_progress";
			tool_use_id: string;
			tool_name: string;
			elapsed_time_seconds: number;
	  }
	| { type: "tool_use_summary"; summary: string; tool_use_ids: string[] }
	| { type: "status_change"; status: string | null }
	| {
			type: "auth_status";
			is_authenticating: boolean;
			output: string[];
			error?: string;
	  }
	| { type: "error"; message: string }
	| { type: "cli_connected" }
	| { type: "cli_disconnected" }
	| { type: "user_message"; content: string; timestamp: number }
	| { type: "message_history"; messages: BrowserIncomingMessage[] }
	| { type: "session_name_update"; name: string }
	| {
			type: "agent_message";
			message_id: string;
			from_session_id: string;
			topic: string;
			content: string;
			auto_execute: boolean;
	  }
	| {
			type: "command_response";
			command: string;
			text: string;
			state_changed: boolean;
	  };

// Browser -> Server messages
export type BrowserOutgoingMessage =
	| {
			type: "user_message";
			content: string;
			session_id?: string;
			images?: { media_type: string; data: string }[];
	  }
	| {
			type: "permission_response";
			request_id: string;
			behavior: "allow" | "deny";
			updated_input?: unknown;
			updated_permissions?: unknown[];
			message?: string;
	  }
	| { type: "interrupt" }
	| { type: "set_model"; model: string }
	| { type: "set_permission_mode"; mode: string }
	| { type: "set_system_prompt"; system_prompt: string }
	| { type: "send_agent_message"; target: string; content: string }
	| { type: "set_auto_execute"; enabled: boolean };
