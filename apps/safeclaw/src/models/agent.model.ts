import { proxy } from "valtio";
import type {
	AgentChatMessage,
	AgentMessage,
	AgentProcessInfo,
	AgentSessionState,
	PermissionRequest,
} from "@/typings/agent";

export interface ToolProgress {
	tool_use_id: string;
	tool_name: string;
	elapsed_time_seconds: number;
	/** Tool input summary (e.g. file path, command) */
	input?: string;
}

/** A completed tool call shown during streaming */
export interface CompletedToolCall {
	tool_use_id: string;
	tool_name: string;
	input: string;
	output: string;
	is_error: boolean;
}

export interface AuthStatus {
	is_authenticating: boolean;
	output: string[];
	error?: string;
}

interface AgentStoreState {
	// Sessions
	sessions: Record<string, AgentSessionState>;
	sdkSessions: AgentProcessInfo[];
	currentSessionId: string | null;

	// Messages
	messages: Record<string, AgentChatMessage[]>;
	streaming: Record<string, string>;
	streamingStartedAt: Record<string, number>;

	// Permissions
	pendingPermissions: Record<string, Record<string, PermissionRequest>>;

	// Agent-to-agent messages (pending confirm-mode messages)
	agentMessages: Record<string, AgentMessage[]>;

	// Active tool progress per session
	activeToolProgress: Record<string, ToolProgress | null>;

	// Completed tool calls during current streaming (cleared on new generation)
	completedTools: Record<string, CompletedToolCall[]>;

	// Auth status per session (OAuth flow)
	authStatus: Record<string, AuthStatus | null>;

	// Connection
	connectionStatus: Record<string, "connecting" | "connected" | "disconnected">;
	cliConnected: Record<string, boolean>;
	sessionStatus: Record<string, "idle" | "running" | "compacting" | null>;

	// UI
	sessionNames: Record<string, string>;
	/** Unread message count per session (for sidebar badges) */
	unreadCounts: Record<string, number>;
}

const STORAGE_KEY_SESSION = "safeclaw-agent-current-session";
const STORAGE_KEY_NAMES = "safeclaw-agent-session-names";

const state = proxy<AgentStoreState>({
	sessions: {},
	sdkSessions: [],
	currentSessionId: localStorage.getItem(STORAGE_KEY_SESSION),
	messages: {},
	streaming: {},
	streamingStartedAt: {},
	pendingPermissions: {},
	agentMessages: {},
	activeToolProgress: {},
	completedTools: {},
	authStatus: {},
	connectionStatus: {},
	cliConnected: {},
	sessionStatus: {},
	sessionNames: JSON.parse(localStorage.getItem(STORAGE_KEY_NAMES) || "{}"),
	unreadCounts: {},
});

const actions = {
	// --- Sessions ---
	setCurrentSession(id: string | null) {
		state.currentSessionId = id;
		if (id) {
			localStorage.setItem(STORAGE_KEY_SESSION, id);
		} else {
			localStorage.removeItem(STORAGE_KEY_SESSION);
		}
	},

	addSession(session: AgentSessionState) {
		state.sessions[session.session_id] = session;
	},

	updateSession(sessionId: string, updates: Partial<AgentSessionState>) {
		const existing = state.sessions[sessionId];
		if (existing) {
			Object.assign(existing, updates);
		}
	},

	removeSession(sessionId: string) {
		delete state.sessions[sessionId];
		delete state.messages[sessionId];
		delete state.streaming[sessionId];
		delete state.streamingStartedAt[sessionId];
		delete state.pendingPermissions[sessionId];
		delete state.agentMessages[sessionId];
		delete state.activeToolProgress[sessionId];
		delete state.authStatus[sessionId];
		delete state.connectionStatus[sessionId];
		delete state.cliConnected[sessionId];
		delete state.sessionStatus[sessionId];
		delete state.sessionNames[sessionId];
		localStorage.setItem(STORAGE_KEY_NAMES, JSON.stringify(state.sessionNames));
		if (state.currentSessionId === sessionId) {
			state.currentSessionId = null;
			localStorage.removeItem(STORAGE_KEY_SESSION);
		}
	},

	setSdkSessions(sessions: AgentProcessInfo[]) {
		state.sdkSessions = sessions;
	},

	// --- Messages ---
	appendMessage(sessionId: string, msg: AgentChatMessage) {
		if (!state.messages[sessionId]) {
			state.messages[sessionId] = [];
		}
		state.messages[sessionId].push(msg);
	},

	setMessages(sessionId: string, msgs: AgentChatMessage[]) {
		state.messages[sessionId] = msgs;
	},

	setStreaming(sessionId: string, text: string | null) {
		if (text === null) {
			delete state.streaming[sessionId];
			delete state.streamingStartedAt[sessionId];
		} else {
			state.streaming[sessionId] = text;
		}
	},

	setStreamingStartedAt(sessionId: string, ts: number) {
		state.streamingStartedAt[sessionId] = ts;
	},

	// --- Permissions ---
	addPermission(sessionId: string, perm: PermissionRequest) {
		if (!state.pendingPermissions[sessionId]) {
			state.pendingPermissions[sessionId] = {};
		}
		state.pendingPermissions[sessionId][perm.request_id] = perm;
	},

	removePermission(sessionId: string, requestId: string) {
		if (state.pendingPermissions[sessionId]) {
			delete state.pendingPermissions[sessionId][requestId];
		}
	},

	// --- Connection ---
	setConnectionStatus(
		sessionId: string,
		status: "connecting" | "connected" | "disconnected",
	) {
		state.connectionStatus[sessionId] = status;
	},

	setCliConnected(sessionId: string, connected: boolean) {
		state.cliConnected[sessionId] = connected;
	},

	setSessionStatus(
		sessionId: string,
		status: "idle" | "running" | "compacting" | null,
	) {
		state.sessionStatus[sessionId] = status;
	},

	// --- Names ---
	setSessionName(sessionId: string, name: string) {
		state.sessionNames[sessionId] = name;
		localStorage.setItem(STORAGE_KEY_NAMES, JSON.stringify(state.sessionNames));
	},

	// --- Unread ---
	incrementUnread(sessionId: string, count = 1) {
		state.unreadCounts[sessionId] =
			(state.unreadCounts[sessionId] || 0) + count;
	},

	clearUnread(sessionId: string) {
		delete state.unreadCounts[sessionId];
	},

	// --- Agent-to-agent messages ---
	addAgentMessage(sessionId: string, msg: AgentMessage) {
		if (!state.agentMessages[sessionId]) {
			state.agentMessages[sessionId] = [];
		}
		state.agentMessages[sessionId].push(msg);
	},

	removeAgentMessage(sessionId: string, messageId: string) {
		if (state.agentMessages[sessionId]) {
			state.agentMessages[sessionId] = state.agentMessages[sessionId].filter(
				(m) => m.message_id !== messageId,
			);
		}
	},

	clearAgentMessages(sessionId: string) {
		delete state.agentMessages[sessionId];
	},

	// --- Tool progress ---
	setToolProgress(sessionId: string, progress: ToolProgress | null) {
		state.activeToolProgress[sessionId] = progress;
	},

	addCompletedTool(sessionId: string, tool: CompletedToolCall) {
		if (!state.completedTools[sessionId]) {
			state.completedTools[sessionId] = [];
		}
		state.completedTools[sessionId].push(tool);
	},

	clearCompletedTools(sessionId: string) {
		delete state.completedTools[sessionId];
	},

	// --- Auth status ---
	setAuthStatus(sessionId: string, status: AuthStatus | null) {
		state.authStatus[sessionId] = status;
	},
};

export default { state, ...actions };
