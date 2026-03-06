import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Input } from "@/components/ui/input";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { connectSession, disconnectSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import {
	FolderOpen,
	GitFork,
	Loader2,
	MessageSquare,
	MoreHorizontal,
	Pencil,
	Plus,
	RefreshCw,
	Search,
	Trash2,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useMemo, useState } from "react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

export function ChatHeader({
	sessionId,
	searchQuery,
	onSearchChange,
	onSessionChange,
	viewMode,
	onViewModeChange,
}: {
	sessionId: string;
	searchQuery?: string;
	onSearchChange?: (q: string) => void;
	onSessionChange?: (id: string) => void;
	viewMode?: "chat" | "workspace";
	onViewModeChange?: (mode: "chat" | "workspace") => void;
}) {
	const { sdkSessions, sessionNames } = useSnapshot(agentModel.state);
	const personaSnap = useSnapshot(personaModel.state);
	const persona = personaModel.getSessionPersona(sessionId);
	const personaId = personaSnap.sessionPersonas[sessionId];
	const avatarConfig = useMemo(
		() => genConfig(persona.avatar),
		[persona.avatar],
	);
	const [actionLoading, setActionLoading] = useState<
		"relaunch" | "delete" | "fork" | "new" | null
	>(null);
	const [searchOpen, setSearchOpen] = useState(false);
	const [editingName, setEditingName] = useState(false);
	const [nameInput, setNameInput] = useState("");

	const currentSession = sdkSessions.find((s) => s.session_id === sessionId);
	const isExited = currentSession?.state === "exited";

	const siblingsSessions = useMemo(() => {
		return [...sdkSessions]
			.filter((s) => !s.archived && s.state !== "exited")
			.sort((a, b) => b.created_at - a.created_at);
	}, [sdkSessions]);

	const handleRelaunch = async () => {
		setActionLoading("relaunch");
		try {
			const result = await agentApi.relaunchSession(sessionId);
			if (result?.session_id) {
				const sessions = await agentApi.listSessions();
				if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
				disconnectSession(sessionId);
				connectSession(result.session_id);
				agentModel.setCurrentSession(result.session_id);
				personaModel.setSessionPersona(
					result.session_id,
					personaId || "general",
				);
			}
		} finally {
			setActionLoading(null);
		}
	};

	const handleFork = async () => {
		setActionLoading("fork");
		try {
			const p = personaId
				? personaModel.getAllPersonas().find((x) => x.id === personaId)
				: null;
			const currentSess = agentModel.state.sessions[sessionId];
			const result = await agentApi.createSession({
				persona_id: personaId || undefined,
				system_prompt: p?.systemPrompt || undefined,
				model: currentSess?.model || undefined,
				permission_mode: currentSess?.permission_mode || undefined,
				cwd: currentSess?.cwd || undefined,
			});
			if (result?.session_id) {
				const sessions = await agentApi.listSessions();
				if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
				connectSession(result.session_id);
				agentModel.setCurrentSession(result.session_id);
				if (personaId)
					personaModel.setSessionPersona(result.session_id, personaId);
				toast.success("已创建分支会话");
			}
		} finally {
			setActionLoading(null);
		}
	};

	const handleNew = async () => {
		setActionLoading("new");
		try {
			const p = personaId
				? personaModel.getAllPersonas().find((x) => x.id === personaId)
				: null;
			const currentSess = agentModel.state.sessions[sessionId];
			const result = await agentApi.createSession({
				persona_id: personaId || undefined,
				system_prompt: p?.systemPrompt || undefined,
				model: currentSess?.model || undefined,
				permission_mode: currentSess?.permission_mode || undefined,
				cwd: currentSess?.cwd || undefined,
			});
			if (result?.session_id) {
				const sessions = await agentApi.listSessions();
				if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
				connectSession(result.session_id);
				agentModel.setCurrentSession(result.session_id);
				if (personaId)
					personaModel.setSessionPersona(result.session_id, personaId);
			}
		} finally {
			setActionLoading(null);
		}
	};

	const handleRename = () => {
		const currentSession = sdkSessions.find((s) => s.session_id === sessionId);
		const displayedName =
			sessionNames[sessionId] ||
			currentSession?.name ||
			`会话 ${sessionId.slice(0, 6)}`;
		setNameInput(displayedName);
		setEditingName(true);
	};

	const commitRename = () => {
		const name = nameInput.trim();
		if (name) {
			agentModel.setSessionName(sessionId, name);
			agentApi.updateSession(sessionId, { name }).catch(() => {});
		}
		setEditingName(false);
	};

	const handleDelete = async () => {
		setActionLoading("delete");
		try {
			await agentApi.deleteSession(sessionId);
			disconnectSession(sessionId);
			agentModel.removeSession(sessionId);
			const sessions = await agentApi.listSessions();
			if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
			const remaining = agentModel.state.sdkSessions.filter(
				(s) => !s.archived && s.session_id !== sessionId,
			);
			if (remaining.length > 0) {
				agentModel.setCurrentSession(remaining[0].session_id);
				onSessionChange?.(remaining[0].session_id);
			} else {
				const p = personaId
					? personaModel.getAllPersonas().find((x) => x.id === personaId)
					: null;
				const result = await agentApi.createSession({
					persona_id: personaId || undefined,
					system_prompt: p?.systemPrompt || undefined,
				});
				if (result?.session_id) {
					const newSessions = await agentApi.listSessions();
					if (Array.isArray(newSessions))
						agentModel.setSdkSessions(newSessions);
					connectSession(result.session_id);
					agentModel.setCurrentSession(result.session_id);
					onSessionChange?.(result.session_id);
					if (personaId)
						personaModel.setSessionPersona(result.session_id, personaId);
				}
			}
		} finally {
			setActionLoading(null);
		}
	};

	const toggleSearch = () => {
		if (searchOpen) {
			onSearchChange?.("");
			setSearchOpen(false);
		} else {
			setSearchOpen(true);
		}
	};

	return (
		<div className="border-b bg-background">
			{searchOpen ? (
				/* WeChat-style: search replaces the entire header row */
				<div className="flex items-center gap-2 px-3 py-2">
					<div className="relative flex-1">
						<Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
						<Input
							className="pl-8 h-8 text-sm bg-muted/50 border-transparent focus-visible:border-input focus-visible:bg-background rounded-full"
							placeholder="搜索聊天记录"
							value={searchQuery ?? ""}
							onChange={(e) => onSearchChange?.(e.target.value)}
							autoFocus
						/>
					</div>
					<button
						type="button"
						className="text-sm text-muted-foreground hover:text-foreground transition-colors shrink-0"
						onClick={toggleSearch}
					>
						取消
					</button>
				</div>
			) : (
				<div className="flex items-center px-3 py-2 gap-3">
					<div className="flex items-center gap-2 min-w-0 flex-1">
						<NiceAvatar className="size-7 shrink-0" {...avatarConfig} />
						{editingName ? (
							<input
								autoFocus
								type="text"
								value={nameInput}
								onChange={(e) => setNameInput(e.target.value)}
								onBlur={commitRename}
								onKeyDown={(e) => {
									if (e.key === "Enter") commitRename();
									else if (e.key === "Escape") setEditingName(false);
								}}
								className="flex-1 min-w-0 text-sm bg-muted rounded-lg px-2 py-0.5 border-0 outline-none focus:ring-1 focus:ring-ring"
							/>
						) : (
							<select
								value={sessionId}
								onChange={(e) => {
									agentModel.setCurrentSession(e.target.value);
									onSessionChange?.(e.target.value);
								}}
								className="flex-1 min-w-0 max-w-[200px] text-sm font-medium bg-transparent border-0 outline-none cursor-pointer text-foreground"
							>
								{siblingsSessions.map((s) => {
									const name =
										sessionNames[s.session_id] ||
										s.name ||
										`会话 ${s.session_id.slice(0, 6)}`;
									return (
										<option key={s.session_id} value={s.session_id}>
											{name}
										</option>
									);
								})}
								{!siblingsSessions.find((s) => s.session_id === sessionId) && (
									<option value={sessionId}>
										{sessionNames[sessionId] || `会话 ${sessionId.slice(0, 6)}`}
									</option>
								)}
							</select>
						)}
						{!editingName && (
							<button
								type="button"
								title="重命名会话"
								onClick={handleRename}
								className="shrink-0 flex items-center justify-center size-6 rounded text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
							>
								<Pencil className="size-3" />
							</button>
						)}
						{isExited && (
							<span className="text-[10px] px-1.5 py-0.5 rounded-full bg-muted text-muted-foreground shrink-0">
								已退出
							</span>
						)}
					</div>


			{/* Mode Switcher - Center */}
			{viewMode && onViewModeChange && (
				<div className="flex items-center gap-1 p-1 rounded-lg bg-muted/50 shrink-0">
					<button
						type="button"
						onClick={() => onViewModeChange("chat")}
						className={cn(
							"flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-all",
							viewMode === "chat"
								? "bg-background text-foreground shadow-sm"
								: "text-muted-foreground hover:text-foreground hover:bg-background/50"
						)}
					>
						<MessageSquare className="size-4" />
						会话
					</button>
					<button
						type="button"
						onClick={() => onViewModeChange("workspace")}
						className={cn(
							"flex items-center gap-1.5 px-3 py-1.5 rounded-md text-sm font-medium transition-all",
							viewMode === "workspace"
								? "bg-background text-foreground shadow-sm"
								: "text-muted-foreground hover:text-foreground hover:bg-background/50"
						)}
					>
						<FolderOpen className="size-4" />
						工作区
					</button>
				</div>
			)}
					<div className="flex items-center gap-1 shrink-0">
						<button
							type="button"
							className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors disabled:opacity-40"
							aria-label="新建会话"
							title="新建会话"
							onClick={handleNew}
							disabled={actionLoading !== null}
						>
							{actionLoading === "new" ? (
								<Loader2 className="size-4 animate-spin" />
							) : (
								<Plus className="size-4" />
							)}
						</button>
						<button
							type="button"
							className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
							aria-label="搜索消息"
							onClick={toggleSearch}
						>
							<Search className="size-4" />
						</button>
						<DropdownMenu>
							<DropdownMenuTrigger asChild>
								<button
									type="button"
									className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
									aria-label="会话操作"
								>
									{actionLoading ? (
										<Loader2 className="size-4 animate-spin" />
									) : (
										<MoreHorizontal className="size-4" />
									)}
								</button>
							</DropdownMenuTrigger>
							<DropdownMenuContent align="end" className="w-40">
								<DropdownMenuItem
									className="gap-2 text-xs cursor-pointer"
									onClick={handleRelaunch}
									disabled={actionLoading !== null}
								>
									<RefreshCw className="size-3.5" />
									重启会话
								</DropdownMenuItem>
								<DropdownMenuItem
									className="gap-2 text-xs cursor-pointer"
									onClick={handleFork}
									disabled={actionLoading !== null}
								>
									<GitFork className="size-3.5" />
									创建分支
								</DropdownMenuItem>
								<DropdownMenuSeparator />
								<DropdownMenuItem
									className="gap-2 text-xs text-destructive focus:text-destructive cursor-pointer"
									onClick={handleDelete}
									disabled={actionLoading !== null}
								>
									<Trash2 className="size-3.5" />
									删除会话
								</DropdownMenuItem>
							</DropdownMenuContent>
						</DropdownMenu>
					</div>
				</div>
			)}
		</div>
	);
}
