import CodeEditor from "@/components/custom/code-editor";
import { DockviewReact, type DockviewReadyEvent, type IDockviewPanelProps, type IDockviewPanelHeaderProps } from "@/components/custom/dockview";
import repoModel from "@/models/repo.model";
import agentModel from "@/models/agent.model";
import { agentApi } from "@/lib/agent-api";
import { getGatewayUrl } from "@/models/settings.model";
import { cn } from "@/lib/utils";
import AgentChat from "@/pages/agent/components/agent-chat";
import { TaskListPanel } from "@/components/custom/task-list-panel";
import {
	ChevronDown,
	ChevronRight,
	File,
	Folder,
	FolderOpen,
	ArrowLeft,
	Loader2,
	FileCode2,
	GitBranch,
	X,
	Bot,
	ListTodo,
	Pencil,
	Trash2,
	FilePlus,
	FolderPlus,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { toast } from "sonner";
import { getClassWithColor } from "file-icons-js";
import { useNavigate, useParams } from "react-router-dom";
import { useSnapshot } from "valtio";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface FsNode {
	name: string;
	path: string;
	is_dir: boolean;
	children?: FsNode[];
}

// ---------------------------------------------------------------------------
// FS API helpers
// ---------------------------------------------------------------------------

function gatewayUrl() {
	return getGatewayUrl();
}

async function fetchTree(path: string, depth = 2): Promise<FsNode> {
	const res = await fetch(
		`${gatewayUrl()}/api/fs/tree?path=${encodeURIComponent(path)}&depth=${depth}`,
	);
	if (!res.ok) throw new Error(await res.text());
	return res.json();
}

async function fetchFile(path: string): Promise<string> {
	const res = await fetch(
		`${gatewayUrl()}/api/fs/file?path=${encodeURIComponent(path)}`,
	);
	if (!res.ok) throw new Error(await res.text());
	const data = await res.json();
	return data.content as string;
}

async function writeFile(path: string, content: string): Promise<void> {
	const res = await fetch(`${gatewayUrl()}/api/fs/file`, {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ path, content }),
	});
	if (!res.ok) throw new Error(await res.text());
}

async function createFsNode(path: string, is_dir: boolean): Promise<void> {
	const res = await fetch(`${gatewayUrl()}/api/fs/create`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ path, is_dir }),
	});
	if (!res.ok) throw new Error(await res.text());
}

async function renameFsNode(from: string, to: string): Promise<void> {
	const res = await fetch(`${gatewayUrl()}/api/fs/rename`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ from, to }),
	});
	if (!res.ok) throw new Error(await res.text());
}

async function deleteFsNode(path: string): Promise<void> {
	const res = await fetch(
		`${gatewayUrl()}/api/fs/delete?path=${encodeURIComponent(path)}`,
		{ method: "DELETE" },
	);
	if (!res.ok) throw new Error(await res.text());
}

// ---------------------------------------------------------------------------
// Language detection
// ---------------------------------------------------------------------------

function detectLanguage(filename: string): string {
	const ext = filename.split(".").pop()?.toLowerCase() ?? "";
	const map: Record<string, string> = {
		rs: "rust", ts: "typescript", tsx: "typescript",
		js: "javascript", jsx: "javascript", py: "python",
		go: "go", java: "java", cpp: "cpp", c: "c",
		cs: "csharp", rb: "ruby", sh: "shell", bash: "shell",
		zsh: "shell", json: "json", toml: "toml", yaml: "yaml",
		yml: "yaml", md: "markdown", mdx: "markdown", html: "html",
		css: "css", scss: "scss", sql: "sql", xml: "xml",
		hcl: "hcl", tf: "hcl", dockerfile: "dockerfile",
	};
	return map[ext] ?? "plaintext";
}

// ---------------------------------------------------------------------------
// File tree node
// ---------------------------------------------------------------------------

type ContextMenuState = {
	x: number;
	y: number;
	node: FsNode;
} | null;

function TreeNode({
	node,
	depth,
	onFileClick,
	onExpandDir,
	activeFile,
	onRefresh,
}: {
	node: FsNode;
	depth: number;
	onFileClick: (path: string) => void;
	onExpandDir: (path: string) => Promise<FsNode[]>;
	activeFile: string | null;
	onRefresh: () => void;
}) {
	const [open, setOpen] = useState(depth === 0);
	const [children, setChildren] = useState<FsNode[]>(node.children ?? []);
	const [loading, setLoading] = useState(false);
	const [renaming, setRenaming] = useState(false);
	const [renameValue, setRenameValue] = useState("");
	const [creating, setCreating] = useState<"file" | "folder" | null>(null);
	const [createValue, setCreateValue] = useState("");
	const [contextMenu, setContextMenu] = useState<ContextMenuState>(null);

	const toggle = async () => {
		if (!node.is_dir) {
			onFileClick(node.path);
			return;
		}
		if (!open && children.length === 0) {
			setLoading(true);
			try {
				const loaded = await onExpandDir(node.path);
				setChildren(loaded);
			} finally {
				setLoading(false);
			}
		}
		setOpen((v) => !v);
	};

	const handleContextMenu = (e: React.MouseEvent) => {
		e.preventDefault();
		e.stopPropagation();
		setContextMenu({ x: e.clientX, y: e.clientY, node });
	};

	const closeMenu = () => setContextMenu(null);

	const handleRename = () => {
		setRenameValue(node.name);
		setRenaming(true);
		closeMenu();
	};

	const commitRename = async () => {
		const newName = renameValue.trim();
		if (!newName || newName === node.name) { setRenaming(false); return; }
		const dir = node.path.substring(0, node.path.lastIndexOf("/"));
		const newPath = `${dir}/${newName}`;
		try {
			await renameFsNode(node.path, newPath);
			onRefresh();
		} catch (e) {
			toast.error(`重命名失败: ${(e as Error).message}`);
		}
		setRenaming(false);
	};

	const handleDelete = async () => {
		closeMenu();
		try {
			await deleteFsNode(node.path);
			onRefresh();
		} catch (e) {
			toast.error(`删除失败: ${(e as Error).message}`);
		}
	};

	const handleCreate = (type: "file" | "folder") => {
		setCreating(type);
		setCreateValue("");
		if (!open) setOpen(true);
		closeMenu();
	};

	const commitCreate = async () => {
		const name = createValue.trim();
		if (!name) { setCreating(null); return; }
		const newPath = `${node.path}/${name}`;
		try {
			await createFsNode(newPath, creating === "folder");
			onRefresh();
		} catch (e) {
			toast.error(`创建失败: ${(e as Error).message}`);
		}
		setCreating(null);
	};

	const [dragOver, setDragOver] = useState(false);

	const handleDragStart = (e: React.DragEvent) => {
		e.dataTransfer.setData("text/plain", node.path);
		e.dataTransfer.effectAllowed = "move";
	};

	const handleDrop = async (e: React.DragEvent) => {
		e.preventDefault();
		e.stopPropagation();
		setDragOver(false);
		const srcPath = e.dataTransfer.getData("text/plain");
		if (!srcPath) return;

		// Determine target dir: if dropped on a file, use its parent dir
		const targetDir = node.is_dir
			? node.path
			: node.path.substring(0, node.path.lastIndexOf("/"));

		if (!targetDir || srcPath === targetDir) return;
		if (srcPath.startsWith(targetDir + "/")) return;

		const name = srcPath.split("/").pop();
		if (!name) return;
		const destPath = `${targetDir}/${name}`;
		if (srcPath === destPath) return;
		try {
			await renameFsNode(srcPath, destPath);
			onRefresh();
		} catch (err) {
			toast.error(`移动失败: ${(err as Error).message}`);
		}
	};

	const indent = depth * 12;

	return (
		<div>
			{/* Context menu overlay */}
			{contextMenu && (
				<div
					className="fixed inset-0 z-40"
					onClick={closeMenu}
					onContextMenu={(e) => { e.preventDefault(); closeMenu(); }}
				/>
			)}
			{contextMenu && (
				<div
					className="fixed z-50 min-w-36 rounded-md border bg-popover text-popover-foreground shadow-md py-1 text-[12px]"
					style={{ left: contextMenu.x, top: contextMenu.y }}
				>
					{node.is_dir && (
						<>
							<button type="button" className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-accent transition-colors" onClick={() => handleCreate("file")}>
								<FilePlus className="size-3 shrink-0" /> 新建文件
							</button>
							<button type="button" className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-accent transition-colors" onClick={() => handleCreate("folder")}>
								<FolderPlus className="size-3 shrink-0" /> 新建文件夹
							</button>
							<div className="my-1 border-t" />
						</>
					)}
					<button type="button" className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-accent transition-colors" onClick={handleRename}>
						<Pencil className="size-3 shrink-0" /> 重命名
					</button>
					<button type="button" className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-accent text-destructive transition-colors" onClick={handleDelete}>
						<Trash2 className="size-3 shrink-0" /> 删除
					</button>
				</div>
			)}

			{renaming ? (
				<div className="flex items-center gap-1 px-2 py-[3px]" style={{ paddingLeft: `${8 + indent}px` }}>
					<input
						autoFocus
						type="text"
						value={renameValue}
						onChange={(e) => setRenameValue(e.target.value)}
						onBlur={commitRename}
						onKeyDown={(e) => { if (e.key === "Enter") commitRename(); else if (e.key === "Escape") setRenaming(false); }}
						className="flex-1 min-w-0 text-[12px] font-mono bg-white border border-blue-400 rounded px-1 outline-none"
					/>
				</div>
			) : (
				<button
					type="button"
					draggable
					className={cn(
						"flex items-center gap-1 w-full text-left px-2 py-[3px] text-[12px] transition-colors [&>*]:pointer-events-none",
						!node.is_dir && activeFile === node.path
							? "bg-blue-100 text-blue-900"
							: "text-slate-700 hover:bg-slate-100",
						node.is_dir && "text-slate-800",
						dragOver && "bg-blue-100 ring-1 ring-blue-400",
					)}
					style={{ paddingLeft: `${8 + indent}px` }}
					onClick={toggle}
					onContextMenu={handleContextMenu}
					onDragStart={handleDragStart}
					onDragOver={(e) => { if (node.is_dir) { e.preventDefault(); e.dataTransfer.dropEffect = "move"; setDragOver(true); } }}
					onDragLeave={() => setDragOver(false)}
					onDrop={handleDrop}
				>
					{node.is_dir ? (
						<>
							{loading ? (
								<Loader2 className="size-3 shrink-0 animate-spin text-slate-400" />
							) : open ? (
								<ChevronDown className="size-3 shrink-0 text-slate-400" />
							) : (
								<ChevronRight className="size-3 shrink-0 text-slate-400" />
							)}
							{open ? (
								<FolderOpen className="size-3.5 shrink-0 text-amber-400" />
							) : (
								<Folder className="size-3.5 shrink-0 text-amber-400" />
							)}
						</>
					) : (
						<>
							<span className="size-3 shrink-0" />
							{getClassWithColor(node.name) ? (
								<span className={cn("size-3.5 shrink-0 text-[14px] leading-none", getClassWithColor(node.name))} />
							) : (
								<File className="size-3.5 shrink-0 text-slate-400" />
							)}
						</>
					)}
					<span className="truncate font-mono">{node.name}</span>
				</button>
			)}

			{node.is_dir && open && (
				<div>
					{creating && (
						<div className="flex items-center gap-1 px-2 py-[3px]" style={{ paddingLeft: `${8 + (depth + 1) * 12}px` }}>
							{creating === "folder"
								? <Folder className="size-3.5 shrink-0 text-amber-400" />
								: <File className="size-3.5 shrink-0 text-slate-400" />
							}
							<input
								autoFocus
								type="text"
								value={createValue}
								onChange={(e) => setCreateValue(e.target.value)}
								onBlur={commitCreate}
								onKeyDown={(e) => { if (e.key === "Enter") commitCreate(); else if (e.key === "Escape") setCreating(null); }}
								className="flex-1 min-w-0 text-[12px] font-mono bg-white border border-blue-400 rounded px-1 outline-none"
							/>
						</div>
					)}
					{children.map((child) => (
						<TreeNode
							key={child.path}
							node={child}
							depth={depth + 1}
							onFileClick={onFileClick}
							onExpandDir={onExpandDir}
							activeFile={activeFile}
							onRefresh={onRefresh}
						/>
					))}
				</div>
			)}
		</div>
	);
}

// ---------------------------------------------------------------------------
// File tree panel (left)
// ---------------------------------------------------------------------------

function FileTreePanel({
	repoPath,
	onFileClick,
	activeFile,
}: {
	repoPath: string;
	onFileClick: (path: string) => void;
	activeFile: string | null;
}) {
	const [root, setRoot] = useState<FsNode | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [refreshKey, setRefreshKey] = useState(0);
	const [rootCreating, setRootCreating] = useState<"file" | "folder" | null>(null);
	const [rootCreateValue, setRootCreateValue] = useState("");
	const [rootContextMenu, setRootContextMenu] = useState<{ x: number; y: number } | null>(null);

	useEffect(() => {
		fetchTree(repoPath, 2)
			.then(setRoot)
			.catch((e) => setError(e.message));
	}, [repoPath, refreshKey]);

	// Watch for file system changes via WebSocket
	useEffect(() => {
		const wsUrl = getGatewayUrl().replace(/^http/, "ws");
		const url = `${wsUrl}/ws/fs/watch?path=${encodeURIComponent(repoPath)}`;
		console.log("[fs-watch] connecting:", url);
		const ws = new WebSocket(url);
		ws.onopen = () => console.log("[fs-watch] connected");
		ws.onerror = (e) => console.warn("[fs-watch] error", e);
		ws.onclose = () => console.log("[fs-watch] closed");
		ws.onmessage = (e) => {
			console.log("[fs-watch] event:", e.data);
			setRefreshKey((k) => k + 1);
		};
		return () => ws.close();
	}, [repoPath]);

	const handleExpandDir = useCallback(async (path: string): Promise<FsNode[]> => {
		const node = await fetchTree(path, 1);
		return node.children ?? [];
	}, []);

	const commitRootCreate = async () => {
		const name = rootCreateValue.trim();
		if (!name) { setRootCreating(null); return; }
		try {
			await createFsNode(`${repoPath}/${name}`, rootCreating === "folder");
			setRefreshKey((k) => k + 1);
		} catch (e) {
			toast.error(`创建失败: ${(e as Error).message}`);
		}
		setRootCreating(null);
	};

	const handleRootContextMenu = (e: React.MouseEvent) => {
		e.preventDefault();
		setRootContextMenu({ x: e.clientX, y: e.clientY });
	};

	if (error) {
		return <div className="p-3 text-[11px] text-destructive">{error}</div>;
	}

	if (!root) {
		return (
			<div className="flex items-center justify-center h-full">
				<Loader2 className="size-4 animate-spin text-muted-foreground" />
			</div>
		);
	}

	return (
		<div
			className="h-full overflow-y-auto py-1"
			onContextMenu={handleRootContextMenu}
		>
			{rootContextMenu && (
				<div className="fixed inset-0 z-40" onClick={() => setRootContextMenu(null)} onContextMenu={(e) => { e.preventDefault(); setRootContextMenu(null); }} />
			)}
			{rootContextMenu && (
				<div
					className="fixed z-50 min-w-36 rounded-md border bg-popover text-popover-foreground shadow-md py-1 text-[12px]"
					style={{ left: rootContextMenu.x, top: rootContextMenu.y }}
				>
					<button type="button" className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-accent transition-colors" onClick={() => { setRootCreating("file"); setRootCreateValue(""); setRootContextMenu(null); }}>
						<FilePlus className="size-3 shrink-0" /> 新建文件
					</button>
					<button type="button" className="flex items-center gap-2 w-full px-3 py-1.5 hover:bg-accent transition-colors" onClick={() => { setRootCreating("folder"); setRootCreateValue(""); setRootContextMenu(null); }}>
						<FolderPlus className="size-3 shrink-0" /> 新建文件夹
					</button>
				</div>
			)}
			{rootCreating && (
				<div className="flex items-center gap-1 px-2 py-[3px]" style={{ paddingLeft: "8px" }}>
					{rootCreating === "folder"
						? <Folder className="size-3.5 shrink-0 text-amber-400" />
						: <File className="size-3.5 shrink-0 text-slate-400" />
					}
					<input
						autoFocus
						type="text"
						value={rootCreateValue}
						onChange={(e) => setRootCreateValue(e.target.value)}
						onBlur={commitRootCreate}
						onKeyDown={(e) => { if (e.key === "Enter") commitRootCreate(); else if (e.key === "Escape") setRootCreating(null); }}
						className="flex-1 min-w-0 text-[12px] font-mono bg-white border border-blue-400 rounded px-1 outline-none"
					/>
				</div>
			)}
			{(root.children ?? []).map((child) => (
				<TreeNode
					key={child.path}
					node={child}
					depth={0}
					onFileClick={onFileClick}
					onExpandDir={handleExpandDir}
					activeFile={activeFile}
					onRefresh={() => setRefreshKey((k) => k + 1)}
				/>
			))}
		</div>
	);
}

// ---------------------------------------------------------------------------
// File type detection
// ---------------------------------------------------------------------------

const IMAGE_EXTS = new Set(["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "ico", "tiff"]);
const PDF_EXTS = new Set(["pdf"]);
const VIDEO_EXTS = new Set(["mp4", "webm", "mov", "avi", "mkv"]);
const AUDIO_EXTS = new Set(["mp3", "wav", "ogg", "flac", "aac", "m4a"]);

function getFileType(filePath: string): "image" | "pdf" | "video" | "audio" | "text" {
	const ext = filePath.split(".").pop()?.toLowerCase() ?? "";
	if (IMAGE_EXTS.has(ext)) return "image";
	if (PDF_EXTS.has(ext)) return "pdf";
	if (VIDEO_EXTS.has(ext)) return "video";
	if (AUDIO_EXTS.has(ext)) return "audio";
	return "text";
}

// ---------------------------------------------------------------------------
// Editor panel (right) — registered as dockview component
// ---------------------------------------------------------------------------

interface EditorPanelParams {
	filePath: string;
	onSave?: (path: string, content: string) => void;
	onCursorChange?: (line: number, col: number) => void;
	onDirtyChange?: (dirty: boolean) => void;
}

function EditorPanel({ params }: IDockviewPanelProps<EditorPanelParams>) {
	const { filePath, onSave, onCursorChange, onDirtyChange } = params;
	const fileType = getFileType(filePath);
	const fileUrl = `${getGatewayUrl()}/api/fs/file?path=${encodeURIComponent(filePath)}&raw=1`;

	// Image viewer
	if (fileType === "image") {
		return (
			<div className="flex items-center justify-center h-full bg-[#1e1e1e] p-4">
				<img
					src={fileUrl}
					alt={filePath.split("/").pop()}
					className="max-w-full max-h-full object-contain rounded"
				/>
			</div>
		);
	}

	// PDF viewer
	if (fileType === "pdf") {
		return (
			<div className="h-full w-full">
				<iframe src={fileUrl} className="w-full h-full border-0" title={filePath.split("/").pop()} />
			</div>
		);
	}

	// Video viewer
	if (fileType === "video") {
		return (
			<div className="flex items-center justify-center h-full bg-black">
				<video src={fileUrl} controls className="max-w-full max-h-full" />
			</div>
		);
	}

	// Audio viewer
	if (fileType === "audio") {
		return (
			<div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground">
				<FileCode2 className="size-12 opacity-20" />
				<p className="text-sm font-mono">{filePath.split("/").pop()}</p>
				<audio src={fileUrl} controls className="w-64" />
			</div>
		);
	}

	// Text / code editor (default)
	return <TextEditorPanel filePath={filePath} onSave={onSave} onCursorChange={onCursorChange} onDirtyChange={onDirtyChange} />;
}

function TextEditorPanel({ filePath, onSave, onCursorChange, onDirtyChange }: {
	filePath: string;
	onSave?: (path: string, content: string) => void;
	onCursorChange?: (line: number, col: number) => void;
	onDirtyChange?: (dirty: boolean) => void;
}) {
	const [content, setContent] = useState<string | null>(null);
	const [error, setError] = useState<string | null>(null);
	const [saving, setSaving] = useState(false);
	const dirtyRef = useRef(false);
	const valueRef = useRef<string>("");

	useEffect(() => {
		setContent(null);
		setError(null);
		dirtyRef.current = false;
		onDirtyChange?.(false);
		fetchFile(filePath)
			.then((c) => {
				setContent(c);
				valueRef.current = c;
			})
			.catch((e) => setError(e.message));
	}, [filePath]);

	const handleChange = (val: string | undefined) => {
		valueRef.current = val ?? "";
		if (!dirtyRef.current) {
			dirtyRef.current = true;
			onDirtyChange?.(true);
		}
	};

	const save = useCallback(async () => {
		if (!dirtyRef.current) return;
		setSaving(true);
		try {
			await writeFile(filePath, valueRef.current);
			dirtyRef.current = false;
			onDirtyChange?.(false);
			onSave?.(filePath, valueRef.current);
		} catch (e) {
			toast.error(`保存失败: ${(e as Error).message}`);
		} finally {
			setSaving(false);
		}
	}, [filePath, onSave, onDirtyChange]);

	// Ctrl/Cmd+S to save
	useEffect(() => {
		const handler = (e: KeyboardEvent) => {
			if ((e.metaKey || e.ctrlKey) && e.key === "s") {
				e.preventDefault();
				save();
			}
		};
		window.addEventListener("keydown", handler);
		return () => window.removeEventListener("keydown", handler);
	}, [save]);

	if (error) {
		return (
			<div className="flex items-center justify-center h-full text-[12px] text-destructive p-4">
				{error}
			</div>
		);
	}

	if (content === null) {
		return (
			<div className="flex items-center justify-center h-full">
				<Loader2 className="size-4 animate-spin text-muted-foreground" />
			</div>
		);
	}

	return (
		<div className="flex flex-col h-full w-full">
			<div className="flex-1 min-h-0">
				<CodeEditor
					value={content}
					language={detectLanguage(filePath.split("/").pop() ?? "")}
					onChange={handleChange}
					path={filePath}
					theme="vs"
					onMount={(editor, monaco) => {
						editor.onDidChangeCursorPosition((e) => {
							onCursorChange?.(e.position.lineNumber, e.position.column);
						});
						// Force layout so Monaco correctly measures its container
						requestAnimationFrame(() => editor.layout());
						const ro = new ResizeObserver(() => editor.layout());
						ro.observe(editor.getContainerDomNode());
					}}
				/>
			</div>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Custom tab component
// ---------------------------------------------------------------------------

function EditorTab({ api }: IDockviewPanelHeaderProps) {
	const [isActive, setIsActive] = useState(api.isGroupActive);
	const [isDirty, setIsDirty] = useState(false);
	const title = api.title ?? "";

	useEffect(() => {
		const d = api.onDidActiveGroupChange((e) => setIsActive(e.isActive));
		return () => d.dispose();
	}, [api]);

	// Listen for dirty state changes via params updates
	useEffect(() => {
		const d = api.onDidParametersChange((params) => {
			setIsDirty(!!(params as EditorPanelParams & { dirty?: boolean }).dirty);
		});
		return () => d.dispose();
	}, [api]);

	return (
		<div
			className={cn(
				"flex items-center gap-1.5 px-3 h-full text-[12px] select-none group border-r border-border/50",
				isActive
					? "text-foreground bg-background"
					: "text-muted-foreground bg-muted/40 hover:bg-muted/70",
			)}
		>
			{getClassWithColor(title) ? (
				<span className={cn("size-3.5 shrink-0 text-[13px] leading-none", getClassWithColor(title))} />
			) : (
				<File className="size-3 shrink-0 opacity-60" />
			)}
			<span className="font-mono max-w-[120px] truncate">{isDirty ? `${title} ●` : title}</span>
			<button
				type="button"
				onClick={(e) => { e.stopPropagation(); api.close(); }}
				className={cn(
					"size-4 flex items-center justify-center rounded transition-colors ml-0.5",
					"opacity-0 group-hover:opacity-100",
					isActive && "opacity-60 hover:opacity-100",
					"hover:bg-foreground/10",
				)}
			>
				<X className="size-2.5" />
			</button>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Watermark (empty state)
// ---------------------------------------------------------------------------

function EditorWatermark() {
	return (
		<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground bg-background">
			<FileCode2 className="size-10 opacity-20" />
			<p className="text-sm">从左侧选择文件打开</p>
		</div>
	);
}

// ---------------------------------------------------------------------------
// Page
// ---------------------------------------------------------------------------

const DOCKVIEW_COMPONENTS = { editor: EditorPanel };

export default function RepoEditorPage() {
	const { id } = useParams<{ id: string }>();
	const navigate = useNavigate();
	const [searchParams] = useSearchParams();
	const { repos } = useSnapshot(repoModel.state);
	const repo = repos.find((r) => r.id === id);

	const [activeFile, setActiveFile] = useState<string | null>(null);
	const [treeWidth, setTreeWidth] = useState(224);
	const [cursor, setCursor] = useState<{ line: number; col: number } | null>(null);
	const [worktrees, setWorktrees] = useState<{ path: string; branch: string; is_current: boolean }[]>([]);
	const [showWorktrees, setShowWorktrees] = useState(false);
	const [showCopilot, setShowCopilot] = useState(true);
	const [showTaskList, setShowTaskList] = useState(false);
	const [copilotWidth, setCopilotWidth] = useState(520);
	const [creatingSession, setCreatingSession] = useState(false);
	const [copilotSessionId, setCopilotSessionId] = useState<string | null>(null);
	const resolvedSessionId = copilotSessionId;
	const dockviewApiRef = useRef<import("@/components/custom/dockview-core").DockviewApi | null>(null);

	const handleCreateCopilotSession = useCallback(async () => {
		if (!repo || creatingSession) return;
		setCreatingSession(true);
		try {
			const session = await agentApi.createSession({ cwd: repo.path });
			if (session?.session_id) {
				const sessions = await agentApi.listSessions();
				if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
				const { connectSession } = await import("@/hooks/use-agent-ws");
				connectSession(session.session_id);
				setCopilotSessionId(session.session_id);
			}
		} finally {
			setCreatingSession(false);
		}
	}, [repo, creatingSession]);

	const handleReady = useCallback((e: DockviewReadyEvent) => {
		dockviewApiRef.current = e.api;
	}, []);

	// Auto-open copilot when ?copilot=1 is in the URL
	useEffect(() => {
		if (searchParams.get("copilot") === "1") {
			setShowCopilot(true);
		}
	}, [searchParams]);

	// Auto-create a session when copilot opens and no session exists
	useEffect(() => {
		if (showCopilot && !resolvedSessionId && !creatingSession && repo) {
			handleCreateCopilotSession();
		}
	}, [showCopilot, resolvedSessionId]);

	// Load worktrees
	useEffect(() => {
		if (!repo) return;
		fetch(`${getGatewayUrl()}/api/git/worktrees?path=${encodeURIComponent(repo.path)}`)
			.then((r) => r.json())
			.then(setWorktrees)
			.catch(() => setWorktrees([]));
	}, [repo?.path]);

	const currentBranch = worktrees.find((w) => w.is_current)?.branch ?? repo?.name ?? "";

	const handleCheckout = async (branch: string) => {
		if (!repo) return;
		setShowWorktrees(false);
		await fetch(`${getGatewayUrl()}/api/git/checkout`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ path: repo.path, branch }),
		});
		// Refresh worktrees
		fetch(`${getGatewayUrl()}/api/git/worktrees?path=${encodeURIComponent(repo.path)}`)
			.then((r) => r.json())
			.then(setWorktrees)
			.catch(() => {});
	};

	const handleResizeMouseDown = useCallback((e: React.MouseEvent) => {
		e.preventDefault();
		const startX = e.clientX;
		const startWidth = treeWidth;

		const onMouseMove = (ev: MouseEvent) => {
			const delta = ev.clientX - startX;
			setTreeWidth(Math.max(140, Math.min(480, startWidth + delta)));
		};
		const onMouseUp = () => {
			document.body.style.cursor = "";
			document.body.style.userSelect = "";
			window.removeEventListener("mousemove", onMouseMove);
			window.removeEventListener("mouseup", onMouseUp);
		};
		document.body.style.cursor = "col-resize";
		document.body.style.userSelect = "none";
		window.addEventListener("mousemove", onMouseMove);
		window.addEventListener("mouseup", onMouseUp);
	}, [treeWidth]);

	const handleCopilotResizeMouseDown = useCallback((e: React.MouseEvent) => {
		e.preventDefault();
		const startX = e.clientX;
		const startWidth = copilotWidth;
		const onMouseMove = (ev: MouseEvent) => {
			const delta = startX - ev.clientX;
			setCopilotWidth(Math.max(240, Math.min(600, startWidth + delta)));
		};
		const onMouseUp = () => {
			document.body.style.cursor = "";
			document.body.style.userSelect = "";
			window.removeEventListener("mousemove", onMouseMove);
			window.removeEventListener("mouseup", onMouseUp);
		};
		document.body.style.cursor = "col-resize";
		document.body.style.userSelect = "none";
		window.addEventListener("mousemove", onMouseMove);
		window.addEventListener("mouseup", onMouseUp);
	}, [copilotWidth]);

	const handleFileClick = useCallback((filePath: string) => {
		setActiveFile(filePath);
		setCursor(null);
		const api = dockviewApiRef.current;
		if (!api) return;

		const panelId = `file:${filePath}`;
		const existing = api.getPanel(panelId);
		if (existing) {
			existing.focus();
			return;
		}

		const filename = filePath.split("/").pop() ?? filePath;
		api.addPanel({
			id: panelId,
			component: "editor",
			title: filename,
			params: {
				filePath,
				onCursorChange: (line: number, col: number) => setCursor({ line, col }),
				onDirtyChange: (dirty: boolean) => {
					const panel = api.getPanel(panelId);
					if (panel) panel.api.updateParameters({ dirty });
				},
			},
		});
	}, []);

	if (!repo) {
		return (
			<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
				<p className="text-sm">工作区不存在</p>
				<button
					type="button"
					className="text-xs underline"
					onClick={() => navigate("/repos")}
				>
					返回工作区列表
				</button>
			</div>
		);
	}

	return (
		<div className="flex flex-col h-full">
			{/* Top bar */}
			<div className="flex items-center gap-2 px-3 py-2 border-b bg-background shrink-0">
				<button
					type="button"
					className="flex items-center justify-center size-6 rounded text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06] transition-colors"
					onClick={() => navigate("/repos")}
					aria-label="返回"
				>
					<ArrowLeft className="size-3.5" />
				</button>
				<span className="text-xs font-semibold">{repo.name}</span>
				<span className="text-[11px] text-muted-foreground font-mono truncate flex-1">
					{repo.path}
				</span>
				<div className="relative shrink-0">
					<button
						type="button"
						title="任务列表"
						onClick={() => setShowTaskList((v) => !v)}
						className={cn(
							"flex items-center justify-center size-6 rounded transition-colors",
							showTaskList ? "text-primary bg-primary/10" : "text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06]",
						)}
					>
						<ListTodo className="size-3.5" />
					</button>
					{showTaskList && resolvedSessionId && (
						<div className="absolute top-full right-0 mt-1 z-50 w-72 rounded-xl border bg-popover shadow-lg overflow-hidden">
							<div className="flex items-center justify-between px-3 py-2 border-b">
								<span className="text-xs font-medium">任务列表</span>
								<button
									type="button"
									onClick={() => setShowTaskList(false)}
									className="text-muted-foreground hover:text-foreground transition-colors"
								>
									<X className="size-3.5" />
								</button>
							</div>
							<div className="h-72">
								<TaskListPanel sessionId={resolvedSessionId} />
							</div>
						</div>
					)}
				</div>
				<button
					type="button"
					title="副驾驶"
					onClick={() => setShowCopilot((v) => !v)}
					className={cn(
						"flex items-center justify-center size-6 rounded transition-colors shrink-0",
						showCopilot ? "text-primary bg-primary/10" : "text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06]",
					)}
				>
					<Bot className="size-3.5" />
				</button>
			</div>

			{/* Main: file tree + dockview editor */}
			<div className="flex flex-1 min-h-0">
				{/* File tree */}
				<div className="shrink-0 border-r bg-slate-50 overflow-hidden flex flex-col relative" style={{ width: treeWidth }}>
					<div className="px-3 py-1.5 text-[10px] font-semibold text-slate-500 uppercase tracking-wider border-b border-slate-200">
						资源管理器
					</div>
					<div className="flex-1 min-h-0">
						<FileTreePanel
							repoPath={repo.path}
							onFileClick={handleFileClick}
							activeFile={activeFile}
						/>
					</div>
					{/* Resize handle */}
					<div
						className="absolute right-0 top-0 h-full w-1 cursor-col-resize hover:bg-blue-400/40 transition-colors"
						onMouseDown={handleResizeMouseDown}
					/>
				</div>

				{/* Dockview editor area */}
				<div className="flex-1 min-w-0">
					<DockviewReact
						className="dockview-theme-light"
						components={DOCKVIEW_COMPONENTS}
						watermarkComponent={EditorWatermark}
						defaultTabComponent={EditorTab}
						onReady={handleReady}
					/>
				</div>

				{/* Copilot panel */}
				{showCopilot && (
					<div className="shrink-0 border-l bg-background relative flex flex-col" style={{ width: copilotWidth }}>
						{/* Resize handle */}
						<div
							className="absolute left-0 top-0 h-full w-1 cursor-col-resize hover:bg-blue-400/40 transition-colors z-10"
							onMouseDown={handleCopilotResizeMouseDown}
						/>
						{resolvedSessionId ? (
							<div className="flex-1 min-h-0 overflow-hidden flex flex-col">
								<AgentChat sessionId={resolvedSessionId} cwd={repo.path} onSessionChange={(id) => setCopilotSessionId(id)} />
							</div>
						) : (
							<div className="flex flex-col items-center justify-center flex-1 gap-3 text-muted-foreground p-4">
								<Bot className="size-8 opacity-20" />
								<p className="text-xs text-center">暂无智能体会话</p>
								<button
									type="button"
									onClick={() => navigate("/")}
									className="text-xs text-primary hover:underline"
								>
									前往创建智能体
								</button>
							</div>
						)}
					</div>
				)}
			</div>

			{/* Status bar */}
			<div className="relative flex items-center justify-between px-3 shrink-0 h-7 text-[11px] font-mono select-none" style={{ background: "hsl(var(--primary) / 0.8)", color: "hsl(var(--primary-foreground))" }}>
				<div className="flex items-center gap-1">
					<button
						type="button"
						className="flex items-center gap-1 px-1.5 py-0.5 rounded hover:bg-white/10 transition-colors"
						onClick={() => setShowWorktrees((v) => !v)}
					>
						<GitBranch className="size-3" />
						<span>{currentBranch}</span>
					</button>
					{showWorktrees && worktrees.length > 0 && (
						<div className="absolute left-2 bottom-7 z-50 min-w-48 rounded-md border bg-popover text-popover-foreground shadow-md py-1 text-[12px]">
							{worktrees.map((wt) => (
								<button
									key={wt.path}
									type="button"
									className={cn(
										"flex items-center gap-2 w-full px-3 py-1.5 text-left hover:bg-accent transition-colors",
										wt.is_current && "font-semibold text-primary",
									)}
									onClick={() => handleCheckout(wt.branch)}
								>
									<GitBranch className="size-3 shrink-0" />
									{wt.branch}
								</button>
							))}
						</div>
					)}
				</div>
				<div className="flex items-center gap-3">
					{activeFile && (
						<span className="opacity-80">{detectLanguage(activeFile.split("/").pop() ?? "")}</span>
					)}
					{cursor && (
						<span className="opacity-80">行 {cursor.line}，列 {cursor.col}</span>
					)}
				</div>
			</div>
		</div>
	);
}
