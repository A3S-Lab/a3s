import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useSnapshot } from "valtio";
import {
	ReactFlow,
	Controls,
	MiniMap,
	Background,
	BackgroundVariant,
	Panel,
	useNodesState,
	useEdgesState,
	useReactFlow,
	addEdge,
	Handle,
	Position,
	BaseEdge,
	EdgeLabelRenderer,
	getBezierPath,
	type Node,
	type Edge,
	type OnConnect,
	type NodeTypes,
	type NodeProps,
	type EdgeProps,
	type EdgeTypes,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./flow.css";
import {
	ResizableHandle as RHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import WorkflowChatPanel from "./workflow-chat-panel";
import {
	NODE_CATALOG,
	TOOL_CATALOG,
	CATEGORIES,
	getCatalogEntry,
	type NodeCatalogEntry,
} from "./node-catalog";
import {
	ArrowLeft,
	Save,
	Loader2,
	Play,
	Square,
	Search,
	X,
	Plus,
	Trash2,
	Maximize2,
	LayoutGrid,
	type LucideIcon,
} from "lucide-react";
import { cn } from "@/lib/utils";
import workflowModel, { type WorkflowDoc } from "@/models/workflow.model";

// =============================================================================
// Custom Edge with hover actions (Dify-style)
// =============================================================================

function CustomEdge({
	id,
	sourceX,
	sourceY,
	targetX,
	targetY,
	sourcePosition,
	targetPosition,
	style = {},
	markerEnd,
	selected,
}: EdgeProps) {
	const [isHovered, setIsHovered] = useState(false);
	const { setEdges } = useReactFlow();

	const [edgePath, labelX, labelY] = getBezierPath({
		sourceX,
		sourceY,
		sourcePosition,
		targetX,
		targetY,
		targetPosition,
	});

	const onEdgeDelete = useCallback(() => {
		setEdges((edges) => edges.filter((edge) => edge.id !== id));
	}, [id, setEdges]);

	// 计算起点、中点、终点的位置
	const startPoint = { x: sourceX, y: sourceY };
	const endPoint = { x: targetX, y: targetY };
	const midPoint = { x: labelX, y: labelY };

	const showControls = isHovered || selected;

	return (
		<>
			{/* 主连接线 */}
			<BaseEdge path={edgePath} markerEnd={markerEnd} style={style} />

			{/* 透明的宽连接线用于捕获 hover */}
			<path
				d={edgePath}
				fill="none"
				stroke="transparent"
				strokeWidth={20}
				onMouseEnter={() => setIsHovered(true)}
				onMouseLeave={() => setIsHovered(false)}
				style={{ cursor: "pointer" }}
			/>

			{/* 连接点和操作按钮 */}
			{showControls && (
				<EdgeLabelRenderer>
					{/* 起点圆形连接点 */}
					<div
						style={{
							position: "absolute",
							transform: `translate(-50%, -50%) translate(${startPoint.x}px,${startPoint.y}px)`,
							pointerEvents: "all",
						}}
						className="w-3 h-3 rounded-full bg-primary border-2 border-background shadow-md"
					/>

					{/* 中点操作按钮 */}
					<div
						style={{
							position: "absolute",
							transform: `translate(-50%, -50%) translate(${midPoint.x}px,${midPoint.y}px)`,
							pointerEvents: "all",
						}}
						className="flex items-center gap-1"
					>
						{/* 中点圆形连接点 */}
						<div className="w-3 h-3 rounded-full bg-primary border-2 border-background shadow-md" />

						{/* 操作按钮组 */}
						<div className="flex items-center gap-0.5 bg-background border border-border rounded-lg shadow-lg px-1 py-0.5 ml-1">
							<button
								type="button"
								onClick={onEdgeDelete}
								className="flex items-center justify-center w-6 h-6 rounded hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
								title="删除连接"
							>
								<Trash2 className="size-3" />
							</button>
							<button
								type="button"
								className="flex items-center justify-center w-6 h-6 rounded hover:bg-primary/10 text-muted-foreground hover:text-primary transition-colors"
								title="插入节点"
							>
								<Plus className="size-3" />
							</button>
						</div>
					</div>

					{/* 终点圆形连接点 */}
					<div
						style={{
							position: "absolute",
							transform: `translate(-50%, -50%) translate(${endPoint.x}px,${endPoint.y}px)`,
							pointerEvents: "all",
						}}
						className="w-3 h-3 rounded-full bg-primary border-2 border-background shadow-md"
					/>
				</EdgeLabelRenderer>
			)}
		</>
	);
}

// =============================================================================
// Handles (Dify-style: round dots)
// =============================================================================

function SourceHandle() {
	return (
		<Handle
			type="source"
			position={Position.Right}
			className={cn(
				"!w-3 !h-3 !border-2 !border-border !bg-background !rounded-full",
				"!transition-all !duration-200",
				"hover:!w-3.5 hover:!h-3.5 hover:!border-primary hover:!bg-primary/10",
			)}
		/>
	);
}

function TargetHandle() {
	return (
		<Handle
			type="target"
			position={Position.Left}
			className={cn(
				"!w-3 !h-3 !border-2 !border-border !bg-background !rounded-full",
				"!transition-all !duration-200",
				"hover:!w-3.5 hover:!h-3.5 hover:!border-primary hover:!bg-primary/10",
			)}
		/>
	);
}

// =============================================================================
// Node icon (Dify-style: larger, more prominent)
// =============================================================================

function NodeIcon({
	entry,
	size = "sm",
}: { entry: NodeCatalogEntry; size?: "sm" | "md" | "lg" }) {
	const Icon = entry.icon;
	return (
		<div
			className={cn(
				"flex items-center justify-center rounded-lg shrink-0 shadow-sm",
				size === "sm" && "w-7 h-7",
				size === "md" && "w-9 h-9",
				size === "lg" && "w-10 h-10",
				entry.headerBg,
			)}
		>
			<Icon
				className={cn(
					size === "sm" && "size-4",
					size === "md" && "size-5",
					size === "lg" && "size-5",
					entry.iconColor,
				)}
			/>
		</div>
	);
}

// =============================================================================
// Canvas node — Dify-style card
// =============================================================================

function FlowNode({ type, data, selected }: NodeProps) {
	const entry = getCatalogEntry(type);
	const d = data as Record<string, unknown>;

	return (
		<>
			<TargetHandle />
			<div
				className={cn(
					"w-[260px] rounded-xl border-2 bg-card transition-all duration-200",
					selected
						? "border-primary shadow-[0_0_0_4px_hsl(var(--primary)/0.15)] shadow-lg"
						: "border-border/60 shadow-sm hover:border-border hover:shadow-md",
				)}
			>
				{/* Header */}
				<div className="flex items-center gap-3 px-3.5 py-3 border-b border-border/40">
					{entry && <NodeIcon entry={entry} size="md" />}
					<div className="flex-1 min-w-0">
						<p className="text-xs font-semibold text-foreground leading-tight truncate">
							{(d.title as string) || entry?.label || type}
						</p>
						<p className="text-[10px] text-muted-foreground leading-tight mt-0.5">
							{entry?.label ?? type}
						</p>
					</div>
				</div>

				{/* Param preview */}
				<NodeParamPreview entry={entry} data={d} />
			</div>
			<SourceHandle />
		</>
	);
}

function NodeParamPreview({
	entry,
	data,
}: { entry: NodeCatalogEntry | undefined; data: Record<string, unknown> }) {
	const rows: { key: string; value: string }[] = [];

	switch (entry?.type) {
		case "http-request":
			rows.push({ key: "方法", value: (data.method as string) ?? "GET" });
			if (data.url)
				rows.push({ key: "URL", value: (data.url as string).slice(0, 35) });
			break;
		case "if-else":
			rows.push({
				key: "分支数",
				value: `${((data.cases as unknown[]) ?? []).length} 个条件`,
			});
			break;
		case "code":
			if (data.script) {
				const preview = (data.script as string).split("\n")[0].slice(0, 30);
				rows.push({ key: "脚本", value: preview || "空脚本" });
			}
			break;
		case "template-transform":
			if (data.template) {
				rows.push({
					key: "模板",
					value: (data.template as string).slice(0, 30),
				});
			}
			break;
		case "variable-aggregator":
			rows.push({
				key: "来源",
				value: `${((data.sources as unknown[]) ?? []).length} 个`,
			});
			break;
		case "iteration":
			if (data.items_path)
				rows.push({ key: "路径", value: data.items_path as string });
			break;
		case "sub-flow":
			if (data.flow_name)
				rows.push({ key: "子流程", value: data.flow_name as string });
			break;
		case "noop":
			rows.push({ key: "操作", value: "透传输入" });
			break;
	}

	if (rows.length === 0) return null;

	return (
		<div className="px-3.5 py-2.5 space-y-1.5">
			{rows.map((row, i) => (
				<div key={i} className="flex items-start gap-2 text-[11px]">
					<span className="text-muted-foreground font-medium shrink-0">
						{row.key}:
					</span>
					<span className="text-foreground/80 truncate font-mono">
						{row.value}
					</span>
				</div>
			))}
		</div>
	);
}

function StartNode({ selected }: NodeProps) {
	return (
		<>
			<div
				className={cn(
					"w-[260px] rounded-xl border-2 bg-card transition-all duration-200",
					selected
						? "border-primary shadow-[0_0_0_4px_hsl(var(--primary)/0.15)] shadow-lg"
						: "border-border/60 shadow-sm hover:border-border hover:shadow-md",
				)}
			>
				<div className="flex items-center gap-3 px-3.5 py-3 border-b border-border/40">
					<div className="flex items-center justify-center w-9 h-9 rounded-lg shrink-0 shadow-sm bg-emerald-50 dark:bg-emerald-950/30">
						<Play className="size-5 text-emerald-500" />
					</div>
					<div className="flex-1 min-w-0">
						<p className="text-xs font-semibold text-foreground leading-tight truncate">
							开始
						</p>
						<p className="text-[10px] text-muted-foreground leading-tight mt-0.5">
							开始
						</p>
					</div>
				</div>
			</div>
			<SourceHandle />
		</>
	);
}

function EndNode({ selected }: NodeProps) {
	return (
		<>
			<TargetHandle />
			<div
				className={cn(
					"w-[260px] rounded-xl border-2 bg-card transition-all duration-200",
					selected
						? "border-primary shadow-[0_0_0_4px_hsl(var(--primary)/0.15)] shadow-lg"
						: "border-border/60 shadow-sm hover:border-border hover:shadow-md",
				)}
			>
				<div className="flex items-center gap-3 px-3.5 py-3 border-b border-border/40">
					<div className="flex items-center justify-center w-9 h-9 rounded-lg shrink-0 shadow-sm bg-slate-100 dark:bg-slate-800/40">
						<Square className="size-5 text-slate-500" />
					</div>
					<div className="flex-1 min-w-0">
						<p className="text-xs font-semibold text-foreground leading-tight truncate">
							结束
						</p>
						<p className="text-[10px] text-muted-foreground leading-tight mt-0.5">
							结束
						</p>
					</div>
				</div>
			</div>
		</>
	);
}

const NODE_TYPES: NodeTypes = {
	start: StartNode,
	end: EndNode,
	noop: FlowNode,
	"http-request": FlowNode,
	"if-else": FlowNode,
	"template-transform": FlowNode,
	"variable-aggregator": FlowNode,
	code: FlowNode,
	iteration: FlowNode,
	"sub-flow": FlowNode,
};

const EDGE_TYPES: EdgeTypes = {
	default: CustomEdge,
};

// =============================================================================
// Auto-layout (topological left-to-right)
// =============================================================================
// =============================================================================

function computeLayout(nodes: Node[], edges: Edge[]): Node[] {
	const adj = new Map<string, string[]>();
	const inDeg = new Map<string, number>();
	for (const n of nodes) {
		adj.set(n.id, []);
		inDeg.set(n.id, 0);
	}
	for (const e of edges) {
		adj.get(e.source)?.push(e.target);
		inDeg.set(e.target, (inDeg.get(e.target) ?? 0) + 1);
	}

	// BFS to assign column (depth level)
	const col = new Map<string, number>();
	const queue = nodes
		.filter((n) => (inDeg.get(n.id) ?? 0) === 0)
		.map((n) => n.id);
	for (const id of queue) col.set(id, 0);

	let qi = 0;
	while (qi < queue.length) {
		const id = queue[qi++];
		const c = col.get(id) ?? 0;
		for (const next of adj.get(id) ?? []) {
			col.set(next, Math.max(col.get(next) ?? 0, c + 1));
			inDeg.set(next, (inDeg.get(next) ?? 1) - 1);
			if ((inDeg.get(next) ?? 0) === 0) queue.push(next);
		}
	}
	// Fill unvisited nodes
	for (const n of nodes) if (!col.has(n.id)) col.set(n.id, 0);

	// Group by column, assign row position
	const byCol = new Map<number, string[]>();
	for (const [id, c] of col) {
		if (!byCol.has(c)) byCol.set(c, []);
		byCol.get(c)!.push(id);
	}

	const COL_W = 300;
	const ROW_H = 130;
	const START_X = 80;
	const START_Y = 240;

	return nodes.map((node) => {
		const c = col.get(node.id) ?? 0;
		const rows = byCol.get(c) ?? [node.id];
		const row = rows.indexOf(node.id);
		return {
			...node,
			position: {
				x: START_X + c * COL_W,
				y: START_Y + (row - (rows.length - 1) / 2) * ROW_H,
			},
		};
	});
}

// =============================================================================
// Node catalog popover
// =============================================================================

function NodeCatalogPopover({
	onAdd,
	onClose,
}: {
	onAdd: (type: string, data: Record<string, unknown>) => void;
	onClose: () => void;
}) {
	const [query, setQuery] = useState("");
	const [activeTab, setActiveTab] = useState<"builtin" | "tools">("builtin");
	const inputRef = useRef<HTMLInputElement>(null);

	useEffect(() => {
		inputRef.current?.focus();
	}, []);

	const filtered = query.trim()
		? NODE_CATALOG.filter(
				(n) =>
					n.label.includes(query) ||
					n.type.includes(query) ||
					n.description.includes(query),
			)
		: NODE_CATALOG;

	const groups: [string, NodeCatalogEntry[]][] = query.trim()
		? [["搜索结果", filtered]]
		: CATEGORIES.map(
				(c) =>
					[c, filtered.filter((n) => n.category === c)] as [
						string,
						NodeCatalogEntry[],
					],
			).filter((item) => item[1].length > 0);

	return (
		<div className="w-72 bg-card border border-border/70 rounded-2xl shadow-xl flex flex-col overflow-hidden max-h-[70vh]">
			{/* Tabs */}
			<div className="flex border-b border-border/50">
				<button
					type="button"
					onClick={() => setActiveTab("builtin")}
					className={cn(
						"flex-1 px-4 py-2.5 text-xs font-medium transition-colors relative",
						activeTab === "builtin"
							? "text-primary"
							: "text-muted-foreground hover:text-foreground",
					)}
				>
					内置节点
					{activeTab === "builtin" && (
						<div className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary" />
					)}
				</button>
				<button
					type="button"
					onClick={() => setActiveTab("tools")}
					className={cn(
						"flex-1 px-4 py-2.5 text-xs font-medium transition-colors relative",
						activeTab === "tools"
							? "text-primary"
							: "text-muted-foreground hover:text-foreground",
					)}
				>
					工具节点
					{activeTab === "tools" && (
						<div className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary" />
					)}
				</button>
			</div>

			{/* Search */}
			<div className="flex items-center gap-2 px-3 py-2.5 border-b border-border/50">
				<Search className="size-3.5 text-muted-foreground shrink-0" />
				<input
					ref={inputRef}
					type="text"
					value={query}
					onChange={(e) => setQuery(e.target.value)}
					onKeyDown={(e) => e.key === "Escape" && onClose()}
					placeholder="搜索节点…"
					className="flex-1 text-xs bg-transparent outline-none placeholder:text-muted-foreground/60"
				/>
				{query && (
					<button
						type="button"
						onClick={() => setQuery("")}
						className="text-muted-foreground hover:text-foreground transition-colors"
					>
						<X className="size-3" />
					</button>
				)}
			</div>

			{/* Node list */}
			<div className="overflow-y-auto flex-1 py-2 px-1.5 flex flex-col gap-3">
				{activeTab === "builtin" ? (
					<>
						{groups.map(([category, nodes]) => (
							<div key={category}>
								<p className="text-[10px] font-semibold text-muted-foreground/70 uppercase tracking-wider px-2 mb-1">
									{category}
								</p>
								{nodes.map((node) => (
									<button
										key={node.type}
										type="button"
										onClick={() => {
											onAdd(node.type, { ...node.defaultData });
											onClose();
										}}
										className="w-full flex items-center gap-2.5 px-2 py-1.5 rounded-xl hover:bg-muted/60 transition-colors text-left"
									>
										<NodeIcon entry={node} />
										<div className="flex-1 min-w-0">
											<p className="text-xs font-medium leading-tight">
												{node.label}
											</p>
											<p className="text-[10px] text-muted-foreground leading-tight mt-0.5 truncate">
												{node.description}
											</p>
										</div>
									</button>
								))}
							</div>
						))}
						{groups.length === 0 && (
							<p className="text-xs text-muted-foreground text-center py-6">
								无匹配节点
							</p>
						)}
					</>
				) : (
					<>
						{TOOL_CATALOG.map((node) => (
							<button
								key={node.type}
								type="button"
								onClick={() => {
									onAdd(node.type, { ...node.defaultData });
									onClose();
								}}
								className="w-full flex items-center gap-2.5 px-2 py-1.5 rounded-xl hover:bg-muted/60 transition-colors text-left"
							>
								<NodeIcon entry={node} />
								<div className="flex-1 min-w-0">
									<p className="text-xs font-medium leading-tight">
										{node.label}
									</p>
									<p className="text-[10px] text-muted-foreground leading-tight mt-0.5 truncate">
										{node.description}
									</p>
								</div>
							</button>
						))}
						{TOOL_CATALOG.length === 0 && (
							<div className="flex flex-col items-center justify-center py-8 px-4 text-center">
								<p className="text-xs text-muted-foreground mb-2">暂无工具节点</p>
								<p className="text-[10px] text-muted-foreground/60">
									工具节点由外部集成提供
								</p>
							</div>
						)}
					</>
				)}
			</div>
		</div>
	);
}

// =============================================================================
// Left toolbar + popover (rendered inside ReactFlow so useReactFlow works)
// =============================================================================

interface ToolbarButton {
	icon: LucideIcon;
	label: string;
	action: () => void;
	active?: boolean;
}

function CanvasToolbar({
	nodes,
	edges,
	onAddNode,
	onSetNodes,
}: {
	nodes: Node[];
	edges: Edge[];
	onAddNode: (type: string, data: Record<string, unknown>) => void;
	onSetNodes: (nodes: Node[]) => void;
}) {
	const { fitView } = useReactFlow();
	const [catalogOpen, setCatalogOpen] = useState(false);

	const handleLayout = useCallback(() => {
		const laid = computeLayout(nodes, edges);
		onSetNodes(laid);
		setTimeout(() => fitView({ padding: 0.2, duration: 400 }), 50);
	}, [nodes, edges, onSetNodes, fitView]);

	const buttons: ToolbarButton[] = [
		{
			icon: Plus,
			label: "添加节点",
			action: () => setCatalogOpen((v) => !v),
			active: catalogOpen,
		},
		{
			icon: LayoutGrid,
			label: "整理布局",
			action: handleLayout,
		},
		{
			icon: Maximize2,
			label: "适应画布",
			action: () => fitView({ padding: 0.15, duration: 400 }),
		},
	];

	return (
		<Panel position="top-left">
			<div className="flex items-start gap-2">
				{/* Vertical toolbar */}
				<div className="flex flex-col gap-0.5 p-1 bg-card border border-border/70 rounded-2xl shadow-md">
					{buttons.map(({ icon: Icon, label, action, active }) => (
						<button
							key={label}
							type="button"
							title={label}
							onClick={action}
							className={cn(
								"flex items-center justify-center w-8 h-8 rounded-xl transition-colors",
								active
									? "bg-primary text-primary-foreground"
									: "text-muted-foreground hover:text-foreground hover:bg-muted",
							)}
						>
							<Icon className="size-4" />
						</button>
					))}
				</div>

				{/* Catalog popover */}
				{catalogOpen && (
					<NodeCatalogPopover
						onAdd={onAddNode}
						onClose={() => setCatalogOpen(false)}
					/>
				)}
			</div>
		</Panel>
	);
}

// =============================================================================
// Node config panel
// =============================================================================

function NodeConfigPanel({
	node,
	onUpdate,
	onClose,
}: {
	node: Node;
	onUpdate: (data: Record<string, unknown>) => void;
	onClose: () => void;
}) {
	const entry = getCatalogEntry(node.type as string);
	const [draft, setDraft] = useState<Record<string, unknown>>(
		node.data as Record<string, unknown>,
	);

	const patch = useCallback(
		(partial: Record<string, unknown>) => {
			const next = { ...draft, ...partial };
			setDraft(next);
			onUpdate(next);
		},
		[draft, onUpdate],
	);

	const isSpecial = node.type === "start" || node.type === "end";

	return (
		<div className="w-[340px] bg-card border-2 border-border/60 rounded-xl shadow-2xl flex flex-col overflow-hidden max-h-[80vh]">
			{/* Colored header */}
			<div
				className={cn(
					"flex items-center gap-3 px-4 py-3.5 border-b-2 border-border/40",
					entry?.headerBg,
				)}
			>
				{entry && <NodeIcon entry={entry} size="md" />}
				<div className="flex-1 min-w-0">
					<p className="text-sm font-semibold text-foreground">
						{entry?.label ?? node.type}
					</p>
					<p className="text-[10px] text-muted-foreground mt-0.5 truncate">
						{entry?.description ?? ""}
					</p>
				</div>
				<button
					type="button"
					onClick={onClose}
					className="text-muted-foreground hover:text-foreground transition-colors p-1.5 rounded-lg hover:bg-background/50"
				>
					<X className="size-4" />
				</button>
			</div>

			{isSpecial ? (
				<div className="px-4 py-8 text-xs text-muted-foreground text-center">
					此节点无需配置
				</div>
			) : (
				<div className="overflow-y-auto flex-1 p-4 flex flex-col gap-6">
					<Field label="节点标题">
						<TextInput
							value={(draft.title as string) ?? ""}
							onChange={(v) => patch({ title: v })}
							placeholder={entry?.label}
						/>
					</Field>
					<div className="border-t border-border/30 pt-4">
						<NodeFields
							type={node.type as string}
							draft={draft}
							patch={patch}
						/>
					</div>
				</div>
			)}
		</div>
	);
}

// ── Form primitives ──────────────────────────────────────────────────────────

function Field({
	label,
	hint,
	children,
}: { label: string; hint?: string; children: React.ReactNode }) {
	return (
		<div className="flex flex-col gap-2">
			<div className="flex items-center justify-between">
				<label className="text-xs font-semibold text-foreground">{label}</label>
				{hint && (
					<span className="text-[10px] text-muted-foreground/70 font-medium">
						{hint}
					</span>
				)}
			</div>
			{children}
		</div>
	);
}

const inputCls =
	"w-full px-3 py-2.5 text-xs border-2 border-border/60 rounded-lg bg-background outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-all placeholder:text-muted-foreground/50";

function TextInput({
	value,
	onChange,
	placeholder,
}: { value: string; onChange: (v: string) => void; placeholder?: string }) {
	return (
		<input
			type="text"
			value={value}
			onChange={(e) => onChange(e.target.value)}
			placeholder={placeholder}
			className={inputCls}
		/>
	);
}

function TextareaInput({
	value,
	onChange,
	placeholder,
	rows = 5,
}: {
	value: string;
	onChange: (v: string) => void;
	placeholder?: string;
	rows?: number;
}) {
	return (
		<textarea
			value={value}
			onChange={(e) => onChange(e.target.value)}
			placeholder={placeholder}
			rows={rows}
			className={cn(inputCls, "resize-y font-mono leading-relaxed")}
		/>
	);
}

function SelectInput({
	value,
	onChange,
	options,
}: { value: string; onChange: (v: string) => void; options: string[] }) {
	return (
		<select
			value={value}
			onChange={(e) => onChange(e.target.value)}
			className={inputCls}
		>
			{options.map((o) => (
				<option key={o} value={o}>
					{o}
				</option>
			))}
		</select>
	);
}

function StringListInput({
	value,
	onChange,
	placeholder,
}: { value: string[]; onChange: (v: string[]) => void; placeholder?: string }) {
	return (
		<div className="flex flex-col gap-1.5">
			{value.map((item, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: positional list
				<div key={i} className="flex items-center gap-1.5">
					<input
						type="text"
						value={item}
						onChange={(e) => {
							const n = [...value];
							n[i] = e.target.value;
							onChange(n);
						}}
						placeholder={placeholder}
						className={cn(inputCls, "flex-1")}
					/>
					<button
						type="button"
						onClick={() => onChange(value.filter((_, j) => j !== i))}
						className="text-muted-foreground hover:text-destructive transition-colors p-1.5 rounded-lg hover:bg-destructive/10"
					>
						<Trash2 className="size-3.5" />
					</button>
				</div>
			))}
			<button
				type="button"
				onClick={() => onChange([...value, ""])}
				className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-primary transition-colors mt-0.5"
			>
				<Plus className="size-3" />
				添加来源
			</button>
		</div>
	);
}

const HTTP_METHODS = ["GET", "POST", "PUT", "DELETE", "PATCH"];

function NodeFields({
	type,
	draft,
	patch,
}: {
	type: string;
	draft: Record<string, unknown>;
	patch: (p: Record<string, unknown>) => void;
}) {
	switch (type) {
		case "http-request":
			return (
				<>
					<Field label="方法">
						<SelectInput
							value={(draft.method as string) ?? "GET"}
							onChange={(v) => patch({ method: v })}
							options={HTTP_METHODS}
						/>
					</Field>
					<Field label="URL">
						<TextInput
							value={(draft.url as string) ?? ""}
							onChange={(v) => patch({ url: v })}
							placeholder="https://api.example.com/endpoint"
						/>
					</Field>
					<Field label="请求头" hint="JSON">
						<TextareaInput
							value={
								typeof draft.headers === "object"
									? JSON.stringify(draft.headers, null, 2)
									: ((draft.headers as string) ?? "")
							}
							onChange={(v) => {
								try {
									patch({ headers: JSON.parse(v) });
								} catch {
									patch({ headers: v });
								}
							}}
							placeholder={'{\n  "Authorization": "Bearer ..."\n}'}
							rows={3}
						/>
					</Field>
					{["POST", "PUT", "PATCH"].includes(
						(draft.method as string) ?? "GET",
					) && (
						<Field label="请求体">
							<TextareaInput
								value={(draft.body as string) ?? ""}
								onChange={(v) => patch({ body: v })}
								placeholder={'{\n  "key": "value"\n}'}
								rows={4}
							/>
						</Field>
					)}
				</>
			);
		case "code":
			return (
				<Field label="Rhai 脚本" hint="返回值即节点输出">
					<TextareaInput
						value={(draft.script as string) ?? ""}
						onChange={(v) => patch({ script: v })}
						placeholder={"// 通过 inputs 访问上游节点输出\ninputs"}
						rows={10}
					/>
				</Field>
			);
		case "template-transform":
			return (
				<Field label="Jinja2 模板">
					<TextareaInput
						value={(draft.template as string) ?? ""}
						onChange={(v) => patch({ template: v })}
						placeholder={"Hello {{ inputs.node_id.field }}!"}
						rows={10}
					/>
				</Field>
			);
		case "iteration":
			return (
				<Field label="数组路径" hint="node_id.field">
					<TextInput
						value={(draft.items_path as string) ?? ""}
						onChange={(v) => patch({ items_path: v })}
						placeholder="upstream_node.items"
					/>
				</Field>
			);
		case "sub-flow":
			return (
				<Field label="子流程名称">
					<TextInput
						value={(draft.flow_name as string) ?? ""}
						onChange={(v) => patch({ flow_name: v })}
						placeholder="my-sub-flow"
					/>
				</Field>
			);
		case "variable-aggregator":
			return (
				<Field label="来源" hint="node_id.path">
					<StringListInput
						value={(draft.sources as string[]) ?? []}
						onChange={(v) => patch({ sources: v })}
						placeholder="node_id.output"
					/>
				</Field>
			);
		case "if-else":
			return (
				<Field label="分支条件" hint="JSON">
					<TextareaInput
						value={JSON.stringify(
							draft.cases ?? [
								{ id: "case_1", logical_operator: "and", conditions: [] },
							],
							null,
							2,
						)}
						onChange={(v) => {
							try {
								patch({ cases: JSON.parse(v) });
							} catch {
								/* ignore */
							}
						}}
						rows={10}
					/>
				</Field>
			);
		case "noop":
			return <p className="text-xs text-muted-foreground">此节点无需配置。</p>;
		default:
			return null;
	}
}

// =============================================================================
// Default document
// =============================================================================

const DEFAULT_NODES: Node[] = [
	{
		id: "start_0",
		type: "start",
		position: { x: 80, y: 200 },
		data: {},
		deletable: false,
	},
	{
		id: "end_0",
		type: "end",
		position: { x: 640, y: 200 },
		data: {},
		deletable: false,
	},
];
const DEFAULT_EDGES: Edge[] = [
	{ id: "e-start-end", source: "start_0", target: "end_0" },
];

function parseDocument(doc: Record<string, unknown>): {
	nodes: Node[];
	edges: Edge[];
} {
	const rawNodes = (doc.nodes as unknown[]) ?? [];
	const rawEdges = (doc.edges as unknown[]) ?? [];
	if (!rawNodes.length) return { nodes: DEFAULT_NODES, edges: DEFAULT_EDGES };

	const first = rawNodes[0] as Record<string, unknown>;
	if (first?.meta && "position" in (first.meta as object)) {
		return {
			nodes: (rawNodes as Array<Record<string, unknown>>).map((n) => ({
				id: n.id as string,
				type: n.type as string,
				position: (n.meta as { position: { x: number; y: number } }).position,
				data: (n.data as Record<string, unknown>) ?? {},
			})),
			edges: (rawEdges as Array<Record<string, unknown>>).map((e, i) => ({
				id: (e.id as string) ?? `e-${i}`,
				source: e.sourceNodeID as string,
				target: e.targetNodeID as string,
			})),
		};
	}
	return { nodes: rawNodes as Node[], edges: rawEdges as Edge[] };
}

// =============================================================================
// FlowCanvas
// =============================================================================

function FlowCanvas({
	wf,
	onSavingChange,
	onSavedChange,
}: {
	wf: WorkflowDoc;
	onSavingChange: (v: boolean) => void;
	onSavedChange: (v: boolean) => void;
}) {
	const { nodes: initNodes, edges: initEdges } = parseDocument(
		wf.document ?? {},
	);
	const [nodes, setNodes, onNodesChange] = useNodesState(initNodes);
	const [edges, setEdges, onEdgesChange] = useEdgesState(initEdges);
	const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
	const selectedNode = selectedNodeId
		? nodes.find((n) => n.id === selectedNodeId)
		: null;

	const onConnect: OnConnect = useCallback(
		(c) => setEdges((eds) => addEdge(c, eds)),
		[setEdges],
	);

	// Auto-save
	const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const nodesRef = useRef(nodes);
	const edgesRef = useRef(edges);
	useEffect(() => {
		nodesRef.current = nodes;
	}, [nodes]);
	useEffect(() => {
		edgesRef.current = edges;
	}, [edges]);
	const mountedRef = useRef(false);
	useEffect(() => {
		mountedRef.current = true;
	}, []);

	const triggerSave = useCallback(() => {
		if (!mountedRef.current) return;
		if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
		onSavingChange(true);
		saveTimerRef.current = setTimeout(() => {
			workflowModel
				.update(wf.id, {
					document: {
						nodes: nodesRef.current,
						edges: edgesRef.current,
					} as unknown as Record<string, unknown>,
				})
				.catch(console.error);
			onSavingChange(false);
			onSavedChange(true);
			setTimeout(() => onSavedChange(false), 2000);
		}, 800);
	}, [wf.id, onSavingChange, onSavedChange]);

	const updateNodeData = useCallback(
		(id: string, data: Record<string, unknown>) => {
			setNodes((nds) => nds.map((n) => (n.id === id ? { ...n, data } : n)));
			triggerSave();
		},
		[setNodes, triggerSave],
	);

	const addNode = useCallback(
		(type: string, data: Record<string, unknown>) => {
			setNodes((nds) => [
				...nds,
				{
					id: `${type}_${Date.now()}`,
					type,
					position: {
						x: 360 + Math.random() * 80 - 40,
						y: 200 + Math.random() * 80 - 40,
					},
					data,
				},
			]);
		},
		[setNodes],
	);

	const handleLayout = useCallback(
		(laid: Node[]) => {
			setNodes(laid);
			triggerSave();
		},
		[setNodes, triggerSave],
	);

	return (
		<ReactFlow
			nodes={nodes}
			edges={edges}
			onNodesChange={(c) => {
				onNodesChange(c);
				triggerSave();
			}}
			onEdgesChange={(c) => {
				onEdgesChange(c);
				triggerSave();
			}}
			onConnect={(c) => {
				onConnect(c);
				triggerSave();
			}}
			onNodeClick={(_, node) => setSelectedNodeId(node.id)}
			onPaneClick={() => setSelectedNodeId(null)}
			nodeTypes={NODE_TYPES}
			edgeTypes={EDGE_TYPES}
			fitView
			proOptions={{ hideAttribution: true }}
		>
			<Background variant={BackgroundVariant.Dots} gap={20} size={1} />
			<Controls showInteractive={false} />
			<MiniMap zoomable pannable nodeStrokeWidth={2} />

			{/* Left toolbar + catalog popover */}
			<CanvasToolbar
				nodes={nodes}
				edges={edges}
				onAddNode={addNode}
				onSetNodes={handleLayout}
			/>

			{/* Right: node config */}
			{selectedNode && (
				<Panel position="top-right">
					<NodeConfigPanel
						key={selectedNode.id}
						node={selectedNode}
						onUpdate={(data) => updateNodeData(selectedNode.id, data)}
						onClose={() => setSelectedNodeId(null)}
					/>
				</Panel>
			)}
		</ReactFlow>
	);
}

// =============================================================================
// Page
// =============================================================================

export default function WorkflowEditorPage() {
	const { id } = useParams<{ id: string }>();
	const nav = useNavigate();
	const [saving, setSaving] = useState(false);
	const [saved, setSaved] = useState(false);

	const { workflows, loaded } = useSnapshot(workflowModel.state);
	const loadedRef = useRef(false);
	useEffect(() => {
		if (!loadedRef.current) {
			loadedRef.current = true;
			if (!workflowModel.state.loaded) workflowModel.load();
		}
	}, []);

	const wf = id ? workflows.find((w) => w.id === id) : undefined;

	if (!loaded)
		return (
			<div className="flex items-center justify-center h-full text-muted-foreground text-sm">
				<Loader2 className="size-4 animate-spin mr-2" />
				加载中…
			</div>
		);

	if (!wf)
		return (
			<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
				<p className="text-sm">工作流不存在</p>
				<button
					type="button"
					className="text-xs underline"
					onClick={() => nav("/workflow")}
				>
					返回列表
				</button>
			</div>
		);

	return (
		<div className="flex flex-col h-full">
			{/* Top bar */}
			<div className="flex items-center gap-3 px-4 py-2.5 border-b bg-background shrink-0">
				<button
					type="button"
					onClick={() => nav("/workflow")}
					className="flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
				>
					<ArrowLeft className="size-3.5" />
				</button>
				<span className="text-sm font-semibold truncate">{wf.name}</span>
				<div className="flex-1" />
				<div className="flex items-center gap-1.5 text-xs text-muted-foreground">
					{saving ? (
						<>
							<Loader2 className="size-3 animate-spin" />
							保存中
						</>
					) : saved ? (
						<span className="text-emerald-600 dark:text-emerald-400">
							已保存
						</span>
					) : null}
				</div>
				<button
					type="button"
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-xl bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
				>
					<Play className="size-3 fill-current" />
					运行
				</button>
				<button
					type="button"
					onClick={() => {
						setSaved(true);
						setTimeout(() => setSaved(false), 2000);
					}}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-xl border hover:bg-muted transition-colors"
				>
					<Save className="size-3" />
					保存
				</button>
			</div>

			{/* Canvas + Chat */}
			<ResizablePanelGroup direction="horizontal" className="flex-1 min-h-0">
				<ResizablePanel defaultSize={65} minSize={40}>
					<FlowCanvas
						key={wf.id}
						wf={wf}
						onSavingChange={setSaving}
						onSavedChange={setSaved}
					/>
				</ResizablePanel>
				<RHandle withHandle />
				<ResizablePanel defaultSize={35} minSize={24} maxSize={55}>
					<div className="h-full border-l bg-background">
						<WorkflowChatPanel workflowId={wf.id} />
					</div>
				</ResizablePanel>
			</ResizablePanelGroup>
		</div>
	);
}
