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
/** Cache diff data from tool_end events, keyed by sessionId → toolUseId */
const diffCache = new Map<
	string,
	Map<string, { before?: string; after?: string; file_path?: string }>
>();
/** Local monotonically increasing sequence per session for stream ordering */
const streamSeq = new Map<string, number>();
/** Next expected seq per session for in-order stream processing */
const nextExpectedStreamSeq = new Map<string, number>();

type StreamStats = {
	received: number;
	processed: number;
	staleDrops: number;
	reanchors: number;
	gapRecoveries: number;
};
const streamStats = new Map<string, StreamStats>();

type TurnPerf = {
	turnId: number;
	startedAt: number;
	wsSentAt?: number;
	messageStartAt?: number;
	firstDeltaAt?: number;
	firstToolStartAt?: number;
	firstPermissionRequestAt?: number;
	firstToolOutputAt?: number;
	firstToolEndAt?: number;
	assistantAt?: number;
	resultAt?: number;
};
const turnPerfBySession = new Map<string, TurnPerf>();
let turnCounter = 0;

function isStreamDebugEnabled(): boolean {
	if (typeof window === "undefined") return false;
	try {
		return localStorage.getItem("safeclaw-stream-debug") === "true";
	} catch {
		return false;
	}
}

function getStreamStats(sessionId: string): StreamStats {
	if (!streamStats.has(sessionId)) {
		streamStats.set(sessionId, {
			received: 0,
			processed: 0,
			staleDrops: 0,
			reanchors: 0,
			gapRecoveries: 0,
		});
	}
	return streamStats.get(sessionId)!;
}

function logStreamDebug(sessionId: string, reason: string): void {
	if (!isStreamDebugEnabled()) return;
	const stats = getStreamStats(sessionId);
	const expected = nextExpectedStreamSeq.get(sessionId) || 1;
	console.debug(`[stream:${sessionId}] ${reason}`, {
		expected,
		buffered: 0,
		seq: streamSeq.get(sessionId) || 0,
		stats,
	});
}

function markTurnPerf(sessionId: string, field: keyof TurnPerf): void {
	const perf = turnPerfBySession.get(sessionId);
	if (!perf) return;
	if (perf[field] == null) {
		(perf as Record<string, unknown>)[field] = performance.now();
	}
}

function emitTurnPerf(
	sessionId: string,
	finalStage: "assistant" | "result",
): void {
	if (!isStreamDebugEnabled()) return;
	const perf = turnPerfBySession.get(sessionId);
	if (!perf) return;

	const base = perf.wsSentAt ?? perf.startedAt;
	const ms = (t?: number) =>
		typeof t === "number" ? Math.round(t - base) : null;
	const firstDeltaMs = ms(perf.firstDeltaAt);
	const firstToolStartMs = ms(perf.firstToolStartAt);
	const firstPermissionRequestMs = ms(perf.firstPermissionRequestAt);
	const resultMs = ms(perf.resultAt);
	let inferredSlowStage:
		| "frontend_send"
		| "model_first_token"
		| "permission_wait"
		| "tool_exec"
		| "unknown" = "unknown";
	if (typeof firstDeltaMs === "number" && firstDeltaMs > 8000) {
		inferredSlowStage = "model_first_token";
	} else if (
		typeof firstPermissionRequestMs === "number" &&
		typeof resultMs === "number" &&
		resultMs - firstPermissionRequestMs > 5000
	) {
		inferredSlowStage = "permission_wait";
	} else if (
		typeof firstToolStartMs === "number" &&
		typeof resultMs === "number" &&
		resultMs - firstToolStartMs > 4000
	) {
		inferredSlowStage = "tool_exec";
	} else if (
		typeof perf.wsSentAt === "number" &&
		perf.wsSentAt - perf.startedAt > 1500
	) {
		inferredSlowStage = "frontend_send";
	}

	console.info(
		`[stream:${sessionId}] turn #${perf.turnId} timeline (${finalStage})`,
		{
			toMessageStartMs: ms(perf.messageStartAt),
			toFirstDeltaMs: firstDeltaMs,
			toFirstToolStartMs: firstToolStartMs,
			toFirstPermissionRequestMs: firstPermissionRequestMs,
			toFirstToolOutputMs: ms(perf.firstToolOutputAt),
			toFirstToolEndMs: ms(perf.firstToolEndAt),
			toAssistantMs: ms(perf.assistantAt),
			toResultMs: resultMs,
			transportOverheadMs:
				typeof perf.wsSentAt === "number"
					? Math.round(perf.wsSentAt - perf.startedAt)
					: null,
			inferredSlowStage,
		},
	);

	agentModel.setStreamPerfHint(sessionId, {
		turn_id: perf.turnId,
		slow_stage: inferredSlowStage,
		to_first_delta_ms: firstDeltaMs ?? undefined,
		to_first_permission_request_ms: firstPermissionRequestMs ?? undefined,
		to_result_ms: resultMs ?? undefined,
		updated_at: Date.now(),
	});
}

function nextStreamSeq(sessionId: string): number {
	const next = (streamSeq.get(sessionId) || 0) + 1;
	streamSeq.set(sessionId, next);
	return next;
}

function resetStreamSeq(sessionId: string): void {
	streamSeq.delete(sessionId);
}

function resetStreamState(sessionId: string): void {
	resetStreamSeq(sessionId);
	nextExpectedStreamSeq.delete(sessionId);
	streamStats.delete(sessionId);
}

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
		turnPerfBySession.delete(sessionId);
		resetStreamState(sessionId);
		diffCache.delete(sessionId);
		pendingUserImages.delete(sessionId);
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
	turnPerfBySession.delete(sessionId);
	resetStreamState(sessionId);
	diffCache.delete(sessionId);
	pendingUserImages.delete(sessionId);
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
		const now = performance.now();
		ws.send(JSON.stringify(msg));

		if (msg.type === "user_message") {
			turnCounter += 1;
			turnPerfBySession.set(sessionId, {
				turnId: turnCounter,
				startedAt: now,
				wsSentAt: now,
			});
			agentModel.setStreamPerfHint(sessionId, null);
			// Start a fresh turn window immediately to avoid stale seq state
			// blocking new stream events when message_start is delayed or missing.
			resetStreamState(sessionId);
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

function handleStreamEventPayload(
	sessionId: string,
	event: Record<string, unknown>,
	seq: number,
): void {
	const eventType = event.type as string;

	if (eventType === "message_start") {
		markTurnPerf(sessionId, "messageStartAt");
		resetStreamState(sessionId);
		nextExpectedStreamSeq.set(sessionId, seq + 1);
		streamSeq.set(sessionId, seq);
		agentModel.setStreamingStartedAt(sessionId, Date.now());
		agentModel.setStreaming(sessionId, "");
		agentModel.setSessionStatus(sessionId, "running");
		agentModel.clearCompletedTools(sessionId); // also clears streamingSegments
		diffCache.delete(sessionId);
		return;
	}

	if (eventType === "content_block_start") {
		if (agentModel.state.streaming[sessionId] === undefined) {
			agentModel.setStreaming(sessionId, "");
			agentModel.setSessionStatus(sessionId, "running");
			agentModel.setStreamingStartedAt(sessionId, Date.now());
		}
		const block = event.content_block as Record<string, unknown> | undefined;
		if (block?.type === "tool_use" && typeof block.name === "string") {
			markTurnPerf(sessionId, "firstToolStartAt");
			const elapsed =
				typeof event.elapsed_time_seconds === "number" &&
				Number.isFinite(event.elapsed_time_seconds)
					? event.elapsed_time_seconds
					: 0;
			const progress = {
				tool_use_id: (block.id as string) || "",
				tool_name: block.name,
				elapsed_time_seconds: elapsed,
			};
			agentModel.setToolProgress(sessionId, progress);
			agentModel.upsertStreamingToolProgressSegment(sessionId, progress, seq);
		}
		return;
	}

	if (eventType === "content_block_delta") {
		if (agentModel.state.streaming[sessionId] === undefined) {
			agentModel.setStreaming(sessionId, "");
			agentModel.setSessionStatus(sessionId, "running");
			agentModel.setStreamingStartedAt(sessionId, Date.now());
		}
		const delta = event.delta as Record<string, unknown> | undefined;
		if (delta?.type === "text_delta" && typeof delta.text === "string") {
			markTurnPerf(sessionId, "firstDeltaAt");
			const current = agentModel.state.streaming[sessionId] || "";
			agentModel.setStreaming(sessionId, current + delta.text);
			agentModel.appendStreamingText(sessionId, delta.text, seq);
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
			const tp = agentModel.state.activeToolProgress[sessionId];
			if (tp) {
				const next = { ...tp, input: (tp.input || "") + delta.partial_json };
				agentModel.setToolProgress(sessionId, next);
				agentModel.upsertStreamingToolProgressSegment(sessionId, next, seq);
			}
		}
		return;
	}

	if (eventType === "tool_end") {
		markTurnPerf(sessionId, "firstToolEndAt");
		const toolUseId = event.tool_use_id as string;
		const toolName = event.tool_name as string;
		const output = (event.output as string) || "";
		const isError = (event.is_error as boolean) || false;
		const before = (event.before as string | null) ?? undefined;
		const after = (event.after as string | null) ?? undefined;
		const filePath = (event.file_path as string | null) ?? undefined;
		if (before != null || after != null) {
			if (!diffCache.has(sessionId)) diffCache.set(sessionId, new Map());
			diffCache
				.get(sessionId)!
				.set(toolUseId, { before, after, file_path: filePath });
		}
		const tp = agentModel.state.activeToolProgress[sessionId];
		const completedTool = {
			tool_use_id: toolUseId,
			tool_name: toolName,
			input: tp?.input || "",
			output,
			is_error: isError,
			before,
			after,
			file_path: filePath,
		};
		agentModel.addCompletedTool(sessionId, completedTool);
		agentModel.replaceStreamingToolProgressWithCompleted(
			sessionId,
			toolUseId,
			completedTool,
			seq,
		);
		agentModel.setToolProgress(sessionId, null);
		return;
	}

	if (eventType === "task_updated") {
		const tasks = event.tasks as AgentTask[] | undefined;
		if (Array.isArray(tasks)) {
			agentModel.setTasks(sessionId, tasks);
		}
		return;
	}

	if (eventType === "tool_output_delta") {
		markTurnPerf(sessionId, "firstToolOutputAt");
		const toolName = event.tool_name as string | undefined;
		if (toolName) {
			const deltaText =
				typeof event.delta === "string"
					? event.delta
					: typeof event.delta === "object" && event.delta
						? JSON.stringify(event.delta)
						: "";
			const tp = agentModel.state.activeToolProgress[sessionId];
			if (tp) {
				const elapsed =
					typeof event.elapsed_time_seconds === "number" &&
					Number.isFinite(event.elapsed_time_seconds)
						? event.elapsed_time_seconds
						: tp.elapsed_time_seconds;
				const next = {
					...tp,
					output: (tp.output || "") + deltaText,
					elapsed_time_seconds: elapsed,
				};
				agentModel.setToolProgress(sessionId, next);
				agentModel.upsertStreamingToolProgressSegment(sessionId, next, seq);
			}
		}
		return;
	}

	if (eventType === "tool_progress") {
		const toolUseId = event.tool_use_id as string | undefined;
		const toolName = event.tool_name as string | undefined;
		if (!toolUseId || !toolName) return;
		const existing = agentModel.state.activeToolProgress[sessionId];
		const next = {
			tool_use_id: toolUseId,
			tool_name: toolName,
			elapsed_time_seconds:
				typeof event.elapsed_time_seconds === "number" &&
				Number.isFinite(event.elapsed_time_seconds)
					? event.elapsed_time_seconds
					: 0,
			input: existing?.tool_use_id === toolUseId ? existing.input : undefined,
			output: existing?.tool_use_id === toolUseId ? existing.output : undefined,
		};
		agentModel.setToolProgress(sessionId, next);
		agentModel.upsertStreamingToolProgressSegment(sessionId, next, seq);
	}
}

function enqueueStreamEvent(
	sessionId: string,
	event: Record<string, unknown>,
): void {
	const stats = getStreamStats(sessionId);
	stats.received += 1;

	const raw = event.seq;
	const seq =
		typeof raw === "number" && Number.isFinite(raw)
			? Math.max(1, Math.floor(raw))
			: nextStreamSeq(sessionId);
	const eventType = event.type as string | undefined;
	if (eventType === "message_start") {
		// Turn boundary should always re-anchor ordering.
		stats.reanchors += 1;
		resetStreamState(sessionId);
		nextExpectedStreamSeq.set(sessionId, seq);
		streamSeq.set(sessionId, Math.max(0, seq - 1));
		logStreamDebug(sessionId, `message_start reanchor seq=${seq}`);
	}

	let expected = nextExpectedStreamSeq.get(sessionId) || 1;
	if (seq < expected) {
		if (seq === 1 || expected - seq > 20) {
			stats.reanchors += 1;
			resetStreamState(sessionId);
			nextExpectedStreamSeq.set(sessionId, seq);
			streamSeq.set(sessionId, Math.max(0, seq - 1));
			logStreamDebug(sessionId, `stale expected reanchor seq=${seq}`);
			expected = seq;
		} else {
			stats.staleDrops += 1;
			if (stats.staleDrops % 10 === 0) {
				logStreamDebug(sessionId, `stale drop x${stats.staleDrops}`);
			}
			return;
		}
	}

	// If there is a gap, prefer responsiveness: skip missing seq and continue.
	if (seq > expected + 1) {
		stats.gapRecoveries += 1;
		nextExpectedStreamSeq.set(sessionId, seq);
		logStreamDebug(sessionId, `gap skip expected=${expected} -> seq=${seq}`);
	}

	streamSeq.set(sessionId, seq);
	handleStreamEventPayload(sessionId, event, seq);
	stats.processed += 1;
	nextExpectedStreamSeq.set(sessionId, seq + 1);

	if (stats.received % 40 === 0) {
		logStreamDebug(sessionId, "periodic");
	}
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
			markTurnPerf(sessionId, "assistantAt");
			const textParts = msg.message.content
				.filter((b) => b.type === "text")
				.map((b) => (b as { type: "text"; text: string }).text)
				.join("");

			// Inject cached diff data into tool_result blocks
			const sessionDiffs = diffCache.get(sessionId);
			const enrichedBlocks = sessionDiffs
				? msg.message.content.map((b) => {
						if (b.type === "tool_result") {
							const diff = sessionDiffs.get(b.tool_use_id);
							if (diff) {
								return { ...b, ...diff };
							}
						}
						return b;
					})
				: msg.message.content;

			const chatMsg: AgentChatMessage = {
				id: msg.message.id,
				role: "assistant",
				content: textParts,
				contentBlocks: enrichedBlocks,
				timestamp: Date.now(),
				parentToolUseId: msg.parent_tool_use_id,
				model: msg.message.model,
				stopReason: msg.message.stop_reason,
			};
			agentModel.appendMessage(sessionId, chatMsg);
			agentModel.setStreaming(sessionId, null);
			agentModel.setToolProgress(sessionId, null);
			agentModel.clearCompletedTools(sessionId);
			emitTurnPerf(sessionId, "assistant");
			resetStreamState(sessionId);
			// Store text for TTS auto-play (only top-level assistant messages)
			if (!msg.parent_tool_use_id && textParts.trim()) {
				agentModel.setLastAssistantText(sessionId, textParts);
			}
			break;
		}

		case "stream_event": {
			const event = msg.event as Record<string, unknown>;
			enqueueStreamEvent(sessionId, event);
			break;
		}

		case "result": {
			markTurnPerf(sessionId, "resultAt");
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
			emitTurnPerf(sessionId, "result");
			turnPerfBySession.delete(sessionId);
			resetStreamState(sessionId);

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
			markTurnPerf(sessionId, "firstPermissionRequestAt");
			console.log("[WS] permission_request received:", msg.request);
			if (msg.request.tool_use_id) {
				const existing = agentModel.state.activeToolProgress[sessionId];
				if (existing?.tool_use_id !== msg.request.tool_use_id) {
					const progress = {
						tool_use_id: msg.request.tool_use_id,
						tool_name: msg.request.tool_name,
						elapsed_time_seconds: 0,
						input: JSON.stringify(msg.request.input || {}, null, 2),
					};
					agentModel.setToolProgress(sessionId, progress);
					agentModel.upsertStreamingToolProgressSegment(
						sessionId,
						progress,
						nextStreamSeq(sessionId),
					);
				}
			}
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
				// Ignore transient idle updates while streaming content is still active.
				const hasStreaming =
					typeof agentModel.state.streaming[sessionId] === "string" ||
					(agentModel.state.streamingSegments[sessionId]?.length || 0) > 0 ||
					Object.keys(agentModel.state.pendingPermissions[sessionId] || {})
						.length > 0;
				if (!hasStreaming) {
					agentModel.setSessionStatus(sessionId, "idle");
				}
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
			enqueueStreamEvent(sessionId, {
				type: "tool_progress",
				tool_use_id: msg.tool_use_id,
				tool_name: msg.tool_name,
				elapsed_time_seconds: msg.elapsed_time_seconds,
				seq: msg.seq,
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
