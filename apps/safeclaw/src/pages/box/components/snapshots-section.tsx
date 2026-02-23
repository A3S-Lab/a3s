/**
 * Snapshots Section — manage VM snapshots.
 */
import { SectionHeader } from "@/components/layout/sidebar-layout";
import boxModel from "@/models/box.model";
import { Camera, Trash2, RotateCw, Loader2, Undo2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useSnapshot } from "valtio";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

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

export function SnapshotsSection() {
	const snap = useSnapshot(boxModel.state);
	const [actionId, setActionId] = useState<string | null>(null);

	useEffect(() => {
		boxModel.fetchSnapshots();
	}, []);

	const handleRestore = async (id: string) => {
		setActionId(id);
		try {
			await boxModel.restoreSnapshot(id);
		} catch {
			/* ignore */
		} finally {
			setActionId(null);
		}
	};

	const handleDelete = async (id: string) => {
		setActionId(id);
		try {
			await boxModel.removeSnapshot(id);
		} catch {
			/* ignore */
		} finally {
			setActionId(null);
		}
	};

	return (
		<div>
			<SectionHeader
				title="快照"
				description="管理 MicroVM 快照与恢复"
				icon={Camera}
			/>

			{/* Toolbar */}
			<div className="flex items-center gap-2 mb-4">
				<button
					type="button"
					className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
					onClick={() => boxModel.fetchSnapshots()}
				>
					<RotateCw className={`size-3 ${snap.loading.snapshots ? "animate-spin" : ""}`} />
					刷新
				</button>
				<span className="ml-auto text-xs text-muted-foreground">
					{snap.snapshots.length} 个快照
				</span>
			</div>

			{/* Snapshot list */}
			{snap.snapshots.length === 0 ? (
				<div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
					{snap.loading.snapshots ? "加载中..." : "暂无快照"}
				</div>
			) : (
				<div className="space-y-2">
					{snap.snapshots.map((snapshot) => (
						<div
							key={snapshot.id}
							className="rounded-xl border bg-card p-4 hover:border-primary/30 transition-colors"
						>
							<div className="flex items-center justify-between">
								<div className="min-w-0 flex-1">
									<div className="flex items-center gap-2 mb-1">
										<span className="font-medium text-sm">
											{snapshot.box_name}
										</span>
										<span className="text-[10px] text-muted-foreground font-mono">
											{snapshot.id.slice(0, 12)}
										</span>
									</div>
									{snapshot.description && (
										<div className="text-xs text-muted-foreground mb-1">
											{snapshot.description}
										</div>
									)}
									<div className="flex items-center gap-3 text-[11px] text-muted-foreground">
										<span className="font-mono">{formatBytes(snapshot.size)}</span>
										<span>{timeAgo(snapshot.created_at)}</span>
									</div>
								</div>
								<div className="flex items-center gap-1 shrink-0">
									{actionId === snapshot.id ? (
										<Loader2 className="size-4 animate-spin text-muted-foreground" />
									) : (
										<>
											<button
												type="button"
												className="p-1.5 rounded-md hover:bg-primary/10 text-muted-foreground hover:text-primary transition-colors"
												onClick={() => handleRestore(snapshot.id)}
												title="恢复快照"
											>
												<Undo2 className="size-3.5" />
											</button>
											<button
												type="button"
												className="p-1.5 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors"
												onClick={() => handleDelete(snapshot.id)}
												title="删除快照"
											>
												<Trash2 className="size-3.5" />
											</button>
										</>
									)}
								</div>
							</div>
						</div>
					))}
				</div>
			)}
		</div>
	);
}
