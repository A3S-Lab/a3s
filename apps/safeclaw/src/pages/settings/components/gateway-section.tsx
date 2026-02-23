import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import { Check, Server } from "lucide-react";
import { useState, useEffect } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
import { fetchGatewayStatus } from "@/lib/security-api";
import { SectionHeader, SettingRow } from "./shared";

export function GatewaySection() {
	const snap = useSnapshot(settingsModel.state);
	const [baseUrl, setBaseUrl] = useState(snap.baseUrl);
	const [dirty, setDirty] = useState(false);
	const [saved, setSaved] = useState(false);
	const [connStatus, setConnStatus] = useState<
		"checking" | "connected" | "error"
	>("checking");
	const [latencyMs, setLatencyMs] = useState<number | null>(null);

	const checkConnection = async (_url?: string) => {
		setConnStatus("checking");
		const t0 = Date.now();
		try {
			await fetchGatewayStatus();
			setLatencyMs(Date.now() - t0);
			setConnStatus("connected");
		} catch (e) {
			setConnStatus("error");
			setLatencyMs(null);
			console.warn("Gateway connection failed:", e);
			toast.error(
				`网关连接失败：${e instanceof Error ? e.message : "请检查 SafeClaw 是否已启动"}`,
			);
		}
	};

	useEffect(() => {
		checkConnection();
	}, []);

	return (
		<div>
			<SectionHeader
				icon={Server}
				title="网关连接"
				description="配置 SafeClaw 网关的连接地址。"
			/>
			<div className="rounded-xl border bg-card p-5">
				<SettingRow
					label="网关地址"
					hint="API 和 WebSocket 连接的服务端地址，留空使用默认值。"
				>
					<div className="relative">
						<Server className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
						<Input
							className="h-9 text-sm font-mono pl-8"
							placeholder="http://127.0.0.1:18790"
							value={baseUrl}
							onChange={(e) => {
								setBaseUrl(e.target.value);
								setDirty(true);
								setSaved(false);
							}}
						/>
					</div>
				</SettingRow>
				<div className="mt-4 flex items-center gap-2 text-xs">
					{connStatus === "checking" && (
						<>
							<span className="relative flex size-2">
								<span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-75" />
								<span className="relative inline-flex rounded-full size-2 bg-amber-400" />
							</span>
							<span className="text-muted-foreground">检测中...</span>
						</>
					)}
					{connStatus === "connected" && (
						<>
							<span className="relative flex size-2">
								<span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75" />
								<span className="relative inline-flex rounded-full size-2 bg-primary" />
							</span>
							<span className="text-muted-foreground">已连接</span>
							<span className="text-muted-foreground/50">·</span>
							<span className="font-mono text-muted-foreground">
								{latencyMs}ms
							</span>
						</>
					)}
					{connStatus === "error" && (
						<>
							<span className="relative flex size-2">
								<span className="relative inline-flex rounded-full size-2 bg-destructive" />
							</span>
							<span className="text-destructive">无法连接</span>
						</>
					)}
					<span className="text-muted-foreground/50 ml-1">·</span>
					<span className="font-mono text-muted-foreground">
						{baseUrl || "http://127.0.0.1:18790"}
					</span>
					<button
						type="button"
						className="ml-auto text-muted-foreground hover:text-foreground transition-colors"
						onClick={() => checkConnection(baseUrl)}
					>
						重新检测
					</button>
				</div>
			</div>
			{(dirty || saved) && (
				<div
					className={cn(
						"flex items-center gap-3 rounded-lg px-4 py-2.5 mt-6",
						dirty
							? "bg-primary/5 border border-primary/20"
							: "bg-muted/50 border border-border",
					)}
				>
					{dirty ? (
						<>
							<div className="flex-1 text-xs text-muted-foreground">
								有未保存的更改
							</div>
							<Button
								size="sm"
								className="h-7 text-xs"
								onClick={() => {
									settingsModel.setBaseUrl(baseUrl);
									setDirty(false);
									setSaved(true);
									checkConnection(baseUrl);
									toast.success("网关设置已保存");
									setTimeout(() => setSaved(false), 2000);
								}}
							>
								<Check className="size-3 mr-1" />
								保存
							</Button>
						</>
					) : (
						<>
							<Check className="size-3.5 text-primary" />
							<span className="text-xs text-primary font-medium">已保存</span>
						</>
					)}
				</div>
			)}
		</div>
	);
}
