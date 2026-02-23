import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import {
	Tooltip,
	TooltipContent,
	TooltipProvider,
	TooltipTrigger,
} from "@/components/ui/tooltip";
import { ScrollArea } from "@/components/ui/scroll-area";
import { agentApi } from "@/lib/agent-api";
import { cn } from "@/lib/utils";
import personaModel from "@/models/persona.model";
import settingsModel, {
	resolveApiKey,
	resolveBaseUrl,
	getAllModels,
} from "@/models/settings.model";
import {
	BookmarkPlus,
	ChevronDown,
	FolderOpen,
	Key,
	Link,
	Bot,
	Loader2,
	Search,
	Shield,
	Shuffle,
	Sparkles,
	Store,
	Trash2,
	User,
	UserPlus,
} from "lucide-react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import type { AvatarFullConfig } from "react-nice-avatar";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

// =============================================================================
// Constants
// =============================================================================

/** All available filter tags — derived from builtin persona categories */
const ALL_TAGS = [
	"工程",
	"量化",
	"金融",
	"产品",
	"数据",
	"自定义",
] as const;

// =============================================================================
// Types
// =============================================================================

interface CreateSessionDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	onCreated: (sessionId: string) => void;
	defaults?: {
		personaId: string;
		sessionName: string;
		systemPrompt: string;
		avatar: AvatarFullConfig;
		model?: string;
		permissionMode?: string;
	};
}

// =============================================================================
// Dialog
// =============================================================================

export default function CreateSessionDialog({
	open,
	onOpenChange,
	onCreated,
	defaults,
}: CreateSessionDialogProps) {
	const personaSnap = useSnapshot(personaModel.state);
	const settingsSnap = useSnapshot(settingsModel.state);

	// Tab state
	const [tab, setTab] = useState<string>(defaults ? "custom" : "market");

	// ── Market tab state ──
	const [marketSearch, setMarketSearch] = useState("");
	const [activeTags, setActiveTags] = useState<Set<string>>(new Set());
	const [selectedPersonaId, setSelectedPersonaId] = useState<string | null>(
		null,
	);
	const searchTimerRef = useRef<ReturnType<typeof setTimeout>>();
	const scrollRef = useRef<HTMLDivElement>(null);

	// ── Custom tab state ──
	const [avatarConfig, setAvatarConfig] = useState<AvatarFullConfig>(
		defaults?.avatar || genConfig(),
	);
	const [sessionName, setSessionName] = useState(defaults?.sessionName || "");
	const [systemPrompt, setSystemPrompt] = useState(
		defaults?.systemPrompt || "",
	);
	const [model, setModel] = useState(
		defaults?.model || settingsModel.state.defaultModel,
	);
	const [permissionMode, setPermissionMode] = useState(
		defaults?.permissionMode || "default",
	);
	const [cwd, setCwd] = useState(settingsModel.state.agentDefaults.defaultCwd);
	const [baseUrl, setBaseUrl] = useState(
		resolveBaseUrl(
			settingsModel.state.defaultProvider,
			settingsModel.state.defaultModel,
		),
	);
	const [apiKey, setApiKey] = useState(
		resolveApiKey(
			settingsModel.state.defaultProvider,
			settingsModel.state.defaultModel,
		),
	);
	const [advancedOpen, setAdvancedOpen] = useState(false);
	const [saveAsPersona, setSaveAsPersona] = useState(false);
	const [personaDescription, setPersonaDescription] = useState("");

	// ── Shared state ──
	const [loading, setLoading] = useState(false);
	const [error, setError] = useState<string | null>(null);

	// Backend models
	const [backends, setBackends] = useState<{ id: string; name: string }[]>([]);
	useEffect(() => {
		if (open && backends.length === 0) {
			agentApi
				.listBackends()
				.then((r) => {
					if (Array.isArray(r)) setBackends(r);
				})
				.catch(() => {});
		}
	}, [open]);

	const currentAvatarConfig = useMemo(
		() => genConfig(avatarConfig),
		[avatarConfig],
	);

	// Sync defaults
	const [prevDefaults, setPrevDefaults] = useState(defaults);
	if (defaults !== prevDefaults) {
		setPrevDefaults(defaults);
		if (defaults) {
			setTab("custom");
			setAvatarConfig(defaults.avatar);
			setSessionName(defaults.sessionName);
			setSystemPrompt(defaults.systemPrompt);
			if (defaults.model) setModel(defaults.model);
			if (defaults.permissionMode) setPermissionMode(defaults.permissionMode);
		}
	}

	// ── Persona data ──
	const allPersonas = useMemo(
		() => personaModel.getAllPersonas(),
		[personaSnap.serverPersonas, personaSnap.customPersonas],
	);

	// Market data from backend (paginated)
	const marketItems = personaSnap.market.items as import("@/typings/persona").AgentPersona[];
	const marketTotal = personaSnap.market.total;
	const marketPage = personaSnap.market.page;
	const marketPageSize = personaSnap.market.pageSize;
	const marketLoading = personaSnap.market.loading;
	const hasMore = marketItems.length < marketTotal;

	// Fetch market personas when dialog opens or filters change
	useEffect(() => {
		if (open && tab === "market") {
			personaModel.fetchMarketPersonas({
				page: 1,
				search: marketSearch,
				tags: Array.from(activeTags),
				reset: true,
			});
		}
	}, [open, tab]);

	// Debounced search
	const handleMarketSearch = useCallback(
		(value: string) => {
			setMarketSearch(value);
			if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
			searchTimerRef.current = setTimeout(() => {
				personaModel.fetchMarketPersonas({
					page: 1,
					search: value,
					tags: Array.from(activeTags),
					reset: true,
				});
			}, 300);
		},
		[activeTags],
	);

	// Load next page
	const handleLoadMore = useCallback(() => {
		if (marketLoading || !hasMore) return;
		personaModel.fetchMarketPersonas({
			page: marketPage + 1,
			search: marketSearch,
			tags: Array.from(activeTags),
		});
	}, [marketLoading, hasMore, marketPage, marketSearch, activeTags]);

	// Scroll-based pagination
	const handleMarketScroll = useCallback(
		(e: React.UIEvent<HTMLDivElement>) => {
			const el = e.currentTarget;
			if (el.scrollHeight - el.scrollTop - el.clientHeight < 100) {
				handleLoadMore();
			}
		},
		[handleLoadMore],
	);

	const availableModels = useMemo(
		() => getAllModels(),
		[settingsSnap.providers],
	);

	// ── Actions ──

	const toggleTag = useCallback((tag: string) => {
		setActiveTags((prev) => {
			const next = new Set(prev);
			if (next.has(tag)) next.delete(tag);
			else next.add(tag);
			personaModel.fetchMarketPersonas({
				page: 1,
				search: marketSearch,
				tags: Array.from(next),
				reset: true,
			});
			return next;
		});
	}, [marketSearch]);

	const handleRandomAvatar = useCallback(() => {
		setAvatarConfig(genConfig());
	}, []);

	const resetForm = useCallback(() => {
		setAvatarConfig(genConfig());
		setSessionName("");
		setSystemPrompt("");
		setModel(settingsModel.state.defaultModel);
		setPermissionMode("default");
		setCwd(settingsModel.state.agentDefaults.defaultCwd);
		setBaseUrl(
			resolveBaseUrl(
				settingsModel.state.defaultProvider,
				settingsModel.state.defaultModel,
			),
		);
		setApiKey(
			resolveApiKey(
				settingsModel.state.defaultProvider,
				settingsModel.state.defaultModel,
			),
		);
		setAdvancedOpen(false);
		setSaveAsPersona(false);
		setPersonaDescription("");
		setError(null);
		setSelectedPersonaId(null);
		setMarketSearch("");
		setActiveTags(new Set());
		setTab("market");
		personaModel.resetMarket();
	}, []);

	const handleCreate = async () => {
		let personaId = defaults?.personaId;
		let finalModel = model;
		let finalPermMode = permissionMode;
		let finalPrompt = systemPrompt;
		let finalName = sessionName;

		if (tab === "market" && selectedPersonaId) {
			const persona =
				allPersonas.find((p) => p.id === selectedPersonaId) ||
				personaModel.state.market.items.find((p) => p.id === selectedPersonaId);
			if (persona) {
				personaId = persona.id;
				finalModel = persona.defaultModel || model;
				finalPermMode = persona.defaultPermissionMode || permissionMode;
				finalPrompt = persona.systemPrompt || systemPrompt;
				finalName = finalName || persona.name;
			}
		}

		setLoading(true);
		setError(null);
		try {
			const result = await agentApi.createSession({
				model: finalModel,
				permission_mode: finalPermMode,
				cwd: cwd || undefined,
				persona_id: personaId || undefined,
				base_url: baseUrl || undefined,
				api_key: apiKey || undefined,
				system_prompt: finalPrompt || undefined,
			});
			if (result.error) {
				setError(result.error);
			} else {
				const sid = result.session_id;
				personaModel.setSessionPersona(sid, personaId || "general");
				if (finalName) {
					const { default: agentModel } = await import(
						"@/models/agent.model"
					);
					agentModel.setSessionName(sid, finalName);
					agentApi.updateSession(sid, { name: finalName }).catch(() => {});
				}

				if (tab === "custom" && saveAsPersona && sessionName.trim()) {
					const newPersonaId = `custom-${Date.now()}`;
					personaModel.addCustomPersona({
						id: newPersonaId,
						name: sessionName.trim(),
						description: personaDescription.trim() || sessionName.trim(),
						avatar: avatarConfig,
						systemPrompt: systemPrompt,
						defaultModel: model,
						defaultPermissionMode: permissionMode,
						tags: ["自定义"],
					});
					personaModel.setSessionPersona(sid, newPersonaId);
					toast.success("智能体已保存", { description: sessionName.trim() });
				}

				onCreated(sid);
				onOpenChange(false);
				resetForm();
			}
		} catch {
			setError("无法连接到网关");
		} finally {
			setLoading(false);
		}
	};

	const handleDeleteCustomPersona = (id: string, e: React.MouseEvent) => {
		e.stopPropagation();
		personaModel.deleteCustomPersona(id);
		if (selectedPersonaId === id) setSelectedPersonaId(null);
		toast.success("已删除自定义智能体");
	};

	const canCreate = tab === "market" ? !!selectedPersonaId : true;

	return (
		<Dialog open={open} onOpenChange={onOpenChange}>
			<DialogContent className="sm:max-w-[620px] max-h-[85vh] flex flex-col gap-0 p-0 overflow-hidden">
				<DialogHeader className="px-6 pt-5 pb-0 shrink-0">
					<DialogTitle className="flex items-center gap-2 text-base">
						<Bot className="size-5" />
						新建 Agent 会话
					</DialogTitle>
				</DialogHeader>

				<Tabs
					value={tab}
					onValueChange={setTab}
					className="flex-1 flex flex-col min-h-0"
				>
					<div className="px-6 pt-4 shrink-0">
						<TabsList className="w-full">
							<TabsTrigger value="market" className="flex-1 gap-1.5">
								<Store className="size-3.5" />
								智能体市场
							</TabsTrigger>
							<TabsTrigger value="custom" className="flex-1 gap-1.5">
								<UserPlus className="size-3.5" />
								自定义创建
							</TabsTrigger>
						</TabsList>
					</div>

					{/* ===== Tab: Market ===== */}
					<TabsContent
						value="market"
						className="flex-1 flex flex-col min-h-0 mt-0"
					>
						{/* Search + tag filters */}
						<div className="px-6 pt-3 pb-2 space-y-2.5 shrink-0">
							<div className="relative">
								<Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
								<Input
									placeholder="搜索智能体名称、描述或标签..."
									value={marketSearch}
									onChange={(e) => handleMarketSearch(e.target.value)}
									className="pl-8 h-9"
								/>
							</div>
							<div className="flex items-center gap-1.5 flex-wrap">
								{ALL_TAGS.map((tag) => (
									<button
										key={tag}
										type="button"
										className={cn(
											"rounded-full px-2.5 py-0.5 text-[11px] font-medium border transition-colors",
											activeTags.has(tag)
												? "bg-primary text-primary-foreground border-primary"
												: "bg-transparent text-muted-foreground border-border hover:border-foreground/20 hover:text-foreground",
										)}
										onClick={() => toggleTag(tag)}
									>
										{tag}
									</button>
								))}
								{activeTags.size > 0 && (
									<button
										type="button"
										className="text-[11px] text-muted-foreground/60 hover:text-foreground transition-colors ml-1"
										onClick={() => {
											setActiveTags(new Set());
											personaModel.fetchMarketPersonas({
												page: 1,
												search: marketSearch,
												tags: [],
												reset: true,
											});
										}}
									>
										清除筛选
									</button>
								)}
							</div>
						</div>

						{/* Persona grid — scrollable with infinite scroll */}
						<ScrollArea className="flex-1 min-h-0 px-6" onScrollCapture={handleMarketScroll}>
							<div className="grid grid-cols-2 gap-2 pb-3">
								{marketItems.map((persona) => {
									const cfg = genConfig(persona.avatar);
									const isCustom = !persona.builtin && !persona.undeletable;
									const isSelected = selectedPersonaId === persona.id;
									return (
										<button
											key={persona.id}
											type="button"
											className={cn(
												"relative flex items-start gap-2.5 rounded-xl border p-3 text-left transition-all hover:bg-accent/50",
												isSelected &&
													"border-primary bg-primary/5 ring-1 ring-primary/20",
											)}
											onClick={() => setSelectedPersonaId(persona.id)}
										>
											<NiceAvatar
												className="w-9 h-9 shrink-0"
												{...cfg}
											/>
											<div className="flex-1 min-w-0">
												<div className="flex items-center gap-1.5">
													<span className="text-xs font-semibold truncate">
														{persona.name}
													</span>
													{isCustom && (
														<Badge
															variant="secondary"
															className="text-[9px] px-1 py-0 h-3.5 shrink-0"
														>
															自定义
														</Badge>
													)}
												</div>
												<p className="text-[10px] text-muted-foreground line-clamp-2 mt-0.5 leading-snug">
													{persona.description}
												</p>
												{persona.tags && persona.tags.length > 0 && (
													<div className="flex items-center gap-1 mt-1.5">
														{persona.tags.map((t) => (
															<span
																key={t}
																className="text-[9px] rounded-full bg-muted px-1.5 py-px text-muted-foreground"
															>
																{t}
															</span>
														))}
													</div>
												)}
											</div>
											{isCustom && (
												<TooltipProvider delayDuration={300}>
													<Tooltip>
														<TooltipTrigger asChild>
															<span
																role="button"
																tabIndex={0}
																className="absolute top-2 right-2 p-0.5 rounded text-muted-foreground/40 hover:text-destructive hover:bg-destructive/10 transition-colors"
																onClick={(e) =>
																	handleDeleteCustomPersona(persona.id, e)
																}
																onKeyDown={(e) => {
																	if (e.key === "Enter")
																		handleDeleteCustomPersona(
																			persona.id,
																			e as unknown as React.MouseEvent,
																		);
																}}
															>
																<Trash2 className="size-3" />
															</span>
														</TooltipTrigger>
														<TooltipContent side="top">
															<p>删除</p>
														</TooltipContent>
													</Tooltip>
												</TooltipProvider>
											)}
										</button>
									);
								})}
							</div>
							{/* Loading indicator */}
							{marketLoading && (
								<div className="flex items-center justify-center py-6 gap-2 text-muted-foreground">
									<Loader2 className="size-4 animate-spin" />
									<span className="text-xs">加载中...</span>
								</div>
							)}
							{/* Load more hint */}
							{!marketLoading && hasMore && marketItems.length > 0 && (
								<div className="flex justify-center py-3">
									<button
										type="button"
										className="text-xs text-muted-foreground hover:text-foreground transition-colors"
										onClick={handleLoadMore}
									>
										加载更多 ({marketItems.length}/{marketTotal})
									</button>
								</div>
							)}
							{/* Pagination info */}
							{!marketLoading && !hasMore && marketItems.length > 0 && (
								<div className="text-center py-2 text-[10px] text-muted-foreground/50">
									共 {marketTotal} 个智能体
								</div>
							)}
							{!marketLoading && marketItems.length === 0 && (
								<div className="py-12 text-center text-sm text-muted-foreground">
									{marketSearch || activeTags.size > 0
										? "未找到匹配的智能体"
										: "暂无可用智能体"}
								</div>
							)}
						</ScrollArea>
					</TabsContent>

					{/* ===== Tab: Custom ===== */}
					<TabsContent
						value="custom"
						className="flex-1 min-h-0 mt-0"
					>
						<ScrollArea className="flex-1 min-h-0 px-6 pt-3">
							<div className="grid gap-4 pb-4">
								{/* Avatar + Name */}
								<div className="flex items-center gap-4">
									<div className="relative group">
										<NiceAvatar
											className="w-14 h-14 shrink-0"
											{...currentAvatarConfig}
										/>
										<button
											type="button"
											className="absolute inset-0 flex items-center justify-center rounded-full bg-black/40 opacity-0 group-hover:opacity-100 transition-opacity"
											onClick={handleRandomAvatar}
											aria-label="随机头像"
										>
											<Shuffle className="size-4 text-white" />
										</button>
									</div>
									<div className="flex-1 space-y-1.5">
										<Label
											htmlFor="session-name"
											className="text-xs flex items-center gap-1"
										>
											<User className="size-3" />
											名称
										</Label>
										<Input
											id="session-name"
											placeholder="给会话起个名字"
											value={sessionName}
											onChange={(e) => setSessionName(e.target.value)}
											className="h-8"
										/>
									</div>
								</div>

								{/* System prompt */}
								<div className="space-y-1.5">
									<Label
										htmlFor="system-prompt"
										className="text-xs flex items-center gap-1"
									>
										<Sparkles className="size-3" />
										系统提示词
									</Label>
									<Textarea
										id="system-prompt"
										placeholder="定义 Agent 的行为和角色..."
										value={systemPrompt}
										onChange={(e) => setSystemPrompt(e.target.value)}
										className="min-h-[72px] resize-y text-xs"
									/>
								</div>

								{/* Model + Permission */}
								<div className="grid grid-cols-2 gap-3">
									<div className="space-y-1.5">
										<Label
											htmlFor="model"
											className="text-xs flex items-center gap-1"
										>
											<Bot className="size-3" />
											模型
										</Label>
										{backends.length > 0 || availableModels.length > 1 ? (
											<Select value={model} onValueChange={setModel}>
												<SelectTrigger id="model" className="h-8 text-xs">
													<SelectValue placeholder="选择模型" />
												</SelectTrigger>
												<SelectContent>
													{(backends.length > 0
														? backends
														: availableModels.map((m) => ({
																id: m.model.id,
																name: m.model.name,
															}))
													).map((m) => (
														<SelectItem
															key={m.id}
															value={m.id}
															className="text-xs"
														>
															{m.name || m.id}
														</SelectItem>
													))}
												</SelectContent>
											</Select>
										) : (
											<Input
												id="model"
												placeholder="claude-sonnet-4-20250514"
												value={model}
												onChange={(e) => setModel(e.target.value)}
												className="h-8 text-xs"
											/>
										)}
									</div>
									<div className="space-y-1.5">
										<Label
											htmlFor="perm-mode"
											className="text-xs flex items-center gap-1"
										>
											<Shield className="size-3" />
											权限模式
										</Label>
										<Select
											value={permissionMode}
											onValueChange={setPermissionMode}
										>
											<SelectTrigger id="perm-mode" className="h-8 text-xs">
												<SelectValue />
											</SelectTrigger>
											<SelectContent>
												<SelectItem value="default" className="text-xs">
													默认
												</SelectItem>
												<SelectItem value="plan" className="text-xs">
													计划模式
												</SelectItem>
												<SelectItem
													value="bypassPermissions"
													className="text-xs"
												>
													跳过权限
												</SelectItem>
											</SelectContent>
										</Select>
									</div>
								</div>

								{/* Working directory */}
								<div className="space-y-1.5">
									<Label
										htmlFor="cwd"
										className="text-xs flex items-center gap-1"
									>
										<FolderOpen className="size-3" />
										工作目录
									</Label>
									<Input
										id="cwd"
										placeholder="默认：当前目录"
										value={cwd}
										onChange={(e) => setCwd(e.target.value)}
										className="h-8 text-xs font-mono"
									/>
								</div>

								{/* Advanced */}
								<div className="border-t pt-3">
									<button
										type="button"
										className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
										onClick={() => setAdvancedOpen(!advancedOpen)}
									>
										<ChevronDown
											className={cn(
												"size-3.5 transition-transform duration-200",
												advancedOpen && "rotate-180",
											)}
										/>
										高级选项
									</button>
									{advancedOpen && (
										<div className="grid gap-3 pt-3">
											<div className="space-y-1.5">
												<Label
													htmlFor="base-url"
													className="text-xs flex items-center gap-1"
												>
													<Link className="size-3" />
													API Base URL
												</Label>
												<Input
													id="base-url"
													placeholder="https://api.anthropic.com"
													value={baseUrl}
													onChange={(e) => setBaseUrl(e.target.value)}
													className="h-8 text-xs font-mono"
												/>
												<p className="text-[10px] text-muted-foreground">
													留空则使用全局设置
												</p>
											</div>
											<div className="space-y-1.5">
												<Label
													htmlFor="api-key"
													className="text-xs flex items-center gap-1"
												>
													<Key className="size-3" />
													API Key
												</Label>
												<Input
													id="api-key"
													type="password"
													placeholder="sk-ant-..."
													value={apiKey}
													onChange={(e) => setApiKey(e.target.value)}
													className="h-8 text-xs font-mono"
												/>
												<p className="text-[10px] text-muted-foreground">
													留空则使用全局设置
												</p>
											</div>
										</div>
									)}
								</div>

								{/* Save as persona */}
								<div className="border-t pt-3">
									<label className="flex items-center gap-2 cursor-pointer group">
										<input
											type="checkbox"
											checked={saveAsPersona}
											onChange={(e) => setSaveAsPersona(e.target.checked)}
											className="rounded border-muted-foreground/30"
										/>
										<BookmarkPlus className="size-3.5 text-muted-foreground group-hover:text-foreground transition-colors" />
										<span className="text-xs text-muted-foreground group-hover:text-foreground transition-colors">
											保存为自定义智能体
										</span>
									</label>
									{saveAsPersona && (
										<div className="mt-2">
											<Input
												placeholder="智能体描述（可选）"
												value={personaDescription}
												onChange={(e) =>
													setPersonaDescription(e.target.value)
												}
												className="h-8 text-xs"
											/>
										</div>
									)}
								</div>

								{error && (
									<div className="text-xs text-destructive bg-destructive/10 rounded-md px-3 py-2">
										{error}
									</div>
								)}
							</div>
						</ScrollArea>
					</TabsContent>
				</Tabs>

				<DialogFooter className="px-6 py-4 border-t shrink-0">
					<Button
						variant="outline"
						size="sm"
						onClick={() => {
							onOpenChange(false);
							resetForm();
						}}
						disabled={loading}
					>
						取消
					</Button>
					<Button
						size="sm"
						onClick={handleCreate}
						disabled={loading || !canCreate}
					>
						{loading ? "创建中..." : "创建会话"}
					</Button>
				</DialogFooter>
			</DialogContent>
		</Dialog>
	);
}
