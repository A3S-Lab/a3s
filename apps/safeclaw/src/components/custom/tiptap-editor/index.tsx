/**
 * TipTap rich text editor with / slash-commands and @ mentions.
 */
import { cn } from "@/lib/utils";
import { BUILTIN_PERSONAS } from "@/lib/builtin-personas";
import Mention from "@tiptap/extension-mention";
import Placeholder from "@tiptap/extension-placeholder";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { User, File, Folder } from "lucide-react";
import {
	forwardRef,
	useCallback,
	useEffect,
	useImperativeHandle,
	useMemo,
	useRef,
	useState,
} from "react";
import { SlashCommand } from "./slash-command";
import { SlashCommandNode } from "./slash-node";
import { createSuggestionRenderer } from "./suggestion-renderer";
import type { SuggestionItem } from "./mention-list";
import "./tiptap.css";

// =============================================================================
// Data sources for / and @
// =============================================================================

/** Items available for @mention: agents and workspace files */
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

/**
 * Fetch workspace files and directories
 * Returns a list of files/folders in the current workspace
 */
async function fetchWorkspaceFiles(
	workspaceDir: string,
): Promise<SuggestionItem[]> {
	try {
		console.log("[TiptapEditor] Fetching workspace files from:", workspaceDir);
		// Use Tauri's fs plugin to read directory
		const { readDir } = await import("@tauri-apps/plugin-fs");
		const entries = await readDir(workspaceDir, { recursive: false });

		console.log("[TiptapEditor] Found entries:", entries.length);

		const items = entries
			.filter((entry) => {
				// Filter out hidden files and common ignore patterns
				const name = entry.name || "";
				return (
					!name.startsWith(".") &&
					name !== "node_modules" &&
					name !== "target" &&
					name !== "dist" &&
					name !== "build"
				);
			})
			.map((entry) => ({
				id: `file:${workspaceDir}/${entry.name}`,
				label: entry.name || "",
				description: entry.isDirectory ? "文件夹" : "文件",
				group: "工作区",
				path: `${workspaceDir}/${entry.name}`,
				isDirectory: entry.isDirectory,
				level: 0,
				expanded: false,
				icon: entry.isDirectory ? (
					<Folder className="size-3 text-blue-500" />
				) : (
					<File className="size-3 text-gray-500" />
				),
			}))
			.slice(0, 20); // Limit to 20 items

		console.log("[TiptapEditor] Filtered items:", items.length);
		return items;
	} catch (error) {
		console.error("[TiptapEditor] Failed to fetch workspace files:", error);
		return [];
	}
}

/**
 * Fetch files in a subdirectory
 */
async function fetchSubdirectoryFiles(
	dirPath: string,
	level: number,
): Promise<SuggestionItem[]> {
	try {
		console.log("[TiptapEditor] fetchSubdirectoryFiles - dirPath:", dirPath, "level:", level);
		const { readDir } = await import("@tauri-apps/plugin-fs");
		const entries = await readDir(dirPath, { recursive: false });
		console.log("[TiptapEditor] fetchSubdirectoryFiles - raw entries:", entries.length, entries);

		const filtered = entries.filter((entry) => {
			const name = entry.name || "";
			return (
				!name.startsWith(".") &&
				name !== "node_modules" &&
				name !== "target" &&
				name !== "dist" &&
				name !== "build"
			);
		});
		console.log("[TiptapEditor] fetchSubdirectoryFiles - filtered entries:", filtered.length);

		return filtered.map((entry) => ({
			id: `file:${dirPath}/${entry.name}`,
			label: entry.name || "",
			description: entry.isDirectory ? "文件夹" : "文件",
			group: "工作区",
			path: `${dirPath}/${entry.name}`,
			isDirectory: entry.isDirectory,
			level,
			expanded: false,
			icon: entry.isDirectory ? (
				<Folder className="size-3 text-blue-500" />
			) : (
				<File className="size-3 text-gray-500" />
			),
		}));
	} catch (error) {
		console.error("[TiptapEditor] Failed to fetch subdirectory:", dirPath, error);
		return [];
	}
}

function filterItems(items: SuggestionItem[], query: string): SuggestionItem[] {
	const q = query.toLowerCase();
	if (!q) return items.slice(0, 50); // Increased limit to show more files and expanded folders
	return items
		.filter(
			(item) =>
				item.label.toLowerCase().includes(q) ||
				item.id.toLowerCase().includes(q) ||
				item.description?.toLowerCase().includes(q),
		)
		.slice(0, 30); // Increased limit for search results
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
	/** Workspace directory for file mentions */
	workspaceDir?: string;
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
			workspaceDir,
		},
		ref,
	) => {
		const [workspaceFiles, setWorkspaceFiles] = useState<SuggestionItem[]>([]);
		const [expandedFolders, setExpandedFolders] = useState<Set<string>>(new Set());
		const workspaceDirRef = useRef<string>("");

		const resolvedMentionItems = useMemo(() => {
			const baseItems = mentionItems ?? MENTION_ITEMS;
			return [...baseItems, ...workspaceFiles];
		}, [mentionItems, workspaceFiles]);

		// Load workspace files when workspaceDir changes
		useEffect(() => {
			console.log("[TiptapEditor] workspaceDir changed:", workspaceDir);
			workspaceDirRef.current = workspaceDir || "";
			if (workspaceDir) {
				fetchWorkspaceFiles(workspaceDir).then((files) => {
					console.log("[TiptapEditor] Setting workspace files:", files.length);
					setWorkspaceFiles(files);
					setExpandedFolders(new Set());
				});
			} else {
				setWorkspaceFiles([]);
				setExpandedFolders(new Set());
			}
		}, [workspaceDir]);

		// Refresh workspace files - called when @ panel opens
		const refreshWorkspaceFiles = useCallback(async () => {
			const dir = workspaceDirRef.current;
			if (dir) {
				console.log("[TiptapEditor] Refreshing workspace files for:", dir);
				const newFiles = await fetchWorkspaceFiles(dir);
				console.log("[TiptapEditor] Refreshed workspace files:", newFiles.length);

				// Only update if the directory hasn't changed
				if (workspaceDirRef.current === dir) {
					// Get currently expanded folder paths
					const currentExpandedPaths = Array.from(expandedFolders);

					if (currentExpandedPaths.length === 0) {
						// No folders expanded, just return new files
						setWorkspaceFiles(newFiles);
					} else {
						// Re-expand folders that were previously expanded
						console.log("[TiptapEditor] Re-expanding folders:", currentExpandedPaths);

						// Build the complete list with expanded folders
						const result: SuggestionItem[] = [];

						for (const file of newFiles) {
							result.push(file);

							// If this folder was expanded, load and insert its children
							if (file.isDirectory && file.path && currentExpandedPaths.includes(file.path)) {
								const children = await fetchSubdirectoryFiles(file.path, (file.level || 0) + 1);
								console.log("[TiptapEditor] Re-loaded children for", file.path, ":", children.length);

								// Mark folder as expanded
								result[result.length - 1] = { ...file, expanded: true };

								// Insert children
								result.push(...children);
							}
						}

						setWorkspaceFiles(result);
					}
				}
			}
		}, [expandedFolders]);

		// Handle folder expansion/collapse
		const handleFolderClick = useCallback(async (folder: SuggestionItem) => {
			console.log("[TiptapEditor] handleFolderClick called:", folder);
			if (!folder.path || !folder.isDirectory) return;

			const folderPath = folder.path;
			const currentlyExpanded = expandedFolders.has(folderPath);

			console.log("[TiptapEditor] Folder path:", folderPath, "currently expanded:", currentlyExpanded);

			if (currentlyExpanded) {
				// Collapse
				console.log("[TiptapEditor] Collapsing folder");
				setExpandedFolders(prev => {
					const next = new Set(prev);
					next.delete(folderPath);
					console.log("[TiptapEditor] Updated expandedFolders (collapse):", next.size);
					return next;
				});

				setWorkspaceFiles(prev => {
					const result = prev
						.filter(item => !item.path || !item.path.startsWith(folderPath + "/"))
						.map(item => item.path === folderPath ? { ...item, expanded: false } : item);
					console.log("[TiptapEditor] Updated workspaceFiles (collapse):", result.length);
					return result;
				});
			} else {
				// Expand
				console.log("[TiptapEditor] Expanding folder");
				const children = await fetchSubdirectoryFiles(folderPath, (folder.level || 0) + 1);
				console.log("[TiptapEditor] Loaded children:", children.length);

				setExpandedFolders(prev => {
					const next = new Set(prev);
					next.add(folderPath);
					console.log("[TiptapEditor] Updated expandedFolders (expand):", next.size);
					return next;
				});

				setWorkspaceFiles(prev => {
					const result: SuggestionItem[] = [];
					for (const item of prev) {
						if (item.path === folderPath) {
							result.push({ ...item, expanded: true });
							result.push(...children);
						} else {
							result.push(item);
						}
					}
					console.log("[TiptapEditor] Updated workspaceFiles (expand):", result.length);
					return result;
				});
			}
			console.log("[TiptapEditor] handleFolderClick completed");
		}, [expandedFolders]); // Remove expandedFolders from dependencies

		// Use a ref to store the latest handleFolderClick so the suggestion
		// closure always reads the latest callback without recreating the editor
		const handleFolderClickRef = useRef(handleFolderClick);
		useEffect(() => {
			handleFolderClickRef.current = handleFolderClick;
		}, [handleFolderClick]);

		// Use a ref for refreshWorkspaceFiles as well
		const refreshWorkspaceFilesRef = useRef(refreshWorkspaceFiles);
		useEffect(() => {
			refreshWorkspaceFilesRef.current = refreshWorkspaceFiles;
		}, [refreshWorkspaceFiles]);

		// Use refs so the suggestion closures always read the latest items
		// without needing to recreate the editor when items change.
		const slashItemsRef = useRef<SuggestionItem[]>(slashItems ?? []);
		useEffect(() => {
			slashItemsRef.current = slashItems ?? [];
		}, [slashItems]);

		const mentionItemsRef = useRef<SuggestionItem[]>(resolvedMentionItems);
		useEffect(() => {
			console.log("[TiptapEditor] Updating mentionItemsRef:", resolvedMentionItems.length, "items");
			mentionItemsRef.current = resolvedMentionItems;
		}, [resolvedMentionItems]);

		// Guard: when a suggestion item is just selected via Enter, skip the
		// next Enter keydown so it doesn't also submit the message.
		const justSelectedRef = useRef(false);

		// Track whether any suggestion menu is currently open
		const suggestionOpenRef = useRef(false);

		const slashSuggestion = useMemo(
			() =>
				createSuggestionRenderer(
					(q) => filterItems(slashItemsRef.current, q),
					() => {
						justSelectedRef.current = true;
					},
					undefined,
					undefined,
					suggestionOpenRef,
				),
			// eslint-disable-next-line react-hooks/exhaustive-deps
			[],
		);

		const mentionSuggestion = useMemo(
			() =>
				createSuggestionRenderer(
					(q) => {
						const items = filterItems(mentionItemsRef.current, q);
						console.log("[TiptapEditor] getItems called for mention, query:", q, "returned:", items.length, "items");
						return items;
					},
					() => {
						justSelectedRef.current = true;
					},
					(item) => handleFolderClickRef.current(item),
					() => refreshWorkspaceFilesRef.current(),
					suggestionOpenRef,
				),
			// eslint-disable-next-line react-hooks/exhaustive-deps
			[],
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
				SlashCommandNode,
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
						// Don't submit if suggestion menu is open
						if (suggestionOpenRef.current) {
							return false; // Let suggestion plugin handle it
						}
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
