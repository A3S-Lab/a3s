/**
 * Memory page — cross-session knowledge browser.
 * Backed by a3s-memory MemoryStore via SafeClaw gateway API.
 *
 * Layer mapping:
 *   - resources → Episodic (raw observations from sessions)
 *   - artifacts → Semantic + Procedural (structured knowledge & patterns)
 *   - insights  → items tagged "insight" (cross-session synthesis)
 */
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { apiFetch } from "@/lib/http";
import { timeAgo } from "@/lib/time";
import {
	Database,
	Search,
	Trash2,
	Shield,
	AlertTriangle,
	ChevronDown,
	ChevronRight,
	Clock,
	Tag,
	Download,
	BarChart3,
	CheckSquare,
	Square,
} from "lucide-react";
import { useState, useEffect, useMemo, useCallback } from "react";
import { toast } from "sonner";

// =============================================================================
// Types
// =============================================================================

type Layer = "resources" | "artifacts" | "insights";

interface MemoryEntry {
	id: string;
	layer: Layer;
	title: string;
	content: string;
	taintLabels: string[];
	tags: string[];
	importance: number;
	memoryType: string;
	createdAt: number;
	lastAccessedAt: number;
	decayDays: number;
	sessionId: string | null;
}

interface MemoryListResponse {
	layer: string;
	entries: MemoryEntry[];
	total: number;
}

interface MemoryStats {
	layers: { resources: number; artifacts: number; insights: number };
	avgImportance: { resources: number; artifacts: number; insights: number };
	expiringIn7Days: number;
	expiringIn30Days: number;
	tainted: number;
	total: number;
}

// =============================================================================
// API
// =============================================================================

async function fetchMemoryLayer(
	layer: Layer,
	search?: string,
): Promise<MemoryEntry[]> {
	const params = new URLSearchParams();
	if (search) params.set("search", search);
	params.set("limit", "200");
	const qs = params.toString();
	const resp: MemoryListResponse = await apiFetch(
		`/memory/${layer}${qs ? `?${qs}` : ""}`,
	);
	return resp.entries ?? [];
}

async function deleteMemoryEntry(layer: Layer, id: string): Promise<void> {
	const singular =
		layer === "resources"
			? "resource"
			: layer === "artifacts"
				? "artifact"
				: "insight";
	await apiFetch(`/memory/${singular}/${id}`, { method: "DELETE" });
}

async function batchDeleteMemory(
	ids: string[],
): Promise<{ deleted: number; errors: number }> {
	return apiFetch("/memory/batch", {
		method: "DELETE",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ ids }),
	});
}

async function fetchMemoryStats(): Promise<MemoryStats> {
	return apiFetch("/memory/stats");
}

function exportUrl(layer?: Layer, format: "json" | "csv" = "json"): string {
	const base = (window as any).__SAFECLAW_GATEWAY__ || "http://127.0.0.1:18790";
	const params = new URLSearchParams({ format });
	if (layer) params.set("layer", layer);
	return `${base}/api/v1/memory/export?${params}`;
}

// =============================================================================
// Helpers
// =============================================================================

function decayColor(days: number): string {
	if (days <= 7) return "text-destructive";
	if (days <= 30) return "text-amber-600 dark:text-amber-400";
	return "text-muted-foreground";
}

function importanceColor(importance: number): string {
	if (importance >= 0.8) return "text-primary font-medium";
	if (importance >= 0.5) return "text-foreground";
	return "text-muted-foreground";
}

// =============================================================================
// Stats Panel
// =============================================================================

function StatsPanel({ stats }: { stats: MemoryStats | null }) {
	if (!stats || stats.total === 0) return null;
	const items = [
		{
			label: "L1 资源",
			value: stats.layers.resources,
			avg: stats.avgImportance.resources,
			color: "text-blue-600 dark:text-blue-400",
		},
		{
			label: "L2 知识",
			value: stats.layers.artifacts,
			avg: stats.avgImportance.artifacts,
			color: "text-purple-600 dark:text-purple-400",
		},
		{
			label: "L3 洞察",
			value: stats.layers.insights,
			avg: stats.avgImportance.insights,
			color: "text-green-600 dark:text-green-400",
		},
	];
	return (
		<div className="grid grid-cols-3 gap-3">
			{items.map((it) => (
				<div key={it.label} className="rounded-lg border bg-card px-3 py-2">
					<div className={cn("text-[11px] font-semibold", it.color)}>
						{it.label}
					</div>
					<div className="text-lg font-bold tabular-nums">{it.value}</div>
					<div className="text-[10px] text-muted-foreground">
						平均重要度 {(it.avg * 100).toFixed(0)}%
					</div>
				</div>
			))}
		</div>
	);
}

// =============================================================================
// Entry Card (with selection)
// =============================================================================

function EntryCard({
	entry,
	selected,
	onToggle,
	onDelete,
}: {
	entry: MemoryEntry;
	selected: boolean;
	onToggle: () => void;
	onDelete: () => void;
}) {
	const [expanded, setExpanded] = useState(false);
	const [confirming, setConfirming] = useState(false);
	const hasTaint = entry.taintLabels.length > 0;

	return (
		<div
			className={cn(
				"rounded-lg border bg-card transition-all",
				hasTaint && "border-amber-500/30",
				selected && "ring-1 ring-primary/40",
			)}
		>
			<div className="flex items-start gap-3 px-4 py-3">
				<button
					type="button"
					className="mt-0.5 shrink-0 text-muted-foreground hover:text-foreground"
					onClick={onToggle}
					aria-label={selected ? "取消选择" : "选择"}
				>
					{selected ? (
						<CheckSquare className="size-4 text-primary" />
					) : (
						<Square className="size-4" />
					)}
				</button>
				<button
					type="button"
					className="text-muted-foreground hover:text-foreground mt-0.5 shrink-0"
					onClick={() => setExpanded(!expanded)}
				>
					{expanded ? (
						<ChevronDown className="size-4" />
					) : (
						<ChevronRight className="size-4" />
					)}
				</button>
				<div className="flex-1 min-w-0">
					<div className="flex items-center gap-2 flex-wrap">
						<span className="text-sm font-medium truncate">{entry.title}</span>
						<span
							className={cn(
								"text-[10px] rounded border px-1.5 py-0.5",
								importanceColor(entry.importance),
							)}
						>
							{(entry.importance * 100).toFixed(0)}%
						</span>
						<span className="text-[10px] rounded border px-1.5 py-0.5 text-muted-foreground font-mono">
							{entry.memoryType}
						</span>
						{hasTaint && (
							<span className="inline-flex items-center gap-1 text-[10px] rounded border px-1.5 py-0.5 bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20">
								<Shield className="size-3" /> 含敏感标记
							</span>
						)}
					</div>
					<div className="flex items-center gap-3 mt-1 text-[11px] text-muted-foreground flex-wrap">
						<span className="flex items-center gap-1">
							<Clock className="size-3" /> {timeAgo(entry.lastAccessedAt)}
						</span>
						{entry.sessionId && (
							<span className="font-mono truncate max-w-[120px]">
								{entry.sessionId}
							</span>
						)}
						<span
							className={cn(
								"flex items-center gap-1",
								decayColor(entry.decayDays),
							)}
						>
							{entry.decayDays <= 30 && <AlertTriangle className="size-3" />}
							{entry.decayDays} 天后归档
						</span>
					</div>
				</div>
				<div className="shrink-0">
					{confirming ? (
						<div className="flex items-center gap-1">
							<span className="text-[11px] text-muted-foreground">确认？</span>
							<button
								type="button"
								className="text-destructive hover:text-destructive/80 p-0.5"
								onClick={onDelete}
							>
								<span className="text-[11px] font-medium">删除</span>
							</button>
							<button
								type="button"
								className="text-muted-foreground hover:text-foreground p-0.5 text-[11px]"
								onClick={() => setConfirming(false)}
							>
								取消
							</button>
						</div>
					) : (
						<button
							type="button"
							className="text-muted-foreground hover:text-destructive p-1 transition-colors"
							onClick={() => setConfirming(true)}
						>
							<Trash2 className="size-3.5" />
						</button>
					)}
				</div>
			</div>

			{expanded && (
				<div className="px-4 pb-3 border-t pt-2 space-y-2">
					<p className="text-[12px] text-muted-foreground leading-relaxed whitespace-pre-wrap">
						{entry.content}
					</p>
					{entry.tags.length > 0 && (
						<div className="flex flex-wrap gap-1">
							{entry.tags.map((tag) => (
								<span
									key={tag}
									className={cn(
										"inline-flex items-center gap-1 text-[10px] rounded border px-1.5 py-0.5 font-mono",
										tag.startsWith("taint:")
											? "bg-amber-500/10 border-amber-500/20"
											: "bg-muted/40",
									)}
								>
									<Tag className="size-2.5" /> {tag}
								</span>
							))}
						</div>
					)}
				</div>
			)}
		</div>
	);
}

// =============================================================================
// Page
// =============================================================================

const LAYER_META: Record<
	Layer,
	{ label: string; description: string; color: string }
> = {
	resources: {
		label: "L1 资源",
		description: "原始会话记录",
		color: "text-blue-600 dark:text-blue-400",
	},
	artifacts: {
		label: "L2 知识",
		description: "结构化知识与模式",
		color: "text-purple-600 dark:text-purple-400",
	},
	insights: {
		label: "L3 洞察",
		description: "跨会话综合",
		color: "text-green-600 dark:text-green-400",
	},
};

export default function MemoryPage() {
	const [activeLayer, setActiveLayer] = useState<Layer>("artifacts");
	const [entries, setEntries] = useState<MemoryEntry[]>([]);
	const [query, setQuery] = useState("");
	const [loading, setLoading] = useState(false);
	const [stats, setStats] = useState<MemoryStats | null>(null);
	const [selected, setSelected] = useState<Set<string>>(new Set());
	const [showStats, setShowStats] = useState(true);

	const loadLayer = useCallback((layer: Layer, search?: string) => {
		setLoading(true);
		setSelected(new Set());
		fetchMemoryLayer(layer, search)
			.then((data) => setEntries(data))
			.catch((e) => {
				console.warn("Failed to load memory layer:", e);
				setEntries([]);
			})
			.finally(() => setLoading(false));
	}, []);

	// Load stats on mount
	useEffect(() => {
		fetchMemoryStats()
			.then(setStats)
			.catch(() => {});
	}, [entries]);

	useEffect(() => {
		loadLayer(activeLayer);
	}, [activeLayer, loadLayer]);

	// Debounced search
	useEffect(() => {
		const timer = setTimeout(() => {
			loadLayer(activeLayer, query.trim() || undefined);
		}, 300);
		return () => clearTimeout(timer);
	}, [query, activeLayer, loadLayer]);

	const filtered = useMemo(() => entries, [entries]);

	const handleDelete = async (entry: MemoryEntry) => {
		try {
			await deleteMemoryEntry(entry.layer, entry.id);
			setEntries((prev) => prev.filter((e) => e.id !== entry.id));
			setSelected((prev) => {
				const next = new Set(prev);
				next.delete(entry.id);
				return next;
			});
			toast.success("已安全删除");
		} catch {
			toast.error("删除失败");
		}
	};

	const handleBatchDelete = async () => {
		if (selected.size === 0) return;
		try {
			const result = await batchDeleteMemory([...selected]);
			setEntries((prev) => prev.filter((e) => !selected.has(e.id)));
			setSelected(new Set());
			toast.success(`已删除 ${result.deleted} 条记忆`);
		} catch {
			toast.error("批量删除失败");
		}
	};

	const toggleSelect = (id: string) => {
		setSelected((prev) => {
			const next = new Set(prev);
			if (next.has(id)) next.delete(id);
			else next.add(id);
			return next;
		});
	};

	const selectAll = () => {
		if (selected.size === filtered.length) {
			setSelected(new Set());
		} else {
			setSelected(new Set(filtered.map((e) => e.id)));
		}
	};

	const handleExport = (format: "json" | "csv") => {
		const url = exportUrl(activeLayer, format);
		window.open(url, "_blank");
	};

	const taintCount = filtered.filter((e) => e.taintLabels.length > 0).length;
	const decayingSoon = filtered.filter((e) => e.decayDays <= 30).length;

	return (
		<div className="flex flex-col h-full px-5 py-4 space-y-4 overflow-y-auto">
			<div className="flex items-center justify-between shrink-0">
				<div>
					<h1 className="text-sm font-bold">记忆浏览器</h1>
					<p className="text-[11px] text-muted-foreground mt-0.5">
						跨会话知识存储，支持搜索、批量操作和导出
					</p>
				</div>
				<div className="flex items-center gap-1">
					<Button
						variant="ghost"
						size="sm"
						className="h-7 text-[11px] gap-1"
						onClick={() => setShowStats(!showStats)}
					>
						<BarChart3 className="size-3" />{" "}
						{showStats ? "隐藏统计" : "显示统计"}
					</Button>
					<Button
						variant="ghost"
						size="sm"
						className="h-7 text-[11px] gap-1"
						onClick={() => handleExport("json")}
					>
						<Download className="size-3" /> JSON
					</Button>
					<Button
						variant="ghost"
						size="sm"
						className="h-7 text-[11px] gap-1"
						onClick={() => handleExport("csv")}
					>
						<Download className="size-3" /> CSV
					</Button>
				</div>
			</div>

			{showStats && <StatsPanel stats={stats} />}

			{/* Layer tabs */}
			<div className="flex gap-1 border-b">
				{(Object.keys(LAYER_META) as Layer[]).map((layer) => {
					const meta = LAYER_META[layer];
					return (
						<button
							key={layer}
							type="button"
							className={cn(
								"flex items-center gap-1.5 px-4 py-2 text-[12px] font-medium border-b-2 -mb-px transition-colors",
								activeLayer === layer
									? "border-primary text-foreground"
									: "border-transparent text-muted-foreground hover:text-foreground",
							)}
							onClick={() => setActiveLayer(layer)}
						>
							<span className={cn("font-semibold", meta.color)}>
								{meta.label}
							</span>
							<span className="text-muted-foreground">{meta.description}</span>
							{activeLayer === layer && (
								<span className="ml-1 rounded-full bg-muted px-1.5 py-0.5 text-[10px] tabular-nums">
									{filtered.length}
								</span>
							)}
						</button>
					);
				})}
			</div>

			{/* Toolbar: search + bulk actions */}
			<div className="flex items-center gap-3">
				<div className="relative flex-1">
					<Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground" />
					<Input
						className="pl-8 h-7 text-[12px]"
						placeholder="搜索标题或内容..."
						value={query}
						onChange={(e) => setQuery(e.target.value)}
					/>
				</div>
				{filtered.length > 0 && (
					<Button
						variant="ghost"
						size="sm"
						className="h-7 text-[11px] gap-1"
						onClick={selectAll}
					>
						{selected.size === filtered.length ? (
							<CheckSquare className="size-3" />
						) : (
							<Square className="size-3" />
						)}
						{selected.size === filtered.length ? "取消全选" : "全选"}
					</Button>
				)}
				{selected.size > 0 && (
					<Button
						variant="destructive"
						size="sm"
						className="h-7 text-[11px] gap-1"
						onClick={handleBatchDelete}
					>
						<Trash2 className="size-3" /> 删除 {selected.size} 条
					</Button>
				)}
				{taintCount > 0 && (
					<span className="text-[11px] flex items-center gap-1 text-amber-600 dark:text-amber-400">
						<Shield className="size-3" /> {taintCount} 条含敏感标记
					</span>
				)}
				{decayingSoon > 0 && (
					<span className="text-[11px] flex items-center gap-1 text-amber-600 dark:text-amber-400">
						<AlertTriangle className="size-3" /> {decayingSoon} 条即将归档
					</span>
				)}
			</div>

			{/* Entries */}
			{loading ? (
				<div className="text-center py-8 text-[12px] text-muted-foreground">
					加载中...
				</div>
			) : filtered.length === 0 ? (
				<div className="flex flex-col items-center justify-center py-16 text-muted-foreground gap-3">
					<Database className="size-10 opacity-20" />
					<p className="text-sm">{query ? "没有匹配的记忆" : "此层暂无记忆"}</p>
				</div>
			) : (
				<div className="space-y-2">
					{filtered.map((entry) => (
						<EntryCard
							key={entry.id}
							entry={entry}
							selected={selected.has(entry.id)}
							onToggle={() => toggleSelect(entry.id)}
							onDelete={() => handleDelete(entry)}
						/>
					))}
				</div>
			)}
		</div>
	);
}
