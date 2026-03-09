import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSnapshot } from "valtio";
import {
	Plus,
	GitBranch,
	Trash2,
	Clock,
	ChevronRight,
	Pencil,
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
				<h2 className="text-base font-semibold">{title}</h2>
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
			className="group relative flex flex-col gap-3 p-4 rounded-xl border bg-card hover:border-primary/40 hover:shadow-sm transition-all cursor-pointer"
			onClick={onOpen}
		>
			<div className="flex items-start justify-between gap-2">
				<div className="flex items-center gap-2 min-w-0">
					<div className="flex items-center justify-center size-8 rounded-lg bg-primary/10 shrink-0">
						<GitBranch className="size-4 text-primary" />
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
						<Pencil className="size-3.5" />
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
						<Trash2 className="size-3.5" />
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

export default function WorkflowPage() {
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

	const editingWf = editingId
		? workflows.find((w) => w.id === editingId)
		: null;

	return (
		<div className="flex flex-col h-full bg-background">
			{/* Header */}
			<div className="flex items-center justify-between px-6 py-4 border-b shrink-0">
				<div>
					<h1 className="text-base font-semibold">工作流</h1>
					<p className="text-xs text-muted-foreground mt-0.5">
						可视化编排 AI 工作流
					</p>
				</div>
				<button
					type="button"
					onClick={() => setShowCreate(true)}
					className="flex items-center gap-1.5 px-3 py-1.5 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
				>
					<Plus className="size-3.5" />
					新建工作流
				</button>
			</div>

			{/* List */}
			<div className="flex-1 overflow-y-auto p-6">
				{!loaded ? (
					<div className="flex items-center justify-center h-full text-muted-foreground text-sm">
						加载中…
					</div>
				) : workflows.length === 0 ? (
					<div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground">
						<GitBranch className="size-12 opacity-20" />
						<p className="text-sm">还没有工作流</p>
						<button
							type="button"
							onClick={() => setShowCreate(true)}
							className="flex items-center gap-1.5 px-4 py-2 text-sm rounded-lg border hover:bg-muted transition-colors"
						>
							<Plus className="size-3.5" />
							创建第一个工作流
						</button>
					</div>
				) : (
					<div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
						{workflows.map((wf) => (
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
					onSubmit={async (name, desc) => {
						const wf = await workflowModel.create(name, desc || undefined);
						setShowCreate(false);
						nav(`/workflow/${wf.id}`);
					}}
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
