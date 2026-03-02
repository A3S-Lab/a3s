import { Button } from "@/components/ui/button";
import { type PowerHealthInfo, powerApi } from "@/lib/power-api";
import { fetchGatewayStatus } from "@/lib/security-api";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import { invoke } from "@tauri-apps/api/core";
import {
	Check,
	Cpu,
	Download,
	Loader2,
	Server,
	ShieldCheck,
	Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { useSnapshot } from "valtio";
import { SectionHeader } from "./shared";

interface DiagResult {
	pass: boolean;
	detail: string;
}

interface PowerRuntimeStatus {
	url: string;
	inferenceBackend: string;
	profile: string;
	totalMemoryGib?: number;
	teeType: string;
	hardwareTee: boolean;
}

type LocalFlowStage =
	| "idle"
	| "checking"
	| "downloading"
	| "starting"
	| "testing"
	| "success"
	| "error";

function recommendedLocalModel(profile?: string): string {
	return profile === "high-memory"
		? "Qwen/Qwen2.5-7B-Instruct-GGUF:Q4_K_M"
		: "Qwen/Qwen2.5-7B-Instruct-GGUF:Q3_K_M";
}

export function DiagnosticsSection() {
	const snap = useSnapshot(settingsModel.state);
	const gatewayUrl = snap.baseUrl || "http://127.0.0.1:18790";

	const [gatewayRunning, setGatewayRunning] = useState(false);
	const [gatewayResult, setGatewayResult] = useState<DiagResult | null>(null);

	const [runtimeStatus, setRuntimeStatus] = useState<PowerRuntimeStatus | null>(
		null,
	);
	const [healthInfo, setHealthInfo] = useState<PowerHealthInfo | null>(null);
	const [installedModels, setInstalledModels] = useState<string[]>([]);
	const [removingModel, setRemovingModel] = useState<string | null>(null);

	const [stage, setStage] = useState<LocalFlowStage>("idle");
	const [flowMessage, setFlowMessage] = useState("等待开始");
	const [downloadProgress, setDownloadProgress] = useState("");
	const [downloadEta, setDownloadEta] = useState("");
	const [localResult, setLocalResult] = useState<DiagResult | null>(null);

	const recommendedModel = recommendedLocalModel(runtimeStatus?.profile);
	const powerUrl = runtimeStatus?.url || "http://127.0.0.1:11435/v1";
	const activeModel = installedModels[0] || null;
	const hasInstalledModel = installedModels.length > 0;
	const isBusy =
		stage === "checking" ||
		stage === "downloading" ||
		stage === "starting" ||
		stage === "testing";
	const isServiceHealthy = healthInfo?.status === "ok";
	const isLocalReady = isServiceHealthy && !!activeModel;
	const showRuntimeParams = hasInstalledModel;

	const flowSteps: Array<{ key: string; label: string }> = [
		{ key: "checking", label: "检查环境" },
		{ key: "downloading", label: "下载模型" },
		{ key: "starting", label: "等待启动" },
		{ key: "testing", label: "诊断测速" },
		{ key: "success", label: "完成" },
	];

	const stageIndex = {
		idle: -1,
		checking: 0,
		downloading: 1,
		starting: 2,
		testing: 3,
		success: 4,
		error: 4,
	}[stage];

	const flowPercent = (() => {
		if (stage === "idle") return 0;
		if (stage === "success") return 100;
		if (stage === "error") return Math.max(10, stageIndex * 25);
		if (stage === "downloading") {
			const match = downloadProgress.match(/(\d{1,3})%/);
			const pct = Number(match?.[1] || 0);
			return 25 + Math.min(25, Math.max(0, pct / 4));
		}
		return Math.min(95, (stageIndex + 1) * 25);
	})();

	const localProvider = useMemo(() => {
		const local = snap.providers.find((p) => p.name === "local-power");
		if (!local) return undefined;
		return {
			...local,
			baseUrl: powerUrl,
		};
	}, [powerUrl, snap.providers]);

	const refreshLocalState = useCallback(async () => {
		if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) {
			setRuntimeStatus(null);
			setHealthInfo(null);
			setInstalledModels([]);
			return { runtime: null, health: null, models: [] as string[] };
		}

		const [runtime, health, models] = await Promise.all([
			invoke<PowerRuntimeStatus>("get_power_runtime_status"),
			powerApi.healthInfo(localProvider),
			powerApi.listModels(localProvider),
		]);

		setRuntimeStatus(runtime);
		setHealthInfo(health);
		setInstalledModels(models.map((m) => m.id));
		return {
			runtime,
			health,
			models: models.map((m) => m.id),
		};
	}, [localProvider]);

	useEffect(() => {
		refreshLocalState().catch(() => undefined);
	}, [refreshLocalState]);

	const runGatewayCheck = async () => {
		setGatewayRunning(true);
		const started = Date.now();
		try {
			await fetchGatewayStatus();
			setGatewayResult({
				pass: true,
				detail: `HTTP 200 · 延迟 ${Date.now() - started}ms`,
			});
		} catch {
			setGatewayResult({
				pass: false,
				detail: "无法连接到网关，请检查地址和服务状态",
			});
		}
		setGatewayRunning(false);
	};

	const runLocalFlow = async () => {
		if (isBusy) return;
		setLocalResult(null);
		setDownloadProgress("");
		setDownloadEta("");
		setStage("checking");
		setFlowMessage("读取本地推理服务状态...");

		try {
			const initial = await refreshLocalState();
			let modelId = initial.models[0] || null;

			if (!modelId) {
				setStage("downloading");
				setFlowMessage(`下载推荐模型 ${recommendedModel.split(":").pop()} ...`);
				const downloadStartedAt = Date.now();
				await powerApi.pullModel(
					recommendedModel,
					(event) => {
						if (
							event.status === "downloading" &&
							event.completed &&
							event.total
						) {
							const pct = Math.floor((event.completed / event.total) * 100);
							setDownloadProgress(`下载中 ${pct}%`);
							const elapsedSec = Math.max(
								1,
								(Date.now() - downloadStartedAt) / 1000,
							);
							const bytesPerSec = event.completed / elapsedSec;
							const remain = Math.max(0, event.total - event.completed);
							const etaSec = Math.round(remain / Math.max(1, bytesPerSec));
							const speedMBps = bytesPerSec / 1024 / 1024;
							setDownloadEta(`约 ${etaSec}s · ${speedMBps.toFixed(1)} MB/s`);
							return;
						}
						if (event.status === "verifying") {
							setDownloadProgress("校验模型...");
							setDownloadEta("");
							return;
						}
						if (event.status === "success") {
							setDownloadProgress("下载完成");
							setDownloadEta("");
							return;
						}
						setDownloadProgress(`状态: ${event.status}`);
						setDownloadEta("");
					},
					localProvider,
				);
				const afterPull = await refreshLocalState();
				modelId = afterPull.models[0] || null;
				if (!modelId) {
					const deadline = Date.now() + 10 * 60 * 1000;
					while (Date.now() < deadline) {
						const [latestModels, pullState] = await Promise.all([
							powerApi.listModels(localProvider),
							powerApi.pullStatus(recommendedModel, localProvider),
						]);
						setInstalledModels(latestModels.map((m) => m.id));
						modelId = latestModels[0]?.id || null;
						if (modelId) break;

						if (pullState?.status === "failed") {
							throw new Error(pullState.error || "模型下载失败");
						}
						if (pullState?.status === "pulling") {
							if (pullState.total > 0) {
								const pct = Math.floor(
									(pullState.completed / pullState.total) * 100,
								);
								setDownloadProgress(`下载中 ${pct}%`);
							} else {
								const mb = (pullState.completed / 1024 / 1024).toFixed(1);
								setDownloadProgress(`下载中... 已下载 ${mb} MB`);
							}
							setFlowMessage("检测到后台下载任务，等待完成...");
						}

						await new Promise((resolve) => setTimeout(resolve, 1000));
					}
					if (!modelId) {
						setStage("checking");
						setFlowMessage(
							"后台仍在下载模型，稍后再次点击即可继续后续启动与测速",
						);
						toast.message("模型仍在后台下载中，请稍后重试");
						return;
					}
				}
			}

			if (!modelId) {
				throw new Error("未检测到可用本地模型");
			}

			setStage("starting");
			setFlowMessage("等待本地推理服务就绪...");
			const deadline = Date.now() + 30000;
			let ready = false;
			while (Date.now() < deadline) {
				const health = await powerApi.healthInfo(localProvider);
				setHealthInfo(health);
				if (health.status === "ok") {
					ready = true;
					break;
				}
				await new Promise((resolve) => setTimeout(resolve, 1000));
			}
			if (!ready) {
				throw new Error("本地推理服务启动超时");
			}

			setStage("testing");
			setFlowMessage("执行诊断与速度测试（3 轮）...");
			const rounds = 3;
			let latencySum = 0;
			let tpsSum = 0;
			for (let i = 0; i < rounds; i++) {
				const result = await powerApi.diagnoseModel(modelId, localProvider);
				latencySum += result.latencyMs;
				tpsSum += result.tokensPerSecond;
			}

			const avgLatency = latencySum / rounds;
			const avgTps = tpsSum / rounds;
			setLocalResult({
				pass: true,
				detail: `${modelId} · 平均延迟 ${avgLatency.toFixed(0)}ms · 平均速度 ${avgTps.toFixed(1)} tok/s（${rounds}轮）`,
			});
			setStage("success");
			setFlowMessage("本地模型诊断与测速完成");
			toast.success("本地模型诊断与测速完成");
		} catch (e) {
			const msg = e instanceof Error ? e.message : "本地模型流程失败";
			setLocalResult({ pass: false, detail: msg });
			setStage("error");
			setFlowMessage(msg);
			toast.error("本地模型流程失败");
		} finally {
			setDownloadEta("");
			await refreshLocalState().catch(() => undefined);
		}
	};

	const deleteModel = async (modelId: string) => {
		if (isBusy) return;
		setRemovingModel(modelId);
		try {
			await powerApi.deleteModel(modelId, localProvider);
			await refreshLocalState();
			setLocalResult({ pass: false, detail: "本地模型已删除，当前未就绪" });
			setStage("idle");
			setFlowMessage("等待开始");
			toast.success(`已删除模型: ${modelId}`);
		} catch (e) {
			toast.error(e instanceof Error ? e.message : "删除模型失败");
		} finally {
			setRemovingModel(null);
		}
	};

	return (
		<div>
			<SectionHeader
				icon={ShieldCheck}
				title="系统诊断"
				description="检查网关与本地推理状态，并执行本地模型完整流程。"
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
						onClick={runGatewayCheck}
						disabled={gatewayRunning}
						className="h-8 text-xs gap-1.5"
					>
						{gatewayRunning ? (
							<>
								<Loader2 className="size-3.5 animate-spin" />
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

				{gatewayResult && (
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
				)}
			</div>

			<div className="rounded-xl border bg-card p-5 mb-4">
				<div className="flex items-center justify-between mb-4">
					<div>
						<div className="text-sm font-semibold">默认本地模型完整流程</div>
						<div className="text-xs text-muted-foreground mt-0.5">
							下载默认模型 / 等待服务就绪 / 诊断并测试速度
						</div>
						<div className="text-xs text-muted-foreground mt-1">{powerUrl}</div>
					</div>
					<Button
						size="sm"
						onClick={runLocalFlow}
						disabled={isBusy}
						className="h-8 text-xs gap-1.5"
					>
						{isBusy ? (
							<>
								<Loader2 className="size-3.5 animate-spin" />
								执行中...
							</>
						) : (
							<>
								<Cpu className="size-3.5" />
								{isLocalReady
									? "重新诊断并测速"
									: `启动默认流程（${recommendedModel.split(":").pop()}）`}
							</>
						)}
					</Button>
				</div>

				<div className="rounded-lg border bg-muted/20 px-3 py-3 mb-4">
					<div className="mb-2">
						<div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
							<div
								className={cn(
									"h-full rounded-full transition-all duration-300",
									stage === "error" ? "bg-destructive" : "bg-primary",
								)}
								style={{ width: `${flowPercent}%` }}
							/>
						</div>
						<div className="mt-1 text-[10px] text-muted-foreground text-right">
							进度 {Math.round(flowPercent)}%
						</div>
					</div>

					<div className="flex items-center gap-2 overflow-x-auto pb-1">
						{flowSteps.map((item, idx) => {
							const done = stage === "success" || idx < stageIndex;
							const active =
								stage !== "idle" && stage !== "error" && idx === stageIndex;
							const failed = stage === "error" && idx === stageIndex;
							return (
								<div
									key={item.key}
									className="flex items-center gap-2 shrink-0"
								>
									<div
										className={cn(
											"size-5 rounded-full border flex items-center justify-center text-[10px]",
											done &&
												"border-green-500/40 bg-green-500/15 text-green-700",
											active && "border-primary/40 bg-primary/15 text-primary",
											failed &&
												"border-destructive/40 bg-destructive/15 text-destructive",
											!done &&
												!active &&
												!failed &&
												"border-border text-muted-foreground",
										)}
									>
										{done ? (
											<Check className="size-3" />
										) : active ? (
											<Loader2 className="size-3 animate-spin" />
										) : (
											idx + 1
										)}
									</div>
									<span className="text-[11px] text-muted-foreground">
										{item.label}
									</span>
									{idx < flowSteps.length - 1 && (
										<div className="w-4 h-px bg-border" />
									)}
								</div>
							);
						})}
					</div>
					<div className="mt-2 text-[11px] text-muted-foreground">
						{flowMessage}
					</div>
				</div>

				{showRuntimeParams ? (
					<div className="rounded-lg bg-muted/30 divide-y divide-border/50 mb-4">
						{[
							{
								label: "服务状态",
								value: !isServiceHealthy
									? healthInfo?.status || "未运行"
									: activeModel
										? "运行中"
										: "未就绪（无模型）",
							},
							{ label: "当前阶段", value: flowMessage },
							{
								label: "推荐模型",
								value: recommendedModel.split(":").pop() || "未知",
							},
							{ label: "已安装模型", value: activeModel || "无" },
							{
								label: "推理后端",
								value: runtimeStatus?.inferenceBackend || "未知",
							},
							{ label: "运行档位", value: runtimeStatus?.profile || "未知" },
						].map((item) => (
							<div
								key={item.label}
								className="flex justify-between items-center px-4 py-2.5 gap-3"
							>
								<span className="text-xs text-muted-foreground">
									{item.label}
								</span>
								<span className="text-xs font-medium font-mono text-right truncate">
									{item.value}
								</span>
							</div>
						))}
					</div>
				) : (
					<div className="rounded-lg border bg-muted/30 px-3 py-2 text-xs text-muted-foreground mb-4">
						模型下载成功后将显示运行状态与参数信息。
					</div>
				)}

				{downloadProgress && (
					<div className="rounded-lg border bg-muted/30 px-3 py-2 text-xs text-muted-foreground mb-3">
						<div>{downloadProgress}</div>
						{downloadEta && <div className="mt-1">{downloadEta}</div>}
					</div>
				)}

				<div className="mb-3 rounded-lg border divide-y">
					{installedModels.length === 0 ? (
						<div className="px-3 py-2 text-xs text-muted-foreground">
							暂无已安装模型
						</div>
					) : (
						installedModels.map((modelId) => (
							<div
								key={modelId}
								className="px-3 py-2 flex items-center justify-between gap-2"
							>
								<span className="text-xs font-mono truncate">{modelId}</span>
								<Button
									size="sm"
									variant="ghost"
									onClick={() => deleteModel(modelId)}
									disabled={isBusy || removingModel === modelId}
									className="h-7 px-2 text-xs text-destructive"
								>
									{removingModel === modelId ? (
										<Loader2 className="size-3.5 animate-spin" />
									) : (
										<Trash2 className="size-3.5" />
									)}
									删除
								</Button>
							</div>
						))
					)}
				</div>

				{localResult && (
					<div
						className={cn(
							"flex items-start gap-3 rounded-lg border px-4 py-3",
							localResult.pass
								? "border-green-500/20 bg-green-500/[0.03]"
								: "border-destructive/20 bg-destructive/[0.03]",
						)}
					>
						<div
							className={cn(
								"size-4 rounded-full flex items-center justify-center shrink-0 mt-0.5",
								localResult.pass ? "bg-green-500/20" : "bg-destructive/20",
							)}
						>
							{localResult.pass ? (
								<Check className="size-2.5 text-green-600 dark:text-green-400" />
							) : (
								<span className="size-1.5 rounded-full bg-destructive" />
							)}
						</div>
						<div className="flex-1 min-w-0">
							<span className="text-xs font-medium">
								{localResult.pass
									? "本地模型就绪并完成测速"
									: "本地模型流程失败"}
							</span>
							<p
								className={cn(
									"text-[11px] font-mono mt-0.5",
									localResult.pass
										? "text-muted-foreground"
										: "text-destructive",
								)}
							>
								{localResult.detail}
							</p>
						</div>
					</div>
				)}
			</div>

			<div className="rounded-xl border bg-card p-5 mt-4">
				<div className="flex items-center gap-2 mb-3">
					<Server className="size-4 text-primary" />
					<span className="text-sm font-semibold">运行时信息</span>
				</div>
				<div className="rounded-lg bg-muted/30 divide-y divide-border/50">
					{[
						{ label: "网关地址", value: gatewayUrl },
						{ label: "本地推理地址", value: powerUrl },
						{ label: "a3s-code", value: "v0.9.0" },
						{ label: "配置文件", value: "~/.config/safeclaw/safeclaw.hcl" },
						{ label: "日志目录", value: "~/.local/share/safeclaw/logs/" },
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
