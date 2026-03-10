/**
 * BoxNotInstalled — shown when `a3s-box` is not found in PATH.
 * Lets the user trigger a Homebrew install and streams the output.
 */
import { installBox } from "@/lib/box-api";
import { cn } from "@/lib/utils";
import { Box, CheckCircle2, Download, ExternalLink, Loader2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";

type InstallState = "idle" | "installing" | "done" | "error";

interface Props {
	onInstalled: () => void;
}

export function BoxNotInstalled({ onInstalled }: Props) {
	const [installState, setInstallState] = useState<InstallState>("idle");
	const [lines, setLines] = useState<string[]>([]);
	const [errorMsg, setErrorMsg] = useState<string | null>(null);
	const logRef = useRef<HTMLDivElement>(null);

	// Auto-scroll log to bottom
	useEffect(() => {
		if (logRef.current) {
			logRef.current.scrollTop = logRef.current.scrollHeight;
		}
	}, [lines]);

	async function handleInstall() {
		setInstallState("installing");
		setLines([]);
		setErrorMsg(null);
		try {
			await installBox((line) => {
				if (line.startsWith("[done]")) {
					setInstallState("done");
					setTimeout(onInstalled, 1200);
				} else if (line.startsWith("[error]")) {
					setErrorMsg(line.replace("[error]", "").trim());
					setInstallState("error");
				} else {
					setLines((prev) => [...prev, line]);
				}
			});
			// If stream ended without [done] marker, treat as done
			setInstallState((s) => (s === "installing" ? "done" : s));
			if (installState === "done") setTimeout(onInstalled, 1200);
		} catch (e) {
			setErrorMsg(e instanceof Error ? e.message : "安装失败");
			setInstallState("error");
		}
	}

	return (
		<div className="flex flex-col items-center justify-center h-full px-8 gap-6">
			{/* Icon */}
			<div className="flex items-center justify-center size-16 rounded-2xl bg-muted">
				<Box className="size-8 text-muted-foreground" />
			</div>

			{/* Title + description */}
			<div className="text-center space-y-1.5 max-w-sm">
				<p className="text-base font-semibold">未检测到 a3s-box</p>
				<p className="text-sm text-muted-foreground leading-relaxed">
					A3S Box 是 SafeClaw 的 MicroVM 运行时，提供硬件级隔离和 TEE 支持。
					安装后可在此管理容器、镜像、网络和存储卷。
				</p>
			</div>

			{/* Install button */}
			{installState === "idle" && (
				<button
					type="button"
					onClick={handleInstall}
					className="flex items-center gap-2 px-5 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors"
				>
					<Download className="size-4" />
					通过 Homebrew 安装
				</button>
			)}

			{installState === "installing" && (
				<button
					type="button"
					disabled
					className="flex items-center gap-2 px-5 py-2 rounded-lg bg-primary/70 text-primary-foreground text-sm font-medium cursor-not-allowed"
				>
					<Loader2 className="size-4 animate-spin" />
					正在安装…
				</button>
			)}

			{installState === "done" && (
				<div className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400 font-medium">
					<CheckCircle2 className="size-4" />
					安装成功，正在加载…
				</div>
			)}

			{installState === "error" && (
				<div className="space-y-2 text-center">
					<p className="text-sm text-destructive">{errorMsg ?? "安装失败"}</p>
					<button
						type="button"
						onClick={handleInstall}
						className="text-sm text-muted-foreground underline underline-offset-2 hover:text-foreground transition-colors"
					>
						重试
					</button>
				</div>
			)}

			{/* Streaming log */}
			{lines.length > 0 && (
				<div
					ref={logRef}
					className={cn(
						"w-full max-w-lg rounded-lg border bg-muted/40 p-3",
						"font-mono text-[11px] leading-relaxed text-muted-foreground",
						"max-h-48 overflow-y-auto",
					)}
				>
					{lines.map((line, i) => (
						// biome-ignore lint/suspicious/noArrayIndexKey: log lines are append-only
						<div key={i}>{line}</div>
					))}
				</div>
			)}

			{/* Manual install link */}
			<a
				href="https://github.com/A3S-Lab/Box/releases"
				target="_blank"
				rel="noreferrer"
				className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
			>
				手动下载安装包
				<ExternalLink className="size-3" />
			</a>
		</div>
	);
}
