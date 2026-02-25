import { cn } from "@/lib/utils";
import { Loader2, Volume2 } from "lucide-react";
import { useState } from "react";

interface TtsToggleProps {
	ttsEnabled: boolean;
	isSpeaking: boolean;
	modelsReady: boolean;
	isDownloading: boolean;
	downloadProgress: { percent: number; lang: string } | null;
	disabled?: boolean;
	onToggle: () => void;
	onStop: () => void;
	onDownload: () => void;
}

export function TtsToggle({
	ttsEnabled,
	isSpeaking,
	modelsReady,
	isDownloading,
	downloadProgress,
	disabled,
	onToggle,
	onStop,
	onDownload,
}: TtsToggleProps) {
	const [showPopover, setShowPopover] = useState(false);

	const handleClick = () => {
		if (isSpeaking) {
			onStop();
			return;
		}

		if (!modelsReady && !isDownloading) {
			setShowPopover(true);
			return;
		}

		onToggle();
	};

	const handleDownload = () => {
		setShowPopover(false);
		onDownload();
	};

	return (
		<div className="relative">
			<button
				type="button"
				className={cn(
					"flex items-center justify-center size-8 rounded-full transition-colors",
					isSpeaking
						? "text-primary bg-primary/10"
						: ttsEnabled
							? "text-primary hover:bg-primary/10"
							: "text-muted-foreground hover:text-foreground hover:bg-foreground/[0.06]",
					disabled && "opacity-40 cursor-not-allowed",
				)}
				title={
					isDownloading
						? `下载中 ${downloadProgress?.percent ?? 0}%`
						: isSpeaking
							? "停止朗读"
							: ttsEnabled
								? "关闭朗读"
								: "开启朗读"
				}
				onClick={handleClick}
				disabled={disabled || isDownloading}
			>
				{isDownloading ? (
					<Loader2 className="size-[18px] animate-spin" />
				) : isSpeaking ? (
					<SpeakingIcon />
				) : ttsEnabled ? (
					<Volume2 className="size-[18px]" />
				) : (
					<VolumeX className="size-[18px]" />
				)}
			</button>

			{/* Download popover */}
			{showPopover && (
				<>
					{/* Backdrop */}
					<div
						className="fixed inset-0 z-30"
						onClick={() => setShowPopover(false)}
						onKeyDown={() => {}}
					/>
					<div className="absolute bottom-full left-0 mb-2 z-40 w-64 rounded-lg border bg-popover p-3 shadow-md text-popover-foreground">
						<p className="text-xs font-medium mb-1">
							需要下载语音模型
						</p>
						<p className="text-[11px] text-muted-foreground mb-3">
							首次使用需下载中文和英文语音模型（约 60MB），下载后可离线使用。
						</p>
						<div className="flex gap-2 justify-end">
							<button
								type="button"
								className="rounded-md px-2.5 py-1 text-xs text-muted-foreground hover:bg-muted transition-colors"
								onClick={() => setShowPopover(false)}
							>
								取消
							</button>
							<button
								type="button"
								className="rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
								onClick={handleDownload}
							>
								下载
							</button>
						</div>
					</div>
				</>
			)}
		</div>
	);
}

/** Animated sound wave icon for speaking state */
function SpeakingIcon() {
	return (
		<svg
			className="size-[18px] text-primary"
			viewBox="0 0 16 16"
			fill="none"
			aria-hidden="true"
		>
			<title>Speaking</title>
			<rect
				x="2"
				y="6"
				width="2"
				height="4"
				rx="1"
				fill="currentColor"
				className="animate-[tts-bar_0.6s_ease-in-out_infinite]"
			/>
			<rect
				x="5.5"
				y="4"
				width="2"
				height="8"
				rx="1"
				fill="currentColor"
				className="animate-[tts-bar_0.6s_ease-in-out_0.15s_infinite]"
			/>
			<rect
				x="9"
				y="5"
				width="2"
				height="6"
				rx="1"
				fill="currentColor"
				className="animate-[tts-bar_0.6s_ease-in-out_0.3s_infinite]"
			/>
			<rect
				x="12.5"
				y="6"
				width="2"
				height="4"
				rx="1"
				fill="currentColor"
				className="animate-[tts-bar_0.6s_ease-in-out_0.45s_infinite]"
			/>
		</svg>
	);
}

function VolumeX({ className }: { className?: string }) {
	return (
		<svg
			className={className}
			viewBox="0 0 24 24"
			fill="none"
			stroke="currentColor"
			strokeWidth="2"
			strokeLinecap="round"
			strokeLinejoin="round"
			aria-hidden="true"
		>
			<polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
			<line x1="23" y1="9" x2="17" y2="15" />
			<line x1="17" y1="9" x2="23" y2="15" />
		</svg>
	);
}
