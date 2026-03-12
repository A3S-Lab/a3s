import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import type { ModelConfig, ProviderConfig } from "@/models/settings.model";
import {
	Bot,
	Check,
	Eye,
	EyeOff,
	KeyRound,
	Pencil,
	Plus,
	Star,
	Trash2,
	X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
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

function EditModelForm({
	model,
	onSave,
	onCancel,
}: {
	model: ModelConfig;
	onSave: (patch: Partial<ModelConfig>) => void;
	onCancel: () => void;
}) {
	const [name, setName] = useState(model.name);
	const [apiKey, setApiKey] = useState(model.apiKey || "");
	const [baseUrl, setBaseUrl] = useState(model.baseUrl || "");
	const [context, setContext] = useState(String(model.limit.context));
	const [output, setOutput] = useState(String(model.limit.output));
	return (
		<div className="rounded-lg border-2 border-primary/50 bg-primary/[0.05] p-3 space-y-2.5 mt-2">
			<div className="flex items-center justify-between">
				<span className="text-xs font-semibold">编辑模型</span>
				<button
					type="button"
					onClick={onCancel}
					className="text-muted-foreground hover:text-foreground"
				>
					<X className="size-3.5" />
				</button>
			</div>
			<div className="space-y-2">
				<div>
					<label className="text-[10px] text-muted-foreground">模型 ID</label>
					<Input className="h-7 text-xs font-mono" value={model.id} disabled />
				</div>
				<div>
					<label className="text-[10px] text-muted-foreground">显示名称</label>
					<Input
						className="h-7 text-xs"
						placeholder="显示名称"
						value={name}
						onChange={(e) => setName(e.target.value)}
					/>
				</div>
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
					onClick={() =>
						onSave({
							name: name.trim() || model.id,
							apiKey: apiKey || undefined,
							baseUrl: baseUrl || undefined,
							limit: {
								context: Number(context) || 128000,
								output: Number(output) || 4096,
							},
						})
					}
				>
					保存
				</Button>
			</div>
		</div>
	);
}

function ProviderCard({
	provider,
	isDefault,
	defaultModelId,
	onSetDefault,
	onRemove,
	onUpdateProvider,
	onAddModel,
	onUpdateModel,
	onRemoveModel,
}: {
	provider: ProviderConfig;
	isDefault: boolean;
	defaultModelId: string;
	onSetDefault: (pName: string, mId: string) => void;
	onRemove: () => void;
	onUpdateProvider: (patch: Partial<Omit<ProviderConfig, "name">>) => void;
	onAddModel: (m: ModelConfig) => void;
	onUpdateModel: (mId: string, patch: Partial<ModelConfig>) => void;
	onRemoveModel: (mId: string) => void;
}) {
	const [showApiKey, setShowApiKey] = useState(false);
	const [apiKey, setApiKey] = useState(provider.apiKey || "");
	const [baseUrl, setBaseUrl] = useState(provider.baseUrl || "");
	const [addingModel, setAddingModel] = useState(false);
	const [editingModelId, setEditingModelId] = useState<string | null>(null);

	const handleSave = () => {
		onUpdateProvider({
			apiKey: apiKey.trim() || undefined,
			baseUrl: baseUrl.trim() || undefined,
		});
		toast.success("已保存");
	};

	return (
		<div className="rounded-xl border bg-card p-4 space-y-3">
			<div className="flex items-center justify-between">
				<span
					className={cn(
						"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-bold uppercase",
						pColor(provider.name),
					)}
				>
					{provider.name}
				</span>
				<Button
					variant="ghost"
					size="sm"
					className="h-6 text-xs text-destructive hover:text-destructive"
					onClick={onRemove}
				>
					<Trash2 className="size-3 mr-1" />
					删除
				</Button>
			</div>

			<div className="space-y-2">
				<div className="space-y-1">
					<label className="text-xs text-muted-foreground flex items-center gap-1">
						<KeyRound className="size-3" />
						API Key
					</label>
					<div className="flex gap-1">
						<Input
							className="h-7 text-xs font-mono flex-1"
							type={showApiKey ? "text" : "password"}
							placeholder="留空使用环境变量"
							value={apiKey}
							onChange={(e) => setApiKey(e.target.value)}
						/>
						<Button
							variant="ghost"
							size="sm"
							className="h-7 w-7 p-0"
							onClick={() => setShowApiKey(!showApiKey)}
						>
							{showApiKey ? (
								<EyeOff className="size-3" />
							) : (
								<Eye className="size-3" />
							)}
						</Button>
					</div>
				</div>
				<div className="space-y-1">
					<label className="text-xs text-muted-foreground">Base URL</label>
					<Input
						className="h-7 text-xs font-mono"
						placeholder="留空使用默认值"
						value={baseUrl}
						onChange={(e) => setBaseUrl(e.target.value)}
					/>
				</div>
				<Button size="sm" className="h-6 text-xs w-full" onClick={handleSave}>
					保存配置
				</Button>
			</div>

			<div className="space-y-2">
				<div className="flex items-center justify-between">
					<span className="text-xs font-semibold">模型列表</span>
					<Button
						variant="ghost"
						size="sm"
						className="h-6 text-xs"
						onClick={() => setAddingModel(true)}
					>
						<Plus className="size-3 mr-1" />
						添加模型
					</Button>
				</div>

				{addingModel && (
					<AddModelForm
						onAdd={(m) => {
							onAddModel(m);
							setAddingModel(false);
						}}
						onCancel={() => setAddingModel(false)}
					/>
				)}

				<div className="space-y-1.5">
					{provider.models.map((model) => (
						<div key={model.id}>
							<div className="flex items-center justify-between rounded-lg border bg-muted/30 px-2.5 py-1.5">
								<div className="flex-1 min-w-0">
									<div className="text-xs font-medium truncate">
										{model.name}
									</div>
									<div className="text-[10px] text-muted-foreground font-mono truncate">
										{model.id}
									</div>
								</div>
								<div className="flex items-center gap-1">
									{isDefault && model.id === defaultModelId ? (
										<div className="flex items-center gap-1 h-5 px-1.5 text-[10px] text-primary">
											<Check className="size-2.5" />
											<span>默认</span>
										</div>
									) : (
										<Button
											variant="ghost"
											size="sm"
											className="h-5 text-[10px] px-1.5"
											onClick={() => onSetDefault(provider.name, model.id)}
										>
											<Star className="size-2.5 mr-0.5" />
											设为默认
										</Button>
									)}
									<Button
										variant="ghost"
										size="sm"
										className="h-5 w-5 p-0"
										onClick={() => setEditingModelId(model.id)}
									>
										<Pencil className="size-2.5" />
									</Button>
									<Button
										variant="ghost"
										size="sm"
										className="h-5 w-5 p-0 text-destructive hover:text-destructive"
										onClick={() => onRemoveModel(model.id)}
									>
										<Trash2 className="size-2.5" />
									</Button>
								</div>
							</div>
							{editingModelId === model.id && (
								<EditModelForm
									model={model}
									onSave={(patch) => {
										onUpdateModel(model.id, patch);
										setEditingModelId(null);
									}}
									onCancel={() => setEditingModelId(null)}
								/>
							)}
						</div>
					))}
					{provider.models.length === 0 && (
						<div className="text-xs text-muted-foreground text-center py-2">
							暂无模型
						</div>
					)}
				</div>
			</div>
		</div>
	);
}

export function AiSection() {
	const snap = useSnapshot(settingsModel.state);
	const [addingProvider, setAddingProvider] = useState(false);
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
						<span className="text-xs text-muted-foreground font-mono">
							{defModel.id}
						</span>
					</div>
				) : (
					<div className="text-sm text-muted-foreground">未设置默认模型</div>
				)}
			</div>

			<div className="space-y-3">
				<div className="flex items-center justify-between">
					<span className="text-sm font-semibold">Providers</span>
					<Button
						variant="outline"
						size="sm"
						className="h-7 text-xs"
						onClick={() => setAddingProvider(true)}
					>
						<Plus className="size-3 mr-1" />
						添加 Provider
					</Button>
				</div>

				{addingProvider && (
					<AddProviderForm
						onAdd={handleAddProvider}
						onCancel={() => setAddingProvider(false)}
					/>
				)}

				{snap.providers.map((provider) => (
					<ProviderCard
						key={provider.name}
						provider={provider}
						isDefault={provider.name === snap.defaultProvider}
						defaultModelId={snap.defaultModel}
						onSetDefault={handleSetDefault}
						onRemove={() => {
							settingsModel.removeProvider(provider.name);
							toast.success(`已删除 ${provider.name}`);
						}}
						onUpdateProvider={(patch) =>
							settingsModel.updateProvider(provider.name, patch)
						}
						onAddModel={(m) => {
							if (provider.models.some((em) => em.id === m.id)) {
								toast.error(`模型 "${m.id}" 已存在`);
								return;
							}
							settingsModel.addModel(provider.name, m);
							toast.success(`已添加模型 ${m.name}`);
						}}
						onUpdateModel={(mId, patch) => {
							settingsModel.updateModel(provider.name, mId, patch);
							toast.success("已更新模型");
						}}
						onRemoveModel={(mId) => {
							settingsModel.removeModel(provider.name, mId);
							toast.success("已删除模型");
						}}
					/>
				))}

				{snap.providers.length === 0 && (
					<div className="text-center py-8 text-sm text-muted-foreground">
						暂无 Provider，点击上方按钮添加
					</div>
				)}
			</div>
		</div>
	);
}
