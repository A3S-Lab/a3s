/**
 * Workflows Manager - List-based workflow management
 */
import workflowModel, { type WorkflowDoc } from "@/models/workflow.model";
import {
	ArrowLeft,
	Loader2,
	Plus,
	Trash2,
	Edit,
	GitBranch,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

export default function WorkflowsPage() {
	const { agentId } = useParams<{ agentId: string }>();
	const navigate = useNavigate();
	const [creating, setCreating] = useState(false);
	const [newName, setNewName] = useState("");
	const [newDescription, setNewDescription] = useState("");
	const inputRef = useRef<HTMLInputElement>(null);

	const { workflows, loaded } = useSnapshot(workflowModel.state);
	const loadedRef = useRef(false);

	useEffect(() => {
		if (!loadedRef.current) {
			loadedRef.current = true;
			if (!loaded) workflowModel.load();
		}
	}, [loaded]);

	// Filter workflows by agentId if provided
	const filteredWorkflows = agentId
		? workflows.filter((w) => w.agent_id === agentId)
		: workflows;

	const handleDelete = async (workflow: WorkflowDoc) => {
		if (!confirm(`确定要删除工作流 "${workflow.name}" 吗？`)) return;
		try {
			await workflowModel.remove(workflow.id);
			toast.success("删除成功");
		} catch (e) {
			toast.error(`删除失败: ${(e as Error).message}`);
		}
	};

	const handleEdit = (workflow: WorkflowDoc) => {
		navigate(`/workflow/${workflow.id}`);
	};

	const handleCreateClick = () => {
		setCreating(true);
		setNewName("");
		setNewDescription("");
		setTimeout(() => inputRef.current?.focus(), 100);
	};

	const handleCreateSubmit = async (e: React.FormEvent) => {
		e.preventDefault();
		const name = newName.trim();
		if (!name) return;

		try {
			const wf = await workflowModel.create(
				name,
				newDescription.trim() || undefined,
			);
			// Bind to agent if agentId is provided
			if (agentId) {
				await workflowModel.setAgentId(wf.id, agentId);
			}
			toast.success("创建成功");
			setCreating(false);
			setNewName("");
			setNewDescription("");
			navigate(`/workflow/${wf.id}`);
		} catch (e) {
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
				<span className="text-sm font-semibold">工作流管理</span>
				<div className="flex-1" />
				<button
					type="button"
					onClick={handleCreateClick}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
				>
					<Plus className="size-3.5" />
					新建工作流
				</button>
			</div>

			<div className="flex flex-1 overflow-hidden">
				{/* Left sidebar */}
				<div className="w-64 border-r flex flex-col bg-card/50">
					<div className="px-4 py-3 border-b">
						<div className="flex items-center gap-2 text-sm font-medium">
							<GitBranch className="size-4 text-muted-foreground" />
							工作流列表
						</div>
						<p className="text-xs text-muted-foreground mt-1">
							{filteredWorkflows.length} 个工作流
						</p>
					</div>
					<div className="flex-1 overflow-y-auto p-2">
						{!loaded ? (
							<div className="flex items-center justify-center py-12">
								<Loader2 className="size-5 animate-spin text-muted-foreground" />
							</div>
						) : filteredWorkflows.length === 0 ? (
							<div className="text-center py-12 px-4">
								<GitBranch className="size-8 mx-auto mb-3 text-muted-foreground/30" />
								<p className="text-sm text-muted-foreground">还没有工作流</p>
								<p className="text-xs text-muted-foreground/60 mt-1">
									点击右上角按钮创建
								</p>
							</div>
						) : (
							<div className="space-y-1">
								{filteredWorkflows.map((workflow) => (
									<button
										key={workflow.id}
										type="button"
										onClick={() => handleEdit(workflow)}
										className="w-full text-left px-3 py-2 rounded-lg hover:bg-muted/60 transition-colors group"
									>
										<div className="text-xs font-medium truncate group-hover:text-primary transition-colors">
											{workflow.name}
										</div>
										{workflow.description && (
											<p className="text-xs text-muted-foreground mt-0.5 truncate line-clamp-2">
												{workflow.description}
											</p>
										)}
									</button>
								))}
							</div>
						)}
					</div>
				</div>

				{/* Right - workflow list table */}
				<div className="flex-1 overflow-y-auto">
					{creating ? (
						<div className="flex items-center justify-center h-full p-6">
							<form onSubmit={handleCreateSubmit} className="w-full max-w-md">
								<div className="rounded-xl border border-border/60 bg-card shadow-sm overflow-hidden">
									<div className="px-6 py-4 border-b bg-muted/30">
										<h2 className="text-base font-semibold">新建工作流</h2>
										<p className="text-xs text-muted-foreground mt-1">
											创建一个新的工作流模板
										</p>
									</div>
									<div className="p-6 space-y-4">
										<div className="space-y-2">
											<label
												htmlFor="workflow-name"
												className="text-sm font-medium flex items-center gap-1"
											>
												名称 <span className="text-destructive text-xs">*</span>
											</label>
											<input
												ref={inputRef}
												id="workflow-name"
												type="text"
												value={newName}
												onChange={(e) => setNewName(e.target.value)}
												placeholder="例如：数据处理流程"
												className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors"
												required
											/>
										</div>
										<div className="space-y-2">
											<label
												htmlFor="workflow-description"
												className="text-sm font-medium"
											>
												描述
											</label>
											<textarea
												id="workflow-description"
												value={newDescription}
												onChange={(e) => setNewDescription(e.target.value)}
												placeholder="简要描述工作流的用途..."
												rows={4}
												className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors resize-none"
											/>
										</div>
									</div>
									<div className="flex items-center gap-2 px-6 py-4 border-t bg-muted/20">
										<button
											type="button"
											onClick={() => {
												setCreating(false);
												setNewName("");
												setNewDescription("");
											}}
											className="flex-1 px-4 py-2 text-sm border border-border rounded-lg hover:bg-muted/60 transition-colors"
										>
											取消
										</button>
										<button
											type="submit"
											disabled={!newName.trim()}
											className="flex-1 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
										>
											创建
										</button>
									</div>
								</div>
							</form>
						</div>
					) : filteredWorkflows.length > 0 ? (
						<div className="p-6">
							<div className="rounded-xl border border-border/60 bg-card shadow-sm overflow-hidden">
								<table className="w-full">
									<thead>
										<tr className="border-b bg-muted/30">
											<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground w-32">
												ID
											</th>
											<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground">
												名称
											</th>
											<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground">
												描述
											</th>
											<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground w-40">
												最新修改时间
											</th>
											<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground w-24">
												操作
											</th>
										</tr>
									</thead>
									<tbody className="divide-y divide-border/40">
										{filteredWorkflows.map((workflow) => {
											const updatedDate = new Date(workflow.updated_at * 1000);
											const formattedDate = updatedDate.toLocaleString(
												"zh-CN",
												{
													year: "numeric",
													month: "2-digit",
													day: "2-digit",
													hour: "2-digit",
													minute: "2-digit",
												},
											);
											return (
												<tr
													key={workflow.id}
													className="hover:bg-muted/30 transition-colors group"
												>
													<td className="px-4 py-3">
														<div className="text-xs font-mono text-muted-foreground truncate">
															{workflow.id.substring(0, 8)}
														</div>
													</td>
													<td className="px-4 py-3">
														<div className="font-medium text-sm">
															{workflow.name}
														</div>
													</td>
													<td className="px-4 py-3">
														<div className="text-xs text-muted-foreground line-clamp-2">
															{workflow.description || "-"}
														</div>
													</td>
													<td className="px-4 py-3 text-xs text-muted-foreground">
														{formattedDate}
													</td>
													<td className="px-4 py-3">
														<div className="flex items-center gap-1">
															<button
																type="button"
																onClick={() => handleEdit(workflow)}
																className="p-1.5 rounded-lg hover:bg-muted transition-colors"
																title="编辑"
															>
																<Edit className="size-3.5" />
															</button>
															<button
																type="button"
																onClick={() => handleDelete(workflow)}
																className="p-1.5 rounded-lg hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
																title="删除"
															>
																<Trash2 className="size-3.5" />
															</button>
														</div>
													</td>
												</tr>
											);
										})}
									</tbody>
								</table>
							</div>
						</div>
					) : (
						<div className="flex flex-col items-center justify-center h-full text-muted-foreground">
							<GitBranch className="size-16 mb-4 opacity-20" />
							<p className="text-sm font-medium">还没有工作流</p>
							<p className="text-xs text-muted-foreground/60 mt-1 mb-4">
								创建第一个工作流模板
							</p>
							<button
								type="button"
								onClick={handleCreateClick}
								className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
							>
								<Plus className="size-4" />
								新建工作流
							</button>
						</div>
					)}
				</div>
			</div>
		</div>
	);
}
