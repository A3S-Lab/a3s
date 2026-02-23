import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { timeAgo } from "@/lib/time";
import securityModel from "@/models/security.model";
import type { AuditEvent, Severity } from "@/models/security.model";
import { ChevronDown, ChevronRight, FileWarning, Search } from "lucide-react";
import { useMemo, useState, useEffect } from "react";
import { useSnapshot } from "valtio";
import {
	fetchAuditEvents,
	fetchAuditStats,
	fetchAlerts,
} from "@/lib/security-api";
import {
	SEVERITY_COLORS,
	SEVERITY_DOT,
	SEVERITY_LABELS,
	VECTOR_LABELS,
} from "../constants";
import { SectionHeader } from "./shared";

function AuditEventCard({ event }: { event: AuditEvent }) {
	const [expanded, setExpanded] = useState(false);
	return (
		<div className="rounded-lg border bg-card p-4 hover:shadow-sm transition-shadow">
			<div className="flex items-center gap-2 mb-2">
				<span
					className={cn(
						"size-2 rounded-full shrink-0",
						SEVERITY_DOT[event.severity],
					)}
				/>
				<span
					className={cn(
						"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium",
						SEVERITY_COLORS[event.severity],
					)}
				>
					{SEVERITY_LABELS[event.severity]}
				</span>
				<span className="text-[10px] font-mono text-muted-foreground">
					{VECTOR_LABELS[event.vector] || event.vector}
				</span>
				{event.sessionId && (
					<span className="text-[10px] font-mono text-muted-foreground/60">
						{event.sessionId}
					</span>
				)}
				<time className="text-[10px] text-muted-foreground ml-auto">
					{timeAgo(event.timestamp)}
				</time>
			</div>
			<p className="text-sm leading-relaxed mb-2">{event.summary}</p>
			{event.detail && (
				<>
					<button
						type="button"
						className="flex items-center gap-1 text-[11px] text-muted-foreground hover:text-foreground transition-colors mb-2"
						onClick={() => setExpanded(!expanded)}
					>
						{expanded ? (
							<ChevronDown className="size-3" />
						) : (
							<ChevronRight className="size-3" />
						)}
						<span>详情</span>
					</button>
					{expanded && (
						<pre className="rounded bg-muted/50 p-2 text-[11px] font-mono overflow-x-auto max-h-32 whitespace-pre-wrap text-muted-foreground mb-2">
							{event.detail}
						</pre>
					)}
				</>
			)}
		</div>
	);
}

export function AuditSection() {
	const snap = useSnapshot(securityModel.state);
	const [severityFilter, setSeverityFilter] = useState<Severity | "all">("all");
	const [vectorFilter, setVectorFilter] = useState<string>("all");
	const [search, setSearch] = useState("");

	useEffect(() => {
		Promise.all([
			fetchAuditEvents({ limit: 100 }).catch((e) => {
				console.warn("Audit events unavailable:", e);
				return null;
			}),
			fetchAuditStats().catch((e) => {
				console.warn("Audit stats unavailable:", e);
				return null;
			}),
			fetchAlerts().catch((e) => {
				console.warn("Alerts unavailable:", e);
				return null;
			}),
		]).then(([events, stats, alerts]) => {
			if (events) securityModel.setAuditEvents(events);
			if (stats) securityModel.setAuditStats(stats);
			if (alerts) securityModel.setAlerts(alerts);
		});
	}, []);

	const q = search.trim().toLowerCase();

	const filtered = useMemo(() => {
		let events = [...snap.auditEvents] as AuditEvent[];
		if (severityFilter !== "all")
			events = events.filter((e) => e.severity === severityFilter);
		if (vectorFilter !== "all")
			events = events.filter((e) => e.vector === vectorFilter);
		if (q)
			events = events.filter(
				(e) =>
					e.summary.toLowerCase().includes(q) ||
					e.detail?.toLowerCase().includes(q),
			);
		return events.sort((a, b) => b.timestamp - a.timestamp);
	}, [snap.auditEvents, severityFilter, vectorFilter, q]);

	return (
		<div>
			<SectionHeader
				icon={FileWarning}
				title="审计日志"
				description="所有安全事件的详细记录与查询。"
			/>

			<div className="rounded-xl border bg-card p-4 mb-4">
				<div className="flex items-center gap-6">
					<div className="text-sm">
						<span className="font-bold text-lg tabular-nums">
							{snap.auditStats.total}
						</span>{" "}
						<span className="text-muted-foreground">总事件</span>
					</div>
					<div className="h-8 w-px bg-border" />
					{(["critical", "high", "warning", "info"] as Severity[]).map((s) => (
						<button
							key={s}
							type="button"
							onClick={() =>
								setSeverityFilter(severityFilter === s ? "all" : s)
							}
							className={cn(
								"flex items-center gap-1.5 text-xs transition-colors rounded-md px-2 py-1",
								severityFilter === s
									? "bg-primary/10 text-primary font-medium"
									: "text-muted-foreground hover:text-foreground",
							)}
						>
							<span className={cn("size-2 rounded-full", SEVERITY_DOT[s])} />
							{SEVERITY_LABELS[s]} ({snap.auditStats.bySeverity[s]})
						</button>
					))}
				</div>
			</div>

			<div className="flex items-center gap-3 mb-4">
				<div className="relative flex-1 max-w-md">
					<Search className="absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
					<Input
						placeholder="搜索审计事件..."
						value={search}
						onChange={(e) => setSearch(e.target.value)}
						className="pl-8 h-9"
					/>
				</div>
				<div className="flex items-center gap-1.5">
					{["all", ...Object.keys(VECTOR_LABELS)].map((v) => (
						<button
							key={v}
							type="button"
							onClick={() => setVectorFilter(v)}
							className={cn(
								"text-[11px] rounded-md border px-2 py-1 transition-colors",
								vectorFilter === v
									? "border-primary bg-primary/10 text-primary font-medium"
									: "text-muted-foreground hover:border-primary/30 hover:text-foreground",
							)}
						>
							{v === "all" ? "全部" : VECTOR_LABELS[v]}
						</button>
					))}
				</div>
			</div>

			<div className="space-y-3">
				{filtered.map((e) => (
					<AuditEventCard key={e.id} event={e} />
				))}
				{filtered.length === 0 && (
					<div className="flex flex-col items-center justify-center py-16 text-muted-foreground">
						<FileWarning className="size-10 mb-3 opacity-30" />
						<p className="text-sm">{q ? "未找到匹配的事件" : "暂无审计事件"}</p>
					</div>
				)}
			</div>
		</div>
	);
}
