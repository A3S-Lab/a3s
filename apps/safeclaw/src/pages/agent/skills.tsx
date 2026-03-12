/**
 * Skills Editor - Manage agent skills (Markdown files with YAML frontmatter)
 */
import CodeEditor from "@/components/custom/code-editor";
import {
	DockviewReact,
	type DockviewReadyEvent,
	type IDockviewPanelProps,
} from "@/components/custom/dockview";
import { getAgentWorkspacePath } from "@/lib/workspace-utils";
import { getGatewayUrl } from "@/models/settings.model";
import { cn } from "@/lib/utils";
import {
	File,
	Folder,
	FolderOpen,
	ArrowLeft,
	Loader2,
	FilePlus,
	FolderPlus,
	Trash2,
	Save,
	ChevronRight,
	ChevronDown,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";

interface FsNode {
	name: string;
	path: string;
	is_dir: boolean;
	children?: FsNode[];
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
	onRefresh,
}: {
	node: FsNode;
	depth: number;
	activeFile: string | null;
	onFileClick: (path: string) => void;
	onDelete: (path: string) => void;
	onRefresh: () => void;
}) {
	const [open, setOpen] = useState(depth === 0);
	const isActive = activeFile === node.path;

	if (node.is_dir) {
		return (
			<div>
				<div className="group flex items-center justify-between hover:bg-muted rounded">
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
						className="opacity-0 group-hover:opacity-100 p-1 mr-1 hover:bg-destructive/10 rounded"
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
								onRefresh={onRefresh}
							/>
						))}
					</div>
				)}
			</div>
		);
	}

	return (
		<div className="group flex items-center justify-between hover:bg-muted rounded">
			<button
				type="button"
				onClick={() => onFileClick(node.path)}
				className={cn(
					"flex items-center gap-1.5 flex-1 px-2 py-1 text-left",
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
				className="opacity-0 group-hover:opacity-100 p-1 mr-1 hover:bg-destructive/10 rounded"
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
	const [saving, setSaving] = useState(false);
	const [dirty, setDirty] = useState(false);

	useEffect(() => {
		if (!params?.path) return;
		setLoading(true);
		fetchFile(params.path)
			.then((c) => {
				setContent(c);
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
				<span className="text-xs text-muted-foreground">
					{params?.path?.split("/").pop()}
				</span>
				<button
					type="button"
					onClick={handleSave}
					disabled={!dirty || saving}
					className="flex items-center gap-1.5 px-2 py-1 text-xs rounded bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
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
				language="markdown"
				value={content}
				onChange={(v) => {
					setContent(v || "");
					setDirty(true);
				}}
			/>
		</div>
	);
}

export default function SkillsPage() {
	const { agentId } = useParams<{ agentId: string }>();
	const navigate = useNavigate();
	const [tree, setTree] = useState<FsNode | null>(null);
	const [loading, setLoading] = useState(true);
	const [activeFile, setActiveFile] = useState<string | null>(null);
	const [skillsPath, setSkillsPath] = useState<string | null>(null);
	const dockviewRef = useRef<DockviewReadyEvent | null>(null);

	// Load workspace path asynchronously
	useEffect(() => {
		if (agentId) {
			getAgentWorkspacePath(agentId).then((path) => {
				setSkillsPath(`${path}/skills`);
			});
		}
	}, [agentId]);

	console.log("Skills page - agentId:", agentId, "skillsPath:", skillsPath);

	const loadTree = useCallback(async () => {
		if (!skillsPath) {
			console.log("Skills - no skillsPath, skipping load");
			return;
		}
		console.log("Skills - loading tree from:", skillsPath);
		setLoading(true);
		try {
			const node = await fetchTree(skillsPath);
			console.log("Skills - tree loaded:", node);
			setTree(node);
		} catch (e) {
			console.error("Skills - load error:", e);
			toast.error(`加载技能列表失败: ${(e as Error).message}`);
		} finally {
			setLoading(false);
		}
	}, [skillsPath]);

	useEffect(() => {
		loadTree();
	}, [loadTree]);

	const handleFileClick = (path: string) => {
		setActiveFile(path);

		if (dockviewRef.current) {
			const existing = dockviewRef.current.api.getPanel(path);
			if (existing) {
				existing.api.setActive();
			} else {
				dockviewRef.current.api.addPanel({
					id: path,
					component: "editor",
					params: { path },
					title: path.split("/").pop() || "",
					renderer: "onlyWhenActive",
				});
			}
		}
	};

	const handleDelete = async (path: string) => {
		const itemType = path.endsWith("/") ? "文件夹" : "文件";
		if (!confirm(`确定要删除${itemType} "${path.split("/").pop()}" 吗？`))
			return;

		try {
			await deleteFile(path);
			toast.success("删除成功");
			loadTree();
			if (dockviewRef.current) {
				const panel = dockviewRef.current.api.getPanel(path);
				if (panel) {
					panel.api.close();
				}
			}
		} catch (e) {
			toast.error(`删除失败: ${(e as Error).message}`);
		}
	};

	const handleCreateFile = async () => {
		console.log("handleCreateFile - skillsPath:", skillsPath);
		if (!skillsPath) {
			toast.error("未配置工作区");
			return;
		}
		const name = prompt("文件名（例如：my-skill.md）：");
		if (!name) return;

		const filename = name.endsWith(".md") ? name : `${name}.md`;
		const template = `---
name: ${filename.replace(".md", "")}
description: 技能描述
kind: instruction
tags:
  - custom
version: 1.0.0
---

# ${filename.replace(".md", "")}

技能内容...
`;

		try {
			console.log("Creating file:", `${skillsPath}/${filename}`);
			await writeFile(`${skillsPath}/${filename}`, template);
			toast.success("创建成功");
			loadTree();
			handleFileClick(`${skillsPath}/${filename}`);
		} catch (e) {
			console.error("Create file error:", e);
			toast.error(`创建失败: ${(e as Error).message}`);
		}
	};

	const handleCreateFolder = async () => {
		console.log("handleCreateFolder - skillsPath:", skillsPath);
		if (!skillsPath) {
			toast.error("未配置工作区");
			return;
		}
		const name = prompt("文件夹名称：");
		if (!name) return;

		try {
			console.log("Creating folder:", `${skillsPath}/${name}`);
			await createNode(`${skillsPath}/${name}`, true);
			toast.success("创建成功");
			loadTree();
		} catch (e) {
			console.error("Create folder error:", e);
			toast.error(`创建失败: ${(e as Error).message}`);
		}
	};

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
				<span className="text-sm font-semibold">技能管理</span>
				<div className="flex-1" />
				<button
					type="button"
					onClick={handleCreateFile}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
					title="新建技能文件"
				>
					<FilePlus className="size-3.5" />
					新建文件
				</button>
				<button
					type="button"
					onClick={handleCreateFolder}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-border hover:bg-muted transition-colors"
					title="新建文件夹"
				>
					<FolderPlus className="size-3.5" />
					新建文件夹
				</button>
			</div>

			<div className="flex flex-1 overflow-hidden">
				{/* Left sidebar - file tree */}
				<div className="w-64 border-r flex flex-col bg-card/50">
					<div className="px-4 py-3 border-b">
						<div className="flex items-center gap-2 text-sm font-medium">
							<File className="size-4 text-muted-foreground" />
							技能文件
						</div>
						<p className="text-xs text-muted-foreground mt-1">
							{tree?.children?.length || 0} 个文件/文件夹
						</p>
					</div>

					<div className="flex-1 overflow-y-auto p-2">
						{loading ? (
							<div className="flex items-center justify-center py-12">
								<Loader2 className="size-5 animate-spin text-muted-foreground" />
							</div>
						) : !tree || !tree.children || tree.children.length === 0 ? (
							<div className="text-center py-12 px-4">
								<File className="size-8 mx-auto mb-3 text-muted-foreground/30" />
								<p className="text-sm text-muted-foreground">还没有技能文件</p>
								<p className="text-xs text-muted-foreground/60 mt-1">
									点击右上角按钮创建
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
									onRefresh={loadTree}
								/>
							))
						)}
					</div>
				</div>

				{/* Right - Dockview editor */}
				<div className="flex-1 [&_.dv-close-action]:hidden">
					<DockviewReact
						onReady={(event) => {
							dockviewRef.current = event;
						}}
						components={{
							editor: EditorPanel,
						}}
						className="h-full"
					/>
				</div>
			</div>
		</div>
	);
}
