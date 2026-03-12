import { MemoizedMarkdown } from "@/components/custom/memoized-markdown";
import { DiffEditor } from "@monaco-editor/react";
import { cn } from "@/lib/utils";
import agentModel, {
	type ToolProgress,
	type CompletedToolCall,
	type StreamingSegment,
	type StreamPerfHint,
} from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import {
	ChevronDown,
	ChevronRight,
	CheckCircle2,
	Clock,
	Code2,
	FileCode,
	FileText,
	Globe,
	Loader2,
	Lock,
	ShieldAlert,
	Search,
	Terminal,
	Wrench,
	XCircle,
} from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import { subscribe } from "valtio";

function langFromPath(filePath?: string): string {
	if (!filePath) return "plaintext";
	const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
	const map: Record<string, string> = {
		rs: "rust",
		ts: "typescript",
		tsx: "typescript",
		js: "javascript",
		jsx: "javascript",
		py: "python",
		go: "go",
		json: "json",
		toml: "toml",
		md: "markdown",
		css: "css",
		html: "html",
		sh: "shell",
	};
	return map[ext] ?? "plaintext";
}

const TOOL_META: Record<string, { icon: React.ReactNode; color: string }> = {
	Read: { icon: <FileText className="size-3" />, color: "text-blue-500" },
	read: { icon: <FileText className="size-3" />, color: "text-blue-500" },
	Write: { icon: <FileCode className="size-3" />, color: "text-emerald-500" },
	write: { icon: <FileCode className="size-3" />, color: "text-emerald-500" },
	Edit: { icon: <Code2 className="size-3" />, color: "text-amber-500" },
	edit: { icon: <Code2 className="size-3" />, color: "text-amber-500" },
	Bash: { icon: <Terminal className="size-3" />, color: "text-purple-500" },
	bash: { icon: <Terminal className="size-3" />, color: "text-purple-500" },
	Grep: { icon: <Search className="size-3" />, color: "text-slate-500" },
	grep: { icon: <Search className="size-3" />, color: "text-slate-500" },
	Glob: { icon: <Search className="size-3" />, color: "text-slate-500" },
	glob: { icon: <Search className="size-3" />, color: "text-slate-500" },
	ls: { icon: <Search className="size-3" />, color: "text-slate-500" },
	web_search: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	WebSearch: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	web_fetch: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	WebFetch: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	Patch: { icon: <Code2 className="size-3" />, color: "text-amber-500" },
	patch: { icon: <Code2 className="size-3" />, color: "text-amber-500" },
	TEEPayment: { icon: <Lock className="size-3" />, color: "text-rose-500" },
};

function getToolMeta(name: string) {
	return (
		TOOL_META[name] || {
			icon: <Wrench className="size-3" />,
			color: "text-primary",
		}
	);
}

function summarizeInput(input: string): string {
	try {
		const parsed = JSON.parse(input);
		if (parsed.command) return parsed.command;
		if (parsed.file_path) return parsed.file_path;
		if (parsed.path) return parsed.path;
		if (parsed.pattern) return `/${parsed.pattern}/`;
		if (parsed.query) return parsed.query;
		if (parsed.url) return parsed.url;
		for (const v of Object.values(parsed)) {
			if (typeof v === "string" && v.length > 0) {
				return v.length > 60 ? `${v.slice(0, 60)}…` : v;
			}
		}
	} catch {
		/* not JSON */
	}
	return input.length > 60 ? `${input.slice(0, 60)}…` : input;
}

function CompletedToolSegment({
	t,
	expandedTools,
	setExpandedTools,
}: {
	t: CompletedToolCall;
	expandedTools: Set<string>;
	setExpandedTools: React.Dispatch<React.SetStateAction<Set<string>>>;
}) {
	const meta = getToolMeta(t.tool_name);
	const summary = summarizeInput(t.input);
	const hasDiff = t.before != null && t.after != null && !t.is_error;
	const hasDetails = !!t.input || !!t.output || hasDiff;
	const isExpanded = expandedTools.has(t.tool_use_id);
	const toggleExpand = hasDetails
		? () =>
				setExpandedTools((prev) => {
					const next = new Set(prev);
					if (next.has(t.tool_use_id)) next.delete(t.tool_use_id);
					else next.add(t.tool_use_id);
					return next;
				})
		: undefined;

	return (
		<div className="rounded-xl border border-border/30 bg-muted/10 overflow-hidden">
			<div
				className={cn(
					"flex items-center gap-2 px-3 py-2",
					hasDetails && "cursor-pointer hover:bg-foreground/[0.03]",
				)}
				onClick={toggleExpand}
			>
				{t.is_error ? (
					<XCircle className="size-3.5 text-destructive/70 shrink-0" />
				) : (
					<CheckCircle2 className="size-3.5 text-emerald-500/80 shrink-0" />
				)}
				<span
					className={cn(
						"shrink-0 flex items-center justify-center size-5 rounded-md bg-muted/40",
						meta.color,
					)}
				>
					{meta.icon}
				</span>
				<span className="text-xs font-semibold text-foreground/90">
					{t.tool_name}
				</span>
				<span className="text-[11px] text-muted-foreground/50 truncate font-mono">
					{summary}
				</span>
				{hasDetails &&
					(isExpanded ? (
						<ChevronDown className="size-3 text-muted-foreground/40 shrink-0 ml-auto" />
					) : (
						<ChevronRight className="size-3 text-muted-foreground/40 shrink-0 ml-auto" />
					))}
			</div>
			{isExpanded && (
				<div className="border-t border-border/30">
					{t.input && (
						<pre className="px-3 py-2 text-[11px] font-mono text-muted-foreground border-b border-border/20 whitespace-pre-wrap break-words">
							{t.input}
						</pre>
					)}
					{t.output && (
						<pre className="px-3 py-2 text-[11px] font-mono text-foreground/80 border-b border-border/20 whitespace-pre-wrap break-words">
							{t.output}
						</pre>
					)}
					{hasDiff && (
						<DiffEditor
							original={t.before}
							modified={t.after}
							language={langFromPath(t.file_path)}
							theme="vs-dark"
							options={{
								readOnly: true,
								renderSideBySide: false,
								minimap: { enabled: false },
								scrollBeyondLastLine: false,
								fontSize: 11,
								lineNumbers: "off",
								folding: false,
								contextmenu: false,
								scrollbar: { vertical: "auto", horizontal: "hidden" },
							}}
							height={Math.min(
								400,
								Math.max(
									80,
									((t.before ?? "").split("\n").length +
										(t.after ?? "").split("\n").length +
										4) *
										18,
								),
							)}
						/>
					)}
				</div>
			)}
		</div>
	);
}

function ActiveToolSegment({
	p,
	expandedTools,
	setExpandedTools,
}: {
	p: ToolProgress;
	expandedTools: Set<string>;
	setExpandedTools: React.Dispatch<React.SetStateAction<Set<string>>>;
}) {
	const meta = getToolMeta(p.tool_name);
	const isExpanded = expandedTools.has(p.tool_use_id);
	const hasDetails = !!p.input || !!p.output;
	const toggleExpand = hasDetails
		? () =>
				setExpandedTools((prev) => {
					const next = new Set(prev);
					if (next.has(p.tool_use_id)) next.delete(p.tool_use_id);
					else next.add(p.tool_use_id);
					return next;
				})
		: undefined;

	return (
		<div className="rounded-xl border border-primary/20 bg-primary/[0.03] overflow-hidden">
			<div
				className={cn(
					"flex items-center gap-2 px-3 py-2",
					hasDetails && "cursor-pointer hover:bg-foreground/[0.03]",
				)}
				onClick={toggleExpand}
			>
				<Loader2 className="size-3.5 text-primary animate-spin shrink-0" />
				<span
					className={cn(
						"shrink-0 flex items-center justify-center size-5 rounded-md bg-primary/10",
						meta.color,
					)}
				>
					{meta.icon}
				</span>
				<span className="text-xs font-semibold text-foreground/90">
					{p.tool_name}
				</span>
				{p.input && (
					<span className="text-[11px] text-muted-foreground/50 truncate font-mono flex-1">
						{summarizeInput(p.input)}
					</span>
				)}
				{p.elapsed_time_seconds > 0 && (
					<span className="text-[10px] text-muted-foreground/40 flex items-center gap-0.5 ml-auto shrink-0 tabular-nums">
						<Clock className="size-2.5" />
						{Math.round(p.elapsed_time_seconds)}s
					</span>
				)}
				{hasDetails &&
					(isExpanded ? (
						<ChevronDown className="size-3 text-muted-foreground/40 shrink-0" />
					) : (
						<ChevronRight className="size-3 text-muted-foreground/40 shrink-0" />
					))}
			</div>
			{hasDetails && isExpanded && (
				<div className="border-t border-border/30">
					{p.input && (
						<pre className="px-3 py-2 text-[11px] font-mono text-muted-foreground border-b border-border/20 whitespace-pre-wrap break-words">
							{p.input}
						</pre>
					)}
					{p.output && (
						<pre className="px-3 py-2 text-[11px] font-mono text-foreground/80 whitespace-pre-wrap break-words">
							{p.output}
						</pre>
					)}
				</div>
			)}
		</div>
	);
}

/**
 * StreamingDisplay — renders streaming content in arrival order.
 *
 * Uses `streamingSegments` (an ordered array of text/tool blocks) instead of
 * the old separate `completedTools` + `streaming` buckets, which caused all
 * tool calls to render before all text regardless of actual arrival order.
 */
export function StreamingDisplay({ sessionId }: { sessionId: string }) {
	const [segments, setSegments] = useState<StreamingSegment[]>([]);
	const [status, setStatus] = useState<string | null>(null);
	const [streamStartedAt, setStreamStartedAt] = useState<number | null>(null);
	const [pendingPermissionCount, setPendingPermissionCount] = useState(0);
	const [perfHint, setPerfHint] = useState<StreamPerfHint | null>(null);
	const [nowMs, setNowMs] = useState(() => Date.now());
	const [expandedTools, setExpandedTools] = useState<Set<string>>(new Set());

	// Direct subscription to agentModel.state — fires on every mutation
	useEffect(() => {
		const readState = () => {
			const s = agentModel.state;
			// Clone array to force React updates for in-place proxy mutations.
			setSegments([...(s.streamingSegments[sessionId] || [])]);
			setStatus(s.sessionStatus[sessionId] ?? null);
			setStreamStartedAt(s.streamingStartedAt[sessionId] ?? null);
			setPendingPermissionCount(
				Object.keys(s.pendingPermissions[sessionId] || {}).length,
			);
			setPerfHint(s.streamPerfHint[sessionId] ?? null);
		};
		readState();

		const unsub = subscribe(agentModel.state, readState);
		return unsub;
	}, [sessionId]);

	const isRunning = status === "running";
	const isCompacting = status === "compacting";
	const persona = personaModel.getSessionPersona(sessionId);
	const avatarConfig = useMemo(() => persona.avatar, [persona.avatar]);
	const orderedSegments = useMemo(
		() => [...segments].sort((a, b) => a.seq - b.seq),
		[segments],
	);

	// Find the last text segment so the blinking cursor appears after it
	let lastTextSegmentIdx = -1;
	for (let i = orderedSegments.length - 1; i >= 0; i--) {
		const seg = orderedSegments[i];
		if (seg.type === "text" && seg.content.length > 0) {
			lastTextSegmentIdx = i;
			break;
		}
	}
	const hasActiveTool = orderedSegments.some(
		(seg) => seg.type === "tool_progress",
	);
	const waitingSec = streamStartedAt
		? Math.max(0, Math.floor((nowMs - streamStartedAt) / 1000))
		: 0;
	const showSlowHint =
		isRunning &&
		pendingPermissionCount === 0 &&
		orderedSegments.length === 0 &&
		waitingSec >= 8;
	const slowStageLabel: Record<string, string> = {
		frontend_send: "前端发送前准备偏慢",
		model_first_token: "模型首字响应偏慢",
		permission_wait: "等待权限确认",
		tool_exec: "工具执行阶段偏慢",
		unknown: "阶段待确认",
	};

	useEffect(() => {
		if (!isRunning || !streamStartedAt) return;
		const timer = setInterval(() => setNowMs(Date.now()), 400);
		return () => clearInterval(timer);
	}, [isRunning, streamStartedAt]);

	const shouldRender =
		isRunning ||
		isCompacting ||
		orderedSegments.length > 0 ||
		pendingPermissionCount > 0 ||
		(streamStartedAt != null && waitingSec < 120);
	if (!shouldRender) return null;

	return (
		<div className="px-5 py-4 border-t border-border/30 shrink-0 bg-gradient-to-b from-foreground/[0.01] to-transparent">
			{/* Header */}
			<div className="flex items-center gap-2.5 mb-2.5">
				<NiceAvatar
					className="size-7 shrink-0 ring-2 ring-background shadow-sm"
					{...avatarConfig}
				/>
				<span className="text-[13px] font-semibold">{persona.name}</span>
				{(isRunning || isCompacting) && (
					<Loader2 className="size-3 text-primary animate-spin" />
				)}
				{isCompacting && (
					<span className="text-[10px] text-orange-500 font-medium px-1.5 py-0.5 rounded-md bg-orange-500/8 border border-orange-500/10">
						压缩上下文中...
					</span>
				)}
			</div>

			<div className="ml-[38px] space-y-1.5">
				{pendingPermissionCount > 0 && (
					<div className="rounded-md border border-amber-500/25 bg-amber-500/10 px-2.5 py-2 text-xs flex items-center gap-2">
						<ShieldAlert className="size-3.5 text-amber-500 shrink-0" />
						<span className="text-amber-700 dark:text-amber-300">
							等待你确认权限（{pendingPermissionCount}）
						</span>
					</div>
				)}

				{showSlowHint && (
					<div className="rounded-md border border-blue-500/25 bg-blue-500/10 px-2.5 py-2 text-xs flex items-center gap-2">
						<Loader2 className="size-3.5 text-blue-500 animate-spin shrink-0" />
						<div className="text-blue-700 dark:text-blue-300">
							<div>模型响应较慢，已等待 {waitingSec}s</div>
							{perfHint && (
								<div className="text-[11px] opacity-80">
									慢点推断：
									{slowStageLabel[perfHint.slow_stage] ?? perfHint.slow_stage}
								</div>
							)}
						</div>
					</div>
				)}

				{/* Ordered segments — text and tool calls interleaved as they arrived */}
				{orderedSegments.map((seg, idx) => {
					if (seg.type === "text") {
						if (seg.content.length === 0) return null;
						const isLastText = idx === lastTextSegmentIdx;
						return (
							<div key={`text-${seg.seq}`} className="text-sm leading-relaxed">
								<MemoizedMarkdown
									id={`streaming-${sessionId}-${seg.seq}`}
									content={seg.content}
								/>
								{isLastText && isRunning && !hasActiveTool && (
									<span className="inline-block w-0.5 h-4 bg-primary/70 animate-pulse ml-0.5 align-middle rounded-full" />
								)}
							</div>
						);
					}
					if (seg.type === "tool_progress") {
						return (
							<ActiveToolSegment
								key={`tool-progress-${seg.progress.tool_use_id}-${seg.seq}`}
								p={seg.progress}
								expandedTools={expandedTools}
								setExpandedTools={setExpandedTools}
							/>
						);
					}
					// seg.type === "tool"
					return (
						<CompletedToolSegment
							key={`${seg.call.tool_use_id}-${seg.seq}`}
							t={seg.call}
							expandedTools={expandedTools}
							setExpandedTools={setExpandedTools}
						/>
					);
				})}

				{/* Thinking indicator — only before any content arrives */}
				{orderedSegments.length === 0 && (
					<div className="flex items-center gap-2.5 text-xs text-muted-foreground/50">
						<span>思考中</span>
						<span className="flex gap-1">
							<span
								className="size-1 rounded-full bg-primary/30 animate-bounce"
								style={{ animationDelay: "0ms" }}
							/>
							<span
								className="size-1 rounded-full bg-primary/30 animate-bounce"
								style={{ animationDelay: "150ms" }}
							/>
							<span
								className="size-1 rounded-full bg-primary/30 animate-bounce"
								style={{ animationDelay: "300ms" }}
							/>
						</span>
					</div>
				)}
			</div>
		</div>
	);
}
