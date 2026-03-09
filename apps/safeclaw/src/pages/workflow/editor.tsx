import React, { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useSnapshot } from "valtio";
import {
	ReactFlow,
	MiniMap,
	Background,
	BackgroundVariant,
	Panel,
	useNodesState,
	useEdgesState,
	useReactFlow,
	useViewport,
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
	type ConnectionLineComponentProps,
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
	Search,
	X,
	Plus,
	Trash2,
	Maximize2,
	LayoutGrid,
	Minus,
	Map,
	Undo2,
	Redo2,
	Copy,
	CopyPlus,
	ChevronDown,
	Variable,
	Globe,
	type LucideIcon,
} from "lucide-react";
import { cn } from "@/lib/utils";
import workflowModel, { type WorkflowDoc } from "@/models/workflow.model";

// =============================================================================
// Custom Connection Line (Dify-style: gray bezier + blue endpoint rect)
// =============================================================================

function CustomConnectionLine({
	fromX,
	fromY,
	toX,
	toY,
}: ConnectionLineComponentProps) {
	const [edgePath] = getBezierPath({
		sourceX: fromX,
		sourceY: fromY,
		sourcePosition: Position.Right,
		targetX: toX,
		targetY: toY,
		targetPosition: Position.Left,
		curvature: 0.16,
	});

	return (
		<g>
			<path fill="none" stroke="#D0D5DD" strokeWidth={2} d={edgePath} />
			<rect x={toX - 1} y={toY - 4} width={2} height={8} fill="#2970FF" />
		</g>
	);
}

// =============================================================================
// Custom Edge (Dify-style: curvature 0.16, midpoint action button)
// =============================================================================

function CustomEdge({
	id,
	source,
	target,
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
	const [catalogOpen, setCatalogOpen] = useState(false);
	const { setEdges, addNodes } = useReactFlow();

	const [edgePath, labelX, labelY] = getBezierPath({
		sourceX: sourceX - 8,
		sourceY,
		sourcePosition,
		targetX: targetX + 8,
		targetY,
		targetPosition,
		curvature: 0.16,
	});

	const onEdgeDelete = useCallback(() => {
		setEdges((edges) => edges.filter((edge) => edge.id !== id));
	}, [id, setEdges]);

	// Insert a new node at the edge midpoint, splitting this edge into two.
	const handleInsertNode = useCallback(
		(type: string, data: Record<string, unknown>) => {
			const newId = `${type}_${Date.now()}`;
			// Place node centered on the midpoint
			addNodes([
				{
					id: newId,
					type,
					position: { x: labelX - 120, y: labelY - 36 },
					data,
				},
			]);
			// Look up sourceHandle/targetHandle from the current edge
			setEdges((edges) => {
				const cur = edges.find((e) => e.id === id);
				return [
					...edges.filter((e) => e.id !== id),
					{
						id: `e-${source}-${newId}`,
						source,
						...(cur?.sourceHandle ? { sourceHandle: cur.sourceHandle } : {}),
						target: newId,
						type: "custom",
					},
					{
						id: `e-${newId}-${target}`,
						source: newId,
						target,
						...(cur?.targetHandle ? { targetHandle: cur.targetHandle } : {}),
						type: "custom",
					},
				];
			});
			setCatalogOpen(false);
		},
		[id, source, target, labelX, labelY, addNodes, setEdges],
	);

	const showButton = isHovered || selected || catalogOpen;

	return (
		<>
			{/* Main edge path */}
			<BaseEdge path={edgePath} markerEnd={markerEnd} style={style} />

			{/* Wide transparent hit area for hover detection */}
			<path
				d={edgePath}
				fill="none"
				stroke="transparent"
				strokeWidth={20}
				onMouseEnter={() => setIsHovered(true)}
				onMouseLeave={() => setIsHovered(false)}
				style={{ cursor: "pointer" }}
			/>

			{showButton && (
				<EdgeLabelRenderer>
					<div
						style={{
							position: "absolute",
							transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
							pointerEvents: "all",
						}}
						className="nopan nodrag"
						onMouseEnter={() => setIsHovered(true)}
						onMouseLeave={() => setIsHovered(false)}
					>
						{/* Circular "+" button — matches node handle style */}
						<button
							type="button"
							onClick={() => setCatalogOpen((v) => !v)}
							className={cn(
								"flex items-center justify-center w-5 h-5 rounded-full",
								"bg-primary text-white shadow-md border-0 outline-none",
								"hover:scale-110 transition-transform duration-150",
								catalogOpen && "scale-110 ring-2 ring-primary/30",
							)}
							title="插入节点"
						>
							<Plus className="size-3" />
						</button>

						{/* Delete button — floats alongside, only when selected */}
						{selected && !catalogOpen && (
							<button
								type="button"
								onClick={onEdgeDelete}
								className="absolute -right-6 top-0 flex items-center justify-center w-5 h-5 rounded-full bg-destructive/90 text-white shadow-md border-0 outline-none hover:bg-destructive transition-colors"
								title="删除连接"
							>
								<X className="size-2.5" />
							</button>
						)}

						{/* Node catalog popover */}
						{catalogOpen && (
							<div className="absolute left-1/2 -translate-x-1/2 top-7 z-50">
								<NodeCatalogPopover
									onAdd={handleInsertNode}
									onClose={() => setCatalogOpen(false)}
								/>
							</div>
						)}
					</div>
				</EdgeLabelRenderer>
			)}
		</>
	);
}

// =============================================================================
// Handles
//
// Design (matches Dify):
//   • Transparent 16×16 hit area so users can click the edge of a node
//   • A 2×8px vertical bar as the visual indicator (bg-border)
//   • BranchHandle — embedded inside a branch row for IF/ELSE and QC nodes;
//     uses absolute positioning so it protrudes at the right edge of the row
// =============================================================================

// Shared handle visual — circular "+" button, Dify-style.
// • Background: primary color (solid fill)
// • Icon: white
// • Opacity controlled by flow.css (.react-flow__node:hover .react-flow__handle)
const handleCircleClass =
	"!h-5 !w-5 !rounded-full !border-0 !bg-primary !shadow-md !outline-none " +
	"!flex !items-center !justify-center !z-10";

function SourceHandle({ id }: { id?: string } = {}) {
	return (
		<Handle
			type="source"
			position={Position.Right}
			id={id}
			className={handleCircleClass}
		>
			<Plus className="pointer-events-none size-3 text-white" />
		</Handle>
	);
}

function TargetHandle() {
	return (
		<Handle
			type="target"
			position={Position.Left}
			className={handleCircleClass}
		>
			<Plus className="pointer-events-none size-3 text-white" />
		</Handle>
	);
}

// Branch row handle — sits inside a `relative` row div, protrudes right.
// React Flow computes the edge endpoint from this element's screen position.
function BranchHandle({ id }: { id: string }) {
	return (
		<Handle
			type="source"
			position={Position.Right}
			id={id}
			className={cn(
				"!absolute !-right-2.5 !top-1/2 !-translate-y-1/2",
				handleCircleClass,
			)}
		>
			<Plus className="pointer-events-none size-3 text-white" />
		</Handle>
	);
}

// =============================================================================
// Node icon
// =============================================================================

function NodeIcon({
	entry,
	size = "sm",
	className,
}: { entry: NodeCatalogEntry; size?: "sm" | "md" | "lg"; className?: string }) {
	const Icon = entry.icon;
	return (
		<div
			className={cn(
				"flex items-center justify-center rounded-lg shrink-0",
				size === "sm" && "w-7 h-7",
				size === "md" && "w-8 h-8",
				size === "lg" && "w-10 h-10",
				entry.headerBg,
				className,
			)}
		>
			<Icon
				className={cn(
					size === "sm" && "size-4",
					size === "md" && "size-4",
					size === "lg" && "size-5",
					entry.iconColor,
				)}
			/>
		</div>
	);
}

// =============================================================================
// Dify-style node body helpers
// =============================================================================

/** Single parameter pill — h-6 muted background, consistent with Dify's
 *  `bg-workflow-block-parma-bg` pattern */
function Pill({
	icon: Icon,
	children,
	mono,
	className,
}: {
	icon?: LucideIcon;
	children: React.ReactNode;
	mono?: boolean;
	className?: string;
}) {
	return (
		<div
			className={cn(
				"flex h-6 items-center gap-1 rounded-md bg-muted/60 px-1.5 text-xs text-foreground/80 overflow-hidden",
				className,
			)}
		>
			{Icon && <Icon className="size-3 shrink-0 text-muted-foreground/60" />}
			<span className={cn("truncate leading-none", mono && "font-mono")}>{children}</span>
		</div>
	);
}

/** Node body wrapper — mb-1 px-3 py-1 space-y-0.5 matching Dify */
function Body({ children }: { children: React.ReactNode }) {
	return <div className="mb-1 px-3 py-1 space-y-0.5">{children}</div>;
}

// =============================================================================
// Per-node body content (Dify-aligned, one component per node type)
// =============================================================================

function NodeBody({
	entry,
	data,
}: { entry: NodeCatalogEntry | undefined; data: Record<string, unknown> }) {
	switch (entry?.type) {
		// ── Start: input variable pills ───────────────────────────────────────
		case "start": {
			const inputs =
				(data.inputs as Array<{ name: string; type: string }>) ?? [];
			if (!inputs.length) return null;
			return (
				<Body>
					{inputs.slice(0, 4).map((v, i) => (
						// biome-ignore lint/suspicious/noArrayIndexKey: ordered list
						<div
							key={i}
							className="flex h-6 items-center justify-between gap-1 rounded-md bg-muted/60 px-1.5 overflow-hidden"
						>
							<div className="flex items-center gap-1 min-w-0">
								<Variable className="size-3 shrink-0 text-primary/50" />
								<span className="text-xs text-foreground/80 truncate leading-none">
									{v.name || "unnamed"}
								</span>
							</div>
							<span className="shrink-0 text-[9px] text-muted-foreground/60 uppercase font-mono">
								{v.type}
							</span>
						</div>
					))}
					{inputs.length > 4 && (
						<p className="text-[10px] text-muted-foreground/50 pl-1">
							+{inputs.length - 4} more
						</p>
					)}
				</Body>
			);
		}

			// ── HTTP: [METHOD] + URL ───────────────────────────────────────────────
		case "http-request": {
			const method = (data.method as string) ?? "GET";
			const url = (data.url as string) ?? "";
			if (!url) return null;
			const methodColors: Record<string, string> = {
				GET: "text-emerald-600 bg-emerald-100 dark:bg-emerald-900/30",
				POST: "text-blue-600 bg-blue-100 dark:bg-blue-900/30",
				PUT: "text-amber-600 bg-amber-100 dark:bg-amber-900/30",
				DELETE: "text-red-600 bg-red-100 dark:bg-red-900/30",
				PATCH: "text-purple-600 bg-purple-100 dark:bg-purple-900/30",
			};
			return (
				<Body>
					<div className="flex items-center gap-1.5 rounded-md bg-muted/60 px-1.5 min-h-6 py-0.5 overflow-hidden">
						<span
							className={cn(
								"shrink-0 text-[10px] font-bold uppercase px-1 py-0.5 rounded",
								methodColors[method] ?? "text-muted-foreground bg-muted",
							)}
						>
							{method}
						</span>
						<span className="text-xs text-foreground/60 font-mono truncate">
							{url.replace(/^https?:\/\//, "")}
						</span>
					</div>
				</Body>
			);
		}

		// ── LLM: model pill ────────────────────────────────────────────────────
		case "llm": {
			const model = (data.model as string) ?? "";
			if (!model) return null;
			return (
				<Body>
					<Pill mono>{model}</Pill>
				</Body>
			);
		}

		// ── Parameter Extractor: model + param count ───────────────────────────
		case "parameter-extractor": {
			const model = (data.model as string) ?? "";
			const params = (data.parameters as unknown[]) ?? [];
			if (!model && !params.length) return null;
			return (
				<Body>
					{model && <Pill mono>{model}</Pill>}
					{params.length > 0 && (
						<Pill>{params.length} 个参数</Pill>
					)}
				</Body>
			);
		}

		// ── Variable Aggregator: source path pills ─────────────────────────────
		case "variable-aggregator": {
			const sources = (data.sources as string[]) ?? [];
			if (!sources.length) return null;
			return (
				<Body>
					{sources.slice(0, 3).map((s, i) => (
						// biome-ignore lint/suspicious/noArrayIndexKey: ordered list
						<Pill key={i} mono>
							{s}
						</Pill>
					))}
					{sources.length > 3 && (
						<p className="text-[10px] text-muted-foreground/50 pl-1">
							+{sources.length - 3} more
						</p>
					)}
				</Body>
			);
		}

			// ── Assign: key → value rows ───────────────────────────────────────────
		case "assign": {
			const assigns = data.assigns as Record<string, string> | undefined;
			const entries = Object.entries(assigns ?? {}).slice(0, 3);
			if (!entries.length) return null;
			const total = Object.keys(assigns ?? {}).length;
			return (
				<Body>
					{entries.map(([key, val]) => (
						<div
							key={key}
							className="flex h-6 items-center gap-1 rounded-md bg-muted/60 px-1.5 overflow-hidden"
						>
							<span className="shrink-0 text-xs font-mono text-muted-foreground/70 truncate max-w-[70px]">
								{key}
							</span>
							<span className="text-muted-foreground/40 shrink-0 text-[10px]">＝</span>
							<span className="text-xs text-foreground/70 font-mono truncate">
								{val || "…"}
							</span>
						</div>
					))}
					{total > 3 && (
						<p className="text-[10px] text-muted-foreground/50 pl-1">
							+{total - 3} more
						</p>
					)}
				</Body>
			);
		}

		// ── List Operator: input selector + sort ──────────────────────────────
		case "list-operator": {
			const input = (data.input_selector as string) ?? "";
			const sortBy = (data.sort_by as string) ?? "";
			if (!input) return null;
			return (
				<Body>
					<Pill mono>{input}</Pill>
					{sortBy && (
						<Pill>
							{sortBy} {(data.sort_order as string) ?? "asc"}
						</Pill>
					)}
				</Body>
			);
		}

		// ── MCP: tool name + server ────────────────────────────────────────────
		case "mcp": {
			const toolName = (data.tool_name as string) ?? "";
			const serverUrl = (data.server_url as string) ?? "";
			if (!toolName && !serverUrl) return null;
			return (
				<Body>
					{toolName && (
						<Pill icon={Globe}>{toolName}</Pill>
					)}
					{serverUrl && (
						<Pill mono>
							{serverUrl.replace(/^https?:\/\//, "").slice(0, 32)}
						</Pill>
					)}
				</Body>
			);
		}

		// code / template-transform / noop: no preview (matches Dify)
		default:
			return null;
	}
}

// =============================================================================
// Canvas node — Dify-aligned card
//   • Outer ring:  rounded-2xl border (transparent → primary/40 when selected)
//   • Inner card:  w-[240px] rounded-[15px] border-border/10 bg-card shadow-sm
//   • Header:      px-3 pb-2 pt-3, NodeIcon (sm=28px) + uppercase title
//   • Body:        per-type NodeBody pills (mb-1 px-3 py-1)
// =============================================================================

function FlowNode({ type, data, selected }: NodeProps) {
	const entry = getCatalogEntry(type);
	const d = data as Record<string, unknown>;

	return (
		<>
			<TargetHandle />
			{/* Outer: selection ring */}
			<div
				className={cn(
					"relative flex rounded-2xl border transition-all duration-200",
					selected ? "border-primary/40" : "border-transparent",
				)}
			>
				{/* Inner: card — rounded-[15px] matches Dify's rounded-[15px] */}
				<div
					className={cn(
						"relative",
						"rounded-[15px] border border-border/10",
						"w-[240px] bg-card shadow-sm",
						"hover:shadow-md transition-shadow duration-200",
					)}
				>
					{/* Header: px-3 pb-2 pt-3 — NodeIcon sm (28px) + uppercase title */}
					<div className="flex items-center px-3 pb-2 pt-3">
						{entry && (
							<NodeIcon entry={entry} size="sm" className="mr-2 shrink-0" />
						)}
						<p className="text-[11px] font-semibold text-foreground uppercase tracking-wider truncate">
							{(d.title as string) || entry?.label || type}
						</p>
					</div>
					{/* Per-type body */}
					<NodeBody entry={entry} data={d} />
				</div>
			</div>
			<SourceHandle />
		</>
	);
}

function StartNode({ data, selected }: NodeProps) {
	const d = data as Record<string, unknown>;
	const entry = getCatalogEntry("start")!;
	return (
		<>
			{/* Outer: selection ring */}
			<div
				className={cn(
					"relative flex rounded-2xl border transition-all duration-200",
					selected ? "border-primary/40" : "border-transparent",
				)}
			>
				<div
					className={cn(
						"relative",
						"rounded-[15px] border border-border/10",
						"w-[240px] bg-card shadow-sm",
						"hover:shadow-md transition-shadow duration-200",
					)}
				>
					<div className="flex items-center px-3 pb-2 pt-3">
						<NodeIcon entry={entry} size="sm" className="mr-2 shrink-0" />
						<p className="text-[11px] font-semibold text-foreground uppercase tracking-wider">
							{(d.title as string) || entry.label}
						</p>
					</div>
					<NodeBody entry={entry} data={d} />
				</div>
			</div>
			<SourceHandle />
		</>
	);
}

// =============================================================================
// Container nodes (Dify-style: large frame + inner editing zone)
//
// React Flow "parent node" pattern:
//   - The container is a normal node with explicit style.width/height
//   - Child nodes carry parentId + extent:'parent' and are positioned
//     relative to the container's top-left corner
//   - useReactFlow().addNodes() adds children directly from inside the node
// =============================================================================

const CONTAINER_W = 700;
const CONTAINER_H = 320;
const CONTAINER_INNER_PADDING = 56; // header height + bottom margin

// Catalog popover rendered inside the container node.
// Needs nodrag + nopan so clicks don't propagate to ReactFlow.
function InlineCatalogPopover({
	onAdd,
	onClose,
}: {
	onAdd: (type: string, data: Record<string, unknown>) => void;
	onClose: () => void;
}) {
	return (
		<div className="nodrag nopan absolute bottom-10 right-0 z-50">
			<NodeCatalogPopover onAdd={onAdd} onClose={onClose} />
		</div>
	);
}

function IterationContainerNode({ id, data, selected }: NodeProps) {
	const entry = getCatalogEntry("iteration");
	const d = data as Record<string, unknown>;
	const { addNodes } = useReactFlow();
	const [catalogOpen, setCatalogOpen] = useState(false);

	const handleAddInner = useCallback(
		(type: string, nodeData: Record<string, unknown>) => {
			addNodes([
				{
					id: `${type}_${Date.now()}`,
					type,
					parentId: id,
					extent: "parent" as const,
					position: { x: 60, y: CONTAINER_INNER_PADDING },
					data: nodeData,
				},
			]);
			setCatalogOpen(false);
		},
		[id, addNodes],
	);

	return (
		<>
			<TargetHandle />
			<div
				style={{ width: CONTAINER_W, minHeight: CONTAINER_H }}
				className={cn(
					"relative rounded-2xl border-2 transition-all duration-200",
					selected
						? "border-violet-400 shadow-lg shadow-violet-200/40 dark:shadow-violet-900/30"
						: "border-violet-300/60 hover:border-violet-400/70",
					"bg-gradient-to-b from-violet-50/80 to-violet-50/20 dark:from-violet-950/15 dark:to-transparent",
				)}
			>
				{/* Header */}
				<div className="flex items-center gap-2.5 px-4 py-3 border-b border-violet-200/50 dark:border-violet-800/25">
					{entry && <NodeIcon entry={entry} size="md" />}
					<p className="flex-1 text-[11px] font-semibold text-foreground uppercase tracking-wider truncate">
						{(d.title as string) || entry?.label}
					</p>
					{!!d.input_selector && (
						<code className="shrink-0 text-[10px] text-violet-600 dark:text-violet-400 bg-violet-100 dark:bg-violet-900/30 px-2 py-0.5 rounded-full font-mono">
							{d.input_selector as string}
						</code>
					)}
					{!!d.mode && (
						<span className="shrink-0 text-[10px] text-muted-foreground bg-muted px-2 py-0.5 rounded-full capitalize">
							{d.mode as string}
						</span>
					)}
				</div>

				{/* Inner editing zone */}
				<div
					className={cn(
						"nodrag relative m-3 rounded-xl border border-dashed",
						"border-violet-300/50 dark:border-violet-700/30",
						"bg-violet-50/20 dark:bg-violet-950/5",
					)}
					style={{ minHeight: CONTAINER_H - CONTAINER_INNER_PADDING }}
				>
					{/* Add inner node button */}
					<div className="nopan nodrag absolute bottom-3 right-3 flex flex-col items-end gap-1">
						{catalogOpen && (
							<InlineCatalogPopover
								onAdd={handleAddInner}
								onClose={() => setCatalogOpen(false)}
							/>
						)}
						<button
							type="button"
							onClick={(e) => { e.stopPropagation(); setCatalogOpen((v) => !v); }}
							className={cn(
								"flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-medium transition-all",
								"border border-dashed border-violet-400/50 text-violet-500 dark:text-violet-400",
								"hover:bg-violet-100/60 dark:hover:bg-violet-900/30 hover:border-violet-400",
							)}
						>
							<Plus className="size-3" />
							添加节点
						</button>
					</div>
				</div>
			</div>
			<SourceHandle />
		</>
	);
}

function LoopContainerNode({ id, data, selected }: NodeProps) {
	const entry = getCatalogEntry("loop");
	const d = data as Record<string, unknown>;
	const { addNodes } = useReactFlow();
	const [catalogOpen, setCatalogOpen] = useState(false);

	const handleAddInner = useCallback(
		(type: string, nodeData: Record<string, unknown>) => {
			addNodes([
				{
					id: `${type}_${Date.now()}`,
					type,
					parentId: id,
					extent: "parent" as const,
					position: { x: 60, y: CONTAINER_INNER_PADDING },
					data: nodeData,
				},
			]);
			setCatalogOpen(false);
		},
		[id, addNodes],
	);

	return (
		<>
			<TargetHandle />
			<div
				style={{ width: CONTAINER_W, minHeight: CONTAINER_H }}
				className={cn(
					"relative rounded-2xl border-2 transition-all duration-200",
					selected
						? "border-indigo-400 shadow-lg shadow-indigo-200/40 dark:shadow-indigo-900/30"
						: "border-indigo-300/60 hover:border-indigo-400/70",
					"bg-gradient-to-b from-indigo-50/80 to-indigo-50/20 dark:from-indigo-950/15 dark:to-transparent",
				)}
			>
				{/* Header */}
				<div className="flex items-center gap-2.5 px-4 py-3 border-b border-indigo-200/50 dark:border-indigo-800/25">
					{entry && <NodeIcon entry={entry} size="md" />}
					<p className="flex-1 text-[11px] font-semibold text-foreground uppercase tracking-wider truncate">
						{(d.title as string) || entry?.label}
					</p>
					{d.max_iterations !== undefined && (
						<span className="shrink-0 text-[10px] font-mono text-indigo-600 dark:text-indigo-400 bg-indigo-100 dark:bg-indigo-900/30 px-2 py-0.5 rounded-full">
							≤ {d.max_iterations as number} 次
						</span>
					)}
					{!!d.output_selector && (
						<code className="shrink-0 text-[10px] text-indigo-500 bg-indigo-100 dark:bg-indigo-900/30 px-2 py-0.5 rounded-full font-mono ml-1">
							{d.output_selector as string}
						</code>
					)}
				</div>

				{/* Inner editing zone */}
				<div
					className={cn(
						"nodrag relative m-3 rounded-xl border border-dashed",
						"border-indigo-300/50 dark:border-indigo-700/30",
						"bg-indigo-50/20 dark:bg-indigo-950/5",
					)}
					style={{ minHeight: CONTAINER_H - CONTAINER_INNER_PADDING }}
				>
					<div className="nopan nodrag absolute bottom-3 right-3 flex flex-col items-end gap-1">
						{catalogOpen && (
							<InlineCatalogPopover
								onAdd={handleAddInner}
								onClose={() => setCatalogOpen(false)}
							/>
						)}
						<button
							type="button"
							onClick={(e) => { e.stopPropagation(); setCatalogOpen((v) => !v); }}
							className={cn(
								"flex items-center gap-1.5 px-2.5 py-1.5 rounded-xl text-[11px] font-medium transition-all",
								"border border-dashed border-indigo-400/50 text-indigo-500 dark:text-indigo-400",
								"hover:bg-indigo-100/60 dark:hover:bg-indigo-900/30 hover:border-indigo-400",
							)}
						>
							<Plus className="size-3" />
							添加节点
						</button>
					</div>
				</div>
			</div>
			<SourceHandle />
		</>
	);
}

// =============================================================================
// IF/ELSE node — one source handle per case + one ELSE handle (Dify-style)
// =============================================================================

function IfElseNode({ data, selected }: NodeProps) {
	const entry = getCatalogEntry("if-else")!;
	const d = data as Record<string, unknown>;
	const cases =
		(d.cases as Array<{ id: string; conditions?: unknown[] }>) ?? [];

	return (
		<>
			<TargetHandle />
			<div
				className={cn(
					"relative flex rounded-2xl border transition-all duration-200",
					selected ? "border-primary/40" : "border-transparent",
				)}
			>
				<div
					className={cn(
						"relative",
						"rounded-[15px] border border-border/10",
						"w-[240px] bg-card shadow-sm",
						"hover:shadow-md transition-shadow duration-200",
					)}
				>
					<div className="flex items-center px-3 pb-2 pt-3">
						<NodeIcon entry={entry} size="sm" className="mr-2 shrink-0" />
						<p className="text-[11px] font-semibold text-foreground uppercase tracking-wider truncate">
							{(d.title as string) || entry.label}
						</p>
					</div>
					{/* Branch rows — each row carries its own source handle */}
					<div className="mb-1 px-3 py-1 space-y-0.5">
						{cases.slice(0, 5).map((c, i) => (
							<div
								key={c.id}
								className="relative flex h-6 items-center justify-between gap-1 rounded-md bg-muted/60 px-1.5"
							>
								<span className="text-[10px] font-semibold text-muted-foreground uppercase">
									{i === 0 ? "IF" : `ELIF ${i}`}
								</span>
								<span className="text-[10px] text-foreground/60">
									{(c.conditions?.length ?? 0) > 0
										? `${c.conditions?.length} 条件`
										: ""}
								</span>
								<BranchHandle id={c.id} />
							</div>
						))}
						{/* ELSE always present */}
						<div className="relative flex h-6 items-center justify-between px-1">
							<span className="text-[10px] font-semibold text-muted-foreground/60 uppercase">
								ELSE
							</span>
							<BranchHandle id="else" />
						</div>
					</div>
				</div>
			</div>
		</>
	);
}

// =============================================================================
// Question Classifier node — one source handle per class (Dify-style)
// =============================================================================

interface ClassItem {
	id: string;
	name: string;
	description?: string;
}

function QuestionClassifierNode({ data, selected }: NodeProps) {
	const entry = getCatalogEntry("question-classifier")!;
	const d = data as Record<string, unknown>;
	const classes = (d.classes as ClassItem[]) ?? [];
	const model = (d.model as string) ?? "";

	return (
		<>
			<TargetHandle />
			<div
				className={cn(
					"relative flex rounded-2xl border transition-all duration-200",
					selected ? "border-primary/40" : "border-transparent",
				)}
			>
				<div
					className={cn(
						"relative",
						"rounded-[15px] border border-border/10",
						"w-[240px] bg-card shadow-sm",
						"hover:shadow-md transition-shadow duration-200",
					)}
				>
					<div className="flex items-center px-3 pb-2 pt-3">
						<NodeIcon entry={entry} size="sm" className="mr-2 shrink-0" />
						<p className="text-[11px] font-semibold text-foreground uppercase tracking-wider truncate">
							{(d.title as string) || entry.label}
						</p>
					</div>
					<div className="mb-1 px-3 py-1 space-y-0.5">
						{/* Model pill */}
						{model && (
							<div className="flex h-6 items-center gap-1 rounded-md bg-muted/60 px-1.5 overflow-hidden">
								<span className="text-xs font-mono text-foreground/70 truncate">
									{model}
								</span>
							</div>
						)}
						{/* Class rows — each row carries its own source handle */}
						{classes.slice(0, 6).map((cls) => (
							<div
								key={cls.id}
								className="relative flex h-6 items-center gap-1 rounded-md bg-muted/60 px-1.5"
							>
								<span className="text-xs text-foreground/80 truncate leading-none flex-1">
									{cls.name || cls.id}
								</span>
								<BranchHandle id={cls.id} />
							</div>
						))}
						{classes.length > 6 && (
							<p className="text-[10px] text-muted-foreground/50 pl-1">
								+{classes.length - 6} more
							</p>
						)}
					</div>
				</div>
			</div>
		</>
	);
}

const NODE_TYPES: NodeTypes = {
	start: StartNode,
	noop: FlowNode,
	"http-request": FlowNode,
	"if-else": IfElseNode,
	"template-transform": FlowNode,
	"variable-aggregator": FlowNode,
	code: FlowNode,
	iteration: IterationContainerNode,
	loop: LoopContainerNode,
	llm: FlowNode,
	"question-classifier": QuestionClassifierNode,
	"parameter-extractor": FlowNode,
	assign: FlowNode,
	"list-operator": FlowNode,
	mcp: FlowNode,
};

const EDGE_TYPES: EdgeTypes = {
	default: CustomEdge,
};

// =============================================================================
// Auto-layout (topological left-to-right)
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

// =============================================================================
// Canvas controls — Dify layout:
//   bottom-left  → UndoRedoControls (撤销 / 重做)
//   bottom-center → ZoomControls (−, %, +, fit, minimap toggle)
// Both must be rendered inside <ReactFlow>.
// =============================================================================

const ctrlBtnCls =
	"flex items-center justify-center w-7 h-7 text-muted-foreground " +
	"hover:text-foreground hover:bg-muted/50 transition-colors disabled:opacity-30 disabled:pointer-events-none";

const ctrlBtnFlexCls =
	"flex items-center justify-center flex-1 h-7 text-muted-foreground " +
	"hover:text-foreground hover:bg-muted/50 transition-colors disabled:opacity-30 disabled:pointer-events-none";

function UndoRedoControls({
	canUndo,
	canRedo,
	onUndo,
	onRedo,
}: {
	canUndo: boolean;
	canRedo: boolean;
	onUndo: () => void;
	onRedo: () => void;
}) {
	return (
		<Panel position="bottom-left">
			<div className="flex items-center rounded-lg border border-border bg-card shadow-sm overflow-hidden h-7">
				<button
					type="button"
					onClick={onUndo}
					disabled={!canUndo}
					title="撤销 (⌘Z)"
					className={ctrlBtnCls}
				>
					<Undo2 className="size-3" />
				</button>
				<button
					type="button"
					onClick={onRedo}
					disabled={!canRedo}
					title="重做 (⌘⇧Z)"
					className={cn(ctrlBtnCls, "border-l border-border")}
				>
					<Redo2 className="size-3" />
				</button>
			</div>
		</Panel>
	);
}

function ZoomControls({
	showMiniMap,
	onToggleMiniMap,
}: {
	showMiniMap: boolean;
	onToggleMiniMap: () => void;
}) {
	const { zoomIn, zoomOut, fitView, zoomTo } = useReactFlow();
	const { zoom } = useViewport();

	return (
		<Panel position="bottom-right">
			<div className="flex items-center rounded-lg border border-border bg-card shadow-sm overflow-hidden h-7 w-[200px]">
				<button
					type="button"
					onClick={() => zoomOut({ duration: 200 })}
					title="缩小"
					className={ctrlBtnFlexCls}
				>
					<Minus className="size-3" />
				</button>
				<button
					type="button"
					onClick={() => zoomTo(1, { duration: 200 })}
					title="重置缩放"
					className="flex-1 h-full text-[11px] font-medium tabular-nums text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors border-x border-border text-center"
				>
					{Math.round(zoom * 100)}%
				</button>
				<button
					type="button"
					onClick={() => zoomIn({ duration: 200 })}
					title="放大"
					className={ctrlBtnFlexCls}
				>
					<Plus className="size-3" />
				</button>
				<button
					type="button"
					onClick={() => fitView({ padding: 0.2, duration: 300 })}
					title="适合视图"
					className={cn(ctrlBtnFlexCls, "border-l border-border")}
				>
					<Maximize2 className="size-3" />
				</button>
				<button
					type="button"
					onClick={onToggleMiniMap}
					title={showMiniMap ? "隐藏小地图" : "显示小地图"}
					className={cn(ctrlBtnFlexCls, "border-l border-border", showMiniMap && "text-primary")}
				>
					<Map className="size-3" />
				</button>
			</div>
		</Panel>
	);
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
		<Panel position="center-left">
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

	return (
		<div className="w-[420px] h-full bg-card border-l border-border/40 flex flex-col overflow-hidden">
			{/* Header: colored by node type, matches canvas card header */}
			<div
				className={cn(
					"flex items-center gap-3 px-4 py-3 border-b border-border/20",
					entry?.headerBg ?? "bg-muted/30",
				)}
			>
				{entry && <NodeIcon entry={entry} size="md" />}
				<div className="flex-1 min-w-0">
					<p
						className={cn(
							"text-[11px] font-semibold uppercase tracking-wider truncate",
							entry?.headerText ?? "text-foreground",
						)}
					>
						{entry?.label ?? node.type}
					</p>
					<p className="text-[10px] text-muted-foreground/60 mt-0.5 line-clamp-1 leading-tight">
						{entry?.description ?? ""}
					</p>
				</div>
				<button
					type="button"
					onClick={onClose}
					className="shrink-0 text-muted-foreground/60 hover:text-foreground transition-colors p-1.5 rounded-lg hover:bg-black/5 dark:hover:bg-white/10"
				>
					<X className="size-3.5" />
				</button>
			</div>

			<div className="overflow-y-auto flex-1">
				{node.type !== "start" && (
					<div className="px-4 pt-4 pb-3 border-b border-border/20">
						<Field label="节点名称">
							<TextInput
								value={(draft.title as string) ?? ""}
								onChange={(v) => patch({ title: v })}
								placeholder={entry?.label}
							/>
						</Field>
					</div>
				)}
				<div className="space-y-5 px-4 pt-4 pb-6">
					<NodeFields
						type={node.type as string}
						draft={draft}
						patch={patch}
					/>
				</div>
			</div>
		</div>
	);
}

// ── Form primitives ──────────────────────────────────────────────────────────

// Field — label in normal case matching Dify's panel style.
function Field({
	label,
	hint,
	children,
}: { label: string; hint?: string; children: React.ReactNode }) {
	return (
		<div className="flex flex-col gap-1.5">
			<div className="flex items-center justify-between min-h-5">
				<label className="text-[11px] font-medium text-foreground/60">
					{label}
				</label>
				{hint && (
					<span className="text-[10px] text-muted-foreground/50">
						{hint}
					</span>
				)}
			</div>
			{children}
		</div>
	);
}

// Section — groups related fields under a subtle divider header (Dify style).
function Section({
	title,
	children,
}: { title: string; children: React.ReactNode }) {
	return (
		<div className="flex flex-col gap-3">
			<div className="flex items-center gap-2 -mx-1">
				<span className="text-[11px] font-semibold text-muted-foreground/70 px-1 shrink-0">
					{title}
				</span>
				<div className="flex-1 h-px bg-border/50" />
			</div>
			{children}
		</div>
	);
}

// ── Base input class — h-8 (32px), matches Dify compact panel spec
const inputCls =
	"w-full h-8 px-3 text-xs border border-border/60 rounded-lg bg-background outline-none focus:border-primary/70 focus:ring-2 focus:ring-primary/10 transition-colors placeholder:text-muted-foreground/30";

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

// Textarea overrides h-8 back to auto; rows drives height
function TextareaInput({
	value,
	onChange,
	placeholder,
	rows = 5,
	mono = false,
}: {
	value: string;
	onChange: (v: string) => void;
	placeholder?: string;
	rows?: number;
	mono?: boolean;
}) {
	return (
		<textarea
			value={value}
			onChange={(e) => onChange(e.target.value)}
			placeholder={placeholder}
			rows={rows}
			className={cn(
				inputCls,
				"h-auto py-2.5 resize-y leading-relaxed",
				mono && "font-mono text-[11px]",
			)}
		/>
	);
}

// Custom select: appearance-none strips browser chrome; ChevronDown adds a
// consistent arrow that matches TextInput visually at exactly h-8.
type SelectOption = string | { value: string; label: string };

function SelectInput({
	value,
	onChange,
	options,
}: { value: string; onChange: (v: string) => void; options: SelectOption[] }) {
	return (
		<div className="relative">
			<select
				value={value}
				onChange={(e) => onChange(e.target.value)}
				className={cn(inputCls, "appearance-none cursor-pointer pr-7")}
			>
				{options.map((o) => {
					const val = typeof o === "string" ? o : o.value;
					const label = typeof o === "string" ? o : o.label;
					return (
						<option key={val} value={val}>
							{label}
						</option>
					);
				})}
			</select>
			<ChevronDown className="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 size-3 text-muted-foreground/60" />
		</div>
	);
}

function StringListInput({
	value,
	onChange,
	placeholder,
	addLabel = "添加",
}: { value: string[]; onChange: (v: string[]) => void; placeholder?: string; addLabel?: string }) {
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
						className="shrink-0 flex items-center justify-center h-8 w-8 text-muted-foreground hover:text-destructive transition-colors rounded-lg hover:bg-destructive/10"
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
				{addLabel}
			</button>
		</div>
	);
}

function NumberInput({
	value,
	onChange,
	placeholder,
	min,
	max,
	step,
}: {
	value: number | undefined;
	onChange: (v: number | undefined) => void;
	placeholder?: string;
	min?: number;
	max?: number;
	step?: number;
}) {
	return (
		<input
			type="number"
			value={value ?? ""}
			onChange={(e) =>
				onChange(e.target.value === "" ? undefined : Number(e.target.value))
			}
			placeholder={placeholder}
			min={min}
			max={max}
			step={step}
			className={inputCls}
		/>
	);
}

// ── LlmConnectionFields — collapsible API settings shared by LLM nodes ────────

function LlmConnectionFields({
	draft,
	patch,
}: { draft: Record<string, unknown>; patch: (p: Record<string, unknown>) => void }) {
	const [open, setOpen] = useState(false);
	return (
		<div className="rounded-xl border border-border/40 overflow-hidden">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className="flex items-center justify-between w-full px-3 py-2.5 bg-muted/20 hover:bg-muted/40 transition-colors"
			>
				<span className="text-[11px] font-semibold text-muted-foreground/70">连接设置</span>
				<ChevronDown
					className={cn(
						"size-3.5 text-muted-foreground/50 transition-transform duration-150",
						open ? "rotate-0" : "-rotate-90",
					)}
				/>
			</button>
			{open && (
				<div className="p-3 space-y-3 border-t border-border/30 bg-background/50">
					<Field label="API Base URL">
						<TextInput
							value={(draft.api_base as string) ?? "https://api.openai.com/v1"}
							onChange={(v) => patch({ api_base: v })}
							placeholder="https://api.openai.com/v1"
						/>
					</Field>
					<Field label="API Key">
						<input
							type="password"
							value={(draft.api_key as string) ?? ""}
							onChange={(e) => patch({ api_key: e.target.value })}
							placeholder="sk-..."
							className={inputCls}
						/>
					</Field>
					<Field label="Temperature">
						<NumberInput
							value={draft.temperature as number | undefined}
							onChange={(v) => patch({ temperature: v ?? 0.7 })}
							placeholder="0.7"
							min={0}
							max={2}
							step={0.1}
						/>
					</Field>
					<Field label="Max Tokens" hint="可选">
						<NumberInput
							value={draft.max_tokens as number | undefined}
							onChange={(v) => patch({ max_tokens: v })}
							placeholder="不限制"
							min={1}
						/>
					</Field>
				</div>
			)}
		</div>
	);
}

// ── IfElseBuilder — visual condition builder (Dify-style) ─────────────────────

interface Condition {
	variable: string;
	comparison_operator: string;
	value: string;
}

interface IfElseCase {
	id: string;
	logical_operator: "and" | "or";
	conditions: Condition[];
}

const COMPARISON_OPS: { value: string; label: string; noValue?: boolean }[] = [
	{ value: "contains", label: "包含" },
	{ value: "not-contains", label: "不包含" },
	{ value: "starts-with", label: "开始是" },
	{ value: "ends-with", label: "结束是" },
	{ value: "is", label: "是" },
	{ value: "is-not", label: "不是" },
	{ value: "is-empty", label: "为空", noValue: true },
	{ value: "is-not-empty", label: "不为空", noValue: true },
	{ value: "gt", label: ">" },
	{ value: "lt", label: "<" },
	{ value: "gte", label: "≥" },
	{ value: "lte", label: "≤" },
];

// Dify-style condition row:
//   Line 1: [variable chip input — full width]
//   Line 2: [operator dropdown]  [value input]  [remove ×]
function ConditionRow({
	condition,
	onChange,
	onRemove,
}: {
	condition: Condition;
	onChange: (c: Condition) => void;
	onRemove: () => void;
}) {
	const op = COMPARISON_OPS.find((o) => o.value === condition.comparison_operator);
	const noValue = op?.noValue ?? false;
	return (
		<div className="flex flex-col gap-1 rounded-lg bg-muted/30 p-2">
			{/* Row 1: variable reference — blue chip style */}
			<input
				type="text"
				value={condition.variable}
				onChange={(e) => onChange({ ...condition, variable: e.target.value })}
				placeholder="{{node_id.field}}"
				className={cn(
					inputCls,
					"font-mono text-[11px] bg-blue-50/60 dark:bg-blue-950/20 border-blue-200/60 dark:border-blue-800/40",
					"focus:border-blue-400/70 focus:ring-blue-400/10 placeholder:text-blue-400/40 text-blue-700 dark:text-blue-300",
				)}
			/>
			{/* Row 2: operator + value + remove */}
			<div className="flex items-center gap-1">
				<div className="relative shrink-0">
					<select
						value={condition.comparison_operator}
						onChange={(e) =>
							onChange({ ...condition, comparison_operator: e.target.value, value: "" })
						}
						className={cn(inputCls, "w-28 appearance-none cursor-pointer pr-6")}
					>
						{COMPARISON_OPS.map((o) => (
							<option key={o.value} value={o.value}>{o.label}</option>
						))}
					</select>
					<ChevronDown className="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2 size-2.5 text-muted-foreground/50" />
				</div>
				{!noValue && (
					<input
						type="text"
						value={condition.value}
						onChange={(e) => onChange({ ...condition, value: e.target.value })}
						placeholder="值"
						className={cn(inputCls, "flex-1 min-w-0")}
					/>
				)}
				<button
					type="button"
					onClick={onRemove}
					className="shrink-0 flex items-center justify-center h-8 w-8 text-muted-foreground hover:text-destructive transition-colors rounded-lg hover:bg-destructive/10"
				>
					<X className="size-3" />
				</button>
			</div>
		</div>
	);
}

function IfElseBuilder({
	value,
	onChange,
}: { value: IfElseCase[]; onChange: (v: IfElseCase[]) => void }) {
	const addCase = () =>
		onChange([
			...value,
			{
				id: `case_${value.length + 1}`,
				logical_operator: "and",
				conditions: [],
			},
		]);
	const removeCase = (i: number) =>
		onChange(value.filter((_, idx) => idx !== i));
	const updateCase = (i: number, patch: Partial<IfElseCase>) =>
		onChange(value.map((c, idx) => (idx === i ? { ...c, ...patch } : c)));

	const addCondition = (i: number) => {
		const newCond: Condition = { variable: "", comparison_operator: "contains", value: "" };
		updateCase(i, { conditions: [...(value[i].conditions ?? []), newCond] });
	};
	const updateCondition = (i: number, j: number, cond: Condition) => {
		const conditions = value[i].conditions.map((c, k) => (k === j ? cond : c));
		updateCase(i, { conditions });
	};
	const removeCondition = (i: number, j: number) => {
		updateCase(i, { conditions: value[i].conditions.filter((_, k) => k !== j) });
	};

	return (
		<div className="flex flex-col gap-2.5">
			{value.map((c, i) => (
				<div
					key={c.id}
					className="rounded-xl border border-border/50 bg-background overflow-hidden"
				>
					{/* Case header — blue accent for IF, gray for ELIF */}
					<div
						className={cn(
							"flex items-center justify-between px-3 py-2 border-b border-border/30",
							i === 0 ? "bg-blue-50/60 dark:bg-blue-950/20" : "bg-muted/30",
						)}
					>
						<span
							className={cn(
								"text-[11px] font-semibold",
								i === 0 ? "text-blue-600 dark:text-blue-400" : "text-muted-foreground",
							)}
						>
							{i === 0 ? "IF" : `ELIF ${i}`}
						</span>
						<div className="flex items-center gap-2">
							{/* AND / OR toggle */}
							<div className="flex rounded-md overflow-hidden border border-border/40 text-[10px] font-bold">
								{(["and", "or"] as const).map((op) => (
									<button
										key={op}
										type="button"
										onClick={() => updateCase(i, { logical_operator: op })}
										className={cn(
											"px-2.5 py-1 transition-colors",
											c.logical_operator === op
												? "bg-primary text-primary-foreground"
												: "text-muted-foreground hover:bg-muted/60",
										)}
									>
										{op.toUpperCase()}
									</button>
								))}
							</div>
							{value.length > 1 && (
								<button
									type="button"
									onClick={() => removeCase(i)}
									className="p-0.5 text-muted-foreground/60 hover:text-destructive transition-colors rounded"
								>
									<X className="size-3" />
								</button>
							)}
						</div>
					</div>
					{/* Condition rows */}
					<div className="p-2.5 space-y-2">
						{c.conditions.map((cond, j) => (
							// biome-ignore lint/suspicious/noArrayIndexKey: ordered list
							<ConditionRow
								key={j}
								condition={cond}
								onChange={(updated) => updateCondition(i, j, updated)}
								onRemove={() => removeCondition(i, j)}
							/>
						))}
						<button
							type="button"
							onClick={() => addCondition(i)}
							className="flex items-center gap-1 text-[11px] text-primary/70 hover:text-primary transition-colors"
						>
							<Plus className="size-3" />
							添加条件
						</button>
					</div>
				</div>
			))}

			{/* ELSE — always exists, no conditions needed */}
			<div className="rounded-xl border border-dashed border-border/40 bg-muted/10 px-3 py-2.5">
				<span className="text-[11px] font-semibold text-muted-foreground/40 uppercase tracking-wide">ELSE</span>
				<p className="text-[10px] text-muted-foreground/30 mt-0.5">以上条件均不满足时执行此分支</p>
			</div>

			<button
				type="button"
				onClick={addCase}
				className="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground hover:text-primary transition-colors"
			>
				<Plus className="size-3" />
				添加条件分支
			</button>
		</div>
	);
}

// ── ClassesEditor (question-classifier) ──────────────────────────────────────

function ClassesEditor({
	value,
	onChange,
}: { value: ClassItem[]; onChange: (v: ClassItem[]) => void }) {
	const update = (i: number, patch: Partial<ClassItem>) => {
		const next = value.map((c, idx) => (idx === i ? { ...c, ...patch } : c));
		onChange(next);
	};
	const remove = (i: number) => onChange(value.filter((_, idx) => idx !== i));
	const add = () =>
		onChange([
			...value,
			{ id: `class_${Date.now()}`, name: "", description: "" },
		]);

	return (
		<Field label="分类" hint="至少 2 个">
			<div className="flex flex-col gap-1.5">
				{value.map((cls, i) => (
					// biome-ignore lint/suspicious/noArrayIndexKey: positional list
					<div key={i} className="flex items-center gap-1.5">
						<input
							type="text"
							value={cls.name || cls.id}
							onChange={(e) =>
								update(i, { name: e.target.value, id: e.target.value.toLowerCase().replace(/\s+/g, "_") || cls.id })
							}
							placeholder={`类别 ${i + 1}`}
							className={cn(inputCls, "flex-1")}
						/>
						<button
							type="button"
							onClick={() => remove(i)}
							className="shrink-0 flex items-center justify-center h-8 w-8 text-muted-foreground hover:text-destructive transition-colors rounded-lg hover:bg-destructive/10"
						>
							<Trash2 className="size-3" />
						</button>
					</div>
				))}
				<button
					type="button"
					onClick={add}
					className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-primary transition-colors"
				>
					<Plus className="size-3" />
					添加类别
				</button>
			</div>
		</Field>
	);
}

// ── ParametersEditor (parameter-extractor) ───────────────────────────────────

interface ParamItem {
	name: string;
	type: string;
	description?: string;
	required?: boolean;
}

const PARAM_TYPES = ["string", "number", "boolean", "object", "array"];

function ParametersEditor({
	value,
	onChange,
}: { value: ParamItem[]; onChange: (v: ParamItem[]) => void }) {
	const update = (i: number, p: Partial<ParamItem>) => {
		onChange(value.map((item, idx) => (idx === i ? { ...item, ...p } : item)));
	};
	const remove = (i: number) => onChange(value.filter((_, idx) => idx !== i));
	const add = () =>
		onChange([
			...value,
			{ name: "", type: "string", description: "", required: false },
		]);

	return (
		<Field label="提取参数" hint="至少 1 个">
			<div className="flex flex-col gap-2">
				{value.map((param, i) => (
					// biome-ignore lint/suspicious/noArrayIndexKey: positional list
					<div
						key={i}
						className="rounded-lg border border-border/40 p-2.5 flex flex-col gap-1.5 bg-muted/10"
					>
						<div className="flex items-center gap-1.5">
							<input
								type="text"
								value={param.name}
								onChange={(e) => update(i, { name: e.target.value })}
								placeholder="参数名"
								className={cn(inputCls, "flex-1")}
							/>
							<div className="relative shrink-0">
								<select
									value={param.type}
									onChange={(e) => update(i, { type: e.target.value })}
									className={cn(inputCls, "w-24 appearance-none cursor-pointer pr-6")}
								>
									{PARAM_TYPES.map((t) => (
										<option key={t} value={t}>{t}</option>
									))}
								</select>
								<ChevronDown className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 size-2.5 text-muted-foreground/50" />
							</div>
							<button
								type="button"
								onClick={() => remove(i)}
								className="shrink-0 flex items-center justify-center h-8 w-8 text-muted-foreground hover:text-destructive transition-colors rounded-lg hover:bg-destructive/10"
							>
								<Trash2 className="size-3" />
							</button>
						</div>
						<input
							type="text"
							value={param.description ?? ""}
							onChange={(e) => update(i, { description: e.target.value })}
							placeholder="描述（帮助 LLM 理解此参数）"
							className={inputCls}
						/>
						<label className="flex items-center gap-2 text-[11px] text-muted-foreground cursor-pointer select-none">
							<input
								type="checkbox"
								checked={param.required ?? false}
								onChange={(e) => update(i, { required: e.target.checked })}
								className="rounded"
							/>
							必填
						</label>
					</div>
				))}
				<button
					type="button"
					onClick={add}
					className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-primary transition-colors"
				>
					<Plus className="size-3" />
					添加参数
				</button>
			</div>
		</Field>
	);
}

// ── AssignsEditor (assign node) ───────────────────────────────────────────────

function AssignsEditor({
	value,
	onChange,
}: {
	value: Record<string, string>;
	onChange: (v: Record<string, string>) => void;
}) {
	const entries = Object.entries(value);
	const set = (key: string, val: string) => onChange({ ...value, [key]: val });
	const rename = (oldKey: string, newKey: string) => {
		const next: Record<string, string> = {};
		for (const [k, v] of Object.entries(value)) {
			next[k === oldKey ? newKey : k] = v;
		}
		onChange(next);
	};
	const remove = (key: string) => {
		const next = { ...value };
		delete next[key];
		onChange(next);
	};
	const add = () => {
		const key = `var_${entries.length + 1}`;
		onChange({ ...value, [key]: "" });
	};

	return (
		<Field label="赋值列表">
			<div className="flex flex-col gap-2">
				{entries.map(([key, val]) => (
					<div
						key={key}
						className="flex flex-col gap-1.5 rounded-lg border border-border/40 p-2.5 bg-muted/10"
					>
						<div className="flex items-center gap-1.5">
							<input
								type="text"
								value={key}
								onChange={(e) => rename(key, e.target.value)}
								placeholder="变量名"
								className={cn(inputCls, "flex-1 font-mono text-[11px]")}
							/>
							<button
								type="button"
								onClick={() => remove(key)}
								className="shrink-0 flex items-center justify-center h-8 w-8 text-muted-foreground hover:text-destructive transition-colors rounded-lg hover:bg-destructive/10"
							>
								<Trash2 className="size-3" />
							</button>
						</div>
						<input
							type="text"
							value={val}
							onChange={(e) => set(key, e.target.value)}
							placeholder="值或 Jinja2 模板，如 {{ node_id.field }}"
							className={inputCls}
						/>
					</div>
				))}
				<button
					type="button"
					onClick={add}
					className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-primary transition-colors"
				>
					<Plus className="size-3" />
					添加变量
				</button>
			</div>
		</Field>
	);
}

// ── StartInputsEditor ─────────────────────────────────────────────────────────

interface InputDecl {
	name: string;
	type: string;
	default?: string;
}

const VAR_TYPES = ["string", "number", "bool", "object", "array"];

function StartInputsEditor({
	value,
	onChange,
}: { value: InputDecl[]; onChange: (v: InputDecl[]) => void }) {
	const update = (i: number, p: Partial<InputDecl>) => {
		onChange(value.map((item, idx) => (idx === i ? { ...item, ...p } : item)));
	};
	const remove = (i: number) => onChange(value.filter((_, idx) => idx !== i));
	const add = () =>
		onChange([...value, { name: "", type: "string", default: "" }]);

	return (
		<div className="flex flex-col gap-2">
			{value.map((decl, i) => (
				// biome-ignore lint/suspicious/noArrayIndexKey: positional list
				<div
					key={i}
					className="rounded-lg border border-border/40 p-2.5 flex flex-col gap-1.5 bg-muted/10"
				>
					<div className="flex items-center gap-1.5">
						<input
							type="text"
							value={decl.name}
							onChange={(e) => update(i, { name: e.target.value })}
							placeholder="变量名"
							className={cn(inputCls, "flex-1 font-mono")}
						/>
						<div className="relative shrink-0">
							<select
								value={decl.type}
								onChange={(e) => update(i, { type: e.target.value })}
								className={cn(inputCls, "w-24 appearance-none cursor-pointer pr-6")}
							>
								{VAR_TYPES.map((t) => (
									<option key={t} value={t}>{t}</option>
								))}
							</select>
							<ChevronDown className="pointer-events-none absolute right-1.5 top-1/2 -translate-y-1/2 size-2.5 text-muted-foreground/50" />
						</div>
						<button
							type="button"
							onClick={() => remove(i)}
							className="shrink-0 flex items-center justify-center h-8 w-8 text-muted-foreground hover:text-destructive transition-colors rounded-lg hover:bg-destructive/10"
						>
							<Trash2 className="size-3" />
						</button>
					</div>
					<input
						type="text"
						value={decl.default ?? ""}
						onChange={(e) => update(i, { default: e.target.value })}
						placeholder="默认值（可选）"
						className={inputCls}
					/>
				</div>
			))}
			<button
				type="button"
				onClick={add}
				className="flex items-center gap-1.5 text-[11px] text-muted-foreground hover:text-primary transition-colors"
			>
				<Plus className="size-3" />
				添加变量
			</button>
		</div>
	);
}

const HTTP_METHODS = ["GET", "POST", "PUT", "DELETE", "PATCH"];

// ── NodeFields — per-type config fields, Dify-aligned ─────────────────────────

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
		// ── 开始：输入字段声明 ───────────────────────────────────────────────
		case "start":
			return (
				<Section title="输入字段">
					<StartInputsEditor
						value={(draft.inputs as InputDecl[]) ?? []}
						onChange={(v) => patch({ inputs: v })}
					/>
				</Section>
			);

		// ── HTTP 请求 ────────────────────────────────────────────────────────
		case "http-request":
			return (
				<>
					<Section title="请求">
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
					</Section>
					<Section title="请求头">
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
						mono
						/>
					</Section>
					{["POST", "PUT", "PATCH"].includes(
						(draft.method as string) ?? "GET",
					) && (
						<Section title="请求体">
							<TextareaInput
								value={(draft.body as string) ?? ""}
								onChange={(v) => patch({ body: v })}
								placeholder={'{\n  "key": "{{ start.field }}"\n}'}
								rows={4}
							mono
							/>
						</Section>
					)}
				</>
			);

		// ── IF/ELSE：可视化条件构建器 ─────────────────────────────────────────
		case "if-else":
			return (
				<Section title="条件">
					<IfElseBuilder
						value={
							(draft.cases as IfElseCase[]) ?? [
								{ id: "case_1", logical_operator: "and", conditions: [] },
							]
						}
						onChange={(v) => patch({ cases: v })}
					/>
				</Section>
			);

		// ── 代码 ─────────────────────────────────────────────────────────────
		case "code":
			return (
				<Section title="Rhai 脚本">
					<TextareaInput
						value={(draft.script as string) ?? ""}
						onChange={(v) => patch({ script: v })}
						placeholder={"// 通过 inputs 访问上游节点输出\n// 返回值即节点输出\ninputs"}
						rows={14}
						mono
					/>
				</Section>
			);

		// ── 模板 ─────────────────────────────────────────────────────────────
		case "template-transform":
			return (
				<Section title="Jinja2 模板">
					<TextareaInput
						value={(draft.template as string) ?? ""}
						onChange={(v) => patch({ template: v })}
						placeholder={"Hello {{ inputs.node_id.field }}!"}
						rows={12}
						mono
					/>
				</Section>
			);

		// ── 迭代 ─────────────────────────────────────────────────────────────
		case "iteration":
			return (
				<>
					<Section title="数据">
						<Field label="输入数组">
							<TextInput
								value={(draft.input_selector as string) ?? ""}
								onChange={(v) => patch({ input_selector: v })}
								placeholder="node_id.items"
							/>
						</Field>
						<Field label="输出收集">
							<TextInput
								value={(draft.output_selector as string) ?? ""}
								onChange={(v) => patch({ output_selector: v })}
								placeholder="summarize.output"
							/>
						</Field>
					</Section>
					<Section title="设置">
						<Field label="执行模式">
							<SelectInput
								value={(draft.mode as string) ?? "parallel"}
								onChange={(v) => patch({ mode: v })}
								options={[{ value: "parallel", label: "并行" }, { value: "sequential", label: "顺序" }]}
							/>
						</Field>
					</Section>
				</>
			);

		// ── 循环 ─────────────────────────────────────────────────────────────
		case "loop":
			return (
				<>
					<Section title="数据">
						<Field label="输出收集">
							<TextInput
								value={(draft.output_selector as string) ?? ""}
								onChange={(v) => patch({ output_selector: v })}
								placeholder="step.result"
							/>
						</Field>
					</Section>
					<Section title="设置">
						<Field label="最大迭代次数">
							<NumberInput
								value={(draft.max_iterations as number) ?? 10}
								onChange={(v) => patch({ max_iterations: v ?? 10 })}
								min={1}
								max={1000}
							/>
						</Field>
					</Section>
				</>
			);

		// ── 变量聚合 ──────────────────────────────────────────────────────────
		case "variable-aggregator":
			return (
				<Section title="来源变量">
					<StringListInput
						value={(draft.sources as string[]) ?? []}
						onChange={(v) => patch({ sources: v })}
						placeholder="node_id.output"
						addLabel="添加来源"
					/>
				</Section>
			);

		// ── LLM ─────────────────────────────────────────────────────────────
		case "llm":
			return (
				<>
					<Section title="模型">
						<Field label="模型名称">
							<TextInput
								value={(draft.model as string) ?? ""}
								onChange={(v) => patch({ model: v })}
								placeholder="gpt-4o-mini"
							/>
						</Field>
					</Section>
					<Section title="提示词">
						<Field label="系统提示词" hint="可选">
							<TextareaInput
								value={(draft.system_prompt as string) ?? ""}
								onChange={(v) => patch({ system_prompt: v })}
								placeholder="You are a helpful assistant."
								rows={4}
							/>
						</Field>
						<Field label="用户提示词">
							<TextareaInput
								value={(draft.user_prompt as string) ?? ""}
								onChange={(v) => patch({ user_prompt: v })}
								placeholder={"根据以下内容作答：\n{{ start.query }}"}
								rows={6}
							/>
						</Field>
					</Section>
					<LlmConnectionFields draft={draft} patch={patch} />
				</>
			);

		// ── 问题分类 ──────────────────────────────────────────────────────────
		case "question-classifier":
			return (
				<>
					<Section title="输入">
						<Field label="查询变量">
							<TextareaInput
								value={(draft.question as string) ?? ""}
								onChange={(v) => patch({ question: v })}
								placeholder="{{ start.user_input }}"
								rows={3}
							/>
						</Field>
					</Section>
					<Section title="模型">
						<Field label="模型名称">
							<TextInput
								value={(draft.model as string) ?? ""}
								onChange={(v) => patch({ model: v })}
								placeholder="gpt-4o-mini"
							/>
						</Field>
					</Section>
					<Section title="分类">
						<ClassesEditor
							value={(draft.classes as ClassItem[]) ?? []}
							onChange={(v) => patch({ classes: v })}
						/>
					</Section>
					<LlmConnectionFields draft={draft} patch={patch} />
				</>
			);

		// ── 参数提取 ──────────────────────────────────────────────────────────
		case "parameter-extractor":
			return (
				<>
					<Section title="输入">
						<Field label="查询文本">
							<TextareaInput
								value={(draft.query as string) ?? ""}
								onChange={(v) => patch({ query: v })}
								placeholder="{{ start.user_input }}"
								rows={3}
							/>
						</Field>
					</Section>
					<Section title="模型">
						<Field label="模型名称">
							<TextInput
								value={(draft.model as string) ?? ""}
								onChange={(v) => patch({ model: v })}
								placeholder="gpt-4o-mini"
							/>
						</Field>
					</Section>
					<Section title="参数">
						<ParametersEditor
							value={(draft.parameters as ParamItem[]) ?? []}
							onChange={(v) => patch({ parameters: v })}
						/>
					</Section>
					<LlmConnectionFields draft={draft} patch={patch} />
				</>
			);

		// ── 变量赋值 ──────────────────────────────────────────────────────────
		case "assign":
			return (
				<Section title="赋值">
					<AssignsEditor
						value={(draft.assigns as Record<string, string>) ?? {}}
						onChange={(v) => patch({ assigns: v })}
					/>
				</Section>
			);

		// ── 列表操作 ──────────────────────────────────────────────────────────
		case "list-operator":
			return (
				<>
					<Section title="数据">
						<Field label="输入数组">
							<TextInput
								value={(draft.input_selector as string) ?? ""}
								onChange={(v) => patch({ input_selector: v })}
								placeholder="node_id.items"
							/>
						</Field>
					</Section>
					<Section title="操作">
						<Field label="排序字段" hint="可选">
							<TextInput
								value={(draft.sort_by as string) ?? ""}
								onChange={(v) => patch({ sort_by: v })}
								placeholder="name"
							/>
						</Field>
						<Field label="排序方向">
							<SelectInput
								value={(draft.sort_order as string) ?? "asc"}
								onChange={(v) => patch({ sort_order: v })}
								options={[{ value: "asc", label: "升序" }, { value: "desc", label: "降序" }]}
							/>
						</Field>
						<Field label="去重字段" hint="可选">
							<TextInput
								value={(draft.deduplicate_by as string) ?? ""}
								onChange={(v) => patch({ deduplicate_by: v })}
								placeholder="id"
							/>
						</Field>
						<Field label="限制数量" hint="可选">
							<NumberInput
								value={draft.limit as number | undefined}
								onChange={(v) => patch({ limit: v })}
								placeholder="不限制"
								min={1}
							/>
						</Field>
					</Section>
				</>
			);

		// ── MCP 工具 ──────────────────────────────────────────────────────────
		case "mcp":
			return (
				<>
					<Section title="服务器">
						<Field label="服务器 URL" hint="SSE 端点">
							<TextInput
								value={(draft.server_url as string) ?? ""}
								onChange={(v) => patch({ server_url: v })}
								placeholder="http://localhost:3000/sse"
							/>
						</Field>
					</Section>
					<Section title="工具">
						<Field label="工具名称">
							<TextInput
								value={(draft.tool_name as string) ?? ""}
								onChange={(v) => patch({ tool_name: v })}
								placeholder="search"
							/>
						</Field>
						<Field label="参数" hint="JSON">
							<TextareaInput
								value={
									typeof draft.arguments === "object"
										? JSON.stringify(draft.arguments, null, 2)
										: ((draft.arguments as string) ?? "")
								}
								onChange={(v) => {
									try {
										patch({ arguments: JSON.parse(v) });
									} catch {
										patch({ arguments: v });
									}
								}}
								placeholder={'{\n  "query": "{{ start.query }}"\n}'}
								rows={5}
							/>
						</Field>
					</Section>
				</>
			);

		// ── 合并：无需配置 ────────────────────────────────────────────────────
		case "noop":
			return (
				<p className="text-xs text-muted-foreground/60 text-center py-6">
					此节点透传所有上游输出，无需配置。
				</p>
			);

		default:
			return null;
	}
}

// =============================================================================
// Node context menu (right-click, Dify-style)
// =============================================================================

interface ContextMenuState {
	x: number;
	y: number;
	nodeId: string;
}

function NodeContextMenu({
	state,
	onClose,
	onDuplicate,
	onDelete,
}: {
	state: ContextMenuState;
	onClose: () => void;
	onDuplicate: (nodeId: string) => void;
	onDelete: (nodeId: string) => void;
}) {
	const menuRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		function onKey(e: KeyboardEvent) {
			if (e.key === "Escape") onClose();
		}
		function onMouseDown(e: MouseEvent) {
			if (menuRef.current && !menuRef.current.contains(e.target as globalThis.Node)) {
				onClose();
			}
		}
		document.addEventListener("keydown", onKey);
		document.addEventListener("mousedown", onMouseDown);
		return () => {
			document.removeEventListener("keydown", onKey);
			document.removeEventListener("mousedown", onMouseDown);
		};
	}, [onClose]);

	const items: Array<
		| { icon: LucideIcon; label: string; shortcut: string; action: () => void; danger: boolean }
		| null
	> = [
		{
			icon: Copy,
			label: "复制",
			shortcut: "⌘C",
			action: () => { onDuplicate(state.nodeId); onClose(); },
			danger: false,
		},
		{
			icon: CopyPlus,
			label: "复制节点",
			shortcut: "⌘D",
			action: () => { onDuplicate(state.nodeId); onClose(); },
			danger: false,
		},
		null,
		{
			icon: Trash2,
			label: "删除",
			shortcut: "⌘⌫",
			action: () => { onDelete(state.nodeId); onClose(); },
			danger: true,
		},
	];

	return (
		<div
			ref={menuRef}
			style={{ position: "fixed", left: state.x, top: state.y, zIndex: 1000 }}
			className="w-[200px] rounded-lg border border-border/60 bg-card shadow-xl py-1"
		>
			{items.map((item, i) =>
				item === null ? (
					// biome-ignore lint/suspicious/noArrayIndexKey: static divider
					<div key={i} className="my-1 h-px bg-border/50" />
				) : (
					<button
						// biome-ignore lint/suspicious/noArrayIndexKey: static menu item
						key={i}
						type="button"
						onClick={item.action}
						className={cn(
							"flex w-full items-center justify-between px-3 py-1.5 text-xs transition-colors",
							item.danger
								? "text-destructive hover:bg-destructive/10"
								: "text-foreground hover:bg-muted",
						)}
					>
						<span className="flex items-center gap-2">
							<item.icon className="size-3.5" />
							{item.label}
						</span>
						<span className="text-muted-foreground/60 font-mono">{item.shortcut}</span>
					</button>
				),
			)}
		</div>
	);
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
];
const DEFAULT_EDGES: Edge[] = [];

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
				...(n.parentId ? { parentId: n.parentId as string } : {}),
				...(n.extent ? { extent: n.extent as "parent" } : {}),
				...(n.style ? { style: n.style as React.CSSProperties } : {}),
			})),
			edges: (rawEdges as Array<Record<string, unknown>>).map((e, i) => ({
				id: (e.id as string) ?? `e-${i}`,
				source: e.sourceNodeID as string,
				target: e.targetNodeID as string,
			})),
		};
	}
	// Plain React Flow format — preserve parentId, extent, style if present
	return {
		nodes: (rawNodes as Array<Record<string, unknown>>).map((n) => ({
			...(n as Node),
			...(n.parentId ? { parentId: n.parentId as string } : {}),
			...(n.extent ? { extent: n.extent as "parent" } : {}),
		})),
		edges: rawEdges as Edge[],
	};
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
	const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
	const [showMiniMap, setShowMiniMap] = useState(true);

	// ── Undo / Redo history ───────────────────────────────────────────────────
	type Snapshot = { nodes: Node[]; edges: Edge[] };
	const historyRef = useRef<Snapshot[]>([]);
	const historyIdxRef = useRef<number>(-1);
	const skipHistoryRef = useRef(false); // prevents recording during undo/redo restore

	// Call this before any intentional mutation to record a snapshot.
	const pushHistory = useCallback(() => {
		if (skipHistoryRef.current) return;
		const snap: Snapshot = { nodes: nodesRef.current, edges: edgesRef.current };
		// Drop any redo future when a new action is made
		historyRef.current = historyRef.current.slice(0, historyIdxRef.current + 1);
		historyRef.current.push(snap);
		// Cap history at 100 entries
		if (historyRef.current.length > 100) {
			historyRef.current.shift();
		}
		historyIdxRef.current = historyRef.current.length - 1;
	}, []);

	const [historyVersion, setHistoryVersion] = useState(0); // triggers re-render for button state
	const canUndo = historyIdxRef.current > 0;
	const canRedo = historyIdxRef.current < historyRef.current.length - 1;

	const selectedNode = selectedNodeId
		? nodes.find((n) => n.id === selectedNodeId)
		: null;

	const onConnect: OnConnect = useCallback(
		(c) => setEdges((eds) => addEdge(c, eds)),
		[setEdges],
	);

	// Dify connection rules:
	//   1. No self-loops
	//   2. Each (source, sourceHandle) pair may have at most one outgoing edge —
	//      enforces that IF/ELSE and QC branches each route to a single next node.
	//   3. A target handle may receive unlimited incoming edges (fan-in is valid).
	const isValidConnection = useCallback(
		(connection: { source: string | null; sourceHandle?: string | null; target: string | null; targetHandle?: string | null }) => {
			if (!connection.source || !connection.target) return false;
			if (connection.source === connection.target) return false;
			return !edges.some(
				(e) =>
					e.source === connection.source &&
					(e.sourceHandle ?? null) === (connection.sourceHandle ?? null),
			);
		},
		[edges],
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

	const undo = useCallback(() => {
		if (historyIdxRef.current <= 0) return;
		historyIdxRef.current -= 1;
		const snap = historyRef.current[historyIdxRef.current];
		skipHistoryRef.current = true;
		setNodes(snap.nodes);
		setEdges(snap.edges);
		skipHistoryRef.current = false;
		setHistoryVersion((v) => v + 1);
		triggerSave();
	}, [setNodes, setEdges, triggerSave]);

	const redo = useCallback(() => {
		if (historyIdxRef.current >= historyRef.current.length - 1) return;
		historyIdxRef.current += 1;
		const snap = historyRef.current[historyIdxRef.current];
		skipHistoryRef.current = true;
		setNodes(snap.nodes);
		setEdges(snap.edges);
		skipHistoryRef.current = false;
		setHistoryVersion((v) => v + 1);
		triggerSave();
	}, [setNodes, setEdges, triggerSave]);

	// Keyboard shortcuts: ⌘Z / ⌘⇧Z
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if (!(e.metaKey || e.ctrlKey)) return;
			if (e.key === "z" && !e.shiftKey) { e.preventDefault(); undo(); }
			if ((e.key === "z" && e.shiftKey) || e.key === "y") { e.preventDefault(); redo(); }
		};
		window.addEventListener("keydown", handler);
		return () => window.removeEventListener("keydown", handler);
	}, [undo, redo]);


	const updateNodeData = useCallback(
		(id: string, data: Record<string, unknown>) => {
			setNodes((nds) => nds.map((n) => (n.id === id ? { ...n, data } : n)));
			triggerSave();
		},
		[setNodes, triggerSave],
	);

	const addNode = useCallback(
		(type: string, data: Record<string, unknown>) => {
			const isContainer = type === "iteration" || type === "loop";
			pushHistory();
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
					// Container nodes need explicit dimensions so child nodes
					// have a defined parent bounding box from creation.
					...(isContainer
						? { style: { width: CONTAINER_W, minHeight: CONTAINER_H } }
						: {}),
				},
			]);
			setHistoryVersion((v) => v + 1);
		},
		[setNodes, pushHistory],
	);

	const duplicateNode = useCallback(
		(nodeId: string) => {
			const node = nodes.find((n) => n.id === nodeId);
			if (!node) return;
			pushHistory();
			setNodes((nds) => [
				...nds,
				{
					...node,
					id: `${node.type}_${Date.now()}`,
					position: { x: node.position.x + 30, y: node.position.y + 30 },
					selected: false,
				},
			]);
			triggerSave();
			setHistoryVersion((v) => v + 1);
		},
		[nodes, setNodes, triggerSave, pushHistory],
	);

	const deleteNode = useCallback(
		(nodeId: string) => {
			pushHistory();
			setNodes((nds) => nds.filter((n) => n.id !== nodeId));
			setEdges((eds) => eds.filter((e) => e.source !== nodeId && e.target !== nodeId));
			if (selectedNodeId === nodeId) setSelectedNodeId(null);
			triggerSave();
			setHistoryVersion((v) => v + 1);
		},
		[setNodes, setEdges, selectedNodeId, triggerSave, pushHistory],
	);

	const handleLayout = useCallback(
		(laid: Node[]) => {
			pushHistory();
			setNodes(laid);
			triggerSave();
			setHistoryVersion((v) => v + 1);
		},
		[setNodes, triggerSave, pushHistory],
	);

	return (
		<div className="relative h-full w-full">
			<div id="workflow-container" className="h-full w-full">
				<ReactFlow
					nodes={nodes}
					edges={edges}
					onNodesChange={(c) => {
						const structural = c.some((ch) => ch.type === "add" || ch.type === "remove");
						if (structural) { pushHistory(); setHistoryVersion((v) => v + 1); }
						onNodesChange(c);
						triggerSave();
					}}
					onEdgesChange={(c) => {
						const structural = c.some((ch) => ch.type === "add" || ch.type === "remove");
						if (structural) { pushHistory(); setHistoryVersion((v) => v + 1); }
						onEdgesChange(c);
						triggerSave();
					}}
					onConnect={(c) => {
						pushHistory();
						onConnect(c);
						triggerSave();
						setHistoryVersion((v) => v + 1);
					}}
					onNodeClick={(_, node) => setSelectedNodeId(node.id)}
					onNodeContextMenu={(e, node) => {
						e.preventDefault();
						setContextMenu({ x: e.clientX, y: e.clientY, nodeId: node.id });
					}}
					onPaneClick={() => { setSelectedNodeId(null); setContextMenu(null); }}
					nodeTypes={NODE_TYPES}
					edgeTypes={EDGE_TYPES}
					connectionLineComponent={CustomConnectionLine}
					isValidConnection={isValidConnection}
					fitView
					proOptions={{ hideAttribution: true }}
				>
					<Background variant={BackgroundVariant.Dots} gap={20} size={1} />
					<UndoRedoControls
						canUndo={canUndo}
						canRedo={canRedo}
						onUndo={undo}
						onRedo={redo}
					/>
					<ZoomControls showMiniMap={showMiniMap} onToggleMiniMap={() => setShowMiniMap(v => !v)} />
					{showMiniMap && (
						<MiniMap
							zoomable
							pannable
							nodeStrokeWidth={2}
							position="bottom-right"
							style={{ width: 200, height: 110, bottom: 36 }}
						/>
					)}

					{/* Left toolbar + catalog popover */}
					<CanvasToolbar
						nodes={nodes}
						edges={edges}
						onAddNode={addNode}
						onSetNodes={handleLayout}
					/>
				</ReactFlow>
			</div>

			{/* Dify-style node config panel: slides in from right, overlays canvas */}
			<div
				className={cn(
					"absolute top-0 right-0 h-full z-20",
					"transition-transform duration-200 ease-out",
					selectedNode ? "translate-x-0" : "translate-x-full",
				)}
				style={{ pointerEvents: selectedNode ? "auto" : "none" }}
			>
				{selectedNode && (
					<NodeConfigPanel
						key={selectedNode.id}
						node={selectedNode}
						onUpdate={(data) => updateNodeData(selectedNode.id, data)}
						onClose={() => setSelectedNodeId(null)}
					/>
				)}
			</div>

			{contextMenu && (
				<NodeContextMenu
					state={contextMenu}
					onClose={() => setContextMenu(null)}
					onDuplicate={duplicateNode}
					onDelete={deleteNode}
				/>
			)}
		</div>
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

			{/* Chat (left) + Canvas (right) */}
			<ResizablePanelGroup direction="horizontal" className="flex-1 min-h-0">
				<ResizablePanel defaultSize={20} minSize={15} maxSize={45}>
					<div className="h-full border-r bg-background">
						<WorkflowChatPanel workflowId={wf.id} />
					</div>
				</ResizablePanel>
				<RHandle withHandle />
				<ResizablePanel defaultSize={80} minSize={40}>
					<FlowCanvas
						key={wf.id}
						wf={wf}
						onSavingChange={setSaving}
						onSavedChange={setSaved}
					/>
				</ResizablePanel>
			</ResizablePanelGroup>
		</div>
	);
}
