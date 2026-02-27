import agentModel from "@/models/agent.model";
import type { AgentTask } from "@/models/agent.model";
import { getGatewayUrl } from "@/models/settings.model";
import dayjs from "dayjs";
import type {
	AgentChatMessage,
	BrowserIncomingMessage,
	BrowserOutgoingMessage,
} from "@/typings/agent";

// Module-level state (outside Valtio to avoid proxy overhead)
const sockets = new Map<string, WebSocket>();
const reconnectTimers = new Map<string, ReturnType<typeof setTimeout>>();
const reconnectAttempts = new Map<string, number>();
/** Temporarily store images from outgoing user messages to attach to the echo */
const pendingUserImages = new Map<
	string,
	{ media_type: string; data: string }[]
>();

function getWsUrl(sessionId: string): string {
	const base = getGatewayUrl().replace(/^http/, "ws");
	return `${base}/ws/agent/browser/${sessionId}`;
}

/** Connect a WebSocket for the given session */
export function connectSession(sessionId: string): void {
	if (sockets.has(sessionId)) return;

	agentModel.setConnectionStatus(sessionId, "connecting");

	const ws = new WebSocket(getWsUrl(sessionId));
	sockets.set(sessionId, ws);

	ws.onopen = () => {
		agentModel.setConnectionStatus(sessionId, "connected");
		reconnectAttempts.delete(sessionId);
		const timer = reconnectTimers.get(sessionId);
		if (timer) {
			clearTimeout(timer);
			reconnectTimers.delete(sessionId);
		}
	};

	ws.onmessage = (event) => {
		try {
			const data = JSON.parse(event.data) as BrowserIncomingMessage;
			handleMessage(sessionId, data);
		} catch {
			// Ignore malformed messages
		}
	};

	ws.onclose = () => {
		sockets.delete(sessionId);
		agentModel.setConnectionStatus(sessionId, "disconnected");
		scheduleReconnect(sessionId);
	};

	ws.onerror = (event) => {
		console.warn(`WebSocket error for session ${sessionId}:`, event);
		ws.close();
	};
}

/** Disconnect a session's WebSocket */
export function disconnectSession(sessionId: string): void {
	const timer = reconnectTimers.get(sessionId);
	if (timer) {
		clearTimeout(timer);
		reconnectTimers.delete(sessionId);
	}
	reconnectAttempts.delete(sessionId);
	const ws = sockets.get(sessionId);
	if (ws) {
		ws.close();
		sockets.delete(sessionId);
	}
	agentModel.setConnectionStatus(sessionId, "disconnected");
}

/** Disconnect all sessions */
export function disconnectAll(): void {
	for (const sessionId of sockets.keys()) {
		disconnectSession(sessionId);
	}
}

/** Send a message to a session's WebSocket. Returns true if sent, false if WS not ready. */
export function sendToSession(
	sessionId: string,
	msg: BrowserOutgoingMessage,
): boolean {
	const ws = sockets.get(sessionId);
	if (ws?.readyState === WebSocket.OPEN) {
		ws.send(JSON.stringify(msg));

		if (msg.type === "user_message") {
			agentModel.setSessionStatus(sessionId, "running");
			// Store user message locally with images (server echo won't include images)
			if (msg.images && msg.images.length > 0) {
				pendingUserImages.set(sessionId, msg.images);
			}
		}
		return true;
	}
	return false;
}

function scheduleReconnect(sessionId: string): void {
	if (reconnectTimers.has(sessionId)) return;

	const attempts = reconnectAttempts.get(sessionId) || 0;
	// Exponential backoff: 1s, 2s, 4s, 8s, 16s, max 30s
	const delay = Math.min(1000 * 2 ** attempts, 30000);
	reconnectAttempts.set(sessionId, attempts + 1);

	const timer = setTimeout(() => {
		reconnectTimers.delete(sessionId);
		// Only reconnect if session still exists
		if (agentModel.state.sessions[sessionId]) {
			connectSession(sessionId);
		}
	}, delay);

	reconnectTimers.set(sessionId, timer);
}

function handleMessage(sessionId: string, msg: BrowserIncomingMessage): void {
	switch (msg.type) {
		case "session_init":
			agentModel.addSession(msg.session);
			agentModel.setCliConnected(sessionId, true);
			agentModel.setSessionStatus(sessionId, "idle");
			break;

		case "session_update":
			agentModel.updateSession(sessionId, msg.session);
			break;

		case "assistant": {
			const textParts = msg.message.content
				.filter((b) => b.type === "text")
				.map((b) => (b as { type: "text"; text: string }).text)
				.join("");

			const chatMsg: AgentChatMessage = {
				id: msg.message.id,
				role: "assistant",
				content: textParts,
				contentBlocks: msg.message.content,
				timestamp: Date.now(),
				parentToolUseId: msg.parent_tool_use_id,
				model: msg.message.model,
				stopReason: msg.message.stop_reason,
			};
			agentModel.appendMessage(sessionId, chatMsg);
			agentModel.setStreaming(sessionId, null);
			// Store text for TTS auto-play (only top-level assistant messages)
			if (!msg.parent_tool_use_id && textParts.trim()) {
				agentModel.setLastAssistantText(sessionId, textParts);
			}
			break;
		}

		case "stream_event": {
			const event = msg.event as Record<string, unknown>;
			const eventType = event.type as string;
			console.log("[WS] stream_event:", eventType, event);

			if (eventType === "message_start") {
				agentModel.setStreamingStartedAt(sessionId, Date.now());
				agentModel.setStreaming(sessionId, "");
				agentModel.setSessionStatus(sessionId, "running");
				agentModel.clearCompletedTools(sessionId);
			} else if (eventType === "content_block_start") {
				// Ensure streaming is initialized even if message_start was never received
				if (agentModel.state.streaming[sessionId] === undefined) {
					agentModel.setStreaming(sessionId, "");
					agentModel.setSessionStatus(sessionId, "running");
					agentModel.setStreamingStartedAt(sessionId, Date.now());
				}
				const block = event.content_block as
					| Record<string, unknown>
					| undefined;
				if (block?.type === "tool_use" && typeof block.name === "string") {
					agentModel.setToolProgress(sessionId, {
						tool_use_id: (block.id as string) || "",
						tool_name: block.name,
						elapsed_time_seconds: 0,
					});
				}
			} else if (eventType === "content_block_delta") {
				// Ensure streaming is initialized even if message_start was never received
				if (agentModel.state.streaming[sessionId] === undefined) {
					agentModel.setStreaming(sessionId, "");
					agentModel.setSessionStatus(sessionId, "running");
					agentModel.setStreamingStartedAt(sessionId, Date.now());
				}
				const delta = event.delta as Record<string, unknown> | undefined;
				if (delta?.type === "text_delta" && typeof delta.text === "string") {
					const current = agentModel.state.streaming[sessionId] || "";
					agentModel.setStreaming(sessionId, current + delta.text);
				} else if (
					delta?.type === "thinking_delta" &&
					typeof delta.thinking === "string"
				) {
					const current = agentModel.state.streaming[sessionId] || "";
					agentModel.setStreaming(sessionId, current + delta.thinking);
				} else if (
					delta?.type === "input_json_delta" &&
					typeof delta.partial_json === "string"
				) {
					// Accumulate tool input JSON for display
					const tp = agentModel.state.activeToolProgress[sessionId];
					if (tp) {
						agentModel.setToolProgress(sessionId, {
							...tp,
							input: (tp.input || "") + delta.partial_json,
						});
					}
				}
			} else if (eventType === "tool_end") {
				// Tool finished — record as completed and clear active progress
				const toolUseId = event.tool_use_id as string;
				const toolName = event.tool_name as string;
				const output = (event.output as string) || "";
				const isError = (event.is_error as boolean) || false;
				const before = (event.before as string | null) ?? undefined;
				const after = (event.after as string | null) ?? undefined;
				const filePath = (event.file_path as string | null) ?? undefined;
				const tp = agentModel.state.activeToolProgress[sessionId];
				agentModel.addCompletedTool(sessionId, {
					tool_use_id: toolUseId,
					tool_name: toolName,
					input: tp?.input || "",
					output,
					is_error: isError,
					before,
					after,
					file_path: filePath,
				});
				agentModel.setToolProgress(sessionId, null);
			} else if (eventType === "task_updated") {
				const tasks = event.tasks as AgentTask[] | undefined;
				if (Array.isArray(tasks)) {
					agentModel.setTasks(sessionId, tasks);
				}
			} else if (eventType === "tool_output_delta") {
				// Real-time tool output — keep tool progress alive
				const toolName = event.tool_name as string | undefined;
				if (toolName) {
					const tp = agentModel.state.activeToolProgress[sessionId];
					if (tp) {
						agentModel.setToolProgress(sessionId, {
							...tp,
							elapsed_time_seconds: tp.elapsed_time_seconds,
						});
					}
				}
			}
			break;
		}

		case "result": {
			const data = msg.data;
			agentModel.updateSession(sessionId, {
				total_cost_usd: data.total_cost_usd as number | undefined,
				num_turns: data.num_turns as number | undefined,
				total_lines_added: data.total_lines_added as number | undefined,
				total_lines_removed: data.total_lines_removed as number | undefined,
				context_used_percent: data.context_used_percent as number | undefined,
				input_tokens: data.input_tokens as number | undefined,
				output_tokens: data.output_tokens as number | undefined,
				cache_read_tokens: data.cache_read_tokens as number | undefined,
				cache_write_tokens: data.cache_write_tokens as number | undefined,
			} as Partial<import("@/typings/agent").AgentSessionState>);
			agentModel.setStreaming(sessionId, null);
			agentModel.setSessionStatus(sessionId, "idle");
			agentModel.setToolProgress(sessionId, null);
			agentModel.clearCompletedTools(sessionId);

			if (data.is_error) {
				agentModel.appendMessage(sessionId, {
					id: nextMsgId(),
					role: "system",
					content: (data.result as string) || "An error occurred",
					timestamp: Date.now(),
				});
			}
			break;
		}

		case "permission_request":
			console.log("[WS] permission_request received:", msg.request);
			agentModel.addPermission(sessionId, msg.request);
			break;

		case "permission_cancelled":
			agentModel.removePermission(sessionId, msg.request_id);
			break;

		case "status_change":
			if (msg.status === "compacting") {
				agentModel.setSessionStatus(sessionId, "compacting");
				agentModel.updateSession(sessionId, { is_compacting: true });
			} else if (msg.status === "running") {
				agentModel.setSessionStatus(sessionId, "running");
				agentModel.updateSession(sessionId, { is_compacting: false });
			} else {
				agentModel.setSessionStatus(sessionId, "idle");
				agentModel.updateSession(sessionId, { is_compacting: false });
			}
			break;

		case "error":
			agentModel.appendMessage(sessionId, {
				id: nextMsgId(),
				role: "system",
				content: msg.message,
				timestamp: Date.now(),
			});
			break;

		case "cli_connected":
			agentModel.setCliConnected(sessionId, true);
			break;

		case "cli_disconnected":
			agentModel.setCliConnected(sessionId, false);
			agentModel.setSessionStatus(sessionId, null);
			break;

		case "user_message": {
			// Echo from server — attach any pending images from the outgoing message
			const images = pendingUserImages.get(sessionId);
			if (images) pendingUserImages.delete(sessionId);
			// Normalize timestamp: backend may send seconds instead of milliseconds
			const rawTs = msg.timestamp || 0;
			const ts = rawTs > 0 && rawTs < 1e12 ? rawTs * 1000 : rawTs || Date.now();
			agentModel.appendMessage(sessionId, {
				id: nextMsgId(),
				role: "user",
				content: msg.content,
				timestamp: ts,
				images,
			});
			break;
		}

		case "message_history": {
			// Backend sends full history on connect — always use it as source of truth.
			const chatMessages = convertHistoryMessages(msg.messages);
			agentModel.setMessages(sessionId, chatMessages);
			break;
		}

		case "session_name_update": {
			agentModel.setSessionName(sessionId, msg.name);
			break;
		}

		case "agent_message":
			agentModel.addAgentMessage(sessionId, {
				message_id: msg.message_id,
				from_session_id: msg.from_session_id,
				topic: msg.topic,
				content: msg.content,
				auto_execute: msg.auto_execute,
			});
			agentModel.incrementUnread(sessionId);
			break;

		case "command_response":
			// /clear command: wipe UI messages before showing the response
			if (msg.command === "/clear" && msg.state_changed) {
				agentModel.setMessages(sessionId, []);
			}
			agentModel.appendMessage(sessionId, {
				id: nextMsgId(),
				role: "assistant",
				content: msg.text,
				timestamp: Date.now(),
				source: `command:${msg.command}`,
			});
			// Slash commands don't trigger LLM generation — clear running state
			agentModel.setSessionStatus(sessionId, "idle");
			break;

		case "tool_progress": {
			console.log(
				"[WS] tool_progress:",
				msg.tool_use_id,
				msg.tool_name,
				msg.elapsed_time_seconds,
			);
			// Preserve existing input from streaming deltas
			const existing = agentModel.state.activeToolProgress[sessionId];
			agentModel.setToolProgress(sessionId, {
				tool_use_id: msg.tool_use_id,
				tool_name: msg.tool_name,
				elapsed_time_seconds: msg.elapsed_time_seconds,
				input:
					existing?.tool_use_id === msg.tool_use_id
						? existing.input
						: undefined,
			});
			break;
		}

		case "tool_use_summary":
			// Clear active tool progress when a summary arrives
			agentModel.setToolProgress(sessionId, null);
			// Insert a system message summarising the tool calls
			agentModel.appendMessage(sessionId, {
				id: nextMsgId(),
				role: "system",
				content: msg.summary,
				timestamp: Date.now(),
			});
			break;

		case "auth_status":
			agentModel.setAuthStatus(sessionId, {
				is_authenticating: msg.is_authenticating,
				output: msg.output,
				error: msg.error,
			});
			break;

		default:
			break;
	}
}

/** Monotonic ID counter to avoid Date.now() collisions */
let _msgIdCounter = 0;
function nextMsgId(): string {
	return `${Date.now()}-${++_msgIdCounter}`;
}

/** Convert message history from server format to chat messages */
function convertHistoryMessages(
	messages: BrowserIncomingMessage[],
): AgentChatMessage[] {
	const result: AgentChatMessage[] = [];
	// Start with 0; the first user_message with a valid server timestamp
	// will anchor the timeline. Messages before that get Date.now() fallback.
	let lastTimestamp = 0;

	for (const msg of messages) {
		if (msg.type === "assistant") {
			const textParts = msg.message.content
				.filter((b) => b.type === "text")
				.map((b) => (b as { type: "text"; text: string }).text)
				.join("");

			// Inherit from last known timestamp (user message or previous assistant)
			// so messages in the same turn stay on the same day.
			if (lastTimestamp === 0) lastTimestamp = Date.now();
			lastTimestamp = lastTimestamp + 1;
			result.push({
				id: msg.message.id,
				role: "assistant",
				content: textParts,
				contentBlocks: msg.message.content,
				timestamp: lastTimestamp,
				parentToolUseId: msg.parent_tool_use_id,
				model: msg.message.model,
				stopReason: msg.message.stop_reason,
			});
		} else if (msg.type === "result" && msg.data.is_error) {
			if (lastTimestamp === 0) lastTimestamp = Date.now();
			lastTimestamp = lastTimestamp + 1;
			result.push({
				id: nextMsgId(),
				role: "system",
				content: (msg.data.result as string) || "An error occurred",
				timestamp: lastTimestamp,
			});
		} else if (msg.type === "user_message") {
			// user_message has a server-provided timestamp — use it to anchor the timeline
			// Normalize: backend may send seconds instead of milliseconds
			const rawTs = msg.timestamp || 0;
			const serverTs = rawTs > 0 && rawTs < 1e12 ? rawTs * 1000 : rawTs;
			if (serverTs > 0 && dayjs(serverTs).isAfter("2000-01-01")) {
				lastTimestamp = serverTs;
			} else {
				if (lastTimestamp === 0) lastTimestamp = Date.now();
				lastTimestamp = lastTimestamp + 1;
			}
			result.push({
				id: nextMsgId(),
				role: "user",
				content: msg.content,
				timestamp: lastTimestamp,
			});
		}
	}

	return result;
}
