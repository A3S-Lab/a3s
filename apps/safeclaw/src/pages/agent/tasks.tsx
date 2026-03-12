/**
 * Scheduled Tasks Manager - List-based task management
 */
import { getAgentWorkspacePath } from "@/lib/workspace-utils";
import { getGatewayUrl } from "@/models/settings.model";
import { cn } from "@/lib/utils";
import {
	ArrowLeft,
	Loader2,
	Plus,
	Trash2,
	Edit,
	Play,
	Pause,
	Clock,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { toast } from "sonner";

interface ScheduledTask {
	id: string;
	name: string;
	command: string;
	enabled: boolean;
	description?: string;
}

// FS API helpers
async function fetchTasks(tasksPath: string): Promise<ScheduledTask[]> {
	const res = await fetch(
		`${getGatewayUrl()}/api/fs/list?path=${encodeURIComponent(tasksPath)}`,
	);
	if (!res.ok) {
		if (res.status === 404) return [];
		throw new Error(await res.text());
	}
	const data = await res.json();
	const files = (data.files || []).filter((f: string) => f.endsWith(".json"));

	const tasks: ScheduledTask[] = [];
	for (const file of files) {
		try {
			const content = await fetchFile(`${tasksPath}/${file}`);
			const task = JSON.parse(content);
			tasks.push({ ...task, id: file.replace(".json", "") });
		} catch (e) {
			console.error(`Failed to load task ${file}:`, e);
		}
	}
	return tasks;
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

export default function TasksPage() {
	const { agentId } = useParams<{ agentId: string }>();
	const navigate = useNavigate();
	const [tasks, setTasks] = useState<ScheduledTask[]>([]);
	const [loading, setLoading] = useState(true);
	const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);
	const [showForm, setShowForm] = useState(false);
	const [name, setName] = useState("");
	const [command, setCommand] = useState("");
	const [description, setDescription] = useState("");
	const [enabled, setEnabled] = useState(true);
	const [tasksPath, setTasksPath] = useState<string | null>(null);

	// Load workspace path asynchronously
	useEffect(() => {
		if (agentId) {
			getAgentWorkspacePath(agentId).then((path) => {
				setTasksPath(`${path}/tasks`);
			});
		}
	}, [agentId]);

	const loadTasks = useCallback(async () => {
		if (!tasksPath) return;
		setLoading(true);
		try {
			const loaded = await fetchTasks(tasksPath);
			setTasks(loaded);
		} catch (e) {
			toast.error(`加载任务失败: ${(e as Error).message}`);
		} finally {
			setLoading(false);
		}
	}, [tasksPath]);

	useEffect(() => {
		loadTasks();
	}, [loadTasks]);

	const handleSave = async (e: React.FormEvent) => {
		e.preventDefault();
		if (!tasksPath) return;
		if (!name.trim() || !command.trim()) {
			toast.error("名称和任务描述不能为空");
			return;
		}
		const id = editingTask?.id ?? `task-${Date.now()}`;
		try {
			await writeFile(
				`${tasksPath}/${id}.json`,
				JSON.stringify({ name, command, description, enabled, id }, null, 2),
			);
			toast.success(editingTask ? "更新成功" : "创建成功");
			setShowForm(false);
			setEditingTask(null);
			setName("");
			setCommand("");
			setDescription("");
			setEnabled(true);
			loadTasks();
		} catch (e) {
			toast.error(`保存失败: ${(e as Error).message}`);
		}
	};

	const handleDelete = async (task: ScheduledTask) => {
		if (!tasksPath) return;
		if (!confirm(`确定要删除任务 "${task.name}" 吗？`)) return;
		try {
			await deleteFile(`${tasksPath}/${task.id}.json`);
			toast.success("删除成功");
			loadTasks();
		} catch (e) {
			toast.error(`删除失败: ${(e as Error).message}`);
		}
	};

	const handleToggle = async (task: ScheduledTask) => {
		if (!tasksPath) return;
		try {
			const updated = { ...task, enabled: !task.enabled };
			await writeFile(
				`${tasksPath}/${task.id}.json`,
				JSON.stringify(updated, null, 2),
			);
			setTasks((prev) => prev.map((t) => (t.id === task.id ? updated : t)));
		} catch (e) {
			toast.error(`更新失败: ${(e as Error).message}`);
		}
	};

	const handleEdit = (task: ScheduledTask) => {
		setEditingTask(task);
		setName(task.name);
		setCommand(task.command);
		setDescription(task.description || "");
		setEnabled(task.enabled);
		setShowForm(true);
	};

	const handleCreate = () => {
		setEditingTask(null);
		setName("");
		setCommand("");
		setDescription("");
		setEnabled(true);
		setShowForm(true);
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
				<span className="text-sm font-semibold">定时任务管理</span>
				<div className="flex-1" />
				<button
					type="button"
					onClick={handleCreate}
					className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
				>
					<Plus className="size-3.5" />
					新建任务
				</button>
			</div>

			<div className="flex flex-1 overflow-hidden">
				{/* Left sidebar */}
				<div className="w-64 border-r flex flex-col bg-card/50">
					<div className="px-4 py-3 border-b">
						<div className="flex items-center gap-2 text-sm font-medium">
							<Clock className="size-4 text-muted-foreground" />
							任务列表
						</div>
						<p className="text-xs text-muted-foreground mt-1">
							{tasks.length} 个任务
						</p>
					</div>
					<div className="flex-1 overflow-y-auto p-2">
						{loading ? (
							<div className="flex items-center justify-center py-12">
								<Loader2 className="size-5 animate-spin text-muted-foreground" />
							</div>
						) : tasks.length === 0 ? (
							<div className="text-center py-12 px-4">
								<Clock className="size-8 mx-auto mb-3 text-muted-foreground/30" />
								<p className="text-sm text-muted-foreground">还没有定时任务</p>
								<p className="text-xs text-muted-foreground/60 mt-1">
									点击右上角按钮创建
								</p>
							</div>
						) : (
							<div className="space-y-1">
								{tasks.map((task) => (
									<button
										key={task.id}
										type="button"
										onClick={() => handleEdit(task)}
										className={cn(
											"w-full text-left px-3 py-2 rounded-lg hover:bg-muted/60 transition-colors group",
											editingTask?.id === task.id && "bg-muted",
										)}
									>
										<div className="flex items-center gap-2">
											<div
												className={cn(
													"size-1.5 rounded-full shrink-0",
													task.enabled ? "bg-green-500" : "bg-muted-foreground",
												)}
											/>
											<span className="text-xs font-medium truncate group-hover:text-primary transition-colors">
												{task.name}
											</span>
										</div>
									</button>
								))}
							</div>
						)}
					</div>
				</div>

				{/* Right - form or empty state */}
				<div className="flex-1 overflow-y-auto">
					{showForm ? (
						<div className="flex items-center justify-center h-full p-6">
							<form onSubmit={handleSave} className="w-full max-w-md">
								<div className="rounded-xl border border-border/60 bg-card shadow-sm overflow-hidden">
									<div className="px-6 py-4 border-b bg-muted/30">
										<h2 className="text-base font-semibold">
											{editingTask ? "编辑任务" : "新建定时任务"}
										</h2>
										<p className="text-xs text-muted-foreground mt-1">
											配置定时任务的执行规则
										</p>
									</div>
									<div className="p-6 space-y-4">
										<div className="space-y-2">
											<label
												htmlFor="task-name"
												className="text-sm font-medium flex items-center gap-1"
											>
												任务名称{" "}
												<span className="text-destructive text-xs">*</span>
											</label>
											<input
												id="task-name"
												type="text"
												value={name}
												onChange={(e) => setName(e.target.value)}
												placeholder="例如：每日报告"
												className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors"
												required
											/>
										</div>
										<div className="space-y-2">
											<label
												htmlFor="task-command"
												className="text-sm font-medium flex items-center gap-1"
											>
												任务描述{" "}
												<span className="text-destructive text-xs">*</span>
											</label>
											<p className="text-xs text-muted-foreground">
												描述定时规则和要执行的任务，智能体会自动解析
											</p>
											<textarea
												id="task-command"
												value={command}
												onChange={(e) => setCommand(e.target.value)}
												placeholder="例如：每天早上9点生成今日工作总结并发送邮件"
												rows={4}
												className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors resize-none"
												required
											/>
										</div>
										<div className="space-y-2">
											<label
												htmlFor="task-description"
												className="text-sm font-medium"
											>
												备注
											</label>
											<input
												id="task-description"
												type="text"
												value={description}
												onChange={(e) => setDescription(e.target.value)}
												placeholder="任务备注信息"
												className="w-full px-3 py-2 text-sm border border-border rounded-lg bg-background focus:outline-none focus:ring-2 focus:ring-primary/20 focus:border-primary transition-colors"
											/>
										</div>
										<div className="flex items-center gap-2">
											<input
												type="checkbox"
												id="enabled"
												checked={enabled}
												onChange={(e) => setEnabled(e.target.checked)}
												className="rounded"
											/>
											<label htmlFor="enabled" className="text-sm">
												启用任务
											</label>
										</div>
									</div>
									<div className="flex items-center gap-2 px-6 py-4 border-t bg-muted/20">
										<button
											type="button"
											onClick={() => {
												setShowForm(false);
												setEditingTask(null);
											}}
											className="flex-1 px-4 py-2 text-sm border border-border rounded-lg hover:bg-muted/60 transition-colors"
										>
											取消
										</button>
										<button
											type="submit"
											disabled={!name.trim() || !command.trim()}
											className="flex-1 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
										>
											保存
										</button>
									</div>
								</div>
							</form>
						</div>
					) : (
						<div className="flex flex-col h-full">
							{/* Task list table */}
							{tasks.length > 0 ? (
								<div className="p-6">
									<div className="rounded-xl border border-border/60 bg-card shadow-sm overflow-hidden">
										<table className="w-full">
											<thead>
												<tr className="border-b bg-muted/30">
													<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground">
														状态
													</th>
													<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground">
														名称
													</th>
													<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground">
														任务描述
													</th>
													<th className="px-4 py-3 text-left text-xs font-medium text-muted-foreground w-24">
														操作
													</th>
												</tr>
											</thead>
											<tbody className="divide-y divide-border/40">
												{tasks.map((task) => (
													<tr
														key={task.id}
														className="hover:bg-muted/30 transition-colors group"
													>
														<td className="px-4 py-3">
															<button
																type="button"
																onClick={() => handleToggle(task)}
																className="p-1.5 rounded-lg hover:bg-muted transition-colors"
																title={task.enabled ? "点击禁用" : "点击启用"}
															>
																{task.enabled ? (
																	<Play className="size-3.5 text-green-500" />
																) : (
																	<Pause className="size-3.5 text-muted-foreground" />
																)}
															</button>
														</td>
														<td className="px-4 py-3">
															<div className="font-medium text-sm">
																{task.name}
															</div>
															{task.description && (
																<div className="text-xs text-muted-foreground mt-0.5 line-clamp-1">
																	{task.description}
																</div>
															)}
														</td>
														<td className="px-4 py-3 text-sm text-muted-foreground max-w-xs truncate">
															{task.command}
														</td>
														<td className="px-4 py-3">
															<div className="flex items-center gap-1">
																<button
																	type="button"
																	onClick={() => handleEdit(task)}
																	className="p-1.5 rounded-lg hover:bg-muted transition-colors"
																	title="编辑"
																>
																	<Edit className="size-3.5" />
																</button>
																<button
																	type="button"
																	onClick={() => handleDelete(task)}
																	className="p-1.5 rounded-lg hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
																	title="删除"
																>
																	<Trash2 className="size-3.5" />
																</button>
															</div>
														</td>
													</tr>
												))}
											</tbody>
										</table>
									</div>
								</div>
							) : (
								<div className="flex flex-col items-center justify-center h-full text-muted-foreground">
									<Clock className="size-16 mb-4 opacity-20" />
									<p className="text-sm font-medium">还没有定时任务</p>
									<p className="text-xs text-muted-foreground/60 mt-1 mb-4">
										创建第一个定时任务
									</p>
									<button
										type="button"
										onClick={handleCreate}
										className="flex items-center gap-2 px-4 py-2 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
									>
										<Plus className="size-4" />
										新建任务
									</button>
								</div>
							)}
						</div>
					)}
				</div>
			</div>
		</div>
	);
}
