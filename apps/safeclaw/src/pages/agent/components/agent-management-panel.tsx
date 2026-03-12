/**
 * Agent Management Panel - Unified panel for managing agent skills, workflows, tasks, and knowledge
 */
import { X } from "lucide-react";
import WorkflowPanel from "./workflow-panel";
import { TaskListPanel } from "@/components/custom/task-list-panel";
import SkillsPanel from "./skills-panel";
import KnowledgePanel from "./knowledge-panel";

export type ManagementType = "skills" | "flows" | "tasks" | "knowledge";

interface AgentManagementPanelProps {
	open: boolean;
	onClose: () => void;
	agentId: string;
	type: ManagementType;
}

const PANEL_TITLES: Record<ManagementType, string> = {
	skills: "技能管理",
	flows: "工作流管理",
	tasks: "定时任务管理",
	knowledge: "知识库管理",
};

export default function AgentManagementPanel({
	open,
	onClose,
	agentId,
	type,
}: AgentManagementPanelProps) {
	if (!open) return null;

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
			<div className="bg-background rounded-xl shadow-xl w-[800px] h-[600px] flex flex-col overflow-hidden">
				{/* Header */}
				<div className="flex items-center justify-between px-4 py-3 border-b shrink-0">
					<h2 className="text-base font-semibold">{PANEL_TITLES[type]}</h2>
					<button
						type="button"
						onClick={onClose}
						className="text-muted-foreground hover:text-foreground transition-colors"
					>
						<X className="size-4" />
					</button>
				</div>

				{/* Content */}
				<div className="flex-1 overflow-hidden">
					{type === "skills" && <SkillsPanel agentId={agentId} />}
					{type === "flows" && <WorkflowPanel agentId={agentId} />}
					{type === "tasks" && <TaskListPanel sessionId={agentId} />}
					{type === "knowledge" && <KnowledgePanel agentId={agentId} />}
				</div>
			</div>
		</div>
	);
}
