/**
 * Images Section — manage OCI images.
 */
import { SectionHeader } from "@/components/layout/sidebar-layout";
import boxModel from "@/models/box.model";
import { Image, Trash2, Download, RotateCw, Loader2 } from "lucide-react";
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
	const days = Math.floor(diff / 86400000);
	if (days === 0) return "今天";
	if (days === 1) return "昨天";
	if (days < 30) return `${days}天前`;
	return `${Math.floor(days / 30)}月前`;
}

export function ImagesSection() {
	const snap = useSnapshot(boxModel.state);
	const [pullInput, setPullInput] = useState("");
	const [pulling, setPulling] = useState(false);
	const [pruning, setPruning] = useState(false);
	const [deletingId, setDeletingId] = useState<string | null>(null);

	useEffect(() => {
		boxModel.fetchImages();
	}, []);

	const handlePull = async () => {
		if (!pullInput.trim()) return;
		setPulling(true);
		try {
			await boxModel.pullImage(pullInput.trim());
			setPullInput("");
		} catch {
			/* ignore */
		} finally {
			setPulling(false);
		}
	};

	const handlePrune = async () => {
		setPruning(true);
		try {
			await boxModel.pruneImages();
		} catch {
			/* ignore */
		} finally {
			setPruning(false);
		}
	};

	const handleDelete = async (id: string) => {
		setDeletingId(id);
		try {
			await boxModel.removeImage(id);
		} catch {
			/* ignore */
		} finally {
			setDeletingId(null);
		}
	};

	return (
		<div>
			<SectionHeader
				title="镜像管理"
				description="管理本地 OCI 镜像缓存"
				icon={Image}
			/>

			{/* Pull image */}
			<div className="flex items-center gap-2 mb-4">
				<input
					type="text"
					className="flex-1 rounded-lg border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
					placeholder="输入镜像名称，如 alpine:latest"
					value={pullInput}
					onChange={(e) => setPullInput(e.target.value)}
					onKeyDown={(e) => e.key === "Enter" && handlePull()}
				/>
				<button
					type="button"
					className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
					onClick={handlePull}
					disabled={pulling || !pullInput.trim()}
				>
					{pulling ? (
						<Loader2 className="size-3.5 animate-spin" />
					) : (
						<Download className="size-3.5" />
					)}
					拉取
				</button>
			</div>

			{/* Toolbar */}
			<div className="flex items-center gap-2 mb-4">
				<button
					type="button"
					className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
					onClick={() => boxModel.fetchImages()}
				>
					<RotateCw className={`size-3 ${snap.loading.images ? "animate-spin" : ""}`} />
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
				<span className="ml-auto text-xs text-muted-foreground">
					{snap.images.length} 个镜像
				</span>
			</div>

			{/* Image list */}
			{snap.images.length === 0 ? (
				<div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
					{snap.loading.images ? "加载中..." : "暂无镜像"}
				</div>
			) : (
				<div className="rounded-xl border bg-card overflow-hidden">
					<table className="w-full text-sm">
						<thead>
							<tr className="border-b bg-muted/30">
								<th className="text-left px-4 py-2.5 text-xs font-medium text-muted-foreground">镜像</th>
								<th className="text-left px-4 py-2.5 text-xs font-medium text-muted-foreground">标签</th>
								<th className="text-right px-4 py-2.5 text-xs font-medium text-muted-foreground">大小</th>
								<th className="text-right px-4 py-2.5 text-xs font-medium text-muted-foreground">创建时间</th>
								<th className="w-10" />
							</tr>
						</thead>
						<tbody>
							{snap.images.map((img) => (
								<tr key={img.id} className="border-b last:border-b-0 hover:bg-muted/20 transition-colors">
									<td className="px-4 py-2.5 font-mono text-xs truncate max-w-[200px]">
										{img.repository}
									</td>
									<td className="px-4 py-2.5">
										<span className="inline-block rounded bg-muted px-1.5 py-0.5 text-[11px] font-mono">
											{img.tag}
										</span>
									</td>
									<td className="px-4 py-2.5 text-right text-xs text-muted-foreground font-mono">
										{formatBytes(img.size)}
									</td>
									<td className="px-4 py-2.5 text-right text-xs text-muted-foreground">
										{timeAgo(img.created_at)}
									</td>
									<td className="px-2 py-2.5">
										<button
											type="button"
											className="p-1 rounded hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
											onClick={() => handleDelete(img.id)}
											disabled={deletingId === img.id}
											title="删除镜像"
										>
											{deletingId === img.id ? (
												<Loader2 className="size-3.5 animate-spin" />
											) : (
												<Trash2 className="size-3.5" />
											)}
										</button>
									</td>
								</tr>
							))}
						</tbody>
					</table>
				</div>
			)}
		</div>
	);
}
