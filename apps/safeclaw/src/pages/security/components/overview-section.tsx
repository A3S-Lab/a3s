import { DonutChart } from "@/components/custom/charts/donut-chart";
import { SparkAreaChart } from "@/components/custom/charts/spark-chart";
import { cn } from "@/lib/utils";
import { timeAgo } from "@/lib/time";
import securityModel from "@/models/security.model";
import type { Severity } from "@/models/security.model";
import {
	Activity,
	AlertTriangle,
	Radio,
	Shield,
	ShieldCheck,
	ShieldX,
	X,
} from "lucide-react";
import { useMemo, useEffect } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
import { fetchTeeStatus, fetchSecurityOverview } from "@/lib/security-api";
import {
	SEVERITY_COLORS,
	SEVERITY_DOT,
	SEVERITY_LABELS,
	VECTOR_LABELS,
	TEE_COLORS,
	TEE_LABELS,
} from "../constants";
import { SectionHeader, StatCard } from "./shared";

export function OverviewSection() {
	const snap = useSnapshot(securityModel.state);

	useEffect(() => {
		Promise.all([
			fetchTeeStatus().catch((e) => {
				console.warn("TEE status unavailable:", e);
				return null;
			}),
			fetchSecurityOverview().catch((e) => {
				console.warn("Security overview unavailable:", e);
				return null;
			}),
		]).then(([tee, overview]) => {
			if (tee) {
				securityModel.state.tee = {
					level: tee.level ?? "ProcessOnly",
					backend: tee.backend ?? "—",
					attestationExpiry: tee.attestation_expiry ?? null,
					healthy: tee.healthy ?? false,
				};
			}
			if (overview) {
				if (overview.today_blocked != null)
					securityModel.state.todayBlocked = overview.today_blocked;
				if (overview.active_sessions != null)
					securityModel.state.activeSessions = overview.active_sessions;
				if (overview.risk_score != null)
					securityModel.state.riskScore = overview.risk_score;
			}
		});
	}, []);

	const donutData = Object.entries(snap.auditStats.bySeverity)
		.filter(([, v]) => v > 0)
		.map(([k, v]) => ({ name: SEVERITY_LABELS[k as Severity], value: v }));

	const vectorData = Object.entries(snap.auditStats.byVector)
		.filter(([, v]) => v > 0)
		.map(([k, v]) => ({ name: VECTOR_LABELS[k] || k, value: v }));

	const trendData = useMemo(() => {
		const now = Date.now();
		const buckets = Array.from({ length: 24 }, (_, i) => ({
			hour: `${i}:00`,
			events: 0,
		}));
		for (const e of snap.auditEvents) {
			const hoursAgo = Math.floor((now - e.timestamp) / 3_600_000);
			if (hoursAgo >= 0 && hoursAgo < 24) {
				buckets[23 - hoursAgo].events++;
			}
		}
		return buckets;
	}, [snap.auditEvents]);

	return (
		<div>
			<SectionHeader
				icon={Shield}
				title="安全概览"
				description="实时安全状态与关键指标。"
			/>

			<div className="grid grid-cols-4 gap-4 mb-6">
				<div className="rounded-xl border bg-card p-5">
					<div className="flex items-center gap-2 mb-2">
						<div className="relative">
							<span
								className={cn(
									"absolute inline-flex h-full w-full rounded-full opacity-75",
									snap.tee.healthy
										? "bg-green-500 animate-ping"
										: "bg-muted-foreground/30",
								)}
							/>
							<span
								className={cn(
									"relative inline-flex rounded-full size-2",
									snap.tee.healthy ? "bg-green-500" : "bg-muted-foreground/40",
								)}
							/>
						</div>
						<span className="text-xs text-muted-foreground">TEE 状态</span>
					</div>
					<p
						className={cn(
							"text-lg font-bold",
							snap.tee.healthy
								? TEE_COLORS[snap.tee.level]
								: "text-muted-foreground",
						)}
					>
						{snap.tee.backend}
					</p>
					<p className="text-[11px] text-muted-foreground mt-0.5">
						{snap.tee.healthy ? TEE_LABELS[snap.tee.level] : "未连接"}
					</p>
				</div>
				<StatCard
					icon={ShieldX}
					label="今日拦截"
					value={snap.todayBlocked}
					sub="次威胁已阻止"
					color="text-destructive"
				/>
				<StatCard
					icon={Activity}
					label="风险评分"
					value={`${snap.riskScore}/100`}
					sub={snap.riskScore >= 80 ? "⚠️ 超过阈值" : "正常范围"}
					color={snap.riskScore >= 80 ? "text-destructive" : "text-amber-500"}
				/>
				<StatCard
					icon={Radio}
					label="活跃追踪"
					value={snap.taintEntries.length}
					sub="个数据污点标记"
					color="text-violet-500"
				/>
			</div>

			<div className="grid grid-cols-3 gap-4 mb-6">
				<div className="rounded-xl border bg-card p-5">
					<div className="text-sm font-semibold mb-4">事件严重度分布</div>
					{donutData.length === 0 ? (
						<div className="flex flex-col items-center justify-center h-36 text-muted-foreground/40">
							<ShieldCheck className="size-8 mb-2" />
							<span className="text-xs">暂无事件</span>
						</div>
					) : (
						<>
							<div className="flex items-center justify-center">
								<DonutChart
									data={donutData}
									category="name"
									value="value"
									className="h-36 w-36"
									colors={["blue", "amber", "pink", "fuchsia"]}
									showLabel
									valueFormatter={(v) => `${v}`}
								/>
							</div>
							<div className="flex flex-wrap justify-center gap-3 mt-4">
								{donutData.map((d) => (
									<div
										key={d.name}
										className="flex items-center gap-1.5 text-[11px] text-muted-foreground"
									>
										<span className="size-2 rounded-full bg-current" />
										{d.name} ({d.value})
									</div>
								))}
							</div>
						</>
					)}
				</div>

				<div className="rounded-xl border bg-card p-5">
					<div className="text-sm font-semibold mb-4">拦截类型分布</div>
					{vectorData.length === 0 ? (
						<div className="flex flex-col items-center justify-center h-36 text-muted-foreground/40">
							<ShieldCheck className="size-8 mb-2" />
							<span className="text-xs">暂无拦截记录</span>
						</div>
					) : (
						<>
							<div className="flex items-center justify-center">
								<DonutChart
									data={vectorData}
									category="name"
									value="value"
									className="h-36 w-36"
									colors={[
										"blue",
										"emerald",
										"violet",
										"amber",
										"slate",
										"pink",
									]}
									showLabel
									valueFormatter={(v) => `${v}`}
								/>
							</div>
							<div className="flex flex-wrap justify-center gap-3 mt-4">
								{vectorData.map((d) => (
									<div
										key={d.name}
										className="flex items-center gap-1.5 text-[11px] text-muted-foreground"
									>
										<span className="size-2 rounded-full bg-current" />
										{d.name} ({d.value})
									</div>
								))}
							</div>
						</>
					)}
				</div>

				<div className="rounded-xl border bg-card p-5">
					<div className="text-sm font-semibold mb-2">24h 安全事件趋势</div>
					<p className="text-[11px] text-muted-foreground mb-4">
						过去 24 小时的安全事件数量
					</p>
					<SparkAreaChart
						data={trendData}
						index="hour"
						categories={["events"]}
						colors={["blue"]}
						className="h-32 w-full"
					/>
				</div>
			</div>

			{snap.alerts.length > 0 && (
				<div className="rounded-xl border border-destructive/20 bg-destructive/[0.03] p-5">
					<div className="flex items-center gap-2 mb-3">
						<AlertTriangle className="size-4 text-destructive" />
						<span className="text-sm font-semibold text-destructive">
							活跃告警 ({snap.alerts.length})
						</span>
					</div>
					<div className="space-y-2">
						{snap.alerts.map((alert) => (
							<div
								key={alert.id}
								className="flex items-center gap-3 rounded-lg border bg-card px-4 py-3"
							>
								<span
									className={cn(
										"size-2 rounded-full shrink-0",
										SEVERITY_DOT[alert.severity],
									)}
								/>
								<div className="flex-1 min-w-0">
									<p className="text-sm">{alert.message}</p>
									<p className="text-[10px] text-muted-foreground mt-0.5">
										触发 {alert.count} 次 · 最近 {timeAgo(alert.lastSeen)}
									</p>
								</div>
								<span
									className={cn(
										"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium",
										SEVERITY_COLORS[alert.severity],
									)}
								>
									{SEVERITY_LABELS[alert.severity]}
								</span>
								<button
									type="button"
									className="text-muted-foreground hover:text-foreground p-1"
									onClick={() => {
										securityModel.dismissAlert(alert.id);
										toast.success("告警已忽略");
									}}
								>
									<X className="size-3.5" />
								</button>
							</div>
						))}
					</div>
				</div>
			)}
		</div>
	);
}
