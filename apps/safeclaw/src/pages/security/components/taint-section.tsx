import { cn } from "@/lib/utils";
import { timeAgo } from "@/lib/time";
import securityModel from "@/models/security.model";
import type { TaintEntry } from "@/models/security.model";
import { Activity, Bug, Lock, ShieldAlert } from "lucide-react";
import { useEffect } from "react";
import { useSnapshot } from "valtio";
import { fetchTaintEntries } from "@/lib/security-api";
import { SectionHeader, StatCard } from "./shared";

function nodeLabel(node: string): string {
	if (node === "user_input") return "用户输入";
	if (node === "output_channel") return "输出渠道";
	if (node.startsWith("tool:")) return node.slice(5);
	if (node.startsWith("tool_result:")) return node.slice(12);
	return node;
}

function nodeColor(node: string): string {
	if (node === "user_input")
		return "bg-primary/10 text-primary border-primary/30";
	if (node === "output_channel")
		return "bg-destructive/10 text-destructive border-destructive/30";
	if (node.startsWith("tool:"))
		return "bg-violet-500/10 text-violet-600 dark:text-violet-400 border-violet-500/20";
	if (node.startsWith("tool_result:"))
		return "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20";
	return "bg-muted text-muted-foreground border-border";
}

function TaintFlowLane({ entry }: { entry: TaintEntry }) {
	const isHigh = entry.propagations >= 3;
	return (
		<div
			className={cn(
				"rounded-xl border bg-card p-4 transition-all",
				isHigh && "border-destructive/30",
			)}
		>
			<div className="flex items-center gap-2 mb-3">
				<div
					className={cn(
						"size-2.5 rounded-full shrink-0",
						isHigh ? "bg-destructive" : "bg-violet-500",
					)}
				/>
				<span className="text-sm font-medium font-mono">{entry.label}</span>
				<span
					className={cn(
						"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium",
						isHigh
							? "bg-destructive/10 text-destructive border-destructive/20"
							: "bg-violet-500/10 text-violet-600 dark:text-violet-400 border-violet-500/20",
					)}
				>
					传播 {entry.propagations} 次
				</span>
				<span className="text-[10px] text-muted-foreground ml-auto">
					{timeAgo(entry.createdAt)}
				</span>
			</div>

			<div className="flex items-center gap-1 flex-wrap">
				{entry.path.map((node, i) => (
					<div key={i} className="flex items-center gap-1">
						<span
							className={cn(
								"inline-flex items-center rounded-md border px-2 py-1 text-[11px] font-medium font-mono whitespace-nowrap",
								nodeColor(node),
							)}
						>
							{nodeLabel(node)}
						</span>
						{i < entry.path.length - 1 && (
							<svg
								className="size-3 text-muted-foreground/50 shrink-0"
								viewBox="0 0 12 12"
								fill="none"
							>
								<path
									d="M2 6h8M7 3l3 3-3 3"
									stroke="currentColor"
									strokeWidth="1.5"
									strokeLinecap="round"
									strokeLinejoin="round"
								/>
							</svg>
						)}
					</div>
				))}
			</div>
		</div>
	);
}

export function TaintSection() {
	const snap = useSnapshot(securityModel.state);

	useEffect(() => {
		fetchTaintEntries()
			.then((entries) => {
				if (Array.isArray(entries)) securityModel.state.taintEntries = entries;
			})
			.catch((e) => console.warn("Taint entries unavailable:", e));
	}, []);

	return (
		<div>
			<SectionHeader
				icon={Bug}
				title="污点追踪"
				description="追踪敏感数据在工具调用间的传播路径。"
			/>

			<div className="flex items-center gap-2 rounded-lg bg-muted/40 border px-3 py-2 mb-4 text-[11px] text-muted-foreground">
				<Lock className="size-3 shrink-0" />
				<span>
					污点标记由运行时自动生成，只读。污点策略配置请编辑{" "}
					<code className="font-mono">safeclaw.hcl</code>。
				</span>
			</div>

			<div className="grid grid-cols-3 gap-4 mb-6">
				<StatCard
					icon={Bug}
					label="活跃污点"
					value={snap.taintEntries.length}
					sub="个数据标记"
					color="text-violet-500"
				/>
				<StatCard
					icon={Activity}
					label="总传播次数"
					value={snap.taintEntries.reduce((s, t) => s + t.propagations, 0)}
					sub="次跨工具传播"
					color="text-amber-500"
				/>
				<StatCard
					icon={ShieldAlert}
					label="高传播标记"
					value={snap.taintEntries.filter((t) => t.propagations >= 3).length}
					sub="个 (≥3 次传播)"
					color="text-destructive"
				/>
			</div>

			<div className="flex items-center gap-4 mb-4 px-1">
				<span className="text-[11px] text-muted-foreground">图例:</span>
				{[
					{
						label: "用户输入",
						cls: "bg-primary/10 text-primary border-primary/30",
					},
					{
						label: "工具调用",
						cls: "bg-violet-500/10 text-violet-600 dark:text-violet-400 border-violet-500/20",
					},
					{
						label: "工具结果",
						cls: "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
					},
					{
						label: "输出渠道",
						cls: "bg-destructive/10 text-destructive border-destructive/30",
					},
				].map((item) => (
					<span
						key={item.label}
						className={cn(
							"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium",
							item.cls,
						)}
					>
						{item.label}
					</span>
				))}
			</div>

			<div className="space-y-3">
				{(snap.taintEntries as TaintEntry[]).map((t) => (
					<TaintFlowLane key={t.id} entry={t} />
				))}
				{snap.taintEntries.length === 0 && (
					<div className="flex flex-col items-center justify-center py-16 text-muted-foreground">
						<Bug className="size-10 mb-3 opacity-30" />
						<p className="text-sm">暂无活跃污点标记</p>
					</div>
				)}
			</div>
		</div>
	);
}
