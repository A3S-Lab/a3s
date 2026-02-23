/**
 * Networks Section — manage virtual networks.
 */
import { SectionHeader } from "@/components/layout/sidebar-layout";
import boxModel from "@/models/box.model";
import { Network, Trash2, Plus, RotateCw, Loader2, Shield } from "lucide-react";
import { useEffect, useState } from "react";
import { useSnapshot } from "valtio";

const ISOLATION_LABELS: Record<string, string> = {
	none: "无隔离",
	strict: "严格隔离",
	custom: "自定义",
};

export function NetworksSection() {
	const snap = useSnapshot(boxModel.state);
	const [showCreate, setShowCreate] = useState(false);
	const [newName, setNewName] = useState("");
	const [newIsolation, setNewIsolation] = useState("none");
	const [creating, setCreating] = useState(false);
	const [deletingId, setDeletingId] = useState<string | null>(null);

	useEffect(() => {
		boxModel.fetchNetworks();
	}, []);

	const handleCreate = async () => {
		if (!newName.trim()) return;
		setCreating(true);
		try {
			await boxModel.createNetwork({
				name: newName.trim(),
				isolation: newIsolation,
			});
			setNewName("");
			setShowCreate(false);
		} catch {
			/* ignore */
		} finally {
			setCreating(false);
		}
	};

	const handleDelete = async (id: string) => {
		setDeletingId(id);
		try {
			await boxModel.removeNetwork(id);
		} catch {
			/* ignore */
		} finally {
			setDeletingId(null);
		}
	};

	return (
		<div>
			<SectionHeader
				title="网络"
				description="管理 MicroVM 虚拟网络"
				icon={Network}
			/>

			{/* Toolbar */}
			<div className="flex items-center gap-2 mb-4">
				<button
					type="button"
					className="flex items-center gap-1.5 rounded-lg bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
					onClick={() => setShowCreate(!showCreate)}
				>
					<Plus className="size-3" />
					创建网络
				</button>
				<button
					type="button"
					className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-xs text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
					onClick={() => boxModel.fetchNetworks()}
				>
					<RotateCw className={`size-3 ${snap.loading.networks ? "animate-spin" : ""}`} />
					刷新
				</button>
			</div>

			{/* Create form */}
			{showCreate && (
				<div className="rounded-xl border bg-card p-4 mb-4 space-y-3">
					<input
						type="text"
						className="w-full rounded-lg border bg-background px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
						placeholder="网络名称"
						value={newName}
						onChange={(e) => setNewName(e.target.value)}
						onKeyDown={(e) => e.key === "Enter" && handleCreate()}
					/>
					<div className="flex items-center gap-2">
						<span className="text-xs text-muted-foreground">隔离策略:</span>
						{["none", "strict", "custom"].map((iso) => (
							<button
								key={iso}
								type="button"
								className={`px-2.5 py-1 rounded-md text-xs transition-colors ${
									newIsolation === iso
										? "bg-primary/10 text-primary"
										: "text-muted-foreground hover:bg-muted/50"
								}`}
								onClick={() => setNewIsolation(iso)}
							>
								{ISOLATION_LABELS[iso]}
							</button>
						))}
					</div>
					<div className="flex justify-end gap-2">
						<button
							type="button"
							className="px-3 py-1.5 rounded-md text-xs text-muted-foreground hover:bg-muted/50 transition-colors"
							onClick={() => setShowCreate(false)}
						>
							取消
						</button>
						<button
							type="button"
							className="flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
							onClick={handleCreate}
							disabled={creating || !newName.trim()}
						>
							{creating && <Loader2 className="size-3 animate-spin" />}
							创建
						</button>
					</div>
				</div>
			)}

			{/* Network list */}
			{snap.networks.length === 0 ? (
				<div className="flex items-center justify-center py-12 text-sm text-muted-foreground">
					{snap.loading.networks ? "加载中..." : "暂无网络"}
				</div>
			) : (
				<div className="space-y-2">
					{snap.networks.map((net) => (
						<div
							key={net.id}
							className="rounded-xl border bg-card p-4 hover:border-primary/30 transition-colors"
						>
							<div className="flex items-center justify-between">
								<div className="min-w-0 flex-1">
									<div className="flex items-center gap-2 mb-1">
										<span className="font-medium text-sm">{net.name}</span>
										<span className="inline-block rounded bg-muted px-1.5 py-0.5 text-[10px] font-mono">
											{net.driver}
										</span>
										{net.isolation !== "none" && (
											<span className="flex items-center gap-1 text-[10px] text-green-500">
												<Shield className="size-2.5" />
												{ISOLATION_LABELS[net.isolation]}
											</span>
										)}
									</div>
									<div className="flex items-center gap-3 text-[11px] text-muted-foreground">
										<span>{net.containers} 个容器</span>
										<span>{net.scope}</span>
									</div>
								</div>
								<button
									type="button"
									className="p-1.5 rounded-md hover:bg-destructive/10 text-muted-foreground hover:text-destructive transition-colors disabled:opacity-50"
									onClick={() => handleDelete(net.id)}
									disabled={deletingId === net.id}
									title="删除网络"
								>
									{deletingId === net.id ? (
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
