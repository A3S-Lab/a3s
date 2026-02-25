import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Input } from "@/components/ui/input";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { connectSession, disconnectSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import {
	Download,
	GitFork,
	Loader2,
	MoreHorizontal,
	RefreshCw,
	Search,
	Settings,
	Trash2,
} from "lucide-react";
import { useMemo, useState, useCallback } from "react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
import { SessionConfigDrawer } from "./session-config-drawer";

function downloadBlob(blob: Blob, filename: string) {
	const url = URL.createObjectURL(blob);
	const a = document.createElement("a");
	a.href = url;
	a.download = filename;
	a.click();
	URL.revokeObjectURL(url);
}

export function ChatHeader({
	sessionId,
	searchQuery,
	onSearchChange,
}: {
	sessionId: string;
	searchQuery?: string;
	onSearchChange?: (q: string) => void;
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
		"relaunch" | "delete" | "fork" | null
	>(null);
	const [configOpen, setConfigOpen] = useState(false);
	const [searchOpen, setSearchOpen] = useState(false);

	const currentSession = sdkSessions.find((s) => s.session_id === sessionId);
	const isExited = currentSession?.state === "exited";

	const siblingsSessions = useMemo(() => {
		return [...sdkSessions]
			.filter(
				(s) =>
					!s.archived &&
					personaSnap.sessionPersonas[s.session_id] === personaId,
			)
			.sort((a, b) => b.created_at - a.created_at);
	}, [sdkSessions, personaSnap.sessionPersonas, personaId]);

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
			agentModel.setCurrentSession(remaining[0]?.session_id ?? null);
		} finally {
			setActionLoading(null);
		}
	};

	const handleExport = useCallback(
		(format: "json" | "md") => {
			const messages = agentModel.state.messages[sessionId] ?? [];
			const name = sessionNames[sessionId] || sessionId.slice(0, 8);
			const date = new Date().toISOString().slice(0, 10);

			if (format === "md") {
				const lines = [
					`# ${name}\n`,
					`导出时间: ${new Date().toLocaleString()}\n`,
				];
				for (const msg of messages) {
					if (msg.role === "user") {
						lines.push(`\n## 用户\n\n${msg.content}\n`);
					} else if (msg.role === "assistant") {
						// Use contentBlocks if available, otherwise fall back to content
						const text = msg.contentBlocks
							? msg.contentBlocks
									.filter((b) => b.type === "text")
									.map((b) => (b as { type: "text"; text: string }).text)
									.join("\n")
							: msg.content;
						lines.push(`\n## 助手\n\n${text}\n`);
					} else if (msg.role === "system") {
						lines.push(`\n> **系统**: ${msg.content}\n`);
					}
				}
				const blob = new Blob([lines.join("")], { type: "text/markdown" });
				downloadBlob(blob, `session-${name}-${date}.md`);
			} else {
				const blob = new Blob([JSON.stringify(messages, null, 2)], {
					type: "application/json",
				});
				downloadBlob(blob, `session-${name}-${date}.json`);
			}
			toast.success("会话已导出");
		},
		[sessionId, sessionNames],
	);

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
				<div className="flex items-center justify-between px-3 py-2 gap-3">
					<div className="flex items-center gap-2 min-w-0">
						<NiceAvatar className="size-7 shrink-0" {...avatarConfig} />
						{siblingsSessions.length <= 1 ? (
							<span className="text-sm font-medium truncate">{persona.name}</span>
						) : (
							<Select
								value={sessionId}
								onValueChange={(val) => agentModel.setCurrentSession(val)}
							>
								<SelectTrigger className="h-7 text-xs font-medium border-none shadow-none px-2 gap-1 min-w-[120px] max-w-[200px]">
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{siblingsSessions.map((s) => {
										const name =
											sessionNames[s.session_id] ||
											s.name ||
											`会话 ${s.session_id.slice(0, 6)}`;
										return (
											<SelectItem
												key={s.session_id}
												value={s.session_id}
												className="text-xs"
											>
												{name}
											</SelectItem>
										);
									})}
								</SelectContent>
							</Select>
						)}
						{isExited && (
							<span className="text-[10px] px-1.5 py-0.5 rounded-full bg-muted text-muted-foreground shrink-0">
								已退出
							</span>
						)}
					</div>

					<div className="flex items-center gap-1 shrink-0">
						<button
							type="button"
							className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
							aria-label="搜索消息"
							onClick={toggleSearch}
						>
							<Search className="size-4" />
						</button>
						<button
							type="button"
							className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
							aria-label="会话配置"
							onClick={() => setConfigOpen(true)}
						>
							<Settings className="size-4" />
						</button>
						<SessionConfigDrawer
							sessionId={sessionId}
							open={configOpen}
							onClose={() => setConfigOpen(false)}
						/>
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
									className="gap-2 text-xs cursor-pointer"
									onClick={() => handleExport("md")}
								>
									<Download className="size-3.5" />
									导出 Markdown
								</DropdownMenuItem>
								<DropdownMenuItem
									className="gap-2 text-xs cursor-pointer"
									onClick={() => handleExport("json")}
								>
									<Download className="size-3.5" />
									导出 JSON
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
