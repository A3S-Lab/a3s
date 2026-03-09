import { Info, ShieldCheck } from "lucide-react";
import { SectionHeader } from "./shared";

const INFO_ITEMS = [
	{ label: "应用名称", value: "SafeClaw" },
	{ label: "版本", value: "0.1.0" },
	{ label: "运行时", value: "Tauri v2 + React 18" },
	{ label: "许可证", value: "MIT" },
];

export function AboutSection() {
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
		</div>
	);
}
