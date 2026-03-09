/**
 * TipTap rich text editor with / slash-commands and @ mentions.
 */
import { cn } from "@/lib/utils";
import { BUILTIN_PERSONAS } from "@/lib/builtin-personas";
import Mention from "@tiptap/extension-mention";
import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { User } from "lucide-react";
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
	/** Slash command items fetched from the API */
	slashItems?: SuggestionItem[];
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
			slashItems,
		},
		ref,
	) => {
		const resolvedMentionItems = mentionItems ?? MENTION_ITEMS;
		// Use a ref so the slash suggestion closure always reads the latest items
		// without needing to recreate the editor when items change.
		const slashItemsRef = useRef<SuggestionItem[]>(slashItems ?? []);
		useEffect(() => {
			slashItemsRef.current = slashItems ?? [];
		}, [slashItems]);

		// Guard: when a suggestion item is just selected via Enter, skip the
		// next Enter keydown so it doesn't also submit the message.
		const justSelectedRef = useRef(false);

		const slashSuggestion = useMemo(
			() =>
				createSuggestionRenderer(
					(q) => filterItems(slashItemsRef.current, q),
					() => {
						justSelectedRef.current = true;
					},
				),
			// eslint-disable-next-line react-hooks/exhaustive-deps
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
						...(slashSuggestion as any),
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
					for (const item of Array.from(items)) {
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
