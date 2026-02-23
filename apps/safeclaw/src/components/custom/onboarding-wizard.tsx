import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import settingsModel from "@/models/settings.model";
import {
	Bot,
	Check,
	ChevronRight,
	Globe,
	Server,
	Shield,
	ShieldCheck,
	Wifi,
	Zap,
} from "lucide-react";
import { useState } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

// =============================================================================
// Persistence
// =============================================================================

const ONBOARDING_KEY = "safeclaw-onboarding-complete";

export function isOnboardingComplete(): boolean {
	try {
		return localStorage.getItem(ONBOARDING_KEY) === "true";
	} catch {
		return true;
	}
}

function markOnboardingComplete() {
	try {
		localStorage.setItem(ONBOARDING_KEY, "true");
	} catch {
		/* ignore */
	}
}

// =============================================================================
// Step types
// =============================================================================

type StepId = "welcome" | "gateway" | "provider" | "done";

const STEPS: { id: StepId; label: string; icon: typeof ShieldCheck }[] = [
	{ id: "welcome", label: "欢迎", icon: ShieldCheck },
	{ id: "gateway", label: "网关", icon: Server },
	{ id: "provider", label: "AI 模型", icon: Bot },
	{ id: "done", label: "完成", icon: Check },
];

const STEP_IDS = STEPS.map((s) => s.id);

// =============================================================================
// Step components
// =============================================================================

function WelcomeStep() {
	return (
		<div className="flex flex-col items-center text-center py-4">
			<div className="flex items-center justify-center size-20 rounded-2xl bg-primary/10 mb-6">
				<ShieldCheck className="size-10 text-primary" />
			</div>
			<h2 className="text-2xl font-bold mb-3">欢迎使用 SafeClaw</h2>
			<p className="text-muted-foreground max-w-sm leading-relaxed mb-6">
				SafeClaw 是一个隐私优先的 AI 助手运行时，支持 TEE 硬件隔离、PII
				检测、Prompt 注入防护和多渠道接入。
			</p>
			<div className="grid grid-cols-2 gap-3 w-full max-w-sm text-left">
				{[
					{
						icon: Shield,
						label: "TEE 硬件隔离",
						desc: "AMD SEV-SNP / Intel TDX",
					},
					{ icon: Zap, label: "实时 PII 检测", desc: "正则 + 语义双引擎" },
					{ icon: Globe, label: "网络防火墙", desc: "出站白名单保护" },
					{ icon: Wifi, label: "多渠道接入", desc: "7 种消息平台" },
				].map((item) => (
					<div key={item.label} className="rounded-lg border bg-card p-3">
						<item.icon className="size-4 text-primary mb-1.5" />
						<div className="text-xs font-medium">{item.label}</div>
						<div className="text-[10px] text-muted-foreground">{item.desc}</div>
					</div>
				))}
			</div>
		</div>
	);
}

function GatewayStep({
	baseUrl,
	onChange,
}: { baseUrl: string; onChange: (v: string) => void }) {
	return (
		<div className="space-y-5">
			<div>
				<h3 className="text-base font-semibold mb-1">配置网关地址</h3>
				<p className="text-sm text-muted-foreground">
					SafeClaw 网关是所有 AI 请求的安全代理入口。
				</p>
			</div>
			<div className="rounded-xl border bg-card p-5 space-y-4">
				<div>
					<label className="text-sm font-medium block mb-1.5">网关地址</label>
					<div className="relative">
						<Server className="absolute left-2.5 top-1/2 -translate-y-1/2 size-3.5 text-muted-foreground pointer-events-none" />
						<Input
							className="h-9 text-sm font-mono pl-8"
							placeholder="http://127.0.0.1:18790"
							value={baseUrl}
							onChange={(e) => onChange(e.target.value)}
						/>
					</div>
					<p className="text-[11px] text-muted-foreground mt-1.5">
						留空使用默认值 http://127.0.0.1:18790
					</p>
				</div>
				<div className="rounded-lg bg-muted/30 divide-y divide-border/50">
					{[
						{
							label: "HTTP API",
							value: `${baseUrl || "http://127.0.0.1:18790"}/api/v1`,
						},
						{
							label: "WebSocket",
							value: `${(baseUrl || "http://127.0.0.1:18790").replace("http", "ws")}/ws`,
						},
					].map((item) => (
						<div
							key={item.label}
							className="flex justify-between items-center px-4 py-2.5"
						>
							<span className="text-xs text-muted-foreground">
								{item.label}
							</span>
							<span className="text-xs font-mono text-muted-foreground">
								{item.value}
							</span>
						</div>
					))}
				</div>
			</div>
		</div>
	);
}

function ProviderStep() {
	const snap = useSnapshot(settingsModel.state);
	const defProvider = snap.providers[0];
	const [apiKey, setApiKey] = useState(defProvider?.apiKey || "");

	return (
		<div className="space-y-5">
			<div>
				<h3 className="text-base font-semibold mb-1">配置 AI 模型</h3>
				<p className="text-sm text-muted-foreground">
					设置默认 AI 模型提供商的 API Key。
				</p>
			</div>
			<div className="rounded-xl border bg-card p-5 space-y-4">
				<div className="flex items-center gap-3 pb-3 border-b">
					<span className="inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-bold bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/20">
						{defProvider?.name || "anthropic"}
					</span>
					<span className="text-sm font-medium">
						{snap.defaultModel || "claude-sonnet-4-20250514"}
					</span>
					<span className="text-[10px] text-muted-foreground ml-auto">
						默认模型
					</span>
				</div>
				<div>
					<label className="text-sm font-medium block mb-1.5">API Key</label>
					<Input
						className="h-9 text-sm font-mono"
						type="password"
						placeholder="sk-ant-..."
						value={apiKey}
						onChange={(e) => setApiKey(e.target.value)}
						onBlur={() => {
							if (defProvider && apiKey)
								settingsModel.updateProvider(defProvider.name, { apiKey });
						}}
					/>
					<p className="text-[11px] text-muted-foreground mt-1.5">
						可跳过，稍后在设置 → AI 服务中配置。
					</p>
				</div>
			</div>
			<div className="rounded-lg bg-muted/30 p-3 text-[11px] text-muted-foreground">
				更多模型提供商（OpenAI、DeepSeek、本地模型）可在设置页面添加。
			</div>
		</div>
	);
}

function DoneStep() {
	return (
		<div className="flex flex-col items-center text-center py-4">
			<div className="flex items-center justify-center size-20 rounded-2xl bg-green-500/10 mb-6">
				<Check className="size-10 text-green-600 dark:text-green-400" />
			</div>
			<h2 className="text-2xl font-bold mb-3">配置完成</h2>
			<p className="text-muted-foreground max-w-sm leading-relaxed mb-6">
				SafeClaw 已准备就绪。渠道接入和 TEE 配置请编辑{" "}
				<code className="font-mono text-xs">safeclaw.hcl</code>。
			</p>
			<div className="rounded-xl border bg-card p-4 w-full max-w-sm text-left space-y-2">
				{["✅ 网关连接已配置", "✅ AI 模型已配置"].map((item) => (
					<div key={item} className="text-sm text-muted-foreground">
						{item}
					</div>
				))}
			</div>
		</div>
	);
}

// =============================================================================
// Main Wizard
// =============================================================================

interface OnboardingWizardProps {
	onComplete: () => void;
}

export default function OnboardingWizard({
	onComplete,
}: OnboardingWizardProps) {
	const snap = useSnapshot(settingsModel.state);
	const [stepIdx, setStepIdx] = useState(0);
	const [gatewayUrl, setGatewayUrl] = useState(snap.baseUrl || "");

	const currentStep = STEP_IDS[stepIdx];
	const isDone = currentStep === "done";

	const handleNext = () => {
		if (currentStep === "gateway") {
			settingsModel.setBaseUrl(gatewayUrl);
		}
		if (isDone) {
			markOnboardingComplete();
			onComplete();
			toast.success("SafeClaw 配置完成，开始使用吧！");
			return;
		}
		setStepIdx((i) => Math.min(i + 1, STEP_IDS.length - 1));
	};

	const handleSkip = () => {
		if (isDone) return;
		setStepIdx((i) => Math.min(i + 1, STEP_IDS.length - 1));
	};

	return (
		<div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
			<div className="relative w-full max-w-lg mx-4 rounded-2xl border bg-card shadow-2xl overflow-hidden">
				{/* Step indicator */}
				<div className="flex items-center gap-1 px-6 pt-5 pb-4 border-b">
					{STEPS.map((step, i) => {
						const done = i < stepIdx;
						const active = i === stepIdx;
						return (
							<div key={step.id} className="flex items-center gap-1">
								<div
									className={cn(
										"flex items-center justify-center size-6 rounded-full text-[10px] font-bold transition-all",
										done
											? "bg-primary text-primary-foreground"
											: active
												? "bg-primary/20 text-primary ring-2 ring-primary/30"
												: "bg-muted text-muted-foreground",
									)}
								>
									{done ? <Check className="size-3" /> : i + 1}
								</div>
								{i < STEPS.length - 1 && (
									<div
										className={cn(
											"h-px w-6 transition-colors",
											done ? "bg-primary" : "bg-border",
										)}
									/>
								)}
							</div>
						);
					})}
					<span className="ml-2 text-xs text-muted-foreground">
						{STEPS[stepIdx].label}
					</span>
				</div>

				{/* Content */}
				<div className="px-6 py-5 min-h-[360px]">
					{currentStep === "welcome" && <WelcomeStep />}
					{currentStep === "gateway" && (
						<GatewayStep baseUrl={gatewayUrl} onChange={setGatewayUrl} />
					)}
					{currentStep === "provider" && <ProviderStep />}
					{currentStep === "done" && <DoneStep />}
				</div>

				{/* Footer */}
				<div className="flex items-center justify-between px-6 py-4 border-t bg-muted/20">
					<div className="text-[11px] text-muted-foreground">
						{stepIdx + 1} / {STEPS.length}
					</div>
					<div className="flex items-center gap-2">
						{!isDone && currentStep !== "welcome" && (
							<Button
								variant="ghost"
								size="sm"
								className="h-8 text-xs"
								onClick={handleSkip}
							>
								跳过
							</Button>
						)}
						<Button
							size="sm"
							className="h-8 text-xs gap-1"
							onClick={handleNext}
						>
							{isDone ? "开始使用" : "下一步"}
							{!isDone && <ChevronRight className="size-3.5" />}
						</Button>
					</div>
				</div>
			</div>
		</div>
	);
}
