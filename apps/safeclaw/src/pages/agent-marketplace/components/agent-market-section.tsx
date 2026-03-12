import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import { useReactive } from "ahooks";
import {
	Bot,
	Download,
	ExternalLink,
	Loader2,
	Search,
	Star,
} from "lucide-react";
import Avatar, { genConfig } from "react-nice-avatar";
import { useEffect } from "react";
import { toast } from "sonner";
import { useNavigate } from "react-router-dom";
import * as marketplaceApi from "@/lib/marketplace-api";
import type { MarketplaceAgent } from "@/lib/marketplace-api";
import { MARKETPLACE_AGENTS } from "@/lib/marketplace-agents-data";

// Industrial domain categories
const CATEGORIES = [
	{ id: "all", label: "全部" },
	{ id: "automation", label: "工业自动化" },
	{ id: "manufacturing", label: "生产制造" },
	{ id: "quality", label: "质量管理" },
	{ id: "maintenance", label: "设备维护" },
	{ id: "safety", label: "安全管理" },
	{ id: "supply-chain", label: "供应链" },
	{ id: "energy", label: "能源管理" },
	{ id: "iiot", label: "工业物联网" },
	{ id: "analytics", label: "数据分析" },
];

export function AgentMarketSection() {
	const navigate = useNavigate();
	const state = useReactive({
		search: "",
		category: "all",
		sortBy: "popular" as "popular" | "rating" | "recent",
		agents: MARKETPLACE_AGENTS,
		loading: false,
		installingId: null as string | null,
	});

	// Load and sync agents with Box on mount
	useEffect(() => {
		loadAgents();
	}, []);

	const loadAgents = async () => {
		state.loading = true;
		try {
			// TODO: Replace with actual API call
			// const agents = await marketplaceApi.listMarketplaceAgents();
			const synced = await marketplaceApi.syncMarketplaceWithBox(
				MARKETPLACE_AGENTS,
				[],
			);
			state.agents = synced.agents;
		} catch (error) {
			console.error("Failed to load agents:", error);
			toast.error("加载智能体列表失败");
		} finally {
			state.loading = false;
		}
	};

	const handleInstall = async (agent: MarketplaceAgent) => {
		state.installingId = agent.id;
		try {
			toast.loading(`正在安装 ${agent.name}...`, { id: agent.id });
			const sessionId = await marketplaceApi.installAgent(agent);

			// Update local state
			const index = state.agents.findIndex((a) => a.id === agent.id);
			if (index !== -1) {
				state.agents[index] = {
					...state.agents[index],
					installed: true,
					sessionId,
				};
			}

			toast.success(`${agent.name} 安装成功，正在跳转...`, { id: agent.id });

			// Navigate to agent page after successful installation
			// Wait a bit to ensure state is updated
			setTimeout(() => {
				navigate("/");
			}, 800);
		} catch (error) {
			console.error("Failed to install agent:", error);
			toast.error(
				`安装失败: ${error instanceof Error ? error.message : "未知错误"}`,
				{
					id: agent.id,
				},
			);
		} finally {
			state.installingId = null;
		}
	};

	const handleUninstall = async (agent: MarketplaceAgent) => {
		if (!agent.sessionId) return;

		state.installingId = agent.id;
		try {
			toast.loading(`正在卸载 ${agent.name}...`, { id: agent.id });
			await marketplaceApi.uninstallAgent(agent.sessionId);

			// Update local state
			const index = state.agents.findIndex((a) => a.id === agent.id);
			if (index !== -1) {
				state.agents[index] = {
					...state.agents[index],
					installed: false,
					sessionId: undefined,
				};
			}

			toast.success(`${agent.name} 已卸载`, { id: agent.id });
		} catch (error) {
			console.error("Failed to uninstall agent:", error);
			toast.error(
				`卸载失败: ${error instanceof Error ? error.message : "未知错误"}`,
				{
					id: agent.id,
				},
			);
		} finally {
			state.installingId = null;
		}
	};

	const filteredAgents = state.agents.filter((agent) => {
		const matchSearch =
			!state.search ||
			agent.name.toLowerCase().includes(state.search.toLowerCase()) ||
			agent.description.toLowerCase().includes(state.search.toLowerCase()) ||
			agent.tags.some((tag) =>
				tag.toLowerCase().includes(state.search.toLowerCase()),
			);
		const matchCategory =
			state.category === "all" || agent.category === state.category;
		return matchSearch && matchCategory;
	});

	const sortedAgents = [...filteredAgents].sort((a, b) => {
		if (state.sortBy === "popular") return b.downloads - a.downloads;
		if (state.sortBy === "rating") return b.rating - a.rating;
		return 0; // recent - 需要添加时间戳字段
	});

	return (
		<div className="flex flex-col h-full">
			{/* Search and Filters */}
			<div className="flex flex-col gap-3 p-4 border-b">
				<div className="flex gap-2">
					<div className="relative flex-1">
						<Search className="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground" />
						<Input
							placeholder="搜索智能体..."
							value={state.search}
							onChange={(e) => (state.search = e.target.value)}
							className="pl-9"
						/>
					</div>
					<Select
						value={state.sortBy}
						onValueChange={(v) =>
							(state.sortBy = v as "popular" | "rating" | "recent")
						}
					>
						<SelectTrigger className="w-[140px]">
							<SelectValue />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="popular">最受欢迎</SelectItem>
							<SelectItem value="rating">评分最高</SelectItem>
							<SelectItem value="recent">最新发布</SelectItem>
						</SelectContent>
					</Select>
				</div>

				{/* Category Filters */}
				<div className="flex gap-2 flex-wrap">
					{CATEGORIES.map((cat) => (
						<Badge
							key={cat.id}
							variant={state.category === cat.id ? "default" : "outline"}
							className={cn(
								"cursor-pointer transition-colors",
								state.category === cat.id
									? "bg-primary text-primary-foreground"
									: "hover:bg-accent",
							)}
							onClick={() => (state.category = cat.id)}
						>
							{cat.label}
						</Badge>
					))}
				</div>
			</div>

			{/* Agent List */}
			<ScrollArea className="flex-1">
				{state.loading ? (
					<div className="flex items-center justify-center h-64">
						<Loader2 className="size-8 animate-spin text-muted-foreground" />
					</div>
				) : (
					<div className="p-4 grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
						{sortedAgents.map((agent) => (
							<AgentCard
								key={agent.id}
								agent={agent}
								installing={state.installingId === agent.id}
								onInstall={() => handleInstall(agent)}
								onUninstall={() => handleUninstall(agent)}
							/>
						))}
					</div>
				)}

				{!state.loading && sortedAgents.length === 0 && (
					<div className="flex flex-col items-center justify-center h-64 text-muted-foreground">
						<Bot className="size-12 mb-4 opacity-20" />
						<p className="text-sm">未找到匹配的智能体</p>
					</div>
				)}
			</ScrollArea>
		</div>
	);
}

function AgentCard({
	agent,
	installing,
	onInstall,
	onUninstall,
}: {
	agent: MarketplaceAgent;
	installing: boolean;
	onInstall: () => void;
	onUninstall: () => void;
}) {
	const avatarConfig = genConfig(agent.avatar);

	return (
		<div className="flex flex-col gap-3 p-4 rounded-lg border bg-card hover:shadow-md transition-shadow">
			{/* Header */}
			<div className="flex items-start gap-3">
				<Avatar className="size-12 shrink-0" {...avatarConfig} />
				<div className="flex-1 min-w-0">
					<h3 className="font-semibold text-sm truncate">{agent.name}</h3>
					<p className="text-xs text-muted-foreground">by {agent.author}</p>
				</div>
				{agent.installed && (
					<Badge variant="secondary" className="shrink-0">
						已安装
					</Badge>
				)}
			</div>

			{/* Description */}
			<p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
				{agent.description}
			</p>

			{/* Tags */}
			<div className="flex gap-1.5 flex-wrap">
				{agent.tags.slice(0, 3).map((tag) => (
					<Badge
						key={tag}
						variant="outline"
						className="text-[10px] px-1.5 py-0"
					>
						{tag}
					</Badge>
				))}
			</div>

			{/* Stats */}
			<div className="flex items-center gap-3 text-xs text-muted-foreground">
				<div className="flex items-center gap-1">
					<Download className="size-3" />
					<span>{agent.downloads.toLocaleString()}</span>
				</div>
				<div className="flex items-center gap-1">
					<Star className="size-3 fill-yellow-400 text-yellow-400" />
					<span>{agent.rating}</span>
				</div>
			</div>

			{/* Actions */}
			<div className="flex gap-2 pt-2 border-t">
				{agent.installed ? (
					<>
						<Button variant="outline" size="sm" className="flex-1">
							<ExternalLink className="size-3 mr-1.5" />
							查看详情
						</Button>
						<Button
							variant="ghost"
							size="sm"
							className="flex-1"
							onClick={onUninstall}
							disabled={installing}
						>
							{installing ? (
								<Loader2 className="size-3 mr-1.5 animate-spin" />
							) : null}
							卸载
						</Button>
					</>
				) : (
					<>
						<Button
							size="sm"
							className="flex-1"
							onClick={onInstall}
							disabled={installing}
						>
							{installing ? (
								<Loader2 className="size-3 mr-1.5 animate-spin" />
							) : (
								<Download className="size-3 mr-1.5" />
							)}
							安装
						</Button>
						<Button variant="outline" size="sm" className="flex-1">
							<ExternalLink className="size-3 mr-1.5" />
							详情
						</Button>
					</>
				)}
			</div>
		</div>
	);
}
