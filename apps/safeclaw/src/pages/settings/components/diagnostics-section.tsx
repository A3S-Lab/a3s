import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import { Check, Server, ShieldCheck } from "lucide-react";
import { useState } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
import { fetchGatewayStatus } from "@/lib/security-api";
import { SectionHeader } from "./shared";

interface DiagResult {
	pass: boolean;
	detail: string;
}

export function DiagnosticsSection() {
	const snap = useSnapshot(settingsModel.state);
	const [running, setRunning] = useState(false);
	const [gatewayResult, setGatewayResult] = useState<DiagResult | null>(null);

	const gatewayUrl = snap.baseUrl || "http://127.0.0.1:18790";

	const handleRun = async () => {
		setRunning(true);
		const t0 = Date.now();
		try {
			await fetchGatewayStatus();
			setGatewayResult({
				pass: true,
				detail: `HTTP 200 · 延迟 ${Date.now() - t0}ms`,
			});
		} catch {
			setGatewayResult({
				pass: false,
				detail: "无法连接到网关，请检查地址和服务状态",
			});
		}
		setRunning(false);
		toast.success("诊断完成");
	};

	return (
		<div>
			<SectionHeader
				icon={ShieldCheck}
				title="系统诊断"
				description="检查 SafeClaw 网关连通性。其他组件状态请查看服务日志。"
			/>

			<div className="rounded-xl border bg-card p-5 mb-4">
				<div className="flex items-center justify-between mb-4">
					<div>
						<div className="text-sm font-semibold">网关连接检测</div>
						<div className="text-xs text-muted-foreground mt-0.5">
							{gatewayUrl}
						</div>
					</div>
					<Button
						size="sm"
						onClick={handleRun}
						disabled={running}
						className="h-8 text-xs gap-1.5"
					>
						{running ? (
							<>
								<span className="size-3 rounded-full border-2 border-primary border-t-transparent animate-spin" />
								检查中...
							</>
						) : (
							<>
								<ShieldCheck className="size-3.5" />
								{gatewayResult ? "重新检查" : "开始检查"}
							</>
						)}
					</Button>
				</div>

				{gatewayResult ? (
					<div
						className={cn(
							"flex items-start gap-3 rounded-lg border px-4 py-3",
							gatewayResult.pass
								? "border-green-500/20 bg-green-500/[0.03]"
								: "border-destructive/20 bg-destructive/[0.03]",
						)}
					>
						<div
							className={cn(
								"size-4 rounded-full flex items-center justify-center shrink-0 mt-0.5",
								gatewayResult.pass ? "bg-green-500/20" : "bg-destructive/20",
							)}
						>
							{gatewayResult.pass ? (
								<Check className="size-2.5 text-green-600 dark:text-green-400" />
							) : (
								<span className="size-1.5 rounded-full bg-destructive" />
							)}
						</div>
						<div className="flex-1 min-w-0">
							<span className="text-xs font-medium">
								{gatewayResult.pass ? "网关可达" : "网关不可达"}
							</span>
							<p
								className={cn(
									"text-[11px] font-mono mt-0.5",
									gatewayResult.pass
										? "text-muted-foreground"
										: "text-destructive",
								)}
							>
								{gatewayResult.detail}
							</p>
						</div>
					</div>
				) : (
					<div className="flex flex-col items-center justify-center py-8 text-muted-foreground">
						<ShieldCheck className="size-8 mb-2 opacity-30" />
						<p className="text-xs">点击"开始检查"运行诊断</p>
					</div>
				)}
			</div>

			<p className="text-xs text-muted-foreground px-1">
				TEE 状态、隐私分类器、审计日志等组件状态请查看{" "}
				<code className="font-mono">~/.local/share/safeclaw/logs/</code> 或运行{" "}
				<code className="font-mono">safeclaw status</code>。
			</p>

			<div className="rounded-xl border bg-card p-5 mt-4">
				<div className="flex items-center gap-2 mb-3">
					<Server className="size-4 text-primary" />
					<span className="text-sm font-semibold">运行时信息</span>
				</div>
				<div className="rounded-lg bg-muted/30 divide-y divide-border/50">
					{[
						{ label: "网关地址", value: gatewayUrl },
						{ label: "a3s-code", value: "v0.9.0" },
						{ label: "配置文件", value: "~/.config/safeclaw/safeclaw.hcl" },
						{ label: "日志目录", value: "~/.local/share/safeclaw/logs/" },
						{ label: "审计存储", value: "~/.local/share/safeclaw/audit.db" },
					].map((item) => (
						<div
							key={item.label}
							className="flex justify-between items-center px-4 py-2.5"
						>
							<span className="text-xs text-muted-foreground">
								{item.label}
							</span>
							<span className="text-xs font-medium font-mono truncate max-w-[240px]">
								{item.value}
							</span>
						</div>
					))}
				</div>
			</div>
		</div>
	);
}
