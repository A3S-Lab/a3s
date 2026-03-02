import { useModal } from "@/components/custom/modal-provider";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { powerApi, type PowerLogEntry } from "@/lib/power-api";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import type { ModelConfig, ProviderConfig } from "@/models/settings.model";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import {
	Bot,
	Check,
	Download,
	Eye,
	EyeOff,
	FolderUp,
	Gauge,
	HardDrive,
	KeyRound,
	Loader2,
	Plus,
	RefreshCw,
	Server,
	Shield,
	Star,
	Trash2,
	X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { useSnapshot } from "valtio";
import { SectionHeader, pColor } from "./shared";

function AddProviderForm({
	onAdd,
	onCancel,
}: { onAdd: (p: ProviderConfig) => void; onCancel: () => void }) {
	const [name, setName] = useState("");
	const [apiKey, setApiKey] = useState("");
	const [baseUrl, setBaseUrl] = useState("");
	return (
		<div className="rounded-xl border-2 border-dashed border-primary/30 bg-primary/[0.02] p-4 space-y-3">
			<div className="flex items-center justify-between">
				<span className="text-sm font-semibold">添加 Provider</span>
				<button
					type="button"
					onClick={onCancel}
					className="text-muted-foreground hover:text-foreground"
				>
					<X className="size-4" />
				</button>
			</div>
			<Input
				className="h-8 text-sm"
				placeholder="Provider 名称 (如 anthropic, openai)"
				value={name}
				onChange={(e) => setName(e.target.value)}
			/>
			<Input
				className="h-8 text-sm font-mono"
				placeholder="API Key (可选)"
				type="password"
				value={apiKey}
				onChange={(e) => setApiKey(e.target.value)}
			/>
			<Input
				className="h-8 text-sm font-mono"
				placeholder="Base URL (可选)"
				value={baseUrl}
				onChange={(e) => setBaseUrl(e.target.value)}
			/>
			<div className="flex justify-end gap-2">
				<Button
					variant="ghost"
					size="sm"
					className="h-7 text-xs"
					onClick={onCancel}
				>
					取消
				</Button>
				<Button
					size="sm"
					className="h-7 text-xs"
					disabled={!name.trim()}
					onClick={() =>
						onAdd({
							name: name.trim().toLowerCase(),
							apiKey,
							baseUrl,
							models: [],
						})
					}
				>
					<Plus className="size-3 mr-1" />
					添加
				</Button>
			</div>
		</div>
	);
}

function AddModelForm({
	onAdd,
	onCancel,
}: { onAdd: (m: ModelConfig) => void; onCancel: () => void }) {
	const [id, setId] = useState("");
	const [name, setName] = useState("");
	const [apiKey, setApiKey] = useState("");
	const [baseUrl, setBaseUrl] = useState("");
	const [context, setContext] = useState("128000");
	const [output, setOutput] = useState("4096");
	return (
		<div className="rounded-lg border-2 border-dashed border-primary/30 bg-primary/[0.02] p-3 space-y-2.5 mt-2">
			<div className="flex items-center justify-between">
				<span className="text-xs font-semibold">添加模型</span>
				<button
					type="button"
					onClick={onCancel}
					className="text-muted-foreground hover:text-foreground"
				>
					<X className="size-3.5" />
				</button>
			</div>
			<div className="grid grid-cols-2 gap-2">
				<Input
					className="h-7 text-xs font-mono"
					placeholder="模型 ID"
					value={id}
					onChange={(e) => setId(e.target.value)}
				/>
				<Input
					className="h-7 text-xs"
					placeholder="显示名称"
					value={name}
					onChange={(e) => setName(e.target.value)}
				/>
			</div>
			<Input
				className="h-7 text-xs font-mono"
				placeholder="API Key (可选，覆盖 Provider)"
				type="password"
				value={apiKey}
				onChange={(e) => setApiKey(e.target.value)}
			/>
			<Input
				className="h-7 text-xs font-mono"
				placeholder="Base URL (可选，覆盖 Provider)"
				value={baseUrl}
				onChange={(e) => setBaseUrl(e.target.value)}
			/>
			<div className="grid grid-cols-2 gap-2">
				<div>
					<label className="text-[10px] text-muted-foreground">
						上下文窗口
					</label>
					<Input
						className="h-7 text-xs font-mono"
						value={context}
						onChange={(e) => setContext(e.target.value)}
					/>
				</div>
				<div>
					<label className="text-[10px] text-muted-foreground">最大输出</label>
					<Input
						className="h-7 text-xs font-mono"
						value={output}
						onChange={(e) => setOutput(e.target.value)}
					/>
				</div>
			</div>
			<div className="flex justify-end gap-2">
				<Button
					variant="ghost"
					size="sm"
					className="h-6 text-[11px]"
					onClick={onCancel}
				>
					取消
				</Button>
				<Button
					size="sm"
					className="h-6 text-[11px]"
					disabled={!id.trim()}
					onClick={() =>
						onAdd({
							id: id.trim(),
							name: name.trim() || id.trim(),
							apiKey: apiKey || undefined,
							baseUrl: baseUrl || undefined,
							toolCall: true,
							temperature: true,
							modalities: { input: ["text"], output: ["text"] },
							limit: {
								context: Number(context) || 128000,
								output: Number(output) || 4096,
							},
						})
					}
				>
					<Plus className="size-3 mr-1" />
					添加
				</Button>
			</div>
		</div>
	);
}

const LOCAL_POWER_PRESETS: Array<{
	name: string;
	label: string;
	context: number;
	help: string;
}> = [
	{
		name: "Qwen/Qwen2.5-7B-Instruct-GGUF:Q3_K_M",
		label: "Qwen2.5 7B Q3_K_M",
		context: 2048,
		help: "默认推荐，适配大多数机器（8GB+）",
	},
	{
		name: "Qwen/Qwen2.5-7B-Instruct-GGUF:Q4_K_M",
		label: "Qwen2.5 7B Q4_K_M",
		context: 4096,
		help: "推荐 12GB+ 内存，质量更稳",
	},
];

interface PowerRuntimeStatus {
	url: string;
	host: string;
	port: number;
	inferenceBackend: string;
	profile: string;
	totalMemoryGib?: number;
	teeType: string;
	hardwareTee: boolean;
}

function formatBytes(n: number): string {
	if (n < 1024) return `${n} B`;
	if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
	if (n < 1024 * 1024 * 1024) return `${(n / 1024 / 1024).toFixed(1)} MB`;
	return `${(n / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

function LocalPowerPanel({ provider }: { provider: ProviderConfig }) {
	const [models, setModels] = useState<string[]>([]);
	const [loading, setLoading] = useState(false);
	const [pulling, setPulling] = useState<string | null>(null);
	const [importing, setImporting] = useState(false);
	const [diagnosing, setDiagnosing] = useState(false);
	const [logs, setLogs] = useState<string[]>([]);
	const [error, setError] = useState<string>("");
	const [runtimeStatus, setRuntimeStatus] = useState<PowerRuntimeStatus | null>(
		null,
	);
	const logsEndRef = useRef<HTMLDivElement>(null);
	const logStreamCtrlRef = useRef<AbortController | null>(null);

	const appendLog = useCallback((msg: string) => {
		const ts = new Date().toLocaleTimeString("en-US", {
			hour12: false,
			hour: "2-digit",
			minute: "2-digit",
			second: "2-digit",
		});
		setLogs((prev) => [...prev, `[${ts}] ${msg}`]);
	}, []);

	/** Start streaming real Power server logs into the log panel. */
	const startLogStream = useCallback(() => {
		// Stop any existing stream first.
		logStreamCtrlRef.current?.abort();
		logStreamCtrlRef.current = powerApi.streamLogs(
			(entry: PowerLogEntry) => {
				// Format: [HH:MM:SS.mmm] LEVEL  message
				const ts = entry.ts.substring(11, 23); // "HH:MM:SS.mmm"
				setLogs((prev) => [
					...prev,
					`[${ts}] ${entry.level.padEnd(5)}  ${entry.message}`,
				]);
			},
			provider,
		);
	}, [provider]);

	const stopLogStream = useCallback(() => {
		logStreamCtrlRef.current?.abort();
		logStreamCtrlRef.current = null;
	}, []);

	useEffect(() => {
		logsEndRef.current?.scrollIntoView({ behavior: "smooth" });
	}, [logs]);

	const isTauri =
		typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

	const profileLabel = (profile: string) => {
		switch (profile) {
			case "high-memory":
				return "高内存";
			case "balanced":
				return "均衡";
			default:
				return "轻量";
		}
	};

	const teeLabel = (status: PowerRuntimeStatus) => {
		if (status.teeType === "sev-snp") return "AMD SEV-SNP";
		if (status.teeType === "tdx") return "Intel TDX";
		if (status.teeType === "simulated") return "软件模拟";
		return "未检测到";
	};

	const refreshRuntimeStatus = useCallback(async () => {
		if (!isTauri) return;
		try {
			const status = await invoke<PowerRuntimeStatus>(
				"get_power_runtime_status",
			);
			setRuntimeStatus(status);
			if ((provider.baseUrl || "") !== status.url) {
				settingsModel.updateProvider(provider.name, { baseUrl: status.url });
			}
		} catch (e) {
			console.warn("Failed to fetch embedded Power runtime status:", e);
		}
	}, [isTauri, provider.baseUrl, provider.name]);

	const refreshModels = useCallback(async () => {
		setLoading(true);
		setError("");
		try {
			const list = await powerApi.listModels(provider);
			setModels(list.map((m) => m.id));
		} catch (e) {
			setError(e instanceof Error ? e.message : "无法连接本地 Power 服务");
		} finally {
			setLoading(false);
		}
	}, [provider]);

	useEffect(() => {
		refreshModels().catch(() => undefined);
	}, [refreshModels]);

	useEffect(() => {
		refreshRuntimeStatus().catch(() => undefined);
	}, [refreshRuntimeStatus]);

	const syncModelToProvider = (modelId: string, context: number) => {
		if (provider.models.some((m) => m.id === modelId)) return;
		settingsModel.addModel(provider.name, {
			id: modelId,
			name: modelId,
			toolCall: true,
			temperature: true,
			reasoning: true,
			modalities: { input: ["text"], output: ["text"] },
			limit: { context, output: 1024 },
		});
	};

	const modelNameFromPath = (filePath: string) => {
		const fileName = filePath.split(/[/\\]/).pop() || filePath;
		return fileName.replace(/\.gguf$/i, "") || `local-model-${Date.now()}`;
	};

	const handleImport = async () => {
		if (!isTauri) {
			toast.error("仅桌面版支持本地模型导入");
			return;
		}

		setError("");
		setLogs([]);
		setImporting(true);
		startLogStream();
		try {
			const selected = await open({
				directory: false,
				multiple: false,
				filters: [
					{ name: "GGUF Models", extensions: ["gguf"] },
					{ name: "All Files", extensions: ["*"] },
				],
			});

			if (!selected || Array.isArray(selected)) return;

			const modelName = modelNameFromPath(selected);
			appendLog(`导入文件: ${selected}`);
			appendLog("计算 SHA-256 并注册到模型列表...");
			await powerApi.registerModel(
				{ name: modelName, path: selected, format: "gguf" },
				provider,
			);
			syncModelToProvider(modelName, 4096);
			await refreshModels();
			appendLog(`✓ 已导入 ${modelName}`);
			toast.success(`已导入 ${modelName}`);
		} catch (e) {
			const msg = e instanceof Error ? e.message : "导入失败";
			appendLog(`✗ ${msg}`);
			setError(msg);
			toast.error("导入本地模型失败");
		} finally {
			setImporting(false);
			stopLogStream();
		}
	};

	const downloadCtxRef = useRef<{
		startMs: number;
		startCompleted: number;
		lastPct: number;
	} | null>(null);

	const handlePull = async (preset: (typeof LOCAL_POWER_PRESETS)[number]) => {
		setError("");
		setLogs([]);
		downloadCtxRef.current = null;
		setPulling(preset.name);
		startLogStream();
		appendLog(`开始下载 ${preset.label}`);
		appendLog(`模型: ${preset.name}`);
		try {
			await powerApi.pullModel(
				preset.name,
				(event) => {
					if (event.status === "resuming") {
						const offset = event.offset || 0;
						const total = event.total || 0;
						appendLog(
							`断点续传 ${formatBytes(offset)} / ${formatBytes(total)}`,
						);
						return;
					}
					if (event.status === "downloading") {
						const total = event.total || 0;
						const completed = event.completed || 0;
						if (total === 0) return;

						const pct = Math.floor((completed / total) * 100);

						// Initialize speed tracking on first downloading event.
						if (!downloadCtxRef.current) {
							downloadCtxRef.current = {
								startMs: Date.now(),
								startCompleted: completed,
								lastPct: -1,
							};
							appendLog(`文件大小: ${formatBytes(total)}`);
						}

						// Log every 5% change to keep the log readable.
						const ctx = downloadCtxRef.current;
						if (pct - ctx.lastPct < 5 && pct < 100) return;
						ctx.lastPct = pct;

						const elapsedSec = (Date.now() - ctx.startMs) / 1000;
						const bytesPerSec =
							elapsedSec > 0
								? (completed - ctx.startCompleted) / elapsedSec
								: 0;

						let eta = "";
						if (bytesPerSec > 0 && completed < total) {
							const remainSec = Math.round((total - completed) / bytesPerSec);
							const h = Math.floor(remainSec / 3600);
							const m = Math.floor((remainSec % 3600) / 60);
							const s = remainSec % 60;
							if (h > 0) {
								eta = ` · 预计剩余 ${h}小时${m}分${s}秒`;
							} else if (m > 0) {
								eta = ` · 预计剩余 ${m}分${s}秒`;
							} else {
								eta = ` · 预计剩余 ${s}秒`;
							}
						}

						appendLog(
							`▼ ${pct}%  ${formatBytes(completed)} / ${formatBytes(total)}${eta}`,
						);
						return;
					}
					if (event.status === "verifying") {
						appendLog("校验 SHA-256...");
						return;
					}
					if (event.status === "already_exists") {
						appendLog("模型已存在，无需重新下载");
						return;
					}
					if (event.status === "success") {
						appendLog("✓ 下载完成，模型已注册");
					}
				},
				provider,
			);
			syncModelToProvider(preset.name, preset.context);
			await refreshModels();
			toast.success(`${preset.label} 已就绪`);
		} catch (e) {
			const msg = e instanceof Error ? e.message : "下载失败";
			appendLog(`✗ ${msg}`);
			setError(msg);
			toast.error("模型下载失败");
		} finally {
			setPulling(null);
			downloadCtxRef.current = null;
			stopLogStream();
		}
	};

	const handleDiagnostic = async () => {
		setError("");
		setLogs([]);
		setDiagnosing(true);
		startLogStream();
		appendLog("开始诊断本地 Power 服务...");
		try {
			appendLog("检查服务健康状态...");
			const info = await powerApi.healthInfo(provider);
			if (info.status !== "ok") {
				throw new Error(`服务健康检查失败 (status=${info.status})`);
			}
			appendLog(
				`✓ 服务正常  版本 ${info.version ?? "unknown"}  运行 ${info.uptimeSeconds ?? 0}s`,
			);

			appendLog("列出已注册模型...");
			const latestModels = await powerApi.listModels(provider);
			const ids = latestModels.map((m) => m.id);
			setModels(ids);
			if (ids.length === 0) {
				appendLog("未发现已注册模型");
			} else {
				appendLog(`✓ 发现 ${ids.length} 个模型: ${ids.join(", ")}`);
			}

			const modelId = ids[0] || provider.models[0]?.id;
			if (!modelId) {
				throw new Error("无可用模型，请先下载或导入 GGUF");
			}

			appendLog(`对 ${modelId} 进行推理测试...`);
			const result = await powerApi.diagnoseModel(modelId, provider);
			appendLog(
				`✓ 推理完成  延迟 ${result.latencyMs}ms · ${result.tokensPerSecond.toFixed(1)} tok/s`,
			);
			toast.success("本地模型诊断完成");
		} catch (e) {
			const msg = e instanceof Error ? e.message : "诊断失败";
			appendLog(`✗ ${msg}`);
			setError(msg);
			toast.error("本地模型诊断失败");
		} finally {
			setDiagnosing(false);
			stopLogStream();
		}
	};

	const handleDelete = async (modelId: string) => {
		try {
			await powerApi.deleteModel(modelId, provider);
			await refreshModels();
			toast.success(`已删除 ${modelId}`);
		} catch (e) {
			setError(e instanceof Error ? e.message : "删除失败");
			toast.error("删除模型失败");
		}
	};

	return (
		<div className="rounded-xl border bg-card p-4 mb-4 space-y-3">
			<div className="flex items-center justify-between">
				<div className="flex items-center gap-2">
					<Shield className="size-4 text-primary" />
					<span className="text-sm font-semibold">本地隐私推理 (Power)</span>
				</div>
				<div className="flex items-center gap-2">
					<Button
						variant="outline"
						size="sm"
						className="h-7 text-xs"
						onClick={() => handleDiagnostic()}
						disabled={loading || !!pulling || importing || diagnosing}
					>
						{diagnosing ? (
							<Loader2 className="size-3 mr-1 animate-spin" />
						) : (
							<Gauge className="size-3 mr-1" />
						)}
						诊断本地模型
					</Button>
					<Button
						variant="outline"
						size="sm"
						className="h-7 text-xs"
						onClick={() => handleImport()}
						disabled={loading || !!pulling || importing || diagnosing}
					>
						{importing ? (
							<Loader2 className="size-3 mr-1 animate-spin" />
						) : (
							<FolderUp className="size-3 mr-1" />
						)}
						导入 GGUF
					</Button>
					<Button
						variant="outline"
						size="sm"
						className="h-7 text-xs"
						onClick={() => refreshModels()}
						disabled={loading || !!pulling || importing || diagnosing}
					>
						<RefreshCw
							className={cn("size-3 mr-1", loading && "animate-spin")}
						/>
						刷新
					</Button>
				</div>
			</div>

			<div className="text-xs text-muted-foreground">
				<div>地址: {provider.baseUrl || "http://127.0.0.1:11435/v1"}</div>
				<div className="mt-1">默认启用日志脱敏、内存解密与流式解密。</div>
			</div>

			{runtimeStatus && (
				<div className="rounded-lg border bg-muted/20 px-3 py-2 text-[11px] text-muted-foreground">
					<div>推理后端: {runtimeStatus.inferenceBackend}（layer-stream）</div>
					<div>
						运行档位: {profileLabel(runtimeStatus.profile)}
						{typeof runtimeStatus.totalMemoryGib === "number"
							? ` · 内存 ${runtimeStatus.totalMemoryGib} GiB`
							: ""}
					</div>
					<div className="mt-0.5">
						TEE 状态: {teeLabel(runtimeStatus)}
						{runtimeStatus.hardwareTee ? "（硬件隔离）" : "（本地隐私模式）"}
					</div>
				</div>
			)}

			<div className="grid gap-2 sm:grid-cols-2">
				{LOCAL_POWER_PRESETS.map((preset) => {
					const installed = models.includes(preset.name);
					const isPulling = pulling === preset.name;
					return (
						<div key={preset.name} className="rounded-lg border p-3 space-y-2">
							<div className="text-xs font-medium">{preset.label}</div>
							<div className="text-[11px] text-muted-foreground">
								{preset.help}
							</div>
							<div className="flex gap-2">
								<Button
									size="sm"
									className="h-7 text-[11px]"
									variant={installed ? "secondary" : "default"}
									onClick={() => handlePull(preset)}
									disabled={!!pulling || importing}
								>
									{isPulling ? (
										<Loader2 className="size-3 mr-1 animate-spin" />
									) : (
										<Download className="size-3 mr-1" />
									)}
									{installed ? "重新拉取" : "下载模型"}
								</Button>
							</div>
						</div>
					);
				})}
			</div>

			{logs.length > 0 && (
				<div className="rounded-lg border border-border/50 bg-black/75 px-3 py-2.5 font-mono text-[10.5px] leading-relaxed max-h-40 overflow-y-auto space-y-px">
					{logs.map((line, i) => (
						<div
							key={i}
							className={cn(
								"text-green-400/80 whitespace-pre-wrap break-all",
								line.includes("✗") && "text-red-400",
								line.includes("✓") && "text-emerald-400",
							)}
						>
							{line}
						</div>
					))}
					<div ref={logsEndRef} />
				</div>
			)}

			<div className="rounded-lg border bg-muted/20 p-3">
				<div className="flex items-center gap-1.5 mb-2">
					<HardDrive className="size-3.5 text-muted-foreground" />
					<span className="text-xs font-medium">
						已安装模型 ({models.length})
					</span>
				</div>
				{models.length === 0 && !pulling && !importing ? (
					<div className="text-[11px] text-muted-foreground">暂无本地模型</div>
				) : models.length === 0 ? null : (
					<div className="space-y-1.5">
						{models.map((modelId) => (
							<div
								key={modelId}
								className="flex items-center justify-between gap-2 rounded border px-2 py-1.5"
							>
								<span className="text-[11px] font-mono truncate">
									{modelId}
								</span>
								<Button
									variant="ghost"
									size="sm"
									className="h-6 text-[10px] text-destructive"
									onClick={() => handleDelete(modelId)}
								>
									<Trash2 className="size-3 mr-1" />
									删除
								</Button>
							</div>
						))}
					</div>
				)}
			</div>

			{error && (
				<div className="text-xs text-destructive break-all">{error}</div>
			)}
		</div>
	);
}

function ProviderCard({
	provider,
	isDefault,
	defaultModel,
	onSetDefault,
	onRemove,
}: {
	provider: ProviderConfig;
	isDefault: boolean;
	defaultModel: string;
	onSetDefault: (p: string, m: string) => void;
	onRemove: (n: string) => void;
}) {
	const [showKey, setShowKey] = useState(false);
	const [addingModel, setAddingModel] = useState(false);
	const [editingKey, setEditingKey] = useState(false);
	const [editingUrl, setEditingUrl] = useState(false);
	const [keyDraft, setKeyDraft] = useState(provider.apiKey || "");
	const [urlDraft, setUrlDraft] = useState(provider.baseUrl || "");
	const [testState, setTestState] = useState<
		"idle" | "testing" | "ok" | "fail"
	>("idle");
	const [testLatency, setTestLatency] = useState<number | null>(null);
	const modal = useModal();

	const handleTest = async () => {
		setTestState("testing");
		setTestLatency(null);
		const t0 = Date.now();
		try {
			const { agentApi } = await import("@/lib/agent-api");
			const backends = await agentApi.listBackends();
			const match = backends?.find(
				(b: any) => b.provider?.toLowerCase() === provider.name.toLowerCase(),
			);
			setTestLatency(Date.now() - t0);
			setTestState(match ? "ok" : "fail");
			if (!match)
				toast.error(`${provider.name} 未在后端注册，请检查 API Key 配置`);
		} catch (e) {
			setTestState("fail");
			toast.error(`连接失败：${e instanceof Error ? e.message : "后端不可用"}`);
		}
	};

	return (
		<div
			className={cn(
				"rounded-xl border bg-card transition-all",
				isDefault && "ring-2 ring-primary/30",
			)}
		>
			<div className="flex items-center gap-3 px-4 py-3 border-b">
				<span
					className={cn(
						"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-bold uppercase",
						pColor(provider.name),
					)}
				>
					{provider.name}
				</span>
				{isDefault && (
					<span className="inline-flex items-center gap-1 text-[10px] text-primary font-medium">
						<Star className="size-3 fill-primary" />
						默认
					</span>
				)}
				<span className="text-[10px] text-muted-foreground">
					{provider.models.length} 个模型
				</span>
				<div className="flex-1" />
				<button
					type="button"
					className={cn(
						"flex items-center gap-1 text-[10px] rounded-md px-2 py-1 border transition-colors",
						testState === "ok" &&
							"border-green-500/40 text-green-600 dark:text-green-400",
						testState === "fail" && "border-destructive/40 text-destructive",
						testState === "testing" && "border-primary/30 text-primary",
						testState === "idle" &&
							"border-border text-muted-foreground hover:text-foreground hover:border-primary/30",
					)}
					onClick={handleTest}
					disabled={testState === "testing"}
					aria-label={`测试 ${provider.name} 连接`}
				>
					{testState === "testing" ? (
						<>
							<Loader2 className="size-3 animate-spin" />
							测试中
						</>
					) : testState === "ok" ? (
						<>
							<Check className="size-3" />
							{testLatency}ms
						</>
					) : testState === "fail" ? (
						<>✗ 失败</>
					) : (
						<>测试连接</>
					)}
				</button>
				<Button
					variant="ghost"
					size="sm"
					className="h-6 text-[10px] text-destructive hover:text-destructive"
					onClick={() => {
						modal.alert({
							title: `删除 ${provider.name}`,
							description: `确认删除 "${provider.name}" 及其所有模型？`,
							confirmText: "删除",
							onConfirm: () => {
								onRemove(provider.name);
								toast.success(`已删除 ${provider.name}`);
							},
						});
					}}
				>
					<Trash2 className="size-3" />
				</Button>
			</div>

			<div className="px-4 py-3 space-y-2 border-b bg-muted/20">
				<div className="flex items-center gap-2">
					<KeyRound className="size-3 text-muted-foreground shrink-0" />
					{editingKey ? (
						<div className="flex-1 flex items-center gap-1.5">
							<Input
								className="h-6 text-[11px] font-mono flex-1"
								type={showKey ? "text" : "password"}
								value={keyDraft}
								onChange={(e) => setKeyDraft(e.target.value)}
								placeholder="API Key"
							/>
							<button
								type="button"
								onClick={() => setShowKey(!showKey)}
								className="text-muted-foreground hover:text-foreground"
							>
								{showKey ? (
									<EyeOff className="size-3" />
								) : (
									<Eye className="size-3" />
								)}
							</button>
							<Button
								size="sm"
								className="h-6 text-[10px] px-2"
								onClick={() => {
									settingsModel.updateProvider(provider.name, {
										apiKey: keyDraft,
									});
									setEditingKey(false);
									toast.success("API Key 已更新");
								}}
							>
								<Check className="size-3" />
							</Button>
							<button
								type="button"
								onClick={() => {
									setEditingKey(false);
									setKeyDraft(provider.apiKey || "");
								}}
								className="text-muted-foreground hover:text-foreground"
							>
								<X className="size-3" />
							</button>
						</div>
					) : (
						<button
							type="button"
							className="text-[11px] font-mono text-muted-foreground hover:text-foreground"
							onClick={() => {
								setEditingKey(true);
								setKeyDraft(provider.apiKey || "");
							}}
						>
							{provider.apiKey
								? `${provider.apiKey.slice(0, 8)}${"•".repeat(12)}`
								: "点击设置 API Key"}
						</button>
					)}
				</div>
				<div className="flex items-center gap-2">
					<Server className="size-3 text-muted-foreground shrink-0" />
					{editingUrl ? (
						<div className="flex-1 flex items-center gap-1.5">
							<Input
								className="h-6 text-[11px] font-mono flex-1"
								value={urlDraft}
								onChange={(e) => setUrlDraft(e.target.value)}
								placeholder="Base URL"
							/>
							<Button
								size="sm"
								className="h-6 text-[10px] px-2"
								onClick={() => {
									settingsModel.updateProvider(provider.name, {
										baseUrl: urlDraft,
									});
									setEditingUrl(false);
									toast.success("Base URL 已更新");
								}}
							>
								<Check className="size-3" />
							</Button>
							<button
								type="button"
								onClick={() => {
									setEditingUrl(false);
									setUrlDraft(provider.baseUrl || "");
								}}
								className="text-muted-foreground hover:text-foreground"
							>
								<X className="size-3" />
							</button>
						</div>
					) : (
						<button
							type="button"
							className="text-[11px] font-mono text-muted-foreground hover:text-foreground"
							onClick={() => {
								setEditingUrl(true);
								setUrlDraft(provider.baseUrl || "");
							}}
						>
							{provider.baseUrl || "点击设置 Base URL"}
						</button>
					)}
				</div>
			</div>

			<div className="px-4 py-3 space-y-1.5">
				{provider.models.map((m) => {
					const isDef = isDefault && defaultModel === m.id;
					return (
						<div
							key={m.id}
							className={cn(
								"flex items-center gap-2.5 rounded-lg border px-3 py-2 transition-all group",
								isDef
									? "border-primary/40 bg-primary/5"
									: "hover:border-primary/20",
							)}
						>
							<div className="flex-1 min-w-0">
								<div className="flex items-center gap-2">
									<span className="text-xs font-medium truncate">{m.name}</span>
									{isDef && (
										<Star className="size-3 text-primary fill-primary shrink-0" />
									)}
								</div>
								<div className="flex items-center gap-2 mt-0.5">
									<span className="text-[10px] font-mono text-muted-foreground">
										{m.id}
									</span>
									{m.limit && (
										<span className="text-[9px] text-muted-foreground">
											{(m.limit.context / 1000).toFixed(0)}K ctx
										</span>
									)}
									{m.apiKey && (
										<span className="text-[9px] text-muted-foreground/60 italic">
											自定义 Key
										</span>
									)}
									{m.baseUrl && (
										<span className="text-[9px] text-muted-foreground/60 italic">
											自定义 URL
										</span>
									)}
								</div>
							</div>
							<div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
								{!isDef && (
									<Button
										variant="ghost"
										size="sm"
										className="h-6 text-[10px] px-2"
										onClick={() => onSetDefault(provider.name, m.id)}
									>
										<Star className="size-3 mr-1" />
										设为默认
									</Button>
								)}
								<button
									type="button"
									className="text-muted-foreground hover:text-destructive p-1"
									onClick={() => {
										modal.alert({
											title: "删除模型",
											description: `确认删除 "${m.name}"？`,
											confirmText: "删除",
											onConfirm: () => {
												settingsModel.removeModel(provider.name, m.id);
												toast.success(`已删除 ${m.name}`);
											},
										});
									}}
								>
									<Trash2 className="size-3" />
								</button>
							</div>
						</div>
					);
				})}
				{addingModel ? (
					<AddModelForm
						onAdd={(m) => {
							settingsModel.addModel(provider.name, m);
							setAddingModel(false);
							toast.success(`已添加 ${m.name}`);
						}}
						onCancel={() => setAddingModel(false)}
					/>
				) : (
					<button
						type="button"
						className="flex items-center gap-1.5 w-full rounded-lg border border-dashed px-3 py-2 text-xs text-muted-foreground hover:text-foreground hover:border-primary/30 transition-colors"
						onClick={() => setAddingModel(true)}
					>
						<Plus className="size-3.5" />
						添加模型
					</button>
				)}
			</div>
		</div>
	);
}

export function AiSection() {
	const snap = useSnapshot(settingsModel.state);
	const [addingProvider, setAddingProvider] = useState(false);
	// Only allow syncToBackend after seedFromBackend has completed,
	// AND skip the first render cycle after seed (which is the seed itself
	// updating state — we don't want to echo that back to the backend).
	const seededRef = useRef(false);
	const skipNextSyncRef = useRef(false);

	useEffect(() => {
		let cancelled = false;
		settingsModel.waitForSeed().then(() => {
			if (!cancelled) {
				skipNextSyncRef.current = true;
				seededRef.current = true;
			}
		});
		return () => {
			cancelled = true;
		};
	}, []);

	useEffect(() => {
		if (!seededRef.current) return;
		if (skipNextSyncRef.current) {
			skipNextSyncRef.current = false;
			return;
		}
		const timer = setTimeout(() => {
			settingsModel
				.syncToBackend()
				.catch((e: unknown) =>
					console.warn("Failed to sync settings to backend:", e),
				);
		}, 800);
		return () => clearTimeout(timer);
	}, [snap.providers, snap.defaultProvider, snap.defaultModel]);

	const handleSetDefault = (pName: string, mId: string) => {
		settingsModel.setDefault(pName, mId);
		toast.success("已设置默认模型");
	};
	const handleAddProvider = (p: ProviderConfig) => {
		if (snap.providers.some((ep) => ep.name === p.name)) {
			toast.error(`"${p.name}" 已存在`);
			return;
		}
		settingsModel.addProvider(p);
		setAddingProvider(false);
		toast.success(`已添加 ${p.name}`);
	};

	const defProvider = snap.providers.find(
		(p) => p.name === snap.defaultProvider,
	);
	const defModel = defProvider?.models.find((m) => m.id === snap.defaultModel);
	const localPowerProvider = snap.providers.find(
		(p) => p.name === "local-power",
	);

	return (
		<div>
			<SectionHeader
				icon={Bot}
				title="AI 服务"
				description="管理模型提供商、模型和默认配置。"
			/>
			{localPowerProvider && (
				<LocalPowerPanel provider={localPowerProvider as ProviderConfig} />
			)}
			<div className="rounded-xl border bg-card p-4 mb-4">
				<div className="flex items-center gap-2 mb-3">
					<Star className="size-4 text-primary fill-primary" />
					<span className="text-sm font-semibold">默认模型</span>
				</div>
				{defProvider && defModel ? (
					<div className="flex items-center gap-3">
						<span
							className={cn(
								"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-bold uppercase",
								pColor(defProvider.name),
							)}
						>
							{defProvider.name}
						</span>
						<span className="text-sm font-medium">{defModel.name}</span>
						<span className="text-[11px] font-mono text-muted-foreground">
							{defModel.id}
						</span>
					</div>
				) : (
					<Select
						value={
							snap.defaultProvider
								? `${snap.defaultProvider}::${snap.defaultModel}`
								: ""
						}
						onValueChange={(v) => {
							const [p, m] = v.split("::");
							if (p && m) handleSetDefault(p, m);
						}}
					>
						<SelectTrigger className="h-8 text-sm">
							<SelectValue placeholder="选择默认模型" />
						</SelectTrigger>
						<SelectContent>
							{snap.providers.flatMap((p) =>
								p.models.map((m) => (
									<SelectItem
										key={`${p.name}::${m.id}`}
										value={`${p.name}::${m.id}`}
									>
										<span className="font-mono text-xs">
											{p.name} / {m.id}
										</span>
									</SelectItem>
								)),
							)}
						</SelectContent>
					</Select>
				)}
			</div>
			<div className="space-y-4">
				{snap.providers.map((p) => (
					<ProviderCard
						key={p.name}
						provider={p as ProviderConfig}
						isDefault={snap.defaultProvider === p.name}
						defaultModel={snap.defaultModel}
						onSetDefault={handleSetDefault}
						onRemove={(n) => settingsModel.removeProvider(n)}
					/>
				))}
				{addingProvider ? (
					<AddProviderForm
						onAdd={handleAddProvider}
						onCancel={() => setAddingProvider(false)}
					/>
				) : (
					<button
						type="button"
						className="flex items-center justify-center gap-2 w-full rounded-xl border-2 border-dashed px-4 py-4 text-sm text-muted-foreground hover:text-foreground hover:border-primary/30 transition-colors"
						onClick={() => setAddingProvider(true)}
					>
						<Plus className="size-4" />
						添加 Provider
					</button>
				)}
			</div>
		</div>
	);
}
