/**
 * Box Overview — system info, resource stats, disk usage.
 */
import { SectionHeader, StatCard } from "@/components/layout/sidebar-layout";
import boxModel from "@/models/box.model";
import { Box, Cpu, HardDrive, Image, Network, Shield, Trash2, Loader2 } from "lucide-react";
import { useEffect } from "react";
import { useSnapshot } from "valtio";

function formatBytes(bytes: number): string {
	if (bytes === 0) return "0 B";
	const k = 1024;
	const sizes = ["B", "KB", "MB", "GB", "TB"];
	const i = Math.floor(Math.log(bytes) / Math.log(k));
	return `${(bytes / k ** i).toFixed(1)} ${sizes[i]}`;
}

export function OverviewSection() {
	const snap = useSnapshot(boxModel.state);
	const info = snap.systemInfo;
	const disk = snap.diskUsage;
	const loading = snap.loading.system;

	useEffect(() => {
		boxModel.fetchSystemInfo();
		boxModel.fetchBoxes();
		boxModel.fetchImages();
		boxModel.fetchNetworks();
		boxModel.fetchVolumes();
	}, []);

	const runningCount = snap.boxes.filter((b) => b.status === "running").length;
	const stoppedCount = snap.boxes.filter((b) => b.status !== "running").length;

	return (
		<div>
			<SectionHeader
				title="系统概览"
				description="A3S Box MicroVM 运行时状态"
				icon={Box}
			/>

			{/* System info */}
			{info && (
				<div className="mb-6 rounded-xl border bg-card p-4">
					<div className="flex items-center gap-2 mb-3">
						<Cpu className="size-4 text-primary" />
						<span className="text-sm font-medium">系统信息</span>
					</div>
					<div className="grid grid-cols-2 gap-x-8 gap-y-1.5 text-sm">
						<div className="flex justify-between">
							<span className="text-muted-foreground">版本</span>
							<span className="font-mono text-xs">{info.version}</span>
						</div>
						<div className="flex justify-between">
							<span className="text-muted-foreground">平台</span>
							<span className="font-mono text-xs">{info.os}/{info.arch}</span>
						</div>
						<div className="flex justify-between">
							<span className="text-muted-foreground">CPU</span>
							<span>{info.cpus} 核</span>
						</div>
						<div className="flex justify-between">
							<span className="text-muted-foreground">内存</span>
							<span>{formatBytes(info.memory_total)}</span>
						</div>
						<div className="flex justify-between">
							<span className="text-muted-foreground">TEE</span>
							<span className={info.tee_available ? "text-green-500" : "text-muted-foreground"}>
								{info.tee_available ? info.tee_backend : "不可用"}
							</span>
						</div>
					</div>
				</div>
			)}

			{/* Stats cards */}
			<div className="grid grid-cols-2 gap-3 mb-6">
				<StatCard
					icon={Box}
					label="运行中"
					value={runningCount}
					sub={`${stoppedCount} 已停止`}
					color="text-green-500"
				/>
				<StatCard
					icon={Image}
					label="镜像"
					value={snap.images.length}
					sub="本地缓存"
				/>
				<StatCard
					icon={Network}
					label="网络"
					value={snap.networks.length}
					sub="虚拟网络"
				/>
				<StatCard
					icon={HardDrive}
					label="存储卷"
					value={snap.volumes.length}
					sub="数据卷"
				/>
			</div>

			{/* Disk usage */}
			{disk && (
				<div className="rounded-xl border bg-card p-4 mb-6">
					<div className="flex items-center gap-2 mb-3">
						<HardDrive className="size-4 text-primary" />
						<span className="text-sm font-medium">磁盘使用</span>
					</div>
					<div className="space-y-2">
						{[
							{ label: "镜像", value: disk.images_size },
							{ label: "容器", value: disk.containers_size },
							{ label: "存储卷", value: disk.volumes_size },
							{ label: "缓存", value: disk.cache_size },
						].map((item) => (
							<div key={item.label} className="flex items-center justify-between text-sm">
								<span className="text-muted-foreground">{item.label}</span>
								<span className="font-mono text-xs">{formatBytes(item.value)}</span>
							</div>
						))}
						<div className="border-t pt-2 flex items-center justify-between text-sm font-medium">
							<span>总计</span>
							<span className="font-mono text-xs">{formatBytes(disk.total)}</span>
						</div>
					</div>
				</div>
			)}

			{/* TEE status */}
			{info && (
				<div className="rounded-xl border bg-card p-4 mb-6">
					<div className="flex items-center gap-2 mb-3">
						<Shield className="size-4 text-primary" />
						<span className="text-sm font-medium">安全隔离</span>
					</div>
					<div className="text-sm text-muted-foreground">
						{info.tee_available ? (
							<div className="flex items-center gap-2">
								<span className="size-2 rounded-full bg-green-500" />
								<span>TEE 硬件加密已启用 ({info.tee_backend})</span>
							</div>
						) : (
							<div className="flex items-center gap-2">
								<span className="size-2 rounded-full bg-amber-500" />
								<span>VM 隔离模式 (MicroVM 硬件隔离)</span>
							</div>
						)}
					</div>
				</div>
			)}

			{/* System prune */}
			<button
				type="button"
				className="flex items-center gap-2 rounded-lg border px-3 py-2 text-sm text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors disabled:opacity-50"
				onClick={() => boxModel.systemPrune()}
				disabled={loading}
			>
				{loading ? (
					<Loader2 className="size-3.5 animate-spin" />
				) : (
					<Trash2 className="size-3.5" />
				)}
				系统清理
			</button>
		</div>
	);
}
