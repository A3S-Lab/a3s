import { MemoizedMarkdown } from "@/components/custom/memoized-markdown";
import { DiffEditor } from "@monaco-editor/react";
import { cn } from "@/lib/utils";
import agentModel, {
	type ToolProgress,
	type CompletedToolCall,
} from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import {
	CheckCircle2,
	Clock,
	Code2,
	FileCode,
	FileText,
	Globe,
	Loader2,
	Lock,
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
		rs: "rust", ts: "typescript", tsx: "typescript",
		js: "javascript", jsx: "javascript", py: "python",
		go: "go", json: "json", toml: "toml", md: "markdown",
		css: "css", html: "html", sh: "shell",
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

/**
 * StreamingDisplay — uses valtio `subscribe` (not useSnapshot) to guarantee
 * re-renders on every streaming state mutation, bypassing proxy reactivity issues.
 */
export function StreamingDisplay({ sessionId }: { sessionId: string }) {
	const [text, setText] = useState<string | undefined>(undefined);
	const [toolProgress, setToolProgress] = useState<ToolProgress | null>(null);
	const [done, setDone] = useState<CompletedToolCall[]>([]);
	const [status, setStatus] = useState<string | null>(null);

	// Direct subscription to agentModel.state — fires on every mutation
	useEffect(() => {
		// Read initial state
		const readState = () => {
			const s = agentModel.state;
			setText(s.streaming[sessionId]);
			setToolProgress(s.activeToolProgress[sessionId] ?? null);
			setDone(s.completedTools[sessionId] || []);
			setStatus(s.sessionStatus[sessionId] ?? null);
		};
		readState();

		const unsub = subscribe(agentModel.state, readState);
		return unsub;
	}, [sessionId]);

	const isRunning = status === "running";
	const isCompacting = status === "compacting";
	const persona = personaModel.getSessionPersona(sessionId);
	const avatarConfig = useMemo(
		() => genConfig(persona.avatar),
		[persona.avatar],
	);

	if (!isRunning && !isCompacting) return null;

	return (
		<div className="px-5 py-4 border-t border-border/30 shrink-0 max-h-[40vh] overflow-y-auto bg-gradient-to-b from-foreground/[0.01] to-transparent">
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
				{/* Active tool execution */}
				{toolProgress && (
					<div className="rounded-xl border border-primary/20 bg-primary/[0.03] px-3 py-2.5">
						<div className="flex items-center gap-2">
							<Loader2 className="size-3.5 text-primary animate-spin shrink-0" />
							<span
								className={cn(
									"shrink-0 flex items-center justify-center size-5 rounded-md bg-primary/10",
									getToolMeta(toolProgress.tool_name).color,
								)}
							>
								{getToolMeta(toolProgress.tool_name).icon}
							</span>
							<span className="text-xs font-semibold text-foreground/90">
								{toolProgress.tool_name}
							</span>
							{toolProgress.input && (
								<span className="text-[11px] text-muted-foreground/50 truncate font-mono flex-1">
									{summarizeInput(toolProgress.input)}
								</span>
							)}
							{toolProgress.elapsed_time_seconds > 0 && (
								<span className="text-[10px] text-muted-foreground/40 flex items-center gap-0.5 ml-auto shrink-0 tabular-nums">
									<Clock className="size-2.5" />
									{Math.round(toolProgress.elapsed_time_seconds)}s
								</span>
							)}
						</div>
					</div>
				)}

				{/* Completed tool calls */}
				{done.map((t) => {
					const meta = getToolMeta(t.tool_name);
					const summary = summarizeInput(t.input);
					return (
						<div key={t.tool_use_id} className="rounded-xl border border-border/30 bg-muted/10 overflow-hidden">
							<div className="flex items-center gap-2 px-3 py-2">
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
							</div>
							{t.before != null && t.after != null && !t.is_error && (
								<div className="border-t border-border/30">
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
											scrollbar: { vertical: "hidden", horizontal: "hidden" },
										}}
										height={Math.min(300, Math.max(80,
											(t.after.split("\n").length + 4) * 18
										))}
									/>
								</div>
							)}
						</div>
					);
				})}

				{/* Streaming text */}
				{text ? (
					<div className="text-sm leading-relaxed">
						<MemoizedMarkdown id={`streaming-${sessionId}`} content={text} />
						{isRunning && !toolProgress && (
							<span className="inline-block w-0.5 h-4 bg-primary/70 animate-pulse ml-0.5 align-middle rounded-full" />
						)}
					</div>
				) : !toolProgress && done.length === 0 ? (
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
				) : null}
			</div>
		</div>
	);
}
