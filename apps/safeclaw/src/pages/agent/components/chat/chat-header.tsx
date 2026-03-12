import {
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
	Popover,
	PopoverContent,
	PopoverTrigger,
} from "@/components/ui/popover";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { connectSession, disconnectSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import { timeAgo } from "@/lib/time";
import PickWorkdirDialog from "../pick-workdir-dialog";
import {
	Archive,
	ChevronDown,
	Circle,
	FolderOpen,
	Loader2,
	MessageSquare,
	MoreHorizontal,
	Pencil,
	Plus,
	Search,
	Trash2,
	ExternalLink,
} from "lucide-react";
import { cn } from "@/lib/utils";
import { useMemo, useState } from "react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import { useSnapshot } from "valtio";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

export function ChatHeader({
	sessionId,
	searchQuery,
	onSearchChange,
	onSessionChange,
	viewMode,
	onViewModeChange,
	cwd,
}: {
	sessionId: string;
	searchQuery?: string;
	onSearchChange?: (q: string) => void;
	onSessionChange?: (id: string) => void;
	viewMode?: "chat" | "workspace";
	onViewModeChange?: (mode: "chat" | "workspace") => void;
	cwd?: string;
}) {
	const { sdkSessions, sessionNames } = useSnapshot(agentModel.state);
	const personaSnap = useSnapshot(personaModel.state);
	const persona = personaModel.getSessionPersona(sessionId);
	const personaId = personaSnap.sessionPersonas[sessionId];
	const avatarConfig = useMemo(() => persona.avatar, [persona.avatar]);
	const [actionLoading, setActionLoading] = useState<"delete" | null>(null);
	const [pickWorkdirOpen, setPickWorkdirOpen] = useState(false);
	const [searchOpen, setSearchOpen] = useState(false);
	const [editingName, setEditingName] = useState(false);
	const [nameInput, setNameInput] = useState("");

	const currentSession = sdkSessions.find((s) => s.session_id === sessionId);
	const isExited = currentSession?.state === "exited";

	// All sessions belonging to the same persona, newest first (including archived/exited)
	const personaSessions = useMemo(() => {
		return [...sdkSessions]
			.filter((s) => personaSnap.sessionPersonas[s.session_id] === personaId)
			.sort((a, b) => b.created_at - a.created_at);
	}, [sdkSessions, personaSnap.sessionPersonas, personaId]);

	const [sessionPickerOpen, setSessionPickerOpen] = useState(false);

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
			// Switch to another session of the same persona, or clear selection
			const remaining = agentModel.state.sdkSessions.filter(
				(s) =>
					!s.archived &&
					s.session_id !== sessionId &&
					personaModel.state.sessionPersonas[s.session_id] === personaId,
			);
			if (remaining.length > 0) {
				agentModel.setCurrentSession(remaining[0].session_id);
				onSessionChange?.(remaining[0].session_id);
			} else {
				agentModel.setCurrentSession(null);
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

	const handleOpenFolder = async () => {
		if (!cwd) {
			toast.error("当前会话没有工作目录");
			return;
		}
		try {
			await invoke("open_folder", { path: cwd });
		} catch (err) {
			console.error("Failed to open folder:", err);
			const errorMsg = err instanceof Error ? err.message : String(err);
			toast.error(`打开文件夹失败: ${errorMsg}`);
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
							<Popover
								open={sessionPickerOpen}
								onOpenChange={setSessionPickerOpen}
							>
								<PopoverTrigger asChild>
									<button
										type="button"
										className="flex items-center gap-1 min-w-0 max-w-[200px] text-sm font-medium hover:text-foreground/80 transition-colors"
									>
										<span className="truncate">
											{sessionNames[sessionId] ||
												currentSession?.name ||
												`会话 ${sessionId.slice(0, 6)}`}
										</span>
										<ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
									</button>
								</PopoverTrigger>
								<PopoverContent align="start" className="w-72 p-0">
									<div className="px-3 py-2 border-b">
										<p className="text-xs font-medium text-muted-foreground">
											历史会话
										</p>
									</div>
									<ScrollArea className="max-h-72">
										<div className="p-1">
											{personaSessions.length === 0 && (
												<p className="text-xs text-muted-foreground text-center py-4">
													暂无会话记录
												</p>
											)}
											{personaSessions.map((s) => {
												const name =
													sessionNames[s.session_id] ||
													s.name ||
													`会话 ${s.session_id.slice(0, 6)}`;
												const isActive = s.session_id === sessionId;
												const isSessionExited = s.state === "exited";
												const isArchived = !!s.archived;
												const ts =
													s.created_at > 0 && s.created_at < 1e12
														? s.created_at * 1000
														: s.created_at;
												return (
													<button
														key={s.session_id}
														type="button"
														className={cn(
															"w-full flex items-start gap-2.5 px-2.5 py-2 rounded-md text-left transition-colors",
															isActive
																? "bg-primary/10 text-primary"
																: "hover:bg-accent/60 text-foreground",
														)}
														onClick={() => {
															if (!isActive) {
																connectSession(s.session_id);
																agentModel.setCurrentSession(s.session_id);
																agentModel.clearUnread(s.session_id);
																onSessionChange?.(s.session_id);
															}
															setSessionPickerOpen(false);
														}}
													>
														<div className="flex-1 min-w-0">
															<div className="flex items-center gap-1.5">
																<span className="truncate text-xs font-medium">
																	{name}
																</span>
																{isArchived && (
																	<Archive className="size-2.5 shrink-0 text-muted-foreground/60" />
																)}
															</div>
															<div className="flex items-center gap-1.5 mt-0.5">
																<Circle
																	className={cn(
																		"size-1.5 shrink-0 fill-current",
																		isSessionExited
																			? "text-muted-foreground/40"
																			: "text-green-500",
																	)}
																/>
																<span className="text-[10px] text-muted-foreground/70">
																	{isSessionExited ? "已退出" : "就绪"} ·{" "}
																	{timeAgo(ts)}
																</span>
															</div>
														</div>
													</button>
												);
											})}
										</div>
									</ScrollArea>
								</PopoverContent>
							</Popover>
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
										: "text-muted-foreground hover:text-foreground hover:bg-background/50",
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
										: "text-muted-foreground hover:text-foreground hover:bg-background/50",
								)}
							>
								<FolderOpen className="size-4" />
								工作区
							</button>
						</div>
					)}
					<div className="flex items-center gap-1 shrink-0">
						{cwd && (
							<button
								type="button"
								className="flex items-center justify-center size-7 rounded-md text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors"
								aria-label={`打开工作文件夹: ${cwd}`}
								title={`打开工作文件夹\n${cwd}`}
								onClick={handleOpenFolder}
							>
								<ExternalLink className="size-4" />
							</button>
						)}
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
									onClick={() => setPickWorkdirOpen(true)}
									disabled={actionLoading !== null}
								>
									<Plus className="size-3.5" />
									新建会话
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
			<PickWorkdirDialog
				open={pickWorkdirOpen}
				onOpenChange={setPickWorkdirOpen}
				personaId={personaId ?? null}
				onCreated={(sid) => onSessionChange?.(sid)}
			/>
		</div>
	);
}
