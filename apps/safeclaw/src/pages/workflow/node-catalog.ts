/**
 * A3S Flow node catalog — metadata for all built-in node types.
 *
 * Mirrors the types registered in `NodeRegistry::with_defaults()` in
 * `crates/flow/src/registry.rs`.  Each entry drives both the searchable
 * node panel and the React Flow canvas renderer.
 *
 * Categories follow Dify's grouping convention:
 *   常用 · AI · 数据处理 · 工具
 */
import type { LucideIcon } from "lucide-react";
import {
	Globe,
	GitBranch,
	FileText,
	Layers,
	Code2,
	RefreshCw,
	ArrowRightLeft,
	Plug,
	Play,
	MessageSquare,
	HelpCircle,
	Variable,
	Scissors,
	Repeat,
	List,
	DatabaseBackup,
	DatabaseZap,
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
	// ── 常用 ──────────────────────────────────────────────────────────
	{
		type: "start",
		label: "开始",
		description: "工作流的起始节点，可以定义输入字段",
		category: "常用",
		icon: Play,
		iconColor: "text-emerald-500",
		headerBg: "bg-emerald-50 dark:bg-emerald-950/30",
		headerText: "text-emerald-700 dark:text-emerald-300",
		defaultData: { inputs: [] },
	},
	{
		type: "if-else",
		label: "IF/ELSE",
		description: "根据 if/else 条件将工作流拆分为两个或多个分支",
		category: "常用",
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
		description: "在工作流中运行沙盒代码，对输入和输出变量进行数据转换",
		category: "常用",
		icon: Code2,
		iconColor: "text-amber-500",
		headerBg: "bg-amber-50 dark:bg-amber-950/30",
		headerText: "text-amber-700 dark:text-amber-300",
		defaultData: { script: "" },
	},
	{
		type: "http-request",
		label: "HTTP 请求",
		description: "允许通过 HTTP 协议向外部服务发起请求",
		category: "常用",
		icon: Globe,
		iconColor: "text-cyan-500",
		headerBg: "bg-cyan-50 dark:bg-cyan-950/30",
		headerText: "text-cyan-700 dark:text-cyan-300",
		defaultData: { method: "GET", url: "", headers: {} },
	},
	{
		type: "iteration",
		label: "迭代",
		description: "对列表中的元素执行相同的操作步骤",
		category: "常用",
		icon: RefreshCw,
		iconColor: "text-violet-500",
		headerBg: "bg-violet-50 dark:bg-violet-950/30",
		headerText: "text-violet-700 dark:text-violet-300",
		defaultData: {
			input_selector: "",
			output_selector: "",
			mode: "parallel",
			flow: { nodes: [], edges: [] },
		},
	},
	{
		type: "loop",
		label: "循环",
		description: "以迭代方式反复执行一组任务，直到满足终止条件",
		category: "常用",
		icon: Repeat,
		iconColor: "text-indigo-500",
		headerBg: "bg-indigo-50 dark:bg-indigo-950/30",
		headerText: "text-indigo-700 dark:text-indigo-300",
		defaultData: {
			output_selector: "",
			max_iterations: 10,
			flow: { nodes: [], edges: [] },
		},
	},

	// ── AI ──────────────────────────────────────────────────────────
	{
		type: "llm",
		label: "LLM",
		description: "调用大语言模型来回答问题或处理自然语言",
		category: "AI",
		icon: MessageSquare,
		iconColor: "text-purple-500",
		headerBg: "bg-purple-50 dark:bg-purple-950/30",
		headerText: "text-purple-700 dark:text-purple-300",
		defaultData: {
			model: "gpt-4o-mini",
			system_prompt: "",
			user_prompt: "",
			api_base: "https://api.openai.com/v1",
			api_key: "",
			temperature: 0.7,
		},
	},
	{
		type: "question-classifier",
		label: "问题分类",
		description: "使用大语言模型根据设置的分类规则对用户问题进行分类",
		category: "AI",
		icon: HelpCircle,
		iconColor: "text-fuchsia-500",
		headerBg: "bg-fuchsia-50 dark:bg-fuchsia-950/30",
		headerText: "text-fuchsia-700 dark:text-fuchsia-300",
		defaultData: {
			model: "gpt-4o-mini",
			question: "",
			classes: [],
			api_base: "https://api.openai.com/v1",
			api_key: "",
			temperature: 0,
		},
	},
	{
		type: "parameter-extractor",
		label: "参数提取",
		description: "利用大语言模型从自然语言中提取结构化参数",
		category: "AI",
		icon: Scissors,
		iconColor: "text-sky-500",
		headerBg: "bg-sky-50 dark:bg-sky-950/30",
		headerText: "text-sky-700 dark:text-sky-300",
		defaultData: {
			model: "gpt-4o-mini",
			query: "",
			parameters: [],
			api_base: "https://api.openai.com/v1",
			api_key: "",
			temperature: 0,
		},
	},

	// ── 数据处理 ──────────────────────────────────────────────────────
	{
		type: "template-transform",
		label: "模板",
		description: "使用 Jinja2 模板语言将多个来源的数据合并为单一文本输出",
		category: "数据处理",
		icon: FileText,
		iconColor: "text-rose-500",
		headerBg: "bg-rose-50 dark:bg-rose-950/30",
		headerText: "text-rose-700 dark:text-rose-300",
		defaultData: { template: "" },
	},
	{
		type: "variable-aggregator",
		label: "变量聚合",
		description: "将多条分支中的变量聚合为单一变量",
		category: "数据处理",
		icon: Layers,
		iconColor: "text-orange-500",
		headerBg: "bg-orange-50 dark:bg-orange-950/30",
		headerText: "text-orange-700 dark:text-orange-300",
		defaultData: { sources: [] },
	},
	{
		type: "assign",
		label: "变量赋值",
		description: "对工作流变量进行赋值操作",
		category: "数据处理",
		icon: Variable,
		iconColor: "text-teal-500",
		headerBg: "bg-teal-50 dark:bg-teal-950/30",
		headerText: "text-teal-700 dark:text-teal-300",
		defaultData: { assigns: {} },
	},
	{
		type: "list-operator",
		label: "列表操作",
		description: "对列表变量进行过滤、排序、去重及限制等操作",
		category: "数据处理",
		icon: List,
		iconColor: "text-pink-500",
		headerBg: "bg-pink-50 dark:bg-pink-950/30",
		headerText: "text-pink-700 dark:text-pink-300",
		defaultData: { input_selector: "", sort_order: "asc" },
	},
	{
		type: "context-set",
		label: "写入上下文",
		description: "将键值对写入工作流共享上下文，所有节点均可读取",
		category: "数据处理",
		icon: DatabaseBackup,
		iconColor: "text-teal-600",
		headerBg: "bg-teal-50 dark:bg-teal-950/30",
		headerText: "text-teal-700 dark:text-teal-300",
		defaultData: { assigns: {} },
	},
	{
		type: "context-get",
		label: "读取上下文",
		description: "从工作流共享上下文读取指定键的值",
		category: "数据处理",
		icon: DatabaseZap,
		iconColor: "text-teal-500",
		headerBg: "bg-teal-50 dark:bg-teal-950/30",
		headerText: "text-teal-700 dark:text-teal-300",
		defaultData: { keys: [] },
	},
	{
		type: "noop",
		label: "合并",
		description: "将多条分支的所有上游输出透传到单一路径",
		category: "数据处理",
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
