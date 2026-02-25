import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { cn } from "@/lib/utils";
import { timeAgo } from "@/lib/time";
import { toast } from "sonner";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import settingsModel, {
	resolveApiKey,
	resolveBaseUrl,
} from "@/models/settings.model";
import { connectSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import type { AgentProcessInfo } from "@/typings/agent";
import type { AgentPersona } from "@/typings/persona";
import type { AvatarFullConfig } from "react-nice-avatar";
import {
	Code2,
	FileCode,
	FileText,
	Globe,
	Lock,
	Loader2,
	Plus,
	Search,
	Terminal,
	Wrench,
} from "lucide-react";
import Avatar, { genConfig } from "react-nice-avatar";
import { useCallback, useMemo, useState } from "react";
import React from "react";
import { useNavigate } from "react-router-dom";
import { useSnapshot } from "valtio";
import CreateSessionDialog from "./create-session-dialog";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function relativeTime(ts: number): string {
	// Normalize: backend may send seconds instead of milliseconds
	const normalized = ts > 0 && ts < 1e12 ? ts * 1000 : ts;
	return timeAgo(normalized);
}

const TOOL_ICONS: Record<string, { icon: React.ReactNode; color: string }> = {
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
	web_search: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	WebSearch: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	web_fetch: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
	WebFetch: { icon: <Globe className="size-3" />, color: "text-indigo-500" },
};

function getToolIcon(name: string) {
	return (
		TOOL_ICONS[name] || {
			icon: <Wrench className="size-3" />,
			color: "text-primary",
		}
	);
}

// ---------------------------------------------------------------------------
// Agent item — single row, no expand
// ---------------------------------------------------------------------------

const AgentItem = React.memo(function AgentItem({
	persona,
	sessions,
	sessionStates,
	isActive,
	unreadCount,
	sessionStatus,
	toolProgress,
	lastMessage,
	lastMessageTime,
	onSelect,
	onAvatarClick,
}: {
	persona: AgentPersona;
	sessions: AgentProcessInfo[];
	sessionStates: Record<string, import("@/typings/agent").AgentSessionState>;
	isActive: boolean;
	unreadCount: number;
	sessionStatus: Record<string, "idle" | "running" | "compacting" | null>;
	toolProgress: Record<
		string,
		{ tool_name: string; elapsed_time_seconds: number } | null
	>;
	lastMessage: Record<string, string>;
	lastMessageTime: Record<string, number>;
	onSelect: () => void;
	onAvatarClick: () => void;
}) {
	const cfg = useMemo(() => genConfig(persona.avatar), [persona.avatar]);
	const activeSessions = useMemo(
		() =>
			[...sessions]
				.filter((s) => !s.archived)
				.sort((a, b) => b.created_at - a.created_at),
		[sessions],
	);
	const hasActiveSessions = activeSessions.length > 0;
	const latestSid = hasActiveSessions ? activeSessions[0].session_id : null;

	// Derive health dot: running > compacting > idle > none
	const healthStatus = useMemo(() => {
		if (!hasActiveSessions) return null;
		const statuses = activeSessions.map((s) => sessionStatus[s.session_id]);
		if (statuses.some((s) => s === "running")) return "running";
		if (statuses.some((s) => s === "compacting")) return "compacting";
		return "idle";
	}, [activeSessions, sessionStatus, hasActiveSessions]);

	// Current task: active tool > last message > description
	const taskLine = useMemo(() => {
		if (!latestSid) return persona.description;
		const progress = toolProgress[latestSid];
		if (progress) return null; // rendered as rich element below
		const last = lastMessage[latestSid];
		if (last) return last;
		return persona.description;
	}, [latestSid, toolProgress, lastMessage, persona.description]);

	const activeProgress = latestSid ? toolProgress[latestSid] : null;

	// Context usage from latest session state
	const contextPct = latestSid
		? (sessionStates[latestSid]?.context_used_percent ?? 0)
		: 0;
	const isCompacting = healthStatus === "compacting";

	return (
		<div
			role="option"
			aria-selected={isActive}
			tabIndex={-1}
			className={cn(
				"group flex items-start gap-3 px-3 py-3 w-full cursor-pointer transition-colors",
				"hover:bg-accent/[0.08]",
				isActive && "bg-primary/[0.08]",
			)}
			onClick={onSelect}
			onKeyDown={(e) => {
				if (e.key === "Enter" || e.key === " ") {
					e.preventDefault();
					onSelect();
				}
			}}
		>
			{/* Avatar with health dot */}
			<div className="relative shrink-0">
				<button
					type="button"
					className="rounded-full focus:outline-none focus:ring-2 focus:ring-primary/50 hover:ring-2 hover:ring-primary/30 transition-all"
					onClick={(e) => {
						e.stopPropagation();
						onAvatarClick();
					}}
					aria-label={`查看 ${persona.name} 详情`}
				>
					<Avatar className="w-9 h-9" {...cfg} />
				</button>
				{healthStatus && (
					<span
						className={cn(
							"absolute bottom-0 right-0 size-2.5 rounded-full border-2 border-background",
							healthStatus === "running" && "bg-green-500",
							healthStatus === "compacting" && "bg-yellow-500",
							healthStatus === "idle" && "bg-muted-foreground/40",
						)}
						title={
							healthStatus === "running"
								? "运行中"
								: healthStatus === "compacting"
									? "压缩中"
									: "就绪"
						}
					/>
				)}
			</div>

			<div className="flex-1 min-w-0">
				{/* Row 1: name + unread + time */}
				<div className="flex justify-between items-center">
					<div className="flex items-center gap-1 min-w-0">
						<span className="text-sm font-medium truncate">{persona.name}</span>
						{persona.undeletable && (
							<Lock className="size-3 text-muted-foreground/50 shrink-0" />
						)}
					</div>
					<div className="flex items-center gap-1.5 shrink-0 ml-2">
						{unreadCount > 0 && (
							<span className="flex items-center justify-center min-w-[18px] h-[18px] rounded-full bg-primary text-primary-foreground text-[10px] font-bold px-1 leading-none">
								{unreadCount > 99 ? "99+" : unreadCount}
							</span>
						)}
						{latestSid && (
							<time className="text-[10px] text-muted-foreground/80">
								{relativeTime(
									lastMessageTime[latestSid] ?? activeSessions[0].created_at,
								)}
							</time>
						)}
					</div>
				</div>

				{/* Row 2: current task */}
				{activeProgress ? (
					<div className="flex items-center gap-1.5 mt-0.5">
						<Loader2 className="size-3 text-primary animate-spin shrink-0" />
						<span
							className={cn(
								"shrink-0",
								getToolIcon(activeProgress.tool_name).color,
							)}
						>
							{getToolIcon(activeProgress.tool_name).icon}
						</span>
						<span className="text-[11px] font-medium text-foreground truncate">
							{activeProgress.tool_name}
						</span>
						{activeProgress.elapsed_time_seconds > 0 && (
							<span className="text-[10px] text-muted-foreground/60 shrink-0">
								{Math.round(activeProgress.elapsed_time_seconds)}s
							</span>
						)}
					</div>
				) : (
					<p
						className={cn(
							"text-[11px] truncate mt-0.5 leading-tight",
							healthStatus === "running"
								? "text-green-600 dark:text-green-400"
								: "text-muted-foreground/70",
						)}
					>
						{healthStatus === "running" && (
							<span className="inline-block size-1.5 rounded-full bg-green-500 mr-1 mb-px animate-pulse" />
						)}
						{taskLine}
					</p>
				)}

				{/* Row 3: context bar — always shown when session exists */}
				{latestSid && (
					<div className="mt-1.5 flex items-center gap-1.5">
						<div className="flex-1 h-[3px] rounded-full bg-foreground/8 overflow-hidden">
							<div
								className={cn(
									"h-full rounded-full transition-all duration-700",
									isCompacting
										? "bg-yellow-500 animate-pulse"
										: contextPct >= 90
											? "bg-red-500"
											: contextPct >= 75
												? "bg-orange-400"
												: contextPct >= 50
													? "bg-primary/60"
													: "bg-primary/30",
								)}
								style={{ width: `${Math.max(contextPct, 2)}%` }}
							/>
						</div>
						<span
							className={cn(
								"text-[10px] tabular-nums shrink-0 w-7 text-right",
								isCompacting
									? "text-yellow-500 font-medium"
									: contextPct >= 90
										? "text-red-500 font-medium"
										: contextPct >= 75
											? "text-orange-400"
											: "text-muted-foreground/60",
							)}
						>
							{isCompacting
								? "压缩"
								: contextPct > 0
									? `${Math.round(contextPct)}%`
									: "—"}
						</span>
					</div>
				)}
			</div>
		</div>
	);
});

// ---------------------------------------------------------------------------
// Main List
// ---------------------------------------------------------------------------

export default function AgentSessionList() {
	const snap = useSnapshot(agentModel.state);
	const sdkSessions = snap.sdkSessions;
	const currentSessionId = snap.currentSessionId;
	const personaSnap = useSnapshot(personaModel.state);

	const navigate = useNavigate();
	const [search, setSearch] = useState("");
	const [createOpen, setCreateOpen] = useState(false);
	const [createDefaults, setCreateDefaults] = useState<
		| {
				personaId: string;
				sessionName: string;
				systemPrompt: string;
				avatar: AvatarFullConfig;
		  }
		| undefined
	>(undefined);
	const q = search.trim().toLowerCase();

	// Group sessions by persona
	const sessionsByPersona = useMemo(() => {
		const map: Record<string, AgentProcessInfo[]> = {};
		for (const s of sdkSessions) {
			const pid = personaSnap.sessionPersonas[s.session_id] || "unknown";
			if (!map[pid]) map[pid] = [];
			map[pid].push(s as AgentProcessInfo);
		}
		return map;
	}, [sdkSessions, personaSnap.sessionPersonas]);

	// Which persona is currently active
	const currentPersonaId = currentSessionId
		? personaSnap.sessionPersonas[currentSessionId] || null
		: null;

	// Filter agents by search — includes builtin + server personas
	const allPersonas = useMemo(
		() => personaModel.getAllPersonas(),
		[personaSnap.serverPersonas, personaSnap.customPersonas],
	);

	const filteredPersonas = useMemo(() => {
		if (!q) return allPersonas;
		return allPersonas.filter(
			(p) =>
				p.name.toLowerCase().includes(q) ||
				p.description.toLowerCase().includes(q) ||
				p.id.toLowerCase().includes(q),
		);
	}, [q, allPersonas]);

	// Compute total unread per persona (sum of all session unreads)
	const unreadByPersona = useMemo(() => {
		const map: Record<string, number> = {};
		for (const s of sdkSessions) {
			const pid = personaSnap.sessionPersonas[s.session_id] || "unknown";
			const count = snap.unreadCounts[s.session_id] || 0;
			if (count > 0) {
				map[pid] = (map[pid] || 0) + count;
			}
		}
		return map;
	}, [sdkSessions, personaSnap.sessionPersonas, snap.unreadCounts]);

	// Last message text + timestamp per session (for task line and time display)
	// Use backward iteration instead of [...arr].reverse() to avoid array copy
	const { lastMessageBySession, lastMessageTimeBySession } = useMemo(() => {
		const text: Record<string, string> = {};
		const time: Record<string, number> = {};
		for (const [sid, msgs] of Object.entries(snap.messages)) {
			const arr = msgs as import("@/typings/agent").AgentChatMessage[];
			for (let i = arr.length - 1; i >= 0; i--) {
				const msg = arr[i];
				if (!time[sid] && msg.timestamp) {
					time[sid] = msg.timestamp;
				}
				if (!text[sid] && msg.role === "assistant" && msg.content) {
					text[sid] = msg.content.slice(0, 60);
				}
				if (text[sid] && time[sid]) break;
			}
		}
		return { lastMessageBySession: text, lastMessageTimeBySession: time };
	}, [snap.messages]);

	// New session created → refresh list, connect WS, select it
	const handleCreated = useCallback(async (sid: string) => {
		const sessions = await agentApi.listSessions();
		if (Array.isArray(sessions))
			agentModel.setSdkSessions(sessions as AgentProcessInfo[]);
		connectSession(sid);
		agentModel.setCurrentSession(sid);
		agentModel.clearUnread(sid);
	}, []);

	// Select agent → select its latest session, or silently create one
	const handleSelectAgent = useCallback(
		async (personaId: string) => {
			const sessions = [...(sessionsByPersona[personaId] || [])]
				.filter((s) => !s.archived)
				.sort((a, b) => b.created_at - a.created_at);

			if (sessions.length > 0) {
				const sid = sessions[0].session_id;
				agentModel.setCurrentSession(sid);
				agentModel.clearUnread(sid);
				return;
			}

			// No active session — silently create one with persona defaults
			const persona = allPersonas.find((p) => p.id === personaId);
			const defaults = settingsModel.state.agentDefaults;
			const modelId =
				persona?.model || settingsModel.state.defaultModel || undefined;
			const providerName = persona?.model?.includes("/")
				? persona.model.split("/")[0]
				: settingsModel.state.defaultProvider;
			const apiKey = modelId
				? resolveApiKey(providerName, modelId.split("/").pop() || modelId)
				: "";
			const baseUrl = modelId
				? resolveBaseUrl(providerName, modelId.split("/").pop() || modelId)
				: "";
			try {
				const result = await agentApi.createSession({
					persona_id: personaId,
					model: modelId,
					permission_mode: persona?.permissionMode || "default",
					cwd: defaults.defaultCwd || undefined,
					system_prompt: persona?.systemPrompt || undefined,
					api_key: apiKey || undefined,
					base_url: baseUrl || undefined,
				});
				if (result?.session_id) {
					personaModel.setSessionPersona(result.session_id, personaId);
					// Clear any stale messages for this new session
					agentModel.setMessages(result.session_id, []);
					const updated = await agentApi.listSessions();
					if (Array.isArray(updated))
						agentModel.setSdkSessions(updated as AgentProcessInfo[]);
					connectSession(result.session_id);
					agentModel.setCurrentSession(result.session_id);
					agentModel.clearUnread(result.session_id);
				}
			} catch (e) {
				console.warn("Failed to create session for persona", personaId, e);
				toast.error("会话创建失败", {
					description: "后端服务不可用，请检查 SafeClaw 是否正在运行",
				});
				// Fall back to create dialog so user can retry manually
				setCreateDefaults(
					persona
						? {
								personaId: persona.id,
								sessionName: persona.name,
								systemPrompt: persona.systemPrompt || "",
								avatar: genConfig(persona.avatar),
							}
						: undefined,
				);
				setCreateOpen(true);
			}
		},
		[sessionsByPersona, allPersonas],
	);

	return (
		<div className="flex flex-col h-full overflow-hidden border-r">
			{/* Header */}
			<div className="flex items-center justify-between px-3 py-3 border-b">
				<h2 className="text-sm font-semibold truncate">智能体</h2>
				<Button
					variant="ghost"
					size="icon"
					className="size-7"
					aria-label="自定义新建会话"
					onClick={() => setCreateOpen(true)}
				>
					<Plus className="size-4" />
				</Button>
			</div>

			{/* Search */}
			<div className="px-3 py-2 border-b">
				<div className="relative">
					<Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
					<Input
						placeholder="搜索智能体..."
						value={search}
						onChange={(e) => setSearch(e.target.value)}
						className="pl-8 h-9"
					/>
				</div>
			</div>

			<ScrollArea className="flex-1 w-full">
				<div role="listbox" aria-label="智能体列表" className="w-full">
					{filteredPersonas.map((persona) => (
						<AgentItem
							key={persona.id}
							persona={persona}
							sessions={sessionsByPersona[persona.id] || []}
							sessionStates={snap.sessions}
							isActive={currentPersonaId === persona.id}
							unreadCount={unreadByPersona[persona.id] || 0}
							sessionStatus={snap.sessionStatus}
							toolProgress={snap.activeToolProgress}
							lastMessage={lastMessageBySession}
							lastMessageTime={lastMessageTimeBySession}
							onSelect={() => handleSelectAgent(persona.id)}
							onAvatarClick={() => navigate(`/agent/${persona.id}`)}
						/>
					))}

					{filteredPersonas.length === 0 && (
						<div className="px-3 py-8 text-center text-sm text-muted-foreground">
							{q ? "未找到匹配的智能体" : "选择智能体开始对话"}
						</div>
					)}
				</div>
			</ScrollArea>

			<CreateSessionDialog
				open={createOpen}
				onOpenChange={(open) => {
					setCreateOpen(open);
					if (!open) setCreateDefaults(undefined);
				}}
				onCreated={handleCreated}
				defaults={createDefaults}
			/>
		</div>
	);
}
