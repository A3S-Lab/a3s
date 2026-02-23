import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { Eye, Fingerprint, FlaskConical, Lock } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { scanPrivacy } from "@/lib/security-api";
import { SectionHeader } from "./shared";

const SENSITIVITY_COLORS: Record<string, string> = {
	Public: "bg-gray-500/10 text-gray-600 dark:text-gray-400 border-gray-500/20",
	Normal: "bg-primary/10 text-primary border-primary/20",
	Sensitive:
		"bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
	HighlySensitive:
		"bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/20",
	Critical: "bg-destructive/10 text-destructive border-destructive/20",
};

const SENSITIVITY_LABELS: Record<string, string> = {
	Public: "公开",
	Normal: "普通",
	Sensitive: "敏感",
	HighlySensitive: "高度敏感",
	Critical: "关键",
};

export function PrivacySection() {
	const [input, setInput] = useState("");
	const [results, setResults] = useState<
		Array<{
			type: string;
			sensitivity: string;
			redacted: boolean;
			snippet: string;
		}>
	>([]);
	const [scanning, setScanning] = useState(false);

	const handleScan = async () => {
		if (!input.trim()) return;
		setScanning(true);
		try {
			const data = await scanPrivacy(input.trim());
			const detections = Array.isArray(data)
				? data
				: (data?.detections ?? data?.results ?? []);
			setResults(detections);
			if (detections.length > 0) {
				toast.success(`扫描完成，检测到 ${detections.length} 项 PII`);
			} else {
				toast.success("扫描完成，未检测到 PII");
			}
		} catch (e) {
			toast.error(`扫描失败：${e instanceof Error ? e.message : "后端不可用"}`);
		} finally {
			setScanning(false);
		}
	};

	return (
		<div>
			<SectionHeader
				icon={Fingerprint}
				title="隐私扫描"
				description="检测文本中的个人身份信息 (PII)，支持正则 + 语义分析。"
			/>

			<div className="rounded-xl border bg-card p-5 mb-4">
				<div className="text-sm font-semibold mb-3">文本扫描</div>
				<textarea
					className="w-full rounded-lg border bg-background px-3 py-2 text-sm min-h-[100px] resize-y focus:outline-none focus:ring-2 focus:ring-ring"
					placeholder={
						"输入或粘贴文本进行 PII 检测...\n\n示例: My credit card is 4111-1111-1111-1111 and my SSN is 123-45-6789"
					}
					value={input}
					onChange={(e) => setInput(e.target.value)}
				/>
				<div className="flex items-center gap-3 mt-3">
					<Button
						size="sm"
						onClick={handleScan}
						disabled={scanning || !input.trim()}
					>
						<FlaskConical className="size-3.5 mr-1.5" />
						{scanning ? "扫描中..." : "开始扫描"}
					</Button>
					<span className="text-[11px] text-muted-foreground">
						支持: 信用卡、SSN、邮箱、电话、API Key、自然语言 PII
					</span>
				</div>
			</div>

			{results.length > 0 && (
				<div className="rounded-xl border bg-card p-5">
					<div className="flex items-center gap-2 mb-4">
						<Eye className="size-4 text-primary" />
						<span className="text-sm font-semibold">检测结果</span>
						<span className="text-[10px] rounded-full px-1.5 py-0.5 bg-primary/20 text-primary font-medium">
							{results.length}
						</span>
					</div>
					<div className="space-y-2">
						{results.map((r, i) => (
							<div
								key={i}
								className="flex items-center gap-3 rounded-lg border px-4 py-3"
							>
								<Fingerprint className="size-4 text-muted-foreground shrink-0" />
								<div className="flex-1 min-w-0">
									<div className="flex items-center gap-2">
										<span className="text-sm font-medium">{r.type}</span>
										<span
											className={cn(
												"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-medium",
												SENSITIVITY_COLORS[r.sensitivity],
											)}
										>
											{SENSITIVITY_LABELS[r.sensitivity]}
										</span>
										{r.redacted && (
											<span className="inline-flex items-center gap-1 text-[10px] text-green-600 dark:text-green-400 font-medium">
												<Lock className="size-3" />
												已脱敏
											</span>
										)}
									</div>
									<span className="text-[11px] font-mono text-muted-foreground mt-0.5">
										{r.snippet}
									</span>
								</div>
							</div>
						))}
					</div>
				</div>
			)}
		</div>
	);
}
