/**
 * Agent Detail Page — shows real data from agentModel and personaModel.
 * Sections: active tool, tool history, skills, tools, MCP servers, cost stats.
 * Cron jobs and context database are omitted until backend APIs exist.
 */
import { cn } from "@/lib/utils";
import personaModel from "@/models/persona.model";
import agentModel from "@/models/agent.model";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import {
	ArrowLeft,
	CheckCircle2,
	Clock,
	Code2,
	DollarSign,
	FileText,
	Hash,
	Loader2,
	Lock,
	MessageSquare,
	RefreshCw,
	Server,
	Settings,
	Sparkles,
	Terminal,
	Timer,
	Wrench,
	XCircle,
	Zap,
} from "lucide-react";
import { useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useSnapshot } from "valtio";
import type { ContentBlock } from "@/typings/agent";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";
import "dayjs/locale/zh-cn";

dayjs.extend(relativeTime);
dayjs.locale("zh-cn");

// =============================================================================
// Types
// =============================================================================

interface ToolHistoryItem {
	id: string;
	tool: string;
	input: string;
	output?: string;
	isError: boolean;
	timestamp: number;
}

// =============================================================================
// Helpers
// =============================================================================

const toolIconMap: Record<string, typeof Terminal> = {
	bash: Terminal,
	read: FileText,
	write: FileText,
	edit: Code2,
	grep: FileText,
	glob: FileText,
	default: Wrench,
};

function getToolIcon(name: string) {
	const key = name.toLowerCase();
	for (const [k, v] of Object.entries(toolIconMap)) {
		if (key.includes(k)) return v;
	}
	return toolIconMap.default;
}

function formatCost(usd: number): string {
	if (usd < 0.01) return `$${(usd * 100).toFixed(2)}¢`;
	return `$${usd.toFixed(4)}`;
}

// =============================================================================
// Sub-components
// =============================================================================

function Section({
	icon: Icon,
	title,
	count,
	children,
	className,
}: {
	icon: typeof Wrench;
	title: string;
	count?: number;
	children: React.ReactNode;
	className?: string;
}) {
	return (
		<div className={cn("mb-6", className)}>
			<div className="flex items-center gap-2 mb-3">
				<Icon className="size-4 text-primary" />
				<span className="text-sm font-semibold">{title}</span>
				{count !== undefined && (
					<span className="text-[11px] text-muted-foreground">({count})</span>
				)}
			</div>
			{children}
		</div>
	);
}

function ActiveToolCard({
	tool_name,
	elapsed_time_seconds,
}: { tool_name: string; elapsed_time_seconds: number }) {
	const Icon = getToolIcon(tool_name);
	return (
		<div className="flex items-center gap-3 rounded-lg border border-primary/30 bg-primary/5 px-3 py-2.5">
			<div className="flex items-center justify-center size-7 rounded-lg bg-primary/10 shrink-0">
				<Loader2 className="size-3.5 text-primary animate-spin" />
			</div>
			<div className="flex-1 min-w-0">
				<div className="flex items-center gap-1.5">
					<Icon className="size-3 text-primary" />
					<code className="text-xs font-mono font-medium text-primary">
						{tool_name}
					</code>
				</div>
				<p className="text-[10px] text-muted-foreground mt-0.5">执行中...</p>
			</div>
			<span className="text-[10px] text-muted-foreground flex items-center gap-0.5 shrink-0">
				<Timer className="size-2.5" />
				{Math.round(elapsed_time_seconds)}s
			</span>
		</div>
	);
}

function ToolHistoryRow({ item }: { item: ToolHistoryItem }) {
	const Icon = getToolIcon(item.tool);
	return (
		<div className="flex items-center gap-3 rounded-lg border bg-card px-3 py-2 hover:bg-accent/30 transition-colors">
			<div
				className={cn(
					"flex items-center justify-center size-6 rounded-md shrink-0",
					item.isError ? "bg-red-500/10" : "bg-muted",
				)}
			>
				{item.isError ? (
					<XCircle className="size-3.5 text-red-500" />
				) : (
					<CheckCircle2 className="size-3.5 text-emerald-500" />
				)}
			</div>
			<Icon className="size-3.5 text-muted-foreground shrink-0" />
			<code className="text-xs font-mono text-primary shrink-0">
				{item.tool}
			</code>
			<span className="text-[11px] text-muted-foreground flex-1 min-w-0 truncate">
				{item.input}
			</span>
			<span className="text-[10px] text-muted-foreground shrink-0">
				{dayjs(item.timestamp).fromNow()}
			</span>
		</div>
	);
}

function StatCard({
	icon: Icon,
	label,
	value,
	sub,
}: { icon: typeof Zap; label: string; value: string; sub?: string }) {
	return (
		<div className="rounded-lg border bg-card px-3 py-2.5 flex items-center gap-3">
			<div className="flex items-center justify-center size-8 rounded-lg bg-muted shrink-0">
				<Icon className="size-4 text-muted-foreground" />
			</div>
			<div className="min-w-0">
				<div className="text-[10px] text-muted-foreground">{label}</div>
				<div className="text-sm font-semibold">{value}</div>
				{sub && <div className="text-[10px] text-muted-foreground">{sub}</div>}
			</div>
		</div>
	);
}

// =============================================================================
// Persona Defaults Section
// =============================================================================

import type { AgentPersona } from "@/typings/persona";
import { toast } from "sonner";

function PersonaDefaultsSection({ persona }: { persona: AgentPersona }) {
	const [model, setModel] = useState(persona.defaultModel ?? "");
	const [permMode, setPermMode] = useState(
		persona.defaultPermissionMode ?? "default",
	);
	const [systemPrompt, setSystemPrompt] = useState(persona.systemPrompt ?? "");

	const handleSave = () => {
		personaModel.updatePersonaDefaults(persona.id, {
			defaultModel: model.trim() || undefined,
			defaultPermissionMode: permMode !== "default" ? permMode : undefined,
			systemPrompt: systemPrompt.trim(),
		});
		toast.success("默认配置已保存");
	};

	return (
		<Section icon={Settings} title="默认配置">
			<div className="space-y-4">
				<div className="space-y-1.5">
					<label className="text-xs font-medium text-muted-foreground">
						默认模型
					</label>
					<Input
						value={model}
						onChange={(e) => setModel(e.target.value)}
						className="h-8 text-xs font-mono"
						placeholder="留空则使用全局默认"
					/>
				</div>
				<div className="space-y-1.5">
					<label className="text-xs font-medium text-muted-foreground">
						默认权限模式
					</label>
					<Select value={permMode} onValueChange={setPermMode}>
						<SelectTrigger className="h-8 text-xs">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="default" className="text-xs">
								Agent（默认）
							</SelectItem>
							<SelectItem value="plan" className="text-xs">
								计划模式
							</SelectItem>
							<SelectItem value="bypassPermissions" className="text-xs">
								跳过权限
							</SelectItem>
						</SelectContent>
					</Select>
				</div>
				<div className="space-y-1.5">
					<label className="text-xs font-medium text-muted-foreground">
						系统提示词
					</label>
					<Textarea
						value={systemPrompt}
						onChange={(e) => setSystemPrompt(e.target.value)}
						className="text-xs font-mono min-h-[80px] resize-y"
						placeholder="输入系统提示词..."
					/>
				</div>
				<Button size="sm" className="h-7 text-xs" onClick={handleSave}>
					保存
				</Button>
			</div>
		</Section>
	);
}

// =============================================================================
// Main Page
// =============================================================================

export default function AgentDetailPage() {
	const { agentId } = useParams<{ agentId: string }>();
	const nav = useNavigate();

	const personaSnap = useSnapshot(personaModel.state);
	const agentSnap = useSnapshot(agentModel.state);

	// Find persona (builtin + server + custom)
	const persona = useMemo(
		() => personaModel.getAllPersonas().find((p) => p.id === agentId) ?? null,
		[agentId, personaSnap.serverPersonas, personaSnap.customPersonas],
	);
	const avatarCfg = useMemo(
		() => (persona ? genConfig(persona.avatar) : genConfig()),
		[persona],
	);

	// Find the latest non-archived session for this persona
	const session = useMemo(() => {
		if (!agentId) return null;
		const sessionIds = Object.entries(personaSnap.sessionPersonas)
			.filter(([, pid]) => pid === agentId)
			.map(([sid]) => sid);
		const sessions = agentSnap.sdkSessions
			.filter((s) => sessionIds.includes(s.session_id) && !s.archived)
			.sort((a, b) => b.created_at - a.created_at);
		return sessions[0] ?? null;
	}, [agentId, personaSnap.sessionPersonas, agentSnap.sdkSessions]);

	const sessionState = session ? agentSnap.sessions[session.session_id] : null;
	const sessionId = session?.session_id ?? null;

	// Active tool progress
	const activeToolProgress = sessionId
		? agentSnap.activeToolProgress[sessionId]
		: null;

	// Extract tool history from messages
	const toolHistory = useMemo((): ToolHistoryItem[] => {
		if (!sessionId) return [];
		const msgs = agentSnap.messages[sessionId] ?? [];
		const items: ToolHistoryItem[] = [];

		// Build tool_use_id → tool_result map
		const resultMap = new Map<string, { content: string; isError: boolean }>();
		for (const msg of msgs) {
			if (!msg.contentBlocks) continue;
			for (const block of msg.contentBlocks) {
				if (block.type === "tool_result") {
					const content =
						typeof block.content === "string"
							? block.content
							: (block.content as ContentBlock[])
									.filter((b) => b.type === "text")
									.map((b) => (b as { type: "text"; text: string }).text)
									.join("");
					resultMap.set(block.tool_use_id, {
						content,
						isError: block.is_error ?? false,
					});
				}
			}
		}

		for (const msg of msgs) {
			if (!msg.contentBlocks) continue;
			for (const block of msg.contentBlocks) {
				if (block.type === "tool_use") {
					const result = resultMap.get(block.id);
					items.push({
						id: block.id,
						tool: block.name,
						input: JSON.stringify(block.input).slice(0, 120),
						output: result?.content?.slice(0, 80),
						isError: result?.isError ?? false,
						timestamp: msg.timestamp,
					});
				}
			}
		}

		return items.reverse();
	}, [sessionId, agentSnap.messages]);

	// Skills, tools, MCP from session state
	const skills = sessionState?.skills ?? [];
	const tools = sessionState?.tools ?? [];
	const mcpServers = sessionState?.mcp_servers ?? [];

	// Stats
	const totalCost = sessionState?.total_cost_usd ?? 0;
	const numTurns = sessionState?.num_turns ?? 0;
	const contextPct = Math.round(sessionState?.context_used_percent ?? 0);
	const linesAdded = sessionState?.total_lines_added ?? 0;
	const linesRemoved = sessionState?.total_lines_removed ?? 0;

	const sessionStatus = sessionId ? agentSnap.sessionStatus[sessionId] : null;
	const isRunning = sessionStatus === "running";

	if (!persona) {
		return (
			<div className="flex items-center justify-center h-full text-muted-foreground">
				<div className="text-center space-y-3">
					<p className="text-lg font-medium text-foreground">智能体不存在</p>
					<Button variant="outline" onClick={() => nav("/")}>
						<ArrowLeft className="size-4 mr-1.5" />
						返回
					</Button>
				</div>
			</div>
		);
	}

	return (
		<div className="flex flex-col h-full w-full">
			{/* Header */}
			<div className="flex items-center gap-4 px-6 py-3 border-b shrink-0">
				<Button
					variant="ghost"
					size="icon"
					className="size-8 shrink-0"
					onClick={() => nav("/")}
				>
					<ArrowLeft className="size-4" />
				</Button>
				<NiceAvatar className="size-9 shrink-0" {...avatarCfg} />
				<div className="min-w-0 flex-1">
					<div className="flex items-center gap-1.5">
						<h1 className="text-sm font-bold truncate">{persona.name}</h1>
						{persona.undeletable && (
							<Lock className="size-3 text-muted-foreground shrink-0" />
						)}
					</div>
					<p className="text-[11px] text-muted-foreground truncate">
						{persona.description}
					</p>
				</div>
				<div className="flex items-center gap-3 shrink-0 text-[11px]">
					{isRunning && (
						<span className="inline-flex items-center gap-1 text-primary font-medium">
							<Loader2 className="size-3 animate-spin" />
							执行中
						</span>
					)}
					{session && (
						<span className="text-muted-foreground">
							<MessageSquare className="size-3 inline mr-0.5" />
							{numTurns} 轮
						</span>
					)}
					{session && (
						<span className="text-muted-foreground">
							<Hash className="size-3 inline mr-0.5" />
							{toolHistory.length} 工具调用
						</span>
					)}
					{session && totalCost > 0 && (
						<span className="text-muted-foreground">
							<DollarSign className="size-3 inline" />
							{formatCost(totalCost)}
						</span>
					)}
				</div>
			</div>

			<ScrollArea className="flex-1">
				<div className="px-6 py-5 space-y-6">
					{/* No session state */}
					{!session && (
						<div className="rounded-lg border border-dashed bg-muted/20 px-6 py-10 text-center">
							<p className="text-sm text-muted-foreground mb-3">
								该智能体暂无活跃会话
							</p>
							<Button variant="outline" size="sm" onClick={() => nav("/")}>
								<RefreshCw className="size-3.5 mr-1.5" />
								返回创建会话
							</Button>
						</div>
					)}

					{/* Persona defaults — always visible */}
					<PersonaDefaultsSection persona={persona} />

					{session && (
						<>
							{/* Stats row */}
							<div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
								<StatCard
									icon={MessageSquare}
									label="对话轮次"
									value={String(numTurns)}
								/>
								<StatCard
									icon={DollarSign}
									label="累计成本"
									value={totalCost > 0 ? formatCost(totalCost) : "—"}
								/>
								<StatCard
									icon={Zap}
									label="上下文使用"
									value={`${contextPct}%`}
									sub={contextPct >= 80 ? "接近上限" : undefined}
								/>
								<StatCard
									icon={Code2}
									label="代码变更"
									value={
										linesAdded + linesRemoved > 0
											? `+${linesAdded} / -${linesRemoved}`
											: "—"
									}
								/>
							</div>

							{/* Active tool */}
							{activeToolProgress && (
								<Section icon={Loader2} title="当前工具">
									<ActiveToolCard
										tool_name={activeToolProgress.tool_name}
										elapsed_time_seconds={
											activeToolProgress.elapsed_time_seconds
										}
									/>
								</Section>
							)}

							{/* Tool history */}
							<Section
								icon={Clock}
								title="工具调用历史"
								count={toolHistory.length}
							>
								{toolHistory.length > 0 ? (
									<div className="space-y-1.5 max-h-[320px] overflow-y-auto pr-1">
										{toolHistory.map((item) => (
											<ToolHistoryRow key={item.id} item={item} />
										))}
									</div>
								) : (
									<p className="text-xs text-muted-foreground py-4 text-center">
										暂无工具调用记录
									</p>
								)}
							</Section>

							{/* Skills + Tools */}
							<div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
								<Section icon={Sparkles} title="技能" count={skills.length}>
									{skills.length > 0 ? (
										<div className="flex flex-wrap gap-1.5">
											{skills.map((s) => (
												<span
													key={s}
													className="inline-flex items-center gap-1 rounded-md border bg-card px-2.5 py-1.5 text-xs text-muted-foreground"
												>
													<Sparkles className="size-3 text-primary" />
													{s}
												</span>
											))}
										</div>
									) : (
										<p className="text-xs text-muted-foreground py-2">
											暂无技能
										</p>
									)}
								</Section>

								<Section icon={Wrench} title="工具" count={tools.length}>
									{tools.length > 0 ? (
										<div className="flex flex-wrap gap-1.5">
											{tools.map((t) => {
												const TIcon = getToolIcon(t);
												return (
													<span
														key={t}
														className="inline-flex items-center gap-1 rounded-md border bg-card px-2.5 py-1.5 text-xs text-muted-foreground"
													>
														<TIcon className="size-3.5" />
														{t}
													</span>
												);
											})}
										</div>
									) : (
										<p className="text-xs text-muted-foreground py-2">
											暂无工具
										</p>
									)}
								</Section>
							</div>

							{/* MCP Servers */}
							{mcpServers.length > 0 && (
								<Section
									icon={Server}
									title="MCP 服务器"
									count={mcpServers.length}
								>
									<div className="flex flex-wrap gap-2">
										{mcpServers.map((mcp) => (
											<div
												key={mcp.name}
												className={cn(
													"inline-flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-xs",
													mcp.status === "connected"
														? "border-emerald-500/30 bg-emerald-500/5 text-emerald-600 dark:text-emerald-400"
														: "border-border text-muted-foreground",
												)}
											>
												<Server className="size-3" />
												<span>{mcp.name}</span>
												<span
													className={cn(
														"text-[9px] rounded px-1",
														mcp.status === "connected"
															? "bg-emerald-500/10"
															: "bg-muted",
													)}
												>
													{mcp.status}
												</span>
											</div>
										))}
									</div>
								</Section>
							)}
						</>
					)}
				</div>
			</ScrollArea>
		</div>
	);
}
