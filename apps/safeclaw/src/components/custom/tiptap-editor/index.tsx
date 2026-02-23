/**
 * TipTap rich text editor with / slash-commands and @ mentions.
 */
import { cn } from "@/lib/utils";
import { BUILTIN_PERSONAS } from "@/lib/builtin-personas";
import Mention from "@tiptap/extension-mention";
import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { HelpCircle, Sparkles, Terminal, User, Wrench } from "lucide-react";
import {
	forwardRef,
	useCallback,
	useEffect,
	useImperativeHandle,
	useMemo,
	useRef,
} from "react";
import { SlashCommand } from "./slash-command";
import { createSuggestionRenderer } from "./suggestion-renderer";
import type { SuggestionItem } from "./mention-list";
import "./tiptap.css";

// =============================================================================
// Data sources for / and @
// =============================================================================

/** Skills available for /slash-command */
const SLASH_ITEMS: SuggestionItem[] = [
	// Session commands (a3s-code v0.9.0)
	{
		id: "help",
		label: "help",
		description: "显示可用命令列表",
		group: "命令",
		icon: <HelpCircle className="size-3 text-blue-500" />,
	},
	{
		id: "cost",
		label: "cost",
		description: "查看 Token 用量与费用",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	{
		id: "model",
		label: "model",
		description: "查看或切换当前模型",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	{
		id: "history",
		label: "history",
		description: "查看对话轮次与 Token 统计",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	{
		id: "tools",
		label: "tools",
		description: "列出已注册的工具",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	{
		id: "compact",
		label: "compact",
		description: "手动触发上下文压缩",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	{
		id: "clear",
		label: "clear",
		description: "清空对话历史",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	{
		id: "mcp",
		label: "mcp",
		description: "查看 MCP 服务器状态",
		group: "命令",
		icon: <Terminal className="size-3 text-blue-500" />,
	},
	// Skills
	{
		id: "factor_analysis",
		label: "factor_analysis",
		description: "批量因子检验 — IC、分层回测、归因",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "model_monitor",
		label: "model_monitor",
		description: "模型监控 — PSI、AUC 衰减、漂移告警",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "k8s_upgrade_preflight",
		label: "k8s_upgrade_preflight",
		description: "K8s 升级预检自动化",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "pipeline_quality_monitor",
		label: "pipeline_quality_monitor",
		description: "数据管道质量监控",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "competitive_intel",
		label: "competitive_intel",
		description: "竞品情报自动采集与分析",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "backtest_report",
		label: "backtest_report",
		description: "策略回测报告生成",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "credit_feature_eng",
		label: "credit_feature_eng",
		description: "信用特征工程自动化",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "schema_migration",
		label: "schema_migration",
		description: "数据库 Schema 迁移管理",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "incident_runbook",
		label: "incident_runbook",
		description: "故障应急 Runbook 执行",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "prd_template",
		label: "prd_template",
		description: "PRD 模板生成与校验",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "payment_approval",
		label: "payment_approval",
		description: "供应商付款审批与执行",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "invoice_reconcile",
		label: "invoice_reconcile",
		description: "发票自动核对与对账",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	{
		id: "ab_test_analyzer",
		label: "ab_test_analyzer",
		description: "A/B 测试显著性分析",
		group: "技能",
		icon: <Sparkles className="size-3 text-primary" />,
	},
	// Tools
	{
		id: "tool_read",
		label: "Read",
		description: "读取文件内容",
		group: "工具",
		icon: <Wrench className="size-3 text-muted-foreground" />,
	},
	{
		id: "tool_write",
		label: "Write",
		description: "写入文件",
		group: "工具",
		icon: <Wrench className="size-3 text-muted-foreground" />,
	},
	{
		id: "tool_bash",
		label: "Bash",
		description: "执行终端命令",
		group: "工具",
		icon: <Wrench className="size-3 text-muted-foreground" />,
	},
	{
		id: "tool_web_search",
		label: "WebSearch",
		description: "联网搜索",
		group: "工具",
		icon: <Wrench className="size-3 text-muted-foreground" />,
	},
	{
		id: "tool_python",
		label: "PythonExec",
		description: "执行 Python 代码",
		group: "工具",
		icon: <Wrench className="size-3 text-muted-foreground" />,
	},
	{
		id: "tool_sql",
		label: "SQLExecute",
		description: "执行 SQL 查询",
		group: "工具",
		icon: <Wrench className="size-3 text-muted-foreground" />,
	},
];

/** Items available for @mention: agents */
const MENTION_ITEMS: SuggestionItem[] = [
	// Agents
	...BUILTIN_PERSONAS.filter((p) => p.id !== "company-group").map((p) => ({
		id: p.id,
		label: p.name,
		description: p.description,
		group: "智能体",
		icon: <User className="size-3 text-primary" />,
	})),
];

function filterItems(items: SuggestionItem[], query: string): SuggestionItem[] {
	const q = query.toLowerCase();
	if (!q) return items.slice(0, 15);
	return items
		.filter(
			(item) =>
				item.label.toLowerCase().includes(q) ||
				item.id.toLowerCase().includes(q) ||
				item.description?.toLowerCase().includes(q),
		)
		.slice(0, 12);
}

// =============================================================================
// Editor component
// =============================================================================

export interface TiptapEditorRef {
	focus: () => void;
	getText: () => string;
	clear: () => void;
	isEmpty: () => boolean;
	/** Returns ids of all @mentioned agents in the current content */
	getMentions: () => string[];
}

interface TiptapEditorProps {
	placeholder?: string;
	disabled?: boolean;
	className?: string;
	onSubmit?: (text: string) => void;
	onChange?: (text: string) => void;
	/** Called when images are pasted from clipboard */
	onPasteImages?: (images: { media_type: string; data: string }[]) => void;
	/** Override @mention items (defaults to BUILTIN_PERSONAS) */
	mentionItems?: SuggestionItem[];
}

const TiptapEditor = forwardRef<TiptapEditorRef, TiptapEditorProps>(
	(
		{
			placeholder,
			disabled,
			className,
			onSubmit,
			onChange,
			onPasteImages,
			mentionItems,
		},
		ref,
	) => {
		const resolvedMentionItems = mentionItems ?? MENTION_ITEMS;
		// Guard: when a suggestion item is just selected via Enter, skip the
		// next Enter keydown so it doesn't also submit the message.
		const justSelectedRef = useRef(false);

		const slashSuggestion = useMemo(
			() =>
				createSuggestionRenderer(
					(q) => filterItems(SLASH_ITEMS, q),
					() => {
						justSelectedRef.current = true;
					},
				),
			[],
		);

		const mentionSuggestion = useMemo(
			() =>
				createSuggestionRenderer(
					(q) => filterItems(resolvedMentionItems, q),
					() => {
						justSelectedRef.current = true;
					},
				),
			[resolvedMentionItems],
		);

		const editor = useEditor({
			extensions: [
				StarterKit.configure({
					// Disable block-level features — this is a chat input, not a document editor
					heading: false,
					blockquote: false,
					codeBlock: false,
					horizontalRule: false,
					bulletList: false,
					orderedList: false,
					listItem: false,
				}),
				Placeholder.configure({
					placeholder: placeholder || "输入消息...",
					emptyEditorClass: "tiptap-empty",
				}),
				Mention.configure({
					HTMLAttributes: {
						class: "tiptap-mention",
					},
					renderHTML({ options, node }) {
						return [
							"span",
							options.HTMLAttributes,
							`@${node.attrs.label ?? node.attrs.id}`,
						];
					},
					suggestion: {
						char: "@",
						...mentionSuggestion,
					},
				}),
				SlashCommand.configure({
					suggestion: {
						...slashSuggestion,
					},
				}),
			],
			editable: !disabled,
			editorProps: {
				attributes: {
					class: "tiptap-content",
				},
				handlePaste: (_view, event) => {
					const items = event.clipboardData?.items;
					if (!items || !onPasteImages) return false;
					const imageFiles: File[] = [];
					for (const item of items) {
						if (item.type.startsWith("image/")) {
							const file = item.getAsFile();
							if (file) imageFiles.push(file);
						}
					}
					if (imageFiles.length === 0) return false;
					event.preventDefault();
					Promise.all(
						imageFiles.map(
							(file) =>
								new Promise<{ media_type: string; data: string }>(
									(resolve, reject) => {
										const reader = new FileReader();
										reader.onload = () => {
											const result = reader.result as string;
											const [header, data] = result.split(",");
											const media_type = header
												.replace("data:", "")
												.replace(";base64", "");
											resolve({ media_type, data });
										};
										reader.onerror = reject;
										reader.readAsDataURL(file);
									},
								),
						),
					).then(onPasteImages);
					return true;
				},
				handleKeyDown: (_view, event) => {
					// Enter without Shift = submit
					if (event.key === "Enter" && !event.shiftKey) {
						// Don't submit if a suggestion was just selected via Enter
						if (justSelectedRef.current) {
							justSelectedRef.current = false;
							return true;
						}
						event.preventDefault();
						const text = editor?.getText().trim();
						if (text) {
							onSubmit?.(text);
							// Clear after a microtask to avoid interfering with ProseMirror
							setTimeout(() => {
								editor?.commands.clearContent();
								editor?.commands.focus();
							}, 0);
						}
						return true;
					}
					return false;
				},
			},
			onUpdate: ({ editor: e }) => {
				onChange?.(e.getText());
			},
		});

		// Sync disabled state
		useEffect(() => {
			if (editor) {
				editor.setEditable(!disabled);
			}
		}, [editor, disabled]);

		useImperativeHandle(
			ref,
			() => ({
				focus: () => editor?.commands.focus(),
				getText: () => editor?.getText() || "",
				clear: () => editor?.commands.clearContent(),
				isEmpty: () => editor?.isEmpty ?? true,
				getMentions: () => {
					if (!editor) return [];
					const ids: string[] = [];
					editor.state.doc.descendants((node) => {
						if (node.type.name === "mention") {
							ids.push(node.attrs.id as string);
						}
					});
					return ids;
				},
			}),
			[editor],
		);

		const handleContainerClick = useCallback(() => {
			editor?.commands.focus();
		}, [editor]);

		return (
			<div
				className={cn("w-full h-full overflow-y-auto cursor-text", className)}
				onClick={handleContainerClick}
			>
				<EditorContent editor={editor} className="h-full" />
			</div>
		);
	},
);

TiptapEditor.displayName = "TiptapEditor";

export default TiptapEditor;
