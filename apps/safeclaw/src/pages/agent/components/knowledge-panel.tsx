/**
 * Knowledge Panel - Manage agent knowledge base files
 */
import { useState, useEffect } from "react";
import { BookOpen, Trash2, FolderOpen } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { getAgentWorkspacePath } from "@/lib/workspace-utils";
import { toast } from "sonner";

interface KnowledgeFile {
	name: string;
	path: string;
}

export default function KnowledgePanel({ agentId }: { agentId: string }) {
	const [files, setFiles] = useState<KnowledgeFile[]>([]);
	const [loading, setLoading] = useState(true);

	useEffect(() => {
		loadFiles();
	}, [agentId]);

	const loadFiles = async () => {
		setLoading(true);
		try {
			const workspacePath = await getAgentWorkspacePath(agentId);
			const knowledgePath = `${workspacePath}/knowledge`;

			try {
				const entries = await invoke<string[]>("plugin:fs|read_dir", {
					path: knowledgePath,
				});

				setFiles(
					entries.map((name) => ({
						name,
						path: `${knowledgePath}/${name}`,
					})),
				);
			} catch {
				setFiles([]);
			}
		} catch (err) {
			console.error("Failed to load knowledge files:", err);
			toast.error("加载知识库失败");
		} finally {
			setLoading(false);
		}
	};

	const handleOpenFolder = async () => {
		try {
			const workspacePath = await getAgentWorkspacePath(agentId);
			await invoke("plugin:shell|open", {
				path: `${workspacePath}/knowledge`,
			});
		} catch {
			toast.error("无法打开文件夹");
		}
	};

	const handleDelete = async (file: KnowledgeFile) => {
		if (!confirm(`确定要删除 "${file.name}" 吗？`)) return;

		try {
			await invoke("plugin:fs|remove", { path: file.path });
			toast.success("文件已删除");
			loadFiles();
		} catch {
			toast.error("删除失败");
		}
	};

	return (
		<div className="flex flex-col h-full">
			{/* Header */}
			<div className="flex items-center justify-between px-4 py-3 border-b shrink-0">
				<p className="text-xs text-muted-foreground">{files.length} 个文件</p>
				<button
					type="button"
					onClick={handleOpenFolder}
					className="flex items-center gap-1.5 px-2 py-1 text-xs rounded-lg border hover:bg-muted transition-colors"
					title="在文件管理器中打开"
				>
					<FolderOpen className="size-3" />
					打开文件夹
				</button>
			</div>

			{/* List */}
			<div className="flex-1 overflow-y-auto p-3">
				{loading ? (
					<div className="flex items-center justify-center h-full text-muted-foreground text-sm">
						加载中…
					</div>
				) : files.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
						<BookOpen className="size-10 opacity-20" />
						<div className="text-center">
							<p className="text-xs">还没有知识库文件</p>
							<p className="text-xs mt-1">
								点击"打开文件夹"添加文档、PDF 等知识文件
							</p>
						</div>
					</div>
				) : (
					<div className="flex flex-col gap-2">
						{files.map((file) => (
							<div
								key={file.path}
								className="group flex items-center justify-between p-3 rounded-lg border bg-card hover:border-primary/40 hover:shadow-sm transition-all"
							>
								<div className="flex items-center gap-2 min-w-0">
									<div className="flex items-center justify-center size-7 rounded-lg bg-primary/10 shrink-0">
										<BookOpen className="size-3.5 text-primary" />
									</div>
									<span className="text-sm font-medium truncate">
										{file.name}
									</span>
								</div>
								<button
									type="button"
									onClick={() => handleDelete(file)}
									className="flex items-center justify-center size-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-all opacity-0 group-hover:opacity-100"
									title="删除"
								>
									<Trash2 className="size-3" />
								</button>
							</div>
						))}
					</div>
				)}
			</div>
		</div>
	);
}
