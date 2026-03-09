/**
 * A3S Flow node catalog — metadata for all 15 built-in node types.
 *
 * Mirrors the types registered in `NodeRegistry::with_defaults()` in
 * `crates/flow/src/registry.rs`.  Each entry drives both the searchable
 * node panel and the React Flow canvas renderer.
 */
import type { LucideIcon } from "lucide-react";
import {
	Globe,
	GitBranch,
	FileText,
	Layers,
	Code2,
	RefreshCw,
	ExternalLink,
	ArrowRightLeft,
	Plug,
	Play,
	Square,
	MessageSquare,
	HelpCircle,
	Variable,
	Scissors,
	Repeat,
	List,
} from "lucide-react";

export interface NodeCatalogEntry {
	/** a3s flow node type string (must match the Rust registry key) */
	type: string;
	label: string;
	description: string;
	category: string;
	icon: LucideIcon;
	/** Tailwind text-* color class for the icon */
	iconColor: string;
	/** Tailwind bg-* color class for the node header */
	headerBg: string;
	/** Tailwind text-* color class for the header label */
	headerText: string;
	/** Default `data` object to pass when creating a new node */
	defaultData: Record<string, unknown>;
}

export const NODE_CATALOG: NodeCatalogEntry[] = [
	// ── 流程控制 ──────────────────────────────────────────────────────────
	{
		type: "start",
		label: "开始",
		description: "工作流入口，声明输入变量和默认值",
		category: "流程控制",
		icon: Play,
		iconColor: "text-emerald-500",
		headerBg: "bg-emerald-50 dark:bg-emerald-950/30",
		headerText: "text-emerald-700 dark:text-emerald-300",
		defaultData: { variables: [] },
	},
	{
		type: "end",
		label: "结束",
		description: "工作流出口，收集输出结果",
		category: "流程控制",
		icon: Square,
		iconColor: "text-red-500",
		headerBg: "bg-red-50 dark:bg-red-950/30",
		headerText: "text-red-700 dark:text-red-300",
		defaultData: { outputs: [] },
	},

	// ── 网络 ──────────────────────────────────────────────────────────
	{
		type: "http-request",
		label: "HTTP 请求",
		description: "发送 GET / POST / PUT / DELETE / PATCH 请求",
		category: "网络",
		icon: Globe,
		iconColor: "text-cyan-500",
		headerBg: "bg-cyan-50 dark:bg-cyan-950/30",
		headerText: "text-cyan-700 dark:text-cyan-300",
		defaultData: { method: "GET", url: "", headers: {} },
	},

	// ── 逻辑 ──────────────────────────────────────────────────────────
	{
		type: "if-else",
		label: "条件分支",
		description: "多条件路由，支持 AND / OR 逻辑",
		category: "逻辑",
		icon: GitBranch,
		iconColor: "text-blue-500",
		headerBg: "bg-blue-50 dark:bg-blue-950/30",
		headerText: "text-blue-700 dark:text-blue-300",
		defaultData: {
			cases: [{ id: "case_1", logical_operator: "and", conditions: [] }],
		},
	},
	{
		type: "code",
		label: "代码",
		description: "使用 Rhai 脚本处理数据（沙盒执行）",
		category: "逻辑",
		icon: Code2,
		iconColor: "text-amber-500",
		headerBg: "bg-amber-50 dark:bg-amber-950/30",
		headerText: "text-amber-700 dark:text-amber-300",
		defaultData: { script: "" },
	},
	{
		type: "iteration",
		label: "迭代",
		description: "遍历数组，对每个元素并发执行子流程",
		category: "逻辑",
		icon: RefreshCw,
		iconColor: "text-violet-500",
		headerBg: "bg-violet-50 dark:bg-violet-950/30",
		headerText: "text-violet-700 dark:text-violet-300",
		defaultData: { items_path: "", flow: { nodes: [], edges: [] } },
	},
	{
		type: "loop",
		label: "循环",
		description: "While 循环，支持中断条件",
		category: "逻辑",
		icon: Repeat,
		iconColor: "text-indigo-500",
		headerBg: "bg-indigo-50 dark:bg-indigo-950/30",
		headerText: "text-indigo-700 dark:text-indigo-300",
		defaultData: {
			max_iterations: 10,
			break_condition: { variable: "", operator: "==", value: "" },
			flow: { nodes: [], edges: [] },
		},
	},
	{
		type: "sub-flow",
		label: "子流程",
		description: "内联调用一个已命名的子流程",
		category: "逻辑",
		icon: ExternalLink,
		iconColor: "text-green-500",
		headerBg: "bg-green-50 dark:bg-green-950/30",
		headerText: "text-green-700 dark:text-green-300",
		defaultData: { flow_name: "" },
	},

	// ── 数据 ──────────────────────────────────────────────────────────
	{
		type: "template-transform",
		label: "模板变换",
		description: "使用 Jinja2 模板渲染输出",
		category: "数据",
		icon: FileText,
		iconColor: "text-rose-500",
		headerBg: "bg-rose-50 dark:bg-rose-950/30",
		headerText: "text-rose-700 dark:text-rose-300",
		defaultData: { template: "" },
	},
	{
		type: "variable-aggregator",
		label: "变量聚合",
		description: "从多个分支取第一个非空输出（扇入合并）",
		category: "数据",
		icon: Layers,
		iconColor: "text-orange-500",
		headerBg: "bg-orange-50 dark:bg-orange-950/30",
		headerText: "text-orange-700 dark:text-orange-300",
		defaultData: { sources: [] },
	},
	{
		type: "assign",
		label: "变量赋值",
		description: "设置工作流变量的值",
		category: "数据",
		icon: Variable,
		iconColor: "text-teal-500",
		headerBg: "bg-teal-50 dark:bg-teal-950/30",
		headerText: "text-teal-700 dark:text-teal-300",
		defaultData: { assignments: [] },
	},
	{
		type: "list-operator",
		label: "列表操作",
		description: "过滤、排序、去重、限制数组元素",
		category: "数据",
		icon: List,
		iconColor: "text-pink-500",
		headerBg: "bg-pink-50 dark:bg-pink-950/30",
		headerText: "text-pink-700 dark:text-pink-300",
		defaultData: { operation: "filter", array_path: "" },
	},

	// ── AI ──────────────────────────────────────────────────────────
	{
		type: "llm",
		label: "LLM",
		description: "调用大语言模型（OpenAI 兼容）",
		category: "AI",
		icon: MessageSquare,
		iconColor: "text-purple-500",
		headerBg: "bg-purple-50 dark:bg-purple-950/30",
		headerText: "text-purple-700 dark:text-purple-300",
		defaultData: {
			model: "gpt-4",
			system_prompt: "",
			user_prompt: "",
		},
	},
	{
		type: "question-classifier",
		label: "问题分类",
		description: "使用 LLM 将输入分类到预定义类别",
		category: "AI",
		icon: HelpCircle,
		iconColor: "text-fuchsia-500",
		headerBg: "bg-fuchsia-50 dark:bg-fuchsia-950/30",
		headerText: "text-fuchsia-700 dark:text-fuchsia-300",
		defaultData: {
			classes: [],
			input_variable: "",
		},
	},
	{
		type: "parameter-extractor",
		label: "参数提取",
		description: "使用 LLM 从自然语言中提取结构化参数",
		category: "AI",
		icon: Scissors,
		iconColor: "text-sky-500",
		headerBg: "bg-sky-50 dark:bg-sky-950/30",
		headerText: "text-sky-700 dark:text-sky-300",
		defaultData: {
			parameters: [],
			input_variable: "",
		},
	},

	// ── 实用 ──────────────────────────────────────────────────────────
	{
		type: "noop",
		label: "合并",
		description: "透传所有上游输入，不做任何处理",
		category: "实用",
		icon: ArrowRightLeft,
		iconColor: "text-slate-500",
		headerBg: "bg-slate-100 dark:bg-slate-800/40",
		headerText: "text-slate-600 dark:text-slate-300",
		defaultData: {},
	},
];

/**
 * Tool node catalog — external integrations (MCP, Zapier, Slack, etc.).
 *
 * These nodes are NOT part of the core flow engine. They are registered
 * externally via the `NodeRegistry::register()` API.
 */
export const TOOL_CATALOG: NodeCatalogEntry[] = [
	{
		type: "mcp",
		label: "MCP 工具",
		description: "调用 Model Context Protocol (MCP) 服务器上的工具",
		category: "工具",
		icon: Plug,
		iconColor: "text-purple-500",
		headerBg: "bg-purple-50 dark:bg-purple-950/30",
		headerText: "text-purple-700 dark:text-purple-300",
		defaultData: {
			transport: "sse",
			server_url: "",
			tool_name: "",
			arguments: {},
		},
	},
];

/** Look up catalog entry by node type string (searches both built-in and tool catalogs). */
export function getCatalogEntry(type: string): NodeCatalogEntry | undefined {
	return (
		NODE_CATALOG.find((n) => n.type === type) ||
		TOOL_CATALOG.find((n) => n.type === type)
	);
}

/** All distinct categories, in display order. */
export const CATEGORIES = Array.from(
	new Set(NODE_CATALOG.map((n) => n.category)),
);
