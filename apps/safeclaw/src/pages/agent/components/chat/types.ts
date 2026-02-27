import type { AgentChatMessage, ContentBlock } from "@/typings/agent";

// =============================================================================
// Rich message types
// =============================================================================

export interface ThinkingBlock {
	type: "thinking";
	content: string;
	durationMs?: number;
}

export interface ToolCallBlock {
	type: "tool_call";
	tool: string;
	input: string;
	output?: string;
	durationMs?: number;
	isError?: boolean;
	before?: string;
	after?: string;
	filePath?: string;
}

export interface SubAgentBlock {
	type: "sub_agent";
	agentName: string;
	task: string;
	result?: string;
	durationMs?: number;
}

export interface TextBlock {
	type: "text";
	content: string;
}

export interface HilOption {
	label: string;
	value: string;
}

export interface HilBlock {
	type: "hil";
	action: string;
	targetAgent: string;
	description: string;
	confirmed?: boolean;
	/** Interaction mode: confirm = simple yes/no, single = radio, multi = checkbox */
	mode?: "confirm" | "single" | "multi";
	options?: HilOption[];
	/** Whether to show a supplementary text input alongside options */
	allowInput?: boolean;
	inputPlaceholder?: string;
}

export interface EventBlock {
	type: "event";
	/** Event source category */
	source: "task" | "news" | "social" | "market" | "system" | "compliance";
	/** Event topic / subscription name */
	topic: string;
	/** Brief event summary */
	summary: string;
	/** Original event payload or detail (collapsible) */
	detail?: string;
	/** Timestamp of the event itself */
	eventTime?: number;
}

export type RichBlock =
	| ThinkingBlock
	| ToolCallBlock
	| SubAgentBlock
	| TextBlock
	| HilBlock
	| EventBlock;

export type MessageSource = "app" | "dingtalk" | "feishu" | "wecom";

export interface RichMessage {
	id: string;
	role: "user" | "assistant" | "system";
	blocks: RichBlock[];
	timestamp: number;
	/** Where this user message was sent from */
	source?: MessageSource;
	/** Non-null when this is a sub-agent reply inside a group/orchestrator session */
	parentToolUseId?: string | null;
	/** Model that generated this assistant message */
	model?: string;
	/** Stop reason (end_turn, max_tokens, etc.) */
	stopReason?: string | null;
	/** Images attached to user messages */
	images?: { media_type: string; data: string }[];
}

// =============================================================================
// Convert AgentChatMessage → RichMessage
// =============================================================================

function contentBlocksToRichBlocks(blocks: ContentBlock[]): RichBlock[] {
	const result: RichBlock[] = [];
	const toolCallIndex: Record<string, number> = {};

	for (const b of blocks) {
		if (b.type === "text") {
			result.push({ type: "text", content: b.text });
		} else if (b.type === "thinking") {
			result.push({
				type: "thinking",
				content: b.thinking,
				durationMs: undefined,
			});
		} else if (b.type === "tool_use") {
			const idx = result.length;
			toolCallIndex[b.id] = idx;
			result.push({
				type: "tool_call",
				tool: b.name,
				input:
					typeof b.input === "string"
						? b.input
						: JSON.stringify(b.input, null, 2),
				output: undefined,
				durationMs: undefined,
			});
		} else if (b.type === "tool_result") {
			const content =
				typeof b.content === "string"
					? b.content
					: JSON.stringify(b.content, null, 2);
			const idx = toolCallIndex[b.tool_use_id];
			if (idx !== undefined && result[idx]?.type === "tool_call") {
				(result[idx] as ToolCallBlock).output = content;
				(result[idx] as ToolCallBlock).isError = b.is_error;
			} else {
				result.push({
					type: "tool_call",
					tool: "result",
					input: "",
					output: content,
					isError: b.is_error,
				});
			}
		}
	}
	return result;
}

export function chatMessageToRich(msg: AgentChatMessage): RichMessage {
	const blocks: RichBlock[] = msg.contentBlocks
		? contentBlocksToRichBlocks(msg.contentBlocks)
		: [{ type: "text", content: msg.content }];

	return {
		id: msg.id,
		role:
			msg.role === "user"
				? "user"
				: msg.role === "system"
					? "system"
					: "assistant",
		timestamp: msg.timestamp,
		source: msg.source as MessageSource | undefined,
		parentToolUseId: msg.parentToolUseId,
		model: msg.model,
		stopReason: msg.stopReason,
		images: msg.images,
		blocks,
	};
}
