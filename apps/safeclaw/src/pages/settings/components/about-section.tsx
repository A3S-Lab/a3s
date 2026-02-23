import { Button } from "@/components/ui/button";
import { useModal } from "@/components/custom/modal-provider";
import settingsModel from "@/models/settings.model";
import { Info, Layers, RotateCcw, ShieldCheck } from "lucide-react";
import { toast } from "sonner";
import { SectionHeader } from "./shared";

const INFO_ITEMS = [
	{ label: "应用名称", value: "SafeClaw" },
	{ label: "版本", value: "0.1.0" },
	{ label: "运行时", value: "Tauri v2 + React 19" },
	{ label: "TEE 支持", value: "Intel SGX / TDX" },
	{ label: "许可证", value: "Apache-2.0" },
];

export function AboutSection() {
	const modal = useModal();
	return (
		<div>
			<SectionHeader
				icon={Info}
				title="关于"
				description="应用信息与数据管理。"
			/>
			<div className="rounded-xl border bg-card p-5 mb-4">
				<div className="flex items-center gap-3 mb-4">
					<div className="flex items-center justify-center size-12 rounded-xl bg-primary/10">
						<ShieldCheck className="size-6 text-primary" />
					</div>
					<div>
						<div className="text-base font-bold">SafeClaw</div>
						<div className="text-xs text-muted-foreground">
							Secure Personal AI Assistant with TEE Support
						</div>
					</div>
				</div>
				<div className="rounded-lg bg-muted/30 divide-y divide-border/50">
					{INFO_ITEMS.map((item) => (
						<div
							key={item.label}
							className="flex justify-between items-center px-4 py-2.5"
						>
							<span className="text-xs text-muted-foreground">
								{item.label}
							</span>
							<span className="text-xs font-medium font-mono">
								{item.value}
							</span>
						</div>
					))}
				</div>
			</div>
			<div className="rounded-xl border bg-card p-5 mb-4">
				<div className="flex items-center gap-2 mb-3">
					<Layers className="size-4 text-primary" />
					<span className="text-sm font-semibold">技术栈</span>
				</div>
				<div className="flex flex-wrap gap-1.5">
					{[
						"Rust",
						"Tauri v2",
						"React 19",
						"TypeScript",
						"Tailwind CSS",
						"Valtio",
						"gRPC",
						"Intel SGX",
						"RA-TLS",
					].map((t) => (
						<span
							key={t}
							className="inline-flex items-center rounded-md border bg-muted/50 px-2 py-0.5 text-[11px] font-medium text-muted-foreground"
						>
							{t}
						</span>
					))}
				</div>
			</div>
			<div className="rounded-xl border border-destructive/20 bg-destructive/[0.03] p-5">
				<div className="flex items-center gap-2 mb-2">
					<RotateCcw className="size-4 text-destructive" />
					<span className="text-sm font-semibold text-destructive">
						危险操作
					</span>
				</div>
				<p className="text-xs text-muted-foreground mb-3">
					重置后所有配置将恢复为默认值，包括所有 Provider、模型和网关地址。
				</p>
				<Button
					variant="destructive"
					size="sm"
					onClick={() => {
						modal.alert({
							title: "重置设置",
							description: "确认重置所有设置为默认值？此操作不可撤销。",
							confirmText: "重置",
							onConfirm: () => {
								settingsModel.resetSettings();
								toast.success("设置已重置");
								setTimeout(() => window.location.reload(), 500);
							},
						});
					}}
				>
					<RotateCcw className="size-3.5 mr-1.5" />
					重置所有设置
				</Button>
			</div>
		</div>
	);
}
