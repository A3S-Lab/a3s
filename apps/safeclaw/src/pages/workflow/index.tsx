import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { useSnapshot } from "valtio";
import { Plus, GitBranch, Trash2, Clock, ChevronRight } from "lucide-react";
import workflowModel from "@/models/workflow.model";
import dayjs from "dayjs";

function CreateDialog({ onClose }: { onClose: () => void }) {
	const [name, setName] = useState("");
	const [desc, setDesc] = useState("");
	const nav = useNavigate();

	const handleCreate = () => {
		if (!name.trim()) return;
		const wf = workflowModel.create(name.trim(), desc.trim() || undefined);
		onClose();
		nav(`/workflow/${wf.id}`);
	};

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-black/40">
			<div className="bg-background rounded-xl shadow-xl w-[400px] p-6 flex flex-col gap-4">
				<h2 className="text-base font-semibold">新建工作流</h2>
				<div className="flex flex-col gap-1">
					<label className="text-xs text-muted-foreground">名称</label>
					<input
						autoFocus
						type="text"
						value={name}
						onChange={(e) => setName(e.target.value)}
						onKeyDown={(e) => e.key === "Enter" && handleCreate()}
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
						onClick={handleCreate}
						disabled={!name.trim()}
						className="px-4 py-1.5 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-40"
					>
						创建
					</button>
				</div>
			</div>
		</div>
	);
}

export default function WorkflowPage() {
	const { workflows } = useSnapshot(workflowModel.state);
	const nav = useNavigate();
	const [showCreate, setShowCreate] = useState(false);

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
				{workflows.length === 0 ? (
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
							<div
								key={wf.id}
								className="group relative flex flex-col gap-3 p-4 rounded-xl border bg-card hover:border-primary/40 hover:shadow-sm transition-all cursor-pointer"
								onClick={() => nav(`/workflow/${wf.id}`)}
							>
								<div className="flex items-start justify-between gap-2">
									<div className="flex items-center gap-2 min-w-0">
										<div className="flex items-center justify-center size-8 rounded-lg bg-primary/10 shrink-0">
											<GitBranch className="size-4 text-primary" />
										</div>
										<span className="text-sm font-medium truncate">
											{wf.name}
										</span>
									</div>
									<button
										type="button"
										onClick={(e) => {
											e.stopPropagation();
											workflowModel.remove(wf.id);
										}}
										className="opacity-0 group-hover:opacity-100 shrink-0 flex items-center justify-center size-6 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-all"
									>
										<Trash2 className="size-3.5" />
									</button>
								</div>
								{wf.description && (
									<p className="text-xs text-muted-foreground line-clamp-2">
										{wf.description}
									</p>
								)}
								<div className="flex items-center justify-between mt-auto pt-1">
									<div className="flex items-center gap-1 text-[11px] text-muted-foreground">
										<Clock className="size-3" />
										{dayjs(wf.updatedAt).format("MM-DD HH:mm")}
									</div>
									<ChevronRight className="size-3.5 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
								</div>
							</div>
						))}
					</div>
				)}
			</div>

			{showCreate && <CreateDialog onClose={() => setShowCreate(false)} />}
		</div>
	);
}
