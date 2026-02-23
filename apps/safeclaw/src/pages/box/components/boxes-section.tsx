/**
 * Boxes Section — list and manage MicroVM containers.
 */
import { SectionHeader } from "@/components/layout/sidebar-layout";
import boxModel from "@/models/box.model";
import {
	Container,
	Play,
	Square,
	RotateCw,
	Trash2,
	Pause,
	Loader2,
	Cpu,
	MemoryStick,
	Shield,
} from "lucide-react";
import { useEffect, useState } from "react";
import { useSnapshot } from "valtio";
import type { BoxStatus } from "@/typings/box";

const STATUS_COLORS: Record<BoxStatus, string> = {
	running: "bg-green-500",
	stopped: "bg-slate-400",
	paused: "bg-amber-500",
	created: "bg-blue-400",
	exited: "bg-slate-400",
	error: "bg-red-500",
};

const STATUS_LABELS: Record<BoxStatus, string> = {
	running: "运行中",
	stopped: "已停止",
	paused: "已暂停",
	created: "已创建",
	exited: "已退出",
	error: "错误",
};

function timeAgo(ts: number): string {
	const diff = Date.now() - ts;
	const mins = Math.floor(diff / 60000);
	if (mins < 1) return "刚刚";
	if (mins < 60) return `${mins}分钟前`;
	const hours = Math.floor(mins / 60);
	if (hours < 24) return `${hours}小时前`;
	const days = Math.floor(hours / 24);
	return `${days}天前`;
}

export function BoxesSection() {
	const snap = useSnapshot(boxModel.state);
	const [filter, setFilter] = useState<"all" | "running" | "stopped">("all");
	const [actionLoading, setActionLoading] = useState<string | null>(null);

	useEffect(() => {
		boxModel.fetchBoxes();
	}, []);

	const filtered = snap.boxes.filter((b) => {
		if (filter === "running") return b.status === "running";
		if (filter === "stopped") return b.status !== "running";
		return true;
	});

	const handleAction = async (action: () => Promise<void>, id: string) => {
		setActionLoading(id);
		try {
			await action();
		} catch {
			/* ignore */
		} finally {
			setActionLoading(null);
		}
	};

	return (
		<div>
			<SectionHeader
				title="容器管理"
				description="管理 MicroVM 实例的生命周期"
				icon={Container}
			/>

			{/* Filter tabs */}
			<div className="flex items-center gap-1 mb-4">
				{(["all", "running", "stopped"] as const).map((f) => (
					<button
						key={f}
						type="button"
						className={`px-3 py-1.5 rounded-md text-xs font-medium transition-colors ${
							filter === f
								? "bg-primary/10 text-primary"
								: "text-muted-foreground hover:text-foreground hover:bg-muted/50"
						}`}
						onClick={() => setFilter(f)}
					>
						{f === "all" ? `全部 (${snap.boxes.length})` : f === "running" ? `运行中 (${snap.boxes.filter((b) => b.status === "running").length})` : `已停止 (${snap.boxes.filter((b) => b.status !== "running").length})`}
					</button>
				))}
				<button
					type="button"
					className="ml-auto flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
					onClick={() => boxModel.fetchBoxes()}
				>
					<RotateCw className={`size-3 ${snap.loading.boxes ? "animate-spin" : ""}`} />
					刷新
				</button>
			</div>

			{/* Box list */}
			{filtered.length === 0 ? (
				<div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
					{snap.loading.boxes ? "加载中..." : "暂无容器"}
				</div>
			) : (
				<div className="space-y-2">
					{filtered.map((box) => {
						const stats = snap.stats.find((s) => s.id === box.id);
						const isLoading = actionLoading === box.id;
						return (
							<div
								key={box.id}
								className="rounded-xl border bg-card p-4 hover:border-primary/30 transition-colors"
							>
								<div className="flex items-start justify-between gap-3">
									<div className="min-w-0 flex-1">
										<div className="flex items-center gap-2 mb-1">
											<span className={`size-2 rounded-full shrink-0 ${STATUS_COLORS[box.status]}`} />
											<span className="font-medium text-sm truncate">{box.name}</span>
											{box.tee && (
												<Shield className="size-3 text-green-500 shrink-0" title="TEE 加密" />
											)}
											<span className="text-[10px] text-muted-foreground">
												{STATUS_LABELS[box.status]}
											</span>
										</div>
										<div className="text-xs text-muted-foreground truncate mb-1.5">
											{box.image}
										</div>
										<div className="flex items-center gap-3 text-[11px] text-muted-foreground">
											<span className="flex items-center gap-1">
												<Cpu className="size-3" />
												{box.cpus} vCPU
											</span>
											<span className="flex items-center gap-1">
												<MemoryStick className="size-3" />
												{box.memory}
											</span>
											{stats && (
												<span className="font-mono">
													CPU {stats.cpu_percent.toFixed(1)}%
												</span>
											)}
											<span>{timeAgo(box.created_at)}</span>
										</div>
									</div>

									{/* Actions */}
									<div className="flex items-center gap-1 shrink-0">
										{isLoading ? (
											<Loader2 className="size-4 animate-spin text-muted-foreground" />
										) : (
											<>
												{box.status === "running" && (
													<>
														<button
															type="button"
															className="p-1.5 rounded-md hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors"
															onClick={() => handleAction(() => boxModel.pauseBox(box.id), box.id)}
															title="暂停"
														>
															<Pause className="size-3.5" />
														</button>
														<button
															type="button"
															className="p-1.5 rounded-md hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors"
															onClick={() => handleAction(() => boxModel.stopBox(box.id), box.id)}
															title="停止"
														>
															<Square className="size-3.5" />
														</button>
														<button
															type="button"
															className="p-1.5 rounded-md hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors"
															onClick={() => handleAction(() => boxModel.restartBox(box.id), box.id)}
															title="重启"
														>
															<RotateCw className="size-3.5" />
														</button>
													</>
												)}
												{box.status === "paused" && (
													<button
														type="button"
														className="p-1.5 rounded-md hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors"
														onClick={() => handleAction(() => boxModel.unpauseBox(box.id), box.id)}
														title="恢复"
													>
														<Play className="size-3.5" />
													</button>
												)}
												{(box.status === "stopped" || box.status === "exited" || box.status === "created") && (
													<button
														type="button"
														className="p-1.5 rounded-md hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors"
														onClick={() => handleAction(() => boxModel.startBox(box.id), box.id)}
														title="启动"
													>
														<Play className="size-3.5" />
													</button>
												)}
												<button
													type="button"
													className="p-1.5 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
													onClick={() => handleAction(() => boxModel.removeBox(box.id, box.status === "running"), box.id)}
													title="删除"
												>
													<Trash2 className="size-3.5" />
												</button>
											</>
										)}
									</div>
								</div>
							</div>
						);
					})}
				</div>
			)}
		</div>
	);
}
