/**
 * Volumes Section — manage data volumes.
 */
import { SectionHeader } from "@/components/layout/sidebar-layout";
import boxModel from "@/models/box.model";
import { HardDrive, Trash2, Plus, RotateCw, Loader2 } from "lucide-react";
import { useEffect, useState } from "react";
import { useSnapshot } from "valtio";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

export function VolumesSection() {
	const snap = useSnapshot(boxModel.state);
	const [showCreate, setShowCreate] = useState(false);
	const [newName, setNewName] = useState("");
	const [creating, setCreating] = useState(false);
	const [pruning, setPruning] = useState(false);
	const [deletingName, setDeletingName] = useState<string | null>(null);

	useEffect(() => {
		boxModel.fetchVolumes();
	}, []);

	const handleCreate = async () => {
		if (!newName.trim()) return;
		setCreating(true);
		try {
			await boxModel.createVolume({ name: newName.trim() });
			setNewName("");
			setShowCreate(false);
		} catch {
			/* ignore */
		} finally {
			setCreating(false);
		}
	};

	const handleDelete = async (name: string) => {
		setDeletingName(name);
		try {
			await boxModel.removeVolume(name);
		} catch {
			/* ignore */
		} finally {
			setDeletingName(null);
		}
	};

	const handlePrune = async () => {
		setPruning(true);
		try {
			await boxModel.pruneVolumes();
		} catch {
			/* ignore */
		} finally {
			setPruning(false);
		}
	};

	return (
		<div>
			<SectionHeader
				title="存储卷"
				description="管理数据持久化存储"
				icon={HardDrive}
			/>

			{/* Toolbar */}
			<div className="flex items-center gap-2 mb-4">
				<button
					type="button"
					className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
					onClick={() => setShowCreate(!showCreate)}
				>
					<Plus className="size-3" />
					创建存储卷
				</button>
				<button
					type="button"
					className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
					onClick={() => boxModel.fetchVolumes()}
				>
					<RotateCw className={`size-3 ${snap.loading.volumes ? "animate-spin" : ""}`} />
					刷新
				</button>
				<button
					type="button"
					className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-destructive hover:bg-destructive/10 transition-colors disabled:opacity-50"
					onClick={handlePrune}
					disabled={pruning}
				>
					{pruning ? <Loader2 className="size-3 animate-spin" /> : <Trash2 className="size-3" />}
					清理未使用
				</button>
			</div>

			{/* Create form */}
			{showCreate && (
				<div className="rounded-xl border bg-card p-4 mb-4">
					<div className="flex items-center gap-2">
						<input
							type="text"
							className="flex-1 rounded-lg border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
							placeholder="存储卷名称"
							value={newName}
							onChange={(e) => setNewName(e.target.value)}
							onKeyDown={(e) => e.key === "Enter" && handleCreate()}
						/>
						<button
							type="button"
							className="px-3 py-2 rounded-md text-xs text-muted-foreground hover:bg-muted/50 transition-colors"
							onClick={() => setShowCreate(false)}
						>
							取消
						</button>
						<button
							type="button"
							className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-2 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
							onClick={handleCreate}
							disabled={creating || !newName.trim()}
						>
							{creating && <Loader2 className="size-3 animate-spin" />}
							创建
						</button>
					</div>
				</div>
			)}

			{/* Volume list */}
			{snap.volumes.length === 0 ? (
				<div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
					{snap.loading.volumes ? "加载中..." : "暂无存储卷"}
				</div>
			) : (
				<div className="space-y-2">
					{snap.volumes.map((vol) => (
						<div
							key={vol.name}
							className="rounded-xl border bg-card p-4 hover:border-primary/30 transition-colors"
						>
							<div className="flex items-center justify-between">
								<div className="min-w-0 flex-1">
									<div className="flex items-center gap-2 mb-1">
										<span className="font-medium text-sm">{vol.name}</span>
										<span className="inline-block rounded bg-muted px-1.5 py-0.5 text-[10px] font-mono">
											{vol.driver}
										</span>
									</div>
									<div className="flex items-center gap-3 text-[11px] text-muted-foreground">
										<span className="font-mono">{formatBytes(vol.size)}</span>
										<span className="truncate max-w-[200px]" title={vol.mountpoint}>
											{vol.mountpoint}
										</span>
									</div>
								</div>
								<button
									type="button"
									className="p-1.5 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
									onClick={() => handleDelete(vol.name)}
									disabled={deletingName === vol.name}
									title="删除存储卷"
								>
									{deletingName === vol.name ? (
										<Loader2 className="size-3.5 animate-spin" />
									) : (
										<Trash2 className="size-3.5" />
									)}
								</button>
							</div>
						</div>
					))}
				</div>
			)}
		</div>
	);
}
