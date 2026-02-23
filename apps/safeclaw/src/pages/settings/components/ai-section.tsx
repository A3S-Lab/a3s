import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { useModal } from "@/components/custom/modal-provider";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import type { ProviderConfig, ModelConfig } from "@/models/settings.model";
import {
	Bot,
	Check,
	Eye,
	EyeOff,
	KeyRound,
	Loader2,
	Plus,
	Server,
	Star,
	Trash2,
	X,
} from "lucide-react";
import { useState, useEffect, useRef } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
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

	return (
		<div>
			<SectionHeader
				icon={Bot}
				title="AI 服务"
				description="管理模型提供商、模型和默认配置。"
			/>
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
