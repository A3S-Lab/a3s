import { MemoizedMarkdown } from "@/components/custom/memoized-markdown";
import { cn } from "@/lib/utils";
import globalModel from "@/models/global.model";
import personaModel from "@/models/persona.model";
import { Check, Copy, RefreshCw, Terminal, Zap } from "lucide-react";
import React, { useCallback, useMemo, useState } from "react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import dayjs from "dayjs";
import { useSnapshot } from "valtio";
import type { RichMessage, TextBlock } from "./types";
import {
	ThinkingBlockView,
	ToolCallBlockView,
	SubAgentBlockView,
	HilBlockView,
	EventBlockView,
	SourceBadge,
} from "./message-blocks";

// =============================================================================
// Date separator — shown between messages on different days
// =============================================================================

export function DateSeparator({ timestamp }: { timestamp: number }) {
	const label = dayjs(timestamp).format("YYYY-MM-DD");
	const isToday = dayjs(timestamp).isSame(dayjs(), "day");
	const isYesterday = dayjs(timestamp).isSame(
		dayjs().subtract(1, "day"),
		"day",
	);
	const display = isToday ? "今天" : isYesterday ? "昨天" : label;

	return (
		<div
			className="flex items-center gap-4 px-6 py-3 select-none"
			aria-label={`日期: ${display}`}
		>
			<div className="flex-1 h-px bg-gradient-to-r from-transparent via-border to-transparent" />
			<span className="text-[10px] text-muted-foreground/50 font-medium tracking-wider uppercase">
				{display}
			</span>
			<div className="flex-1 h-px bg-gradient-to-r from-transparent via-border to-transparent" />
		</div>
	);
}

// =============================================================================
// Hover action bar
// =============================================================================

function MessageActions({
	msg,
	onCopy,
	onRetry,
}: {
	msg: RichMessage;
	onCopy: () => void;
	onRetry?: () => void;
}) {
	const [copied, setCopied] = useState(false);

	const handleCopy = useCallback(() => {
		onCopy();
		setCopied(true);
		setTimeout(() => setCopied(false), 2000);
	}, [onCopy]);

	return (
		<div className="absolute -top-3.5 right-3 hidden group-hover:flex items-center gap-0.5 rounded-lg border bg-background/95 backdrop-blur-md shadow-md px-1 py-0.5 z-10 transition-all">
			<button
				type="button"
				className="flex items-center justify-center size-6 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.08] transition-colors"
				onClick={handleCopy}
				aria-label="复制消息"
				title="复制"
			>
				{copied ? (
					<Check className="size-3 text-emerald-500" />
				) : (
					<Copy className="size-3" />
				)}
			</button>
			{msg.role === "assistant" && onRetry && (
				<button
					type="button"
					className="flex items-center justify-center size-6 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.08] transition-colors"
					onClick={onRetry}
					aria-label="重新生成"
					title="重新生成"
				>
					<RefreshCw className="size-3" />
				</button>
			)}
		</div>
	);
}

// =============================================================================
// Model badge for assistant messages
// =============================================================================

function ModelBadge({ model }: { model?: string }) {
	if (!model) return null;
	// Extract short model name: "anthropic/claude-3.5-sonnet" → "claude-3.5-sonnet"
	const short = model.includes("/") ? model.split("/").pop()! : model;
	return (
		<span className="text-[9px] rounded-md border border-border/50 bg-muted/40 px-1.5 py-0.5 text-muted-foreground/60 font-mono leading-none">
			{short}
		</span>
	);
}

// =============================================================================
// Command badge for slash command responses
// =============================================================================

function CommandBadge({ source }: { source?: string }) {
	if (!source?.startsWith("command:")) return null;
	const cmd = source.slice(8); // strip "command:" prefix
	return (
		<span className="inline-flex items-center gap-1 rounded-full bg-blue-500/8 border border-blue-500/10 px-2 py-0.5 text-[9px] font-medium text-blue-600 dark:text-blue-400">
			<Terminal className="size-2.5" />
			{cmd}
		</span>
	);
}

// =============================================================================
// Inline image display for user messages
// =============================================================================

function InlineImages({
	images,
}: { images?: { media_type: string; data: string }[] }) {
	if (!images || images.length === 0) return null;
	return (
		<div className="flex flex-wrap gap-2 mt-2">
			{images.map((img, i) => (
				<a
					key={`inline-img-${i}`}
					href={`data:${img.media_type};base64,${img.data}`}
					target="_blank"
					rel="noopener noreferrer"
					className="block"
				>
					<img
						src={`data:${img.media_type};base64,${img.data}`}
						alt={`图片 ${i + 1}`}
						className="max-h-48 max-w-xs rounded-lg border object-contain hover:opacity-90 transition-opacity cursor-zoom-in"
					/>
				</a>
			))}
		</div>
	);
}

// =============================================================================
// MessageItem
// =============================================================================

const MessageItem = React.memo(function MessageItem({
	msg,
	sessionId,
	onRetry,
}: {
	msg: RichMessage;
	sessionId: string;
	onRetry?: () => void;
}) {
	const isUser = msg.role === "user";
	const isSubAgent = !isUser && !!msg.parentToolUseId;
	const persona = personaModel.getSessionPersona(sessionId);
	const avatarConfig = useMemo(
		() => genConfig(persona.avatar),
		[persona.avatar],
	);
	const { user } = useSnapshot(globalModel.state);

	// Extract plain text for copy
	const getPlainText = useCallback(() => {
		return msg.blocks
			.map((block) => {
				if (block.type === "text") return block.content;
				if (block.type === "thinking") return block.content;
				if (block.type === "tool_call")
					return `[${block.tool}] ${block.input}${block.output ? `\n→ ${block.output}` : ""}`;
				return "";
			})
			.filter(Boolean)
			.join("\n\n");
	}, [msg.blocks]);

	const handleCopy = useCallback(async () => {
		const text = getPlainText();
		await navigator.clipboard.writeText(text);
	}, [getPlainText]);

	if (msg.role === "system") {
		return (
			<div className="flex justify-center px-6 py-2.5">
				<div className="rounded-full bg-muted/60 backdrop-blur-sm px-4 py-1.5 text-[11px] text-muted-foreground/70 max-w-md text-center shadow-sm border border-border/30">
					{msg.blocks[0]?.type === "text"
						? (msg.blocks[0] as TextBlock).content
						: ""}
				</div>
			</div>
		);
	}

	return (
		<div
			className={cn(
				"group relative px-5 py-4 transition-colors duration-200",
				isUser && "bg-foreground/[0.02]",
				!isUser && "hover:bg-foreground/[0.015]",
				isSubAgent && "pl-12 border-l-2 border-primary/15 ml-5",
			)}
		>
			<MessageActions
				msg={msg}
				onCopy={handleCopy}
				onRetry={msg.role === "assistant" ? onRetry : undefined}
			/>

			<div className="flex items-center gap-2.5 mb-2">
				{isUser ? (
					<img
						src={user.avatar}
						alt={user.nickname}
						className="size-7 shrink-0 rounded-full object-cover ring-2 ring-background shadow-sm"
					/>
				) : (
					<NiceAvatar
						className="size-7 shrink-0 ring-2 ring-background shadow-sm"
						{...avatarConfig}
					/>
				)}
				<span className="text-[13px] font-semibold leading-none">
					{isUser ? user.nickname : persona.name}
				</span>
				{isSubAgent && (
					<span className="inline-flex items-center gap-0.5 rounded-full bg-primary/8 px-2 py-0.5 text-[9px] font-medium text-primary border border-primary/10">
						<Zap className="size-2.5" />
						Sub-Agent
					</span>
				)}
				{isUser && msg.source && <SourceBadge source={msg.source} />}
				{!isUser && msg.source?.startsWith("command:") && (
					<CommandBadge source={msg.source} />
				)}
				{!isUser && !msg.source?.startsWith("command:") && (
					<ModelBadge model={msg.model} />
				)}
				<time className="text-[10px] text-muted-foreground/40 ml-auto shrink-0 tabular-nums">
					{dayjs(msg.timestamp).format("HH:mm:ss")}
				</time>
			</div>

			<div className="ml-[38px]">
				{msg.blocks.map((block, i) => {
					switch (block.type) {
						case "thinking":
							return <ThinkingBlockView key={i} block={block} />;
						case "tool_call":
							return <ToolCallBlockView key={i} block={block} />;
						case "sub_agent":
							return <SubAgentBlockView key={i} block={block} />;
						case "hil":
							return <HilBlockView key={i} block={block} />;
						case "event":
							return <EventBlockView key={i} block={block} />;
						case "text":
							return (
								<div key={i} className="text-sm leading-relaxed">
									<MemoizedMarkdown
										id={`${msg.id}-${i}`}
										content={block.content}
									/>
								</div>
							);
						default:
							return null;
					}
				})}
				{isUser && <InlineImages images={msg.images} />}
			</div>

			{/* Stop reason indicator */}
			{msg.stopReason === "max_tokens" && (
				<div className="ml-[38px] mt-1.5 text-[10px] text-amber-600 dark:text-amber-400 flex items-center gap-1">
					<span className="size-1 rounded-full bg-amber-500" />
					输出被截断 (max_tokens)
				</div>
			)}
		</div>
	);
});

export default MessageItem;
