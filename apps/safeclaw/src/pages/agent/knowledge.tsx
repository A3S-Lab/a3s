/**
 * Knowledge Base Manager - Multi-knowledge base management with VS Code-style editor
 */
import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import CodeEditor from "@/components/custom/code-editor";
import {
	DockviewReact,
	type DockviewReadyEvent,
	type IDockviewPanelProps,
} from "@/components/custom/dockview";
import {
	SidebarList,
	SidebarListItem,
	SidebarListEmpty,
} from "@/components/custom/sidebar-list";
import { getGatewayUrl } from "@/models/settings.model";
import settingsModel from "@/models/settings.model";
import { cn } from "@/lib/utils";
import {
	ArrowLeft,
	Loader2,
	Plus,
	Trash2,
	Save,
	File,
	Folder,
	FolderOpen,
	ChevronRight,
	ChevronDown,
	FilePlus,
	FolderPlus,
	BookOpen,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState, useMemo } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useSnapshot } from "valtio";

interface FsNode {
	name: string;
	path: string;
	is_dir: boolean;
	children?: FsNode[];
}

interface KnowledgeBase {
	name: string;
	path: string;
}

// FS API helpers
async function fetchTree(path: string): Promise<FsNode> {
	const res = await fetch(
		`${getGatewayUrl()}/api/fs/tree?path=${encodeURIComponent(path)}&depth=3`,
	);
	if (!res.ok) {
		if (res.status === 404 || res.status === 400) {
			return { name: "", path, is_dir: true, children: [] };
		}
		throw new Error(await res.text());
	}
	return res.json();
}

async function fetchFile(path: string): Promise<string> {
	const res = await fetch(
		`${getGatewayUrl()}/api/fs/file?path=${encodeURIComponent(path)}`,
	);
	if (!res.ok) throw new Error(await res.text());
	const data = await res.json();
	return data.content as string;
}

async function writeFile(path: string, content: string): Promise<void> {
	const res = await fetch(`${getGatewayUrl()}/api/fs/file`, {
		method: "PUT",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ path, content }),
	});
	if (!res.ok) throw new Error(await res.text());
}

async function deleteFile(path: string): Promise<void> {
	const res = await fetch(
		`${getGatewayUrl()}/api/fs/delete?path=${encodeURIComponent(path)}`,
		{ method: "DELETE" },
	);
	if (!res.ok) throw new Error(await res.text());
}

async function createNode(path: string, is_dir: boolean): Promise<void> {
	const res = await fetch(`${getGatewayUrl()}/api/fs/create`, {
		method: "POST",
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify({ path, is_dir }),
	});
	if (!res.ok) throw new Error(await res.text());
}

// Tree node component
function TreeNode({
	node,
	depth,
	activeFile,
	onFileClick,
	onDelete,
}: {
	node: FsNode;
	depth: number;
	activeFile: string | null;
	onFileClick: (path: string) => void;
	onDelete: (path: string) => void;
}) {
	const [open, setOpen] = useState(depth === 0);
	const isActive = activeFile === node.path;

	if (node.is_dir) {
		return (
			<div>
				<div className="group flex items-center justify-between hover:bg-muted/60 rounded-lg">
					<button
						type="button"
						onClick={() => setOpen(!open)}
						className="flex items-center gap-1 flex-1 px-2 py-1 text-left"
						style={{ paddingLeft: `${8 + depth * 12}px` }}
					>
						{open ? (
							<ChevronDown className="size-3 shrink-0 text-muted-foreground" />
						) : (
							<ChevronRight className="size-3 shrink-0 text-muted-foreground" />
						)}
						{open ? (
							<FolderOpen className="size-3.5 shrink-0 text-yellow-500" />
						) : (
							<Folder className="size-3.5 shrink-0 text-yellow-500" />
						)}
						<span className="text-xs truncate">{node.name}</span>
					</button>
					<button
						type="button"
						onClick={(e) => {
							e.stopPropagation();
							onDelete(node.path);
						}}
						className="opacity-0 group-hover:opacity-100 p-1 mr-1 hover:bg-destructive/10 rounded transition-opacity"
					>
						<Trash2 className="size-3 text-destructive" />
					</button>
				</div>
				{open && node.children && (
					<div>
						{node.children.map((child) => (
							<TreeNode
								key={child.path}
								node={child}
								depth={depth + 1}
								activeFile={activeFile}
								onFileClick={onFileClick}
								onDelete={onDelete}
							/>
						))}
					</div>
				)}
			</div>
		);
	}

	return (
		<div className="group flex items-center justify-between hover:bg-muted/60 rounded-lg">
			<button
				type="button"
				onClick={() => onFileClick(node.path)}
				className={cn(
					"flex items-center gap-1.5 flex-1 px-2 py-1 text-left transition-colors",
					isActive && "bg-muted",
				)}
				style={{ paddingLeft: `${8 + depth * 12 + 16}px` }}
			>
				<File className="size-3.5 shrink-0 text-muted-foreground" />
				<span className="text-xs truncate">{node.name}</span>
			</button>
			<button
				type="button"
				onClick={(e) => {
					e.stopPropagation();
					onDelete(node.path);
				}}
				className="opacity-0 group-hover:opacity-100 p-1 mr-1 hover:bg-destructive/10 rounded transition-opacity"
			>
				<Trash2 className="size-3 text-destructive" />
			</button>
		</div>
	);
}

// Editor panel component
function EditorPanel({ params }: IDockviewPanelProps<{ path: string }>) {
	const [content, setContent] = useState("");
	const [loading, setLoading] = useState(true);
	const [dirty, setDirty] = useState(false);
	const [saving, setSaving] = useState(false);

	useEffect(() => {
		if (!params?.path) return;
		setLoading(true);
		fetchFile(params.path)
			.then((data) => {
				setContent(data);
				setDirty(false);
			})
			.catch((e) => toast.error(`加载失败: ${e.message}`))
			.finally(() => setLoading(false));
	}, [params?.path]);

	const handleSave = async () => {
		if (!params?.path) return;
		setSaving(true);
		try {
			await writeFile(params.path, content);
			setDirty(false);
			toast.success("保存成功");
		} catch (e) {
			toast.error(`保存失败: ${(e as Error).message}`);
		} finally {
			setSaving(false);
		}
	};

	if (loading) {
		return (
			<div className="flex items-center justify-center h-full">
				<Loader2 className="size-5 animate-spin text-muted-foreground" />
			</div>
		);
	}

	return (
		<div className="flex flex-col h-full">
			<div className="flex items-center justify-between px-3 py-2 border-b bg-muted/30">
				<span className="text-xs text-muted-foreground truncate">
					{params?.path?.split("/").pop()}
				</span>
				<button
					type="button"
					onClick={handleSave}
					disabled={!dirty || saving}
					className="flex items-center gap-1.5 px-2 py-1 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
				>
					{saving ? (
						<Loader2 className="size-3 animate-spin" />
					) : (
						<Save className="size-3" />
					)}
					保存
				</button>
			</div>
			<CodeEditor
				value={content}
				onChange={(v) => {
					setContent(v || "");
					setDirty(true);
				}}
				language="markdown"
			/>
		</div>
	);
}

export default function KnowledgePage() {
	const navigate = useNavigate();
	const snap = useSnapshot(settingsModel.state);
	const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBase[]>([]);
	const [selectedKB, setSelectedKB] = useState<string | null>(null);
	const [tree, setTree] = useState<FsNode | null>(null);
	const [loading, setLoading] = useState(false);
	const [activeFile, setActiveFile] = useState<string | null>(null);
	const [searchQuery, setSearchQuery] = useState("");
	const dockviewRef = useRef<DockviewReact>(null);

	const workspaceRoot = snap.agentDefaults.workspaceRoot.trim();
	const knowledgeRoot = workspaceRoot ? `${workspaceRoot}/knowledge` : null;

	// Filter knowledge bases based on search query
	const filteredKnowledgeBases = useMemo(() => {
		const query = searchQuery.trim().toLowerCase();
		if (!query) return knowledgeBases;
		return knowledgeBases.filter((kb) => kb.name.toLowerCase().includes(query));
	}, [searchQuery, knowledgeBases]);

	// Load knowledge bases list
	const loadKnowledgeBases = useCallback(async () => {
		if (!knowledgeRoot) return;
		try {
			const rootTree = await fetchTree(knowledgeRoot);
			const kbs = (rootTree.children || [])
				.filter((node) => node.is_dir)
				.map((node) => ({
					name: node.name,
					path: node.path,
				}));
			setKnowledgeBases(kbs);
		} catch (e) {
			console.error("Failed to load knowledge bases:", e);
		}
	}, [knowledgeRoot]);

	// Load selected knowledge base tree
	const loadTree = useCallback(async () => {
		if (!selectedKB) return;
		setLoading(true);
		try {
			const data = await fetchTree(selectedKB);
			setTree(data);
		} catch (e) {
			toast.error(`加载失败: ${(e as Error).message}`);
		} finally {
			setLoading(false);
		}
	}, [selectedKB]);

	useEffect(() => {
		loadKnowledgeBases();
	}, [loadKnowledgeBases]);

	useEffect(() => {
		if (selectedKB) {
			loadTree();
		}
	}, [selectedKB, loadTree]);

	const handleCreateKB = async () => {
		if (!knowledgeRoot) {
			toast.error("未配置工作区");
			return;
		}
		const name = prompt("知识库名称：");
		if (!name) return;

		try {
			await createNode(`${knowledgeRoot}/${name}`, true);
			toast.success("创建成功");
			loadKnowledgeBases();
		} catch (e) {
			toast.error(`创建失败: ${(e as Error).message}`);
		}
	};

	const handleDeleteKB = async (path: string) => {
		if (!confirm("确定要删除此知识库吗？")) return;
		try {
			await deleteFile(path);
			toast.success("删除成功");
			if (selectedKB === path) {
				setSelectedKB(null);
			}
			loadKnowledgeBases();
		} catch (e) {
			toast.error(`删除失败: ${(e as Error).message}`);
		}
	};

	const handleCreateFile = async () => {
		if (!selectedKB) {
			toast.error("请先选择知识库");
			return;
		}
		const name = prompt("文件名（如 note.md）：");
		if (!name) return;

		try {
			await createNode(`${selectedKB}/${name}`, false);
			await writeFile(`${selectedKB}/${name}`, "");
			toast.success("创建成功");
			loadTree();
		} catch (e) {
			toast.error(`创建失败: ${(e as Error).message}`);
		}
	};

	const handleCreateFolder = async () => {
		if (!selectedKB) {
			toast.error("请先选择知识库");
			return;
		}
		const name = prompt("文件夹名称：");
		if (!name) return;

		try {
			await createNode(`${selectedKB}/${name}`, true);
			toast.success("创建成功");
			loadTree();
		} catch (e) {
			toast.error(`创建失败: ${(e as Error).message}`);
		}
	};

	const handleDelete = async (path: string) => {
		if (!confirm("确定要删除吗？")) return;
		try {
			await deleteFile(path);
			toast.success("删除成功");
			loadTree();
			// Close panel if it's open
			const api = dockviewRef.current?.api;
			if (api) {
				const panel = api.panels.find((p) => p.params?.path === path);
				if (panel) {
					api.removePanel(panel);
				}
			}
		} catch (e) {
			toast.error(`删除失败: ${(e as Error).message}`);
		}
	};

	const handleFileClick = (path: string) => {
		setActiveFile(path);
		const api = dockviewRef.current?.api;
		if (!api) return;

		// Check if panel already exists
		const existing = api.panels.find((p) => p.params?.path === path);
		if (existing) {
			existing.api.setActive();
			return;
		}

		// Create new panel
		api.addPanel({
			id: path,
			component: "editor",
			params: { path },
			title: path.split("/").pop() || "Untitled",
		});
	};

	const onReady = (event: DockviewReadyEvent) => {
		// Panel is ready
	};

	if (!workspaceRoot) {
		return (
			<div className="flex flex-col items-center justify-center h-full text-muted-foreground">
				<BookOpen className="size-16 mb-4 opacity-20" />
				<p className="text-sm">请先在设置中配置工作区根目录</p>
				<button
					type="button"
					onClick={() => navigate("/settings")}
					className="mt-4 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90"
				>
					前往设置
				</button>
			</div>
		);
	}

	return (
		<div className="flex flex-col h-full bg-background">
			{/* Top bar */}
			<div className="flex items-center gap-3 px-4 py-2.5 border-b shrink-0">
				<button
					type="button"
					onClick={() => navigate("/")}
					className="flex items-center justify-center size-7 rounded-lg text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
				>
					<ArrowLeft className="size-3.5" />
				</button>
				<span className="text-sm font-semibold">知识库管理</span>
				<div className="flex-1" />
				<button
					type="button"
					onClick={handleCreateKB}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
				>
					<Plus className="size-3.5" />
					新建知识库
				</button>
			</div>

			<ResizablePanelGroup direction="horizontal" className="flex-1">
				{/* Left sidebar - knowledge bases list */}
				<ResizablePanel defaultSize={20} minSize={15} maxSize={35}>
					<SidebarList
						title="知识库"
						enableSearch={true}
						searchPlaceholder="搜索知识库..."
						searchValue={searchQuery}
						onSearchChange={setSearchQuery}
					>
						{filteredKnowledgeBases.length === 0 ? (
							<SidebarListEmpty
								message={searchQuery ? "未找到匹配的知识库" : "还没有知识库"}
							/>
						) : (
							filteredKnowledgeBases.map((kb) => (
								<div
									key={kb.path}
									className="group flex items-center justify-between"
								>
									<button
										type="button"
										onClick={() => setSelectedKB(kb.path)}
										className={cn(
											"flex-1 text-left px-3 py-3 hover:bg-accent/[0.08] transition-colors",
											selectedKB === kb.path && "bg-primary/[0.08]",
										)}
									>
										<div className="text-sm font-medium truncate">
											{kb.name}
										</div>
									</button>
									<button
										type="button"
										onClick={() => handleDeleteKB(kb.path)}
										className="opacity-0 group-hover:opacity-100 p-1 mr-3 hover:bg-destructive/10 rounded transition-opacity"
									>
										<Trash2 className="size-3 text-destructive" />
									</button>
								</div>
							))
						)}
					</SidebarList>
				</ResizablePanel>

				<ResizableHandle />

				{/* Middle - file tree (only show when KB is selected) */}
				{selectedKB && (
					<>
						<ResizablePanel defaultSize={25} minSize={15} maxSize={40}>
							<div className="border-r flex flex-col bg-card/50 h-full">
								<div className="px-4 py-3 border-b">
									<div className="flex items-center justify-between">
										<div>
											<div className="flex items-center gap-2 text-sm font-medium">
												<Folder className="size-4 text-muted-foreground" />
												文件列表
											</div>
											<p className="text-xs text-muted-foreground mt-1 truncate">
												{selectedKB.split("/").pop()}
											</p>
										</div>
										<div className="flex items-center gap-1">
											<button
												type="button"
												onClick={handleCreateFile}
												className="p-1 rounded hover:bg-muted transition-colors"
												title="新建文件"
											>
												<FilePlus className="size-3.5" />
											</button>
											<button
												type="button"
												onClick={handleCreateFolder}
												className="p-1 rounded hover:bg-muted transition-colors"
												title="新建文件夹"
											>
												<FolderPlus className="size-3.5" />
											</button>
										</div>
									</div>
								</div>
								<div className="flex-1 overflow-y-auto p-2">
									{loading ? (
										<div className="flex items-center justify-center py-12">
											<Loader2 className="size-5 animate-spin text-muted-foreground" />
										</div>
									) : !tree || !tree.children || tree.children.length === 0 ? (
										<div className="text-center py-12 px-4">
											<File className="size-8 mx-auto mb-3 text-muted-foreground/30" />
											<p className="text-xs text-muted-foreground">
												还没有文件
											</p>
											<p className="text-xs text-muted-foreground/60 mt-1">
												点击上方按钮创建
											</p>
										</div>
									) : (
										tree.children.map((node) => (
											<TreeNode
												key={node.path}
												node={node}
												depth={0}
												activeFile={activeFile}
												onFileClick={handleFileClick}
												onDelete={handleDelete}
											/>
										))
									)}
								</div>
							</div>
						</ResizablePanel>

						<ResizableHandle />
					</>
				)}

				{/* Right - editor */}
				<ResizablePanel defaultSize={55} minSize={40}>
					<div className="flex-1 [&_.dv-close-action]:hidden h-full">
						<DockviewReact
							ref={dockviewRef}
							onReady={onReady}
							components={{ editor: EditorPanel }}
							className="h-full"
						/>
					</div>
				</ResizablePanel>
			</ResizablePanelGroup>
		</div>
	);
}
