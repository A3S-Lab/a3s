import { MemoizedMarkdown } from "@/components/custom/memoized-markdown";
import { cn } from "@/lib/utils";
import {
	CheckCircle2,
	ChevronDown,
	ChevronRight,
	Clock,
	Code2,
	FileCode,
	FileText,
	Loader2,
	Search,
	Terminal,
	XCircle,
	Wrench,
	Lock,
	Globe,
} from "lucide-react";
import React, { useState } from "react";
import type {
	ThinkingBlock,
	ToolCallBlock,
	SubAgentBlock,
	HilBlock,
	EventBlock,
	MessageSource,
} from "./types";

// =============================================================================
// ThinkingBlockView — collapsible thinking process
// =============================================================================

export function ThinkingBlockView({ block }: { block: ThinkingBlock }) {
	const [open, setOpen] = useState(false);
	return (
		<div className="my-2">
			<button
				type="button"
				className="flex items-center gap-1.5 text-[11px] text-muted-foreground/60 hover:text-muted-foreground transition-colors group/think"
				onClick={() => setOpen(!open)}
			>
				{open ? (
					<ChevronDown className="size-3" />
				) : (
					<ChevronRight className="size-3" />
				)}
				<span className="italic">Thinking</span>
				{block.durationMs && (
					<span className="opacity-50 tabular-nums">
						{(block.durationMs / 1000).toFixed(1)}s
					</span>
				)}
			</button>
			{open && (
				<div className="mt-1.5 ml-4 pl-3 border-l-2 border-muted-foreground/10 text-xs text-muted-foreground/70 whitespace-pre-wrap leading-relaxed">
					{block.content}
				</div>
			)}
		</div>
	);
}

// =============================================================================
// ToolCallBlockView — Claude Code-style tool call display
// =============================================================================

const TOOL_META: Record<
	string,
	{ icon: React.ReactNode; label: string; color: string }
> = {
	Read: {
		icon: <FileText className="size-3.5" />,
		label: "Read",
		color: "text-blue-500",
	},
	read: {
		icon: <FileText className="size-3.5" />,
		label: "Read",
		color: "text-blue-500",
	},
	Write: {
		icon: <FileCode className="size-3.5" />,
		label: "Write",
		color: "text-emerald-500",
	},
	write: {
		icon: <FileCode className="size-3.5" />,
		label: "Write",
		color: "text-emerald-500",
	},
	Edit: {
		icon: <Code2 className="size-3.5" />,
		label: "Edit",
		color: "text-amber-500",
	},
	edit: {
		icon: <Code2 className="size-3.5" />,
		label: "Edit",
		color: "text-amber-500",
	},
	Bash: {
		icon: <Terminal className="size-3.5" />,
		label: "Bash",
		color: "text-purple-500",
	},
	bash: {
		icon: <Terminal className="size-3.5" />,
		label: "Bash",
		color: "text-purple-500",
	},
	Grep: {
		icon: <Search className="size-3.5" />,
		label: "Grep",
		color: "text-slate-500",
	},
	grep: {
		icon: <Search className="size-3.5" />,
		label: "Grep",
		color: "text-slate-500",
	},
	Glob: {
		icon: <Search className="size-3.5" />,
		label: "Glob",
		color: "text-slate-500",
	},
	glob: {
		icon: <Search className="size-3.5" />,
		label: "Glob",
		color: "text-slate-500",
	},
	ls: {
		icon: <Search className="size-3.5" />,
		label: "ls",
		color: "text-slate-500",
	},
	web_search: {
		icon: <Globe className="size-3.5" />,
		label: "WebSearch",
		color: "text-indigo-500",
	},
	WebSearch: {
		icon: <Globe className="size-3.5" />,
		label: "WebSearch",
		color: "text-indigo-500",
	},
	web_fetch: {
		icon: <Globe className="size-3.5" />,
		label: "WebFetch",
		color: "text-indigo-500",
	},
	WebFetch: {
		icon: <Globe className="size-3.5" />,
		label: "WebFetch",
		color: "text-indigo-500",
	},
	Patch: {
		icon: <Code2 className="size-3.5" />,
		label: "Patch",
		color: "text-amber-500",
	},
	patch: {
		icon: <Code2 className="size-3.5" />,
		label: "Patch",
		color: "text-amber-500",
	},
	TEEPayment: {
		icon: <Lock className="size-3.5" />,
		label: "TEEPayment",
		color: "text-rose-500",
	},
};

function getToolMeta(name: string) {
	return (
		TOOL_META[name] || {
			icon: <Wrench className="size-3.5" />,
			label: name,
			color: "text-primary",
		}
	);
}

/** Format tool input for one-line summary */
function formatToolSummary(_tool: string, input: string): string {
	try {
		const parsed = JSON.parse(input);
		// Show the most relevant field as summary
		if (parsed.command) return parsed.command;
		if (parsed.file_path) return parsed.file_path;
		if (parsed.path) return parsed.path;
		if (parsed.pattern) return `/${parsed.pattern}/`;
		if (parsed.query) return parsed.query;
		if (parsed.url) return parsed.url;
		if (parsed.content && typeof parsed.content === "string") {
			return parsed.content.length > 60
				? `${parsed.content.slice(0, 60)}…`
				: parsed.content;
		}
		// Fallback: first string value
		for (const v of Object.values(parsed)) {
			if (typeof v === "string" && v.length > 0) {
				return v.length > 80 ? `${v.slice(0, 80)}…` : v;
			}
		}
	} catch {
		// Not JSON, show raw
	}
	return input.length > 80 ? `${input.slice(0, 80)}…` : input;
}

export function ToolCallBlockView({ block }: { block: ToolCallBlock }) {
	const [open, setOpen] = useState(false);
	const meta = getToolMeta(block.tool);
	const summary = formatToolSummary(block.tool, block.input);
	const hasOutput = block.output !== undefined;
	const hasDisplayableOutput = !!block.output;
	const isRunning = !hasOutput && !block.isError;

	return (
		<div
			className={cn(
				"my-1.5 rounded-xl border overflow-hidden transition-colors duration-200",
				isRunning
					? "border-primary/20 bg-primary/[0.02]"
					: block.isError
						? "border-destructive/15 bg-destructive/[0.02]"
						: "border-border/40 bg-muted/15",
			)}
		>
			{/* Tool header: clickable to toggle output */}
			<button
				type="button"
				className={cn(
					"flex items-center gap-2 px-3 py-2 w-full text-left transition-colors",
					hasDisplayableOutput && "hover:bg-foreground/[0.03]",
				)}
				onClick={() => hasDisplayableOutput && setOpen(!open)}
			>
				{isRunning ? (
					<Loader2 className="size-3.5 text-primary animate-spin shrink-0" />
				) : block.isError ? (
					<XCircle className="size-3.5 text-destructive/70 shrink-0" />
				) : (
					<CheckCircle2 className="size-3.5 text-emerald-500/80 shrink-0" />
				)}
				<span
					className={cn(
						"shrink-0 flex items-center justify-center size-5 rounded-md",
						isRunning
							? "bg-primary/10"
							: block.isError
								? "bg-destructive/8"
								: "bg-muted/50",
						meta.color,
					)}
				>
					{meta.icon}
				</span>
				<span className="text-xs font-semibold text-foreground/90">
					{meta.label}
				</span>
				<span className="text-[11px] text-muted-foreground/60 truncate flex-1 font-mono">
					{summary}
				</span>
				{block.durationMs && (
					<span className="text-[10px] text-muted-foreground/40 shrink-0 flex items-center gap-0.5 tabular-nums">
						<Clock className="size-2.5" />
						{(block.durationMs / 1000).toFixed(1)}s
					</span>
				)}
				{hasDisplayableOutput &&
					(open ? (
						<ChevronDown className="size-3 text-muted-foreground/40 shrink-0" />
					) : (
						<ChevronRight className="size-3 text-muted-foreground/40 shrink-0" />
					))}
			</button>

			{/* Output (collapsed by default) */}
			{open && block.output && (
				<div className="border-t border-border/30 px-3 py-2.5">
					{block.isError ? (
						<pre className="rounded-lg border border-destructive/15 p-3 text-[11px] font-mono overflow-auto max-h-60 whitespace-pre-wrap leading-relaxed bg-destructive/[0.03] text-destructive/80">
							{block.output}
						</pre>
					) : (
						<div className="overflow-auto max-h-60 p-2.5">
							<MemoizedMarkdown
								id={`tool-${block.tool}-${block.durationMs ?? 0}`}
								content={block.output}
							/>
						</div>
					)}
				</div>
			)}
		</div>
	);
}

// =============================================================================
// SubAgentBlockView
// =============================================================================

export function SubAgentBlockView({ block }: { block: SubAgentBlock }) {
	const [open, setOpen] = useState(false);
	return (
		<div className="my-2 rounded-xl border border-primary/15 bg-gradient-to-b from-primary/[0.03] to-transparent overflow-hidden">
			<button
				type="button"
				className="flex items-center gap-2 w-full text-left px-3 py-2.5 hover:bg-primary/[0.04] transition-colors"
				onClick={() => setOpen(!open)}
			>
				{open ? (
					<ChevronDown className="size-3 text-primary/50" />
				) : (
					<ChevronRight className="size-3 text-primary/50" />
				)}
				<span className="inline-flex items-center gap-0.5 rounded-full bg-primary/8 px-2 py-0.5 text-[10px] text-primary font-medium border border-primary/10">
					@{block.agentName}
				</span>
				<span className="text-[11px] text-muted-foreground/60 truncate flex-1">
					{block.task}
				</span>
				{block.durationMs && (
					<span className="text-[10px] text-muted-foreground/40 shrink-0 tabular-nums">
						{(block.durationMs / 1000).toFixed(1)}s
					</span>
				)}
			</button>
			{open && block.result && (
				<div className="border-t border-primary/10 px-3 py-2.5">
					<pre className="text-xs text-muted-foreground/70 whitespace-pre-wrap leading-relaxed">
						{block.result}
					</pre>
				</div>
			)}
		</div>
	);
}

// =============================================================================
// HilBlockView — HITL confirmation inline block
// =============================================================================

export function HilBlockView({ block }: { block: HilBlock }) {
	const status =
		block.confirmed === true
			? "confirmed"
			: block.confirmed === false
				? "rejected"
				: "pending";

	return (
		<div
			className={cn(
				"my-2.5 rounded-xl border p-3.5",
				status === "confirmed"
					? "border-emerald-500/15 bg-emerald-500/[0.03]"
					: status === "rejected"
						? "border-muted-foreground/10 bg-muted/20"
						: "border-amber-500/20 bg-gradient-to-b from-amber-500/[0.04] to-transparent",
			)}
		>
			<div className="flex items-center gap-2 mb-2.5">
				<div
					className={cn(
						"flex items-center justify-center size-6 rounded-lg",
						status === "confirmed"
							? "bg-emerald-500/10"
							: status === "rejected"
								? "bg-muted"
								: "bg-amber-500/10",
					)}
				>
					<Lock
						className={cn(
							"size-3.5",
							status === "confirmed"
								? "text-emerald-600 dark:text-emerald-400"
								: status === "rejected"
									? "text-muted-foreground"
									: "text-amber-600 dark:text-amber-400",
						)}
					/>
				</div>
				<span
					className={cn(
						"text-xs font-semibold",
						status === "confirmed"
							? "text-emerald-700 dark:text-emerald-300"
							: status === "rejected"
								? "text-muted-foreground"
								: "text-amber-700 dark:text-amber-300",
					)}
				>
					需要确认
				</span>
			</div>
			<div className="text-sm mb-1.5">
				<span className="text-foreground/90">{block.action}</span>
				<span className="mx-2 text-muted-foreground/30">→</span>
				<span className="inline-flex items-center gap-0.5 rounded-full bg-primary/8 px-2 py-0.5 text-xs text-primary font-medium border border-primary/10">
					@{block.targetAgent}
				</span>
			</div>
			<p className="text-xs text-muted-foreground/60 leading-relaxed">
				{block.description}
			</p>
			{status === "confirmed" && (
				<div className="flex items-center gap-1.5 text-xs text-emerald-600 dark:text-emerald-400 mt-2.5 pt-2 border-t border-emerald-500/10">
					<CheckCircle2 className="size-3.5" />
					<span className="font-medium">已确认</span>
				</div>
			)}
			{status === "rejected" && (
				<div className="flex items-center gap-1.5 text-xs text-muted-foreground/60 mt-2.5 pt-2 border-t border-border/30">
					<XCircle className="size-3.5" />
					<span>已拒绝</span>
				</div>
			)}
		</div>
	);
}

// =============================================================================
// EventBlockView
// =============================================================================

export function EventBlockView({ block }: { block: EventBlock }) {
	const [open, setOpen] = useState(false);
	return (
		<div className="my-2.5 rounded-xl border border-primary/10 bg-gradient-to-b from-primary/[0.03] to-transparent p-3.5">
			<div className="flex items-center gap-2 mb-2">
				<span className="text-[10px] font-semibold text-primary/60 uppercase tracking-wider">
					{block.source}
				</span>
				<span className="size-0.5 rounded-full bg-muted-foreground/20" />
				<span className="text-[10px] text-muted-foreground/50">
					{block.topic}
				</span>
			</div>
			<p className="text-sm leading-relaxed text-foreground/90">
				{block.summary}
			</p>
			{block.detail && (
				<>
					<button
						type="button"
						className="flex items-center gap-1 mt-2 text-[11px] text-muted-foreground/50 hover:text-muted-foreground transition-colors"
						onClick={() => setOpen(!open)}
					>
						{open ? (
							<ChevronDown className="size-3" />
						) : (
							<ChevronRight className="size-3" />
						)}
						<span>详情</span>
					</button>
					{open && (
						<pre className="mt-2 rounded-lg bg-muted/30 border border-border/20 p-2.5 text-[11px] font-mono overflow-x-auto max-h-40 whitespace-pre-wrap text-muted-foreground/70 leading-relaxed">
							{block.detail}
						</pre>
					)}
				</>
			)}
		</div>
	);
}

// =============================================================================
// SourceBadge
// =============================================================================

const SOURCE_CONFIG: Record<
	MessageSource,
	{ label: string; color: string; bg: string; icon: string }
> = {
	app: {
		label: "SafeClaw",
		color: "text-primary",
		bg: "bg-primary/10",
		icon: "🛡️",
	},
	dingtalk: {
		label: "钉钉",
		color: "text-blue-600 dark:text-blue-400",
		bg: "bg-blue-500/10",
		icon: "💬",
	},
	feishu: {
		label: "飞书",
		color: "text-indigo-600 dark:text-indigo-400",
		bg: "bg-indigo-500/10",
		icon: "📮",
	},
	wecom: {
		label: "企业微信",
		color: "text-slate-600 dark:text-slate-400",
		bg: "bg-slate-500/10",
		icon: "💼",
	},
};

export function SourceBadge({ source }: { source: MessageSource }) {
	const cfg = SOURCE_CONFIG[source];
	return (
		<span
			className={cn(
				"inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium border",
				cfg.bg,
				cfg.color,
				"border-current/10",
			)}
		>
			<span className="text-[9px] leading-none">{cfg.icon}</span>
			{cfg.label}
		</span>
	);
}
