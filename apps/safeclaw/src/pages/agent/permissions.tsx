/**
 * Agent Permissions Manager - Configure agent access permissions
 */
import settingsModel from "@/models/settings.model";
import { getGatewayUrl } from "@/models/settings.model";
import { cn } from "@/lib/utils";
import { ArrowLeft, Loader2, BookOpen, Check, Shield } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";
import { useSnapshot } from "valtio";

interface KnowledgeBase {
	name: string;
	path: string;
}

interface AgentPermissions {
	knowledgeBases: string[]; // Array of knowledge base paths
}

// FS API helpers
async function fetchTree(path: string): Promise<any> {
	const res = await fetch(
		`${getGatewayUrl()}/api/fs/tree?path=${encodeURIComponent(path)}&depth=1`,
	);
	if (!res.ok) {
		if (res.status === 404 || res.status === 400) {
			return { name: "", path, is_dir: true, children: [] };
		}
		throw new Error(await res.text());
	}
	return res.json();
}

export default function PermissionsPage() {
	const { agentId } = useParams<{ agentId: string }>();
	const navigate = useNavigate();
	const snap = useSnapshot(settingsModel.state);
	const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBase[]>([]);
	const [selectedKBs, setSelectedKBs] = useState<Set<string>>(new Set());
	const [loading, setLoading] = useState(true);
	const [saving, setSaving] = useState(false);

	const workspaceRoot = snap.agentDefaults.workspaceRoot.trim();
	const knowledgeRoot = workspaceRoot ? `${workspaceRoot}/knowledge` : null;

	// Load knowledge bases list
	const loadKnowledgeBases = useCallback(async () => {
		if (!knowledgeRoot) return;
		setLoading(true);
		try {
			const rootTree = await fetchTree(knowledgeRoot);
			const kbs = (rootTree.children || [])
				.filter((node: any) => node.is_dir)
				.map((node: any) => ({
					name: node.name,
					path: node.path,
				}));
			setKnowledgeBases(kbs);
		} catch (e) {
			console.error("Failed to load knowledge bases:", e);
		} finally {
			setLoading(false);
		}
	}, [knowledgeRoot]);

	// Load agent permissions
	const loadPermissions = useCallback(async () => {
		if (!agentId || !workspaceRoot) return;
		try {
			const permissionsPath = `${workspaceRoot}/agents/${agentId}/permissions.json`;
			const res = await fetch(
				`${getGatewayUrl()}/api/fs/file?path=${encodeURIComponent(permissionsPath)}`,
			);
			if (res.ok) {
				const data = await res.json();
				const permissions: AgentPermissions = JSON.parse(data.content);
				setSelectedKBs(new Set(permissions.knowledgeBases || []));
			}
		} catch (e) {
			// Permissions file doesn't exist yet, that's ok
			console.log("No existing permissions file");
		}
	}, [agentId, workspaceRoot]);

	useEffect(() => {
		loadKnowledgeBases();
		loadPermissions();
	}, [loadKnowledgeBases, loadPermissions]);

	const handleToggleKB = (path: string) => {
		const newSelected = new Set(selectedKBs);
		if (newSelected.has(path)) {
			newSelected.delete(path);
		} else {
			newSelected.add(path);
		}
		setSelectedKBs(newSelected);
	};

	const handleSave = async () => {
		if (!agentId || !workspaceRoot) return;
		setSaving(true);
		try {
			const permissions: AgentPermissions = {
				knowledgeBases: Array.from(selectedKBs),
			};

			// Ensure agents directory exists
			const agentsDir = `${workspaceRoot}/agents`;
			await fetch(`${getGatewayUrl()}/api/fs/create`, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ path: agentsDir, is_dir: true }),
			});

			// Ensure agent directory exists
			const agentDir = `${agentsDir}/${agentId}`;
			await fetch(`${getGatewayUrl()}/api/fs/create`, {
				method: "POST",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ path: agentDir, is_dir: true }),
			});

			// Save permissions file
			const permissionsPath = `${agentDir}/permissions.json`;
			await fetch(`${getGatewayUrl()}/api/fs/file`, {
				method: "PUT",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({
					path: permissionsPath,
					content: JSON.stringify(permissions, null, 2),
				}),
			});

			toast.success("保存成功");
		} catch (e) {
			toast.error(`保存失败: ${(e as Error).message}`);
		} finally {
			setSaving(false);
		}
	};

	if (!workspaceRoot) {
		return (
			<div className="flex flex-col items-center justify-center h-full text-muted-foreground">
				<Shield className="size-16 mb-4 opacity-20" />
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
				<span className="text-sm font-semibold">权限管理</span>
				<div className="flex-1" />
				<button
					type="button"
					onClick={handleSave}
					disabled={saving}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
				>
					{saving ? (
						<Loader2 className="size-3.5 animate-spin" />
					) : (
						<Check className="size-3.5" />
					)}
					保存
				</button>
			</div>

			<div className="flex-1 overflow-y-auto p-6">
				<div className="max-w-3xl mx-auto space-y-6">
					{/* Knowledge Base Access */}
					<div className="rounded-xl border border-border/60 bg-card shadow-sm overflow-hidden">
						<div className="px-6 py-4 border-b bg-muted/30">
							<div className="flex items-center gap-2">
								<BookOpen className="size-4 text-muted-foreground" />
								<h2 className="text-sm font-semibold">知识库访问权限</h2>
							</div>
							<p className="text-xs text-muted-foreground mt-1">
								选择此智能体可以访问的知识库
							</p>
						</div>
						<div className="p-6">
							{loading ? (
								<div className="flex items-center justify-center py-12">
									<Loader2 className="size-5 animate-spin text-muted-foreground" />
								</div>
							) : knowledgeBases.length === 0 ? (
								<div className="text-center py-12">
									<BookOpen className="size-12 mx-auto mb-3 text-muted-foreground/30" />
									<p className="text-sm text-muted-foreground">还没有知识库</p>
									<p className="text-xs text-muted-foreground/60 mt-1">
										请先创建知识库
									</p>
									<button
										type="button"
										onClick={() => navigate("/knowledge")}
										className="mt-4 px-4 py-2 text-sm rounded-lg border border-border hover:bg-muted transition-colors"
									>
										前往知识库管理
									</button>
								</div>
							) : (
								<div className="space-y-2">
									{knowledgeBases.map((kb) => {
										const isSelected = selectedKBs.has(kb.path);
										return (
											<button
												key={kb.path}
												type="button"
												onClick={() => handleToggleKB(kb.path)}
												className={cn(
													"w-full flex items-center gap-3 px-4 py-3 rounded-lg border transition-colors",
													isSelected
														? "border-primary bg-primary/5"
														: "border-border hover:bg-muted/50",
												)}
											>
												<div
													className={cn(
														"flex items-center justify-center size-5 rounded border-2 transition-colors",
														isSelected
															? "border-primary bg-primary"
															: "border-muted-foreground/30",
													)}
												>
													{isSelected && (
														<Check className="size-3 text-primary-foreground" />
													)}
												</div>
												<div className="flex-1 text-left">
													<div className="text-sm font-medium">{kb.name}</div>
													<div className="text-xs text-muted-foreground mt-0.5">
														{kb.path}
													</div>
												</div>
											</button>
										);
									})}
								</div>
							)}
						</div>
					</div>

					{/* Future: Add more permission types here */}
					{/* e.g., File system access, API access, etc. */}
				</div>
			</div>
		</div>
	);
}
