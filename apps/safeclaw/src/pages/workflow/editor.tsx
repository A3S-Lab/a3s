import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
	FreeLayoutEditorProvider,
	EditorRenderer,
	WorkflowNodeRenderer,
	useNodeRender,
	usePlaygroundTools,
	useClientContext,
	type FreeLayoutProps,
	type FreeLayoutPluginContext,
	type WorkflowNodeRegistry,
	type WorkflowNodeEntity,
} from "@flowgram.ai/free-layout-editor";
import "@flowgram.ai/free-layout-editor/index.css";
import { createFreeLinesPlugin } from "@flowgram.ai/free-lines-plugin";
import { createFreeSnapPlugin } from "@flowgram.ai/free-snap-plugin";
import { createMinimapPlugin } from "@flowgram.ai/minimap-plugin";
import { createFreeStackPlugin } from "@flowgram.ai/free-stack-plugin";
import { createFreeNodePanelPlugin } from "@flowgram.ai/free-node-panel-plugin";
import {
	ArrowLeft,
	Save,
	Loader2,
	Play,
	Zap,
	Square,
	ZoomIn,
	ZoomOut,
	Maximize2,
	Plus,
	GitBranch,
	Code2,
	MessageSquare,
	SplitSquareHorizontal,
} from "lucide-react";
import { cn } from "@/lib/utils";
import workflowModel from "@/models/workflow.model";

// ── BaseNode — official pattern: renderDefaultNode handles outer shell + ports ─

function BaseNode({ node }: { node: WorkflowNodeEntity }) {
	const nodeRender = useNodeRender();
	return (
		<WorkflowNodeRenderer node={node}>
			{nodeRender.form?.render()}
		</WorkflowNodeRenderer>
	);
}

// ── Node inner content (no WorkflowNodeRenderer — BaseNode owns that) ─────────

function StartNodeContent() {
	const { data } = useNodeRender();
	return (
		<div className="flex items-center gap-2 px-4 py-2 bg-emerald-500 text-white text-sm font-medium rounded-full shadow-sm min-w-[80px] justify-center">
			<Play className="size-3 fill-white" />
			<span>{(data as { title?: string })?.title ?? "开始"}</span>
		</div>
	);
}

function EndNodeContent() {
	const { data } = useNodeRender();
	return (
		<div className="flex items-center gap-2 px-4 py-2 bg-slate-600 text-white text-sm font-medium rounded-full shadow-sm min-w-[80px] justify-center">
			<Square className="size-3 fill-white" />
			<span>{(data as { title?: string })?.title ?? "结束"}</span>
		</div>
	);
}

function LLMNodeContent() {
	const { data } = useNodeRender();
	const d = data as { title?: string; model?: string } | undefined;
	return (
		<div className="bg-background border rounded-xl shadow-sm w-52 overflow-hidden">
			<div className="flex items-center gap-2 px-3 py-2 bg-violet-50 dark:bg-violet-950/30 border-b">
				<Zap className="size-3.5 text-violet-500" />
				<span className="text-xs font-medium text-violet-700 dark:text-violet-300">
					{d?.title ?? "LLM"}
				</span>
			</div>
			<div className="px-3 py-2 text-xs text-muted-foreground">
				{d?.model ?? "gpt-4o"}
			</div>
		</div>
	);
}

function CodeNodeContent() {
	const { data } = useNodeRender();
	return (
		<div className="bg-background border rounded-xl shadow-sm w-52 overflow-hidden">
			<div className="flex items-center gap-2 px-3 py-2 bg-amber-50 dark:bg-amber-950/30 border-b">
				<Code2 className="size-3.5 text-amber-500" />
				<span className="text-xs font-medium text-amber-700 dark:text-amber-300">
					{(data as { title?: string })?.title ?? "代码"}
				</span>
			</div>
			<div className="px-3 py-2 text-xs text-muted-foreground">Python / JS</div>
		</div>
	);
}

function ConditionNodeContent() {
	const { data } = useNodeRender();
	return (
		<div className="bg-background border rounded-xl shadow-sm w-52 overflow-hidden">
			<div className="flex items-center gap-2 px-3 py-2 bg-blue-50 dark:bg-blue-950/30 border-b">
				<SplitSquareHorizontal className="size-3.5 text-blue-500" />
				<span className="text-xs font-medium text-blue-700 dark:text-blue-300">
					{(data as { title?: string })?.title ?? "条件"}
				</span>
			</div>
			<div className="px-3 py-2 text-xs text-muted-foreground">IF / ELSE</div>
		</div>
	);
}

function TemplateNodeContent() {
	const { data } = useNodeRender();
	return (
		<div className="bg-background border rounded-xl shadow-sm w-52 overflow-hidden">
			<div className="flex items-center gap-2 px-3 py-2 bg-rose-50 dark:bg-rose-950/30 border-b">
				<MessageSquare className="size-3.5 text-rose-500" />
				<span className="text-xs font-medium text-rose-700 dark:text-rose-300">
					{(data as { title?: string })?.title ?? "模板"}
				</span>
			</div>
			<div className="px-3 py-2 text-xs text-muted-foreground">Jinja2 模板</div>
		</div>
	);
}

// ── Node registries ─────────────────────────────────────────────────────────

const NODE_REGISTRIES: WorkflowNodeRegistry[] = [
	{
		type: "start",
		meta: {
			defaultPorts: [{ type: "output" }],
			deleteDisable: true,
			copyDisable: true,
		},
		formMeta: { render: () => <StartNodeContent /> },
	},
	{
		type: "end",
		meta: {
			defaultPorts: [{ type: "input" }],
			deleteDisable: true,
			copyDisable: true,
		},
		formMeta: { render: () => <EndNodeContent /> },
	},
	{
		type: "llm",
		meta: { defaultPorts: [{ type: "input" }, { type: "output" }] },
		formMeta: { render: () => <LLMNodeContent /> },
	},
	{
		type: "code",
		meta: { defaultPorts: [{ type: "input" }, { type: "output" }] },
		formMeta: { render: () => <CodeNodeContent /> },
	},
	{
		type: "condition",
		meta: { defaultPorts: [{ type: "input" }, { type: "output" }] },
		formMeta: { render: () => <ConditionNodeContent /> },
	},
	{
		type: "template",
		meta: { defaultPorts: [{ type: "input" }, { type: "output" }] },
		formMeta: { render: () => <TemplateNodeContent /> },
	},
];

const DEFAULT_DOCUMENT = {
	nodes: [
		{
			id: "start_0",
			type: "start",
			meta: { position: { x: 180, y: 300 } },
			data: { title: "开始" },
		},
		{
			id: "end_0",
			type: "end",
			meta: { position: { x: 680, y: 300 } },
			data: { title: "结束" },
		},
	],
	edges: [{ sourceNodeID: "start_0", targetNodeID: "end_0" }],
};

// ── Node palette items ──────────────────────────────────────────────────────

const PALETTE_ITEMS = [
	{
		type: "llm",
		label: "LLM",
		icon: Zap,
		color: "text-violet-500",
		bg: "bg-violet-50 dark:bg-violet-950/30",
		defaultData: { title: "LLM", model: "gpt-4o" },
	},
	{
		type: "code",
		label: "代码",
		icon: Code2,
		color: "text-amber-500",
		bg: "bg-amber-50 dark:bg-amber-950/30",
		defaultData: { title: "代码" },
	},
	{
		type: "condition",
		label: "条件",
		icon: SplitSquareHorizontal,
		color: "text-blue-500",
		bg: "bg-blue-50 dark:bg-blue-950/30",
		defaultData: { title: "条件" },
	},
	{
		type: "template",
		label: "模板",
		icon: MessageSquare,
		color: "text-rose-500",
		bg: "bg-rose-50 dark:bg-rose-950/30",
		defaultData: { title: "模板" },
	},
] as const;

// ── Toolbar ─────────────────────────────────────────────────────────────────

function CanvasToolbar() {
	const { zoomin, zoomout, fitView, zoom } = usePlaygroundTools();
	return (
		<div className="absolute bottom-5 left-1/2 -translate-x-1/2 flex items-center gap-1 px-2 py-1.5 bg-background border rounded-xl shadow-md z-10">
			<button
				type="button"
				onClick={() => zoomout()}
				className="flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
				title="缩小"
			>
				<ZoomOut className="size-3.5" />
			</button>
			<span className="text-xs text-muted-foreground w-10 text-center tabular-nums">
				{Math.round(zoom * 100)}%
			</span>
			<button
				type="button"
				onClick={() => zoomin()}
				className="flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
				title="放大"
			>
				<ZoomIn className="size-3.5" />
			</button>
			<div className="w-px h-4 bg-border mx-0.5" />
			<button
				type="button"
				onClick={() => fitView(true)}
				className="flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
				title="适应画布"
			>
				<Maximize2 className="size-3.5" />
			</button>
		</div>
	);
}

// ── Node palette ─────────────────────────────────────────────────────────────

function NodePalette() {
	const ctx = useClientContext();
	const [open, setOpen] = useState(true);

	const addNode = useCallback(
		(type: string, defaultData: Record<string, unknown>) => {
			const x = 400 + Math.random() * 80 - 40;
			const y = 200 + Math.random() * 80 - 40;
			// eslint-disable-next-line @typescript-eslint/no-explicit-any
			(ctx.document as any).createWorkflowNodeByType(
				type,
				{ x, y },
				{ data: defaultData },
			);
		},
		[ctx],
	);

	return (
		<div className="absolute left-3 top-3 z-10 flex flex-col gap-1">
			<button
				type="button"
				onClick={() => setOpen((v) => !v)}
				className={cn(
					"flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border bg-background shadow-sm hover:bg-muted transition-colors",
					open && "bg-muted",
				)}
			>
				<Plus className="size-3.5" />
				添加节点
			</button>
			{open && (
				<div className="flex flex-col gap-1 p-2 bg-background border rounded-xl shadow-md w-36">
					{PALETTE_ITEMS.map((item) => (
						<button
							key={item.type}
							type="button"
							onClick={() => addNode(item.type, item.defaultData)}
							className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-muted transition-colors text-left"
						>
							<div
								className={cn(
									"flex items-center justify-center size-6 rounded-md",
									item.bg,
								)}
							>
								<item.icon className={cn("size-3.5", item.color)} />
							</div>
							<span className="text-xs">{item.label}</span>
						</button>
					))}
				</div>
			)}
		</div>
	);
}

function CanvasOverlay() {
	return (
		<>
			<NodePalette />
			<CanvasToolbar />
		</>
	);
}

// ── Page ─────────────────────────────────────────────────────────────────────

export default function WorkflowEditorPage() {
	const { id } = useParams<{ id: string }>();
	const nav = useNavigate();
	const [saving, setSaving] = useState(false);
	const [saved, setSaved] = useState(false);

	const wf = id ? workflowModel.get(id) : undefined;

	const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
	const saveRef = useRef<(ctx: FreeLayoutPluginContext) => void>(() => {});
	useEffect(() => {
		saveRef.current = (ctx: FreeLayoutPluginContext) => {
			if (!id || ctx.document.disposed) return;
			if (saveTimerRef.current) clearTimeout(saveTimerRef.current);
			setSaving(true);
			saveTimerRef.current = setTimeout(() => {
				workflowModel.update(id, {
					document: ctx.document.toJSON() as unknown as Record<string, unknown>,
				});
				setSaving(false);
				setSaved(true);
				setTimeout(() => setSaved(false), 2000);
			}, 800);
		};
	}, [id]);

	const editorProps = useMemo<FreeLayoutProps>(
		() => ({
			initialData: (wf?.document ??
				DEFAULT_DOCUMENT) as FreeLayoutProps["initialData"],
			nodeRegistries: NODE_REGISTRIES,
			background: true,
			readonly: false,
			twoWayConnection: true,
			nodeEngine: { enable: true },
			playground: { preventGlobalGesture: true },
			history: { enable: true, enableChangeNode: true },
			materials: {
				// Official pattern: renderDefaultNode owns the outer shell + ports
				renderDefaultNode: BaseNode,
			},
			onAllLayersRendered(ctx) {
				ctx.tools.fitView(false);
			},
			onContentChange(ctx) {
				saveRef.current(ctx);
			},
			plugins: () => [
				createFreeStackPlugin({}),
				createFreeLinesPlugin({}),
				createFreeSnapPlugin({}),
				createMinimapPlugin({}),
				createFreeNodePanelPlugin({ renderer: (() => null) as any }),
			],
		}),
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[wf?.id],
	);

	if (!wf) {
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
	}

	return (
		<div className="flex flex-col h-full">
			<div className="flex items-center gap-3 px-3 py-2 border-b bg-background shrink-0">
				<button
					type="button"
					onClick={() => nav("/workflow")}
					className="flex items-center justify-center size-6 rounded text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06] transition-colors"
				>
					<ArrowLeft className="size-3.5" />
				</button>
				<div className="flex items-center gap-1.5">
					<GitBranch className="size-3.5 text-muted-foreground" />
					<span className="text-sm font-medium truncate">{wf.name}</span>
				</div>
				<div className="flex-1" />
				<div className="flex items-center gap-1.5 text-xs text-muted-foreground shrink-0">
					{saving ? (
						<>
							<Loader2 className="size-3 animate-spin" />
							保存中
						</>
					) : saved ? (
						<span className="text-green-600">已保存</span>
					) : null}
				</div>
				<button
					type="button"
					className="flex items-center gap-1.5 px-3 py-1 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
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
					className="flex items-center gap-1.5 px-3 py-1 text-xs rounded-lg border hover:bg-muted transition-colors"
				>
					<Save className="size-3" />
					保存
				</button>
			</div>

			<div className="flex-1 min-h-0 relative">
				<FreeLayoutEditorProvider {...editorProps}>
					<div className="relative w-full h-full">
						<EditorRenderer className="w-full h-full" />
						<CanvasOverlay />
					</div>
				</FreeLayoutEditorProvider>
			</div>
		</div>
	);
}
