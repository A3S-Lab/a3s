import agentModel from "@/models/agent.model";
import type { AgentTask } from "@/models/agent.model";
import { cn } from "@/lib/utils";
import { CheckCircle2, Circle, Clock, Loader2, XCircle } from "lucide-react";
import { useMemo, useState } from "react";
import { useSnapshot } from "valtio";

type TaskStatus = "pending" | "running" | "completed";

const TAB_LABELS: Record<TaskStatus, string> = {
	pending: "待处理",
	running: "处理中",
	completed: "已完成",
};

function StatusIcon({ status }: { status: AgentTask["status"] }) {
	if (status === "in_progress")
		return <Loader2 className="size-3 text-primary animate-spin" />;
	if (status === "pending") return <Clock className="size-3 text-amber-500" />;
	if (status === "failed")
		return <XCircle className="size-3 text-destructive/70" />;
	if (status === "skipped" || status === "cancelled")
		return <Circle className="size-3 text-muted-foreground/40" />;
	return <CheckCircle2 className="size-3 text-emerald-500/80" />;
}

function taskToTab(status: AgentTask["status"]): TaskStatus {
	if (status === "in_progress") return "running";
	if (status === "completed") return "completed";
	return "pending";
}

export function TaskListPanel({ sessionId }: { sessionId: string }) {
	const [activeTab, setActiveTab] = useState<TaskStatus>("running");
	const { tasks, activeToolProgress, sessionStatus } = useSnapshot(
		agentModel.state,
	);

	const planningTasks = tasks[sessionId] ?? [];
	const isRunning = sessionStatus[sessionId] === "running";

	// Group planning tasks by tab
	const grouped = useMemo<Record<TaskStatus, AgentTask[]>>(() => {
		const result: Record<TaskStatus, AgentTask[]> = {
			pending: [],
			running: [],
			completed: [],
		};
		for (const t of planningTasks) {
			result[taskToTab(t.status)].push(t as AgentTask);
		}
		return result;
	}, [planningTasks]);

	// If no planning tasks, fall back to tool-level activity
	const hasPlanningTasks = planningTasks.length > 0;
	const toolProgress = activeToolProgress[sessionId];

	const counts: Record<TaskStatus, number> = {
		pending: grouped.pending.length,
		running:
			grouped.running.length + (!hasPlanningTasks && toolProgress ? 1 : 0),
		completed: grouped.completed.length,
	};

	const displayTasks: {
		id: string;
		label: string;
		summary: string;
		status: AgentTask["status"];
	}[] = hasPlanningTasks
		? grouped[activeTab].map((t) => ({
				id: t.id,
				label: t.content,
				summary: t.success_criteria ?? t.tool ?? "",
				status: t.status,
			}))
		: activeTab === "running" && toolProgress
			? [
					{
						id: toolProgress.tool_use_id,
						label: toolProgress.tool_name,
						summary: toolProgress.input ?? "",
						status: "in_progress" as const,
					},
				]
			: activeTab === "running" && isRunning
				? [
						{
							id: "__thinking__",
							label: "思考中",
							summary: "",
							status: "in_progress" as const,
						},
					]
				: [];

	return (
		<div className="flex flex-col h-full text-xs">
			{/* Tabs */}
			<div className="flex border-b shrink-0">
				{(["running", "pending", "completed"] as TaskStatus[]).map((tab) => (
					<button
						key={tab}
						type="button"
						onClick={() => setActiveTab(tab)}
						className={cn(
							"flex-1 py-2 text-[11px] font-medium transition-colors relative",
							activeTab === tab
								? "text-foreground"
								: "text-muted-foreground hover:text-foreground",
						)}
					>
						{TAB_LABELS[tab]}
						{counts[tab] > 0 && (
							<span
								className={cn(
									"ml-1 inline-flex items-center justify-center rounded-full px-1.5 py-0.5 text-[9px] font-semibold leading-none",
									tab === "pending"
										? "bg-amber-500/15 text-amber-600 dark:text-amber-400"
										: tab === "running"
											? "bg-primary/15 text-primary"
											: "bg-muted text-muted-foreground",
								)}
							>
								{counts[tab]}
							</span>
						)}
						{activeTab === tab && (
							<span className="absolute bottom-0 left-0 right-0 h-0.5 bg-primary rounded-full" />
						)}
					</button>
				))}
			</div>

			{/* Task list */}
			<div className="flex-1 overflow-y-auto">
				{displayTasks.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-full gap-2 text-muted-foreground/50 py-8">
						<Circle className="size-6 opacity-30" />
						<span className="text-[11px]">暂无任务</span>
					</div>
				) : (
					<div className="p-2 space-y-1.5">
						{displayTasks.map((task) => (
							<div
								key={task.id}
								className={cn(
									"flex items-start gap-2 rounded-lg border px-2.5 py-2",
									task.status === "pending"
										? "border-amber-500/20 bg-amber-500/[0.03]"
										: task.status === "in_progress"
											? "border-primary/20 bg-primary/[0.03]"
											: task.status === "failed"
												? "border-destructive/15 bg-destructive/[0.02]"
												: "border-border/50 bg-muted/30",
								)}
							>
								<div className="mt-0.5 shrink-0">
									<StatusIcon status={task.status} />
								</div>
								<div className="flex-1 min-w-0">
									<p className="font-medium text-[11px] text-foreground/90 leading-tight">
										{task.label}
									</p>
									{task.summary && (
										<p className="text-[10px] text-muted-foreground/60 font-mono truncate mt-0.5 leading-tight">
											{task.summary}
										</p>
									)}
								</div>
							</div>
						))}
					</div>
				)}
			</div>
		</div>
	);
}
