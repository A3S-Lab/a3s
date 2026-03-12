/**
 * BoxNotInstalled — shown when `a3s-box` is not found.
 * Downloads the latest release from GitHub and streams install progress.
 */
import { installBox } from "@/lib/box-api";
import { cn } from "@/lib/utils";
import {
	Box,
	CheckCircle2,
	Download,
	ExternalLink,
	Loader2,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";

type InstallState = "idle" | "installing" | "done" | "error";

interface Progress {
	downloaded: number;
	total: number;
}

interface Props {
	onInstalled: () => void;
}

export function BoxNotInstalled({ onInstalled }: Props) {
	const [installState, setInstallState] = useState<InstallState>("idle");
	const [stage, setStage] = useState<string>("");
	const [progress, setProgress] = useState<Progress | null>(null);
	const [errorMsg, setErrorMsg] = useState<string | null>(null);
	const [lines, setLines] = useState<string[]>([]);
	const logRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		if (logRef.current) {
			logRef.current.scrollTop = logRef.current.scrollHeight;
		}
	}, [lines]);

	async function handleInstall() {
		setInstallState("installing");
		setStage("");
		setProgress(null);
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
				} else if (line.startsWith("[progress:")) {
					// Structured progress: [progress:downloaded:total]
					const parts = line
						.slice("[progress:".length)
						.replace("]", "")
						.split(":");
					const downloaded = Number(parts[0]);
					const total = Number(parts[1]);
					setProgress({ downloaded, total });
				} else if (line.trim()) {
					setStage(line.trim());
					setLines((prev) => [...prev, line]);
				}
			});
			setInstallState((s) => (s === "installing" ? "done" : s));
		} catch (e) {
			setErrorMsg(e instanceof Error ? e.message : "安装失败");
			setInstallState("error");
		}
	}

	const pct =
		progress && progress.total > 0
			? Math.min(100, (progress.downloaded / progress.total) * 100)
			: null;

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
					点击下方按钮可从 GitHub
					自动下载并安装，完成后即可在此管理容器、镜像、网络和存储卷。
				</p>
			</div>

			{/* Idle: install button */}
			{installState === "idle" && (
				<button
					type="button"
					onClick={handleInstall}
					className="flex items-center gap-2 px-5 py-2 rounded-lg bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors"
				>
					<Download className="size-4" />
					下载并安装
				</button>
			)}

			{/* Installing: stage text + progress bar */}
			{installState === "installing" && (
				<div className="w-full max-w-sm space-y-3">
					<div className="flex items-center gap-2 text-sm text-muted-foreground">
						<Loader2 className="size-3.5 animate-spin shrink-0" />
						<span className="truncate">{stage || "正在准备…"}</span>
					</div>

					{progress !== null && (
						<div className="space-y-1.5">
							{/* Bar */}
							<div className="w-full h-1.5 rounded-full bg-muted overflow-hidden">
								{pct !== null ? (
									<div
										className="h-full bg-primary rounded-full transition-all duration-150 ease-out"
										style={{ width: `${pct}%` }}
									/>
								) : (
									// Indeterminate stripe when total is unknown
									<div className="h-full w-1/3 bg-primary rounded-full animate-slide" />
								)}
							</div>
							{/* Byte counts */}
							<div className="flex justify-between text-[11px] text-muted-foreground font-mono">
								<span>{(progress.downloaded / 1e6).toFixed(1)} MB</span>
								{progress.total > 0 && (
									<span>
										{pct !== null ? `${pct.toFixed(0)}%` : ""} /{" "}
										{(progress.total / 1e6).toFixed(1)} MB
									</span>
								)}
							</div>
						</div>
					)}
				</div>
			)}

			{/* Done */}
			{installState === "done" && (
				<div className="flex items-center gap-2 text-sm text-green-600 dark:text-green-400 font-medium">
					<CheckCircle2 className="size-4" />
					安装成功，正在加载…
				</div>
			)}

			{/* Error */}
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

			{/* Log (non-progress lines) */}
			{lines.length > 0 && installState !== "done" && (
				<div
					ref={logRef}
					className={cn(
						"w-full max-w-sm rounded-lg border bg-muted/40 p-3",
						"font-mono text-[11px] leading-relaxed text-muted-foreground",
						"max-h-32 overflow-y-auto",
					)}
				>
					{lines.map((line, i) => (
						// biome-ignore lint/suspicious/noArrayIndexKey: append-only log
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
