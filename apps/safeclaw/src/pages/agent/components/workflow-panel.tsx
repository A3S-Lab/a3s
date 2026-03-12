/**
 * Workflow Panel — shows workflows associated with current agent (persona)
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSnapshot } from "valtio";
import {
	Plus,
	GitBranch,
	Trash2,
	Clock,
	ChevronRight,
	Pencil,
	X,
} from "lucide-react";
import workflowModel, { type WorkflowDoc } from "@/models/workflow.model";
import dayjs from "dayjs";

function WorkflowFormDialog({
	initial,
	title,
	submitLabel,
	onSubmit,
	onClose,
}: {
	initial?: { name: string; description: string };
	title: string;
	submitLabel: string;
	onSubmit: (name: string, description: string) => void;
	onClose: () => void;
}) {
	const [name, setName] = useState(initial?.name ?? "");
	const [desc, setDesc] = useState(initial?.description ?? "");

	const handleSubmit = () => {
		if (!name.trim()) return;
		onSubmit(name.trim(), desc.trim());
	};

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
			<div className="bg-background rounded-xl shadow-xl w-[400px] p-6 flex flex-col gap-4">
				<div className="flex items-center justify-between">
					<h2 className="text-base font-semibold">{title}</h2>
					<button
						type="button"
						onClick={onClose}
						className="text-muted-foreground hover:text-foreground transition-colors"
					>
						<X className="size-4" />
					</button>
				</div>
				<div className="flex flex-col gap-1">
					<label className="text-xs text-muted-foreground">名称</label>
					<input
						autoFocus
						type="text"
						value={name}
						onChange={(e) => setName(e.target.value)}
						onKeyDown={(e) => e.key === "Enter" && handleSubmit()}
						placeholder="工作流名称"
						className="border rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring bg-background"
					/>
				</div>
				<div className="flex flex-col gap-1">
					<label className="text-xs text-muted-foreground">描述（可选）</label>
					<input
						type="text"
						value={desc}
						onChange={(e) => setDesc(e.target.value)}
						placeholder="简短描述"
						className="border rounded-lg px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring bg-background"
					/>
				</div>
				<div className="flex justify-end gap-2 pt-1">
					<button
						type="button"
						onClick={onClose}
						className="px-4 py-1.5 text-sm rounded-lg border hover:bg-muted transition-colors"
					>
						取消
					</button>
					<button
						type="button"
						onClick={handleSubmit}
						disabled={!name.trim()}
						className="px-4 py-1.5 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-40"
					>
						{submitLabel}
					</button>
				</div>
			</div>
		</div>
	);
}

function WorkflowCard({
	wf,
	onOpen,
	onEdit,
	onDelete,
}: {
	wf: WorkflowDoc;
	onOpen: () => void;
	onEdit: () => void;
	onDelete: () => void;
}) {
	return (
		<div
			className="group relative flex flex-col gap-2 p-3 rounded-lg border bg-card hover:border-primary/40 hover:shadow-sm transition-all cursor-pointer"
			onClick={onOpen}
		>
			<div className="flex items-start justify-between gap-2">
				<div className="flex items-center gap-2 min-w-0">
					<div className="flex items-center justify-center size-7 rounded-lg bg-primary/10 shrink-0">
						<GitBranch className="size-3.5 text-primary" />
					</div>
					<span className="text-sm font-medium truncate">{wf.name}</span>
				</div>
				<div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all shrink-0">
					<button
						type="button"
						onClick={(e) => {
							e.stopPropagation();
							onEdit();
						}}
						className="flex items-center justify-center size-6 rounded text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06] transition-all"
						title="重命名"
					>
						<Pencil className="size-3" />
					</button>
					<button
						type="button"
						onClick={(e) => {
							e.stopPropagation();
							onDelete();
						}}
						className="flex items-center justify-center size-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-all"
						title="删除"
					>
						<Trash2 className="size-3" />
					</button>
				</div>
			</div>
			{wf.description && (
				<p className="text-xs text-muted-foreground line-clamp-2">
					{wf.description}
				</p>
			)}
			<div className="flex items-center justify-between mt-auto pt-1">
				<div className="flex items-center gap-1 text-[11px] text-muted-foreground">
					<Clock className="size-3" />
					{dayjs(wf.updated_at).format("MM-DD HH:mm")}
				</div>
				<ChevronRight className="size-3.5 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
			</div>
		</div>
	);
}

export default function WorkflowPanel({ agentId }: { agentId: string }) {
	const { workflows, loaded } = useSnapshot(workflowModel.state);
	const nav = useNavigate();
	const [showCreate, setShowCreate] = useState(false);
	const [editingId, setEditingId] = useState<string | null>(null);
	const loadedRef = useRef(false);

	useEffect(() => {
		if (!loadedRef.current) {
			loadedRef.current = true;
			workflowModel.load();
		}
	}, []);

	// Filter workflows by agent (persona) id
	const agentWorkflows = useMemo(() => {
		if (!agentId) return [];
		return workflows.filter((wf) => wf.agent_id === agentId);
	}, [workflows, agentId]);

	const editingWf = editingId
		? workflows.find((w) => w.id === editingId)
		: null;

	const handleCreate = async (name: string, desc: string) => {
		const wf = await workflowModel.create(name, desc || undefined);
		if (agentId) {
			await workflowModel.setAgentId(wf.id, agentId);
		}
		setShowCreate(false);
		nav(`/workflow/${wf.id}`);
	};

	return (
		<div className="flex flex-col h-full bg-background border-l">
			{/* Header */}
			<div className="flex items-center justify-between px-4 py-3 border-b shrink-0">
				<div>
					<h2 className="text-sm font-semibold">工作流</h2>
					<p className="text-xs text-muted-foreground mt-0.5">
						{agentWorkflows.length} 个工作流
					</p>
				</div>
				<button
					type="button"
					onClick={() => setShowCreate(true)}
					className="flex items-center gap-1 px-2 py-1 text-xs rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
					title="新建工作流"
				>
					<Plus className="size-3" />
				</button>
			</div>

			{/* List */}
			<div className="flex-1 overflow-y-auto p-3">
				{!loaded ? (
					<div className="flex items-center justify-center h-full text-muted-foreground text-sm">
						加载中…
					</div>
				) : agentWorkflows.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
						<GitBranch className="size-10 opacity-20" />
						<p className="text-xs text-center">
							还没有工作流
							<br />
							点击 + 创建第一个
						</p>
					</div>
				) : (
					<div className="flex flex-col gap-2">
						{agentWorkflows.map((wf) => (
							<WorkflowCard
								key={wf.id}
								wf={wf}
								onOpen={() => nav(`/workflow/${wf.id}`)}
								onEdit={() => setEditingId(wf.id)}
								onDelete={() =>
									workflowModel.remove(wf.id).catch(console.error)
								}
							/>
						))}
					</div>
				)}
			</div>

			{showCreate && (
				<WorkflowFormDialog
					title="新建工作流"
					submitLabel="创建"
					onClose={() => setShowCreate(false)}
					onSubmit={handleCreate}
				/>
			)}

			{editingWf && (
				<WorkflowFormDialog
					title="编辑工作流"
					submitLabel="保存"
					initial={{
						name: editingWf.name,
						description: editingWf.description ?? "",
					}}
					onClose={() => setEditingId(null)}
					onSubmit={async (name, desc) => {
						await workflowModel.update(editingId!, {
							name,
							description: desc || undefined,
						});
						setEditingId(null);
					}}
				/>
			)}
		</div>
	);
}
