import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { cn } from "@/lib/utils";
import agentModel from "@/models/agent.model";
import settingsModel from "@/models/settings.model";
import { sendToSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import {
	Ban,
	ChevronDown,
	Circle,
	CornerDownLeft,
	Cpu,
	Gauge,
	Loader2,
	MessageSquare,
	Sparkles,
} from "lucide-react";
import { useMemo, useState } from "react";
import { useSnapshot } from "valtio";

export function SessionStatusBar({ sessionId }: { sessionId: string }) {
	const settingsSnap = useSnapshot(settingsModel.state);
	const { sessionStatus, sessions, connectionStatus } = useSnapshot(
		agentModel.state,
	);
	const session = sessions[sessionId];
	const pct = Math.round(session?.context_used_percent ?? 0);
	const status = sessionStatus[sessionId] || "idle";
	const connStatus = connectionStatus[sessionId];
	const model = session?.model || settingsSnap.defaultModel || "";
	const [backends, setBackends] = useState<{ id: string; name: string }[]>([]);
	const [modelOpen, setModelOpen] = useState(false);

	const modelShort = useMemo(() => {
		const m = model;
		if (m.includes("opus")) return "Opus";
		if (m.includes("sonnet")) return "Sonnet";
		if (m.includes("haiku")) return "Haiku";
		if (m.includes("gpt-4")) return "GPT-4o";
		if (m.includes("gpt-3")) return "GPT-3.5";
		return m.split("/").pop()?.split("-").slice(0, 2).join("-") || m;
	}, [model]);

	const handleModelClick = async () => {
		if (!modelOpen && backends.length === 0) {
			const result = await agentApi.listBackends().catch(() => []);
			if (Array.isArray(result)) setBackends(result);
		}
		setModelOpen((v) => !v);
	};

	const handleSelectModel = (newModel: string) => {
		sendToSession(sessionId, { type: "set_model", model: newModel });
		setModelOpen(false);
	};

	return (
		<div className="flex items-center gap-3 px-3 py-1.5 border-t bg-muted/30 text-[11px] text-muted-foreground shrink-0 select-none">
			{/* Model switcher */}
			<div className="relative">
				<button
					type="button"
					className="flex items-center gap-1.5 hover:text-foreground transition-colors"
					title={model}
					onClick={handleModelClick}
				>
					<Cpu className="size-3" />
					<span className="font-medium text-foreground/80">
						{modelShort || "—"}
					</span>
					<ChevronDown className="size-2.5 opacity-50" />
				</button>
				{modelOpen && (
					<div className="absolute bottom-full mb-1 left-0 z-50 bg-popover border rounded-lg shadow-lg py-1 min-w-[200px]">
						{backends.length === 0 ? (
							<div className="px-3 py-2 text-xs text-muted-foreground">
								加载中...
							</div>
						) : (
							backends.map((b) => (
								<button
									key={b.id}
									type="button"
									className={cn(
										"w-full text-left px-3 py-1.5 text-xs hover:bg-foreground/[0.04] transition-colors",
										b.id === model && "text-primary font-medium",
									)}
									onClick={() => handleSelectModel(b.id)}
								>
									{b.name || b.id}
								</button>
							))
						)}
					</div>
				)}
			</div>

			<div className="w-px h-3 bg-border" />

			{/* Permission mode */}
			<div className="flex items-center gap-1">
				<Sparkles className="size-3 text-primary" />
				<Select
					value={session?.permission_mode || "default"}
					onValueChange={(mode) =>
						sendToSession(sessionId, { type: "set_permission_mode", mode })
					}
				>
					<SelectTrigger className="h-auto border-0 bg-transparent p-0 text-[11px] text-muted-foreground shadow-none focus:ring-0 gap-0.5 [&>svg]:size-2.5 [&>svg]:opacity-50">
						<SelectValue />
					</SelectTrigger>
					<SelectContent>
						<SelectItem value="default">Agent</SelectItem>
						<SelectItem value="plan">计划模式</SelectItem>
						<SelectItem value="bypassPermissions">跳过权限</SelectItem>
					</SelectContent>
				</Select>
			</div>

			<div className="w-px h-3 bg-border" />

			{/* Session status */}
			<div className="flex items-center gap-1">
				{status === "running" ? (
					<Loader2 className="size-3 text-primary animate-spin" />
				) : status === "compacting" ? (
					<Loader2 className="size-3 text-orange-500 animate-spin" />
				) : (
					<Circle className="size-2.5 fill-green-500 text-green-500" />
				)}
				<span>
					{status === "running"
						? "运行中"
						: status === "compacting"
							? "压缩中"
							: "就绪"}
				</span>
			</div>

			<div className="w-px h-3 bg-border" />

			{/* Context usage */}
			<div className="flex items-center gap-1.5" title={`${pct}% context used`}>
				<Gauge className="size-3" />
				<span>上下文</span>
				<div className="w-16 h-1.5 bg-muted rounded-full overflow-hidden">
					<div
						className={cn(
							"h-full rounded-full transition-all",
							pct >= 80
								? "bg-red-500"
								: pct >= 50
									? "bg-yellow-500"
									: "bg-primary",
						)}
						style={{ width: `${pct}%` }}
					/>
				</div>
				<span className={cn(pct >= 80 && "text-red-500 font-medium")}>
					{pct}%
				</span>
			</div>

			{/* Cost + turns */}
			{(session?.total_cost_usd ?? 0) > 0 && (
				<>
					<div className="w-px h-3 bg-border" />
					<div className="flex items-center gap-1" title="累计成本">
						<span className="text-muted-foreground/70">$</span>
						<span>{session!.total_cost_usd.toFixed(4)}</span>
					</div>
					{(session?.input_tokens ?? 0) > 0 && (
						<span
							className="text-muted-foreground/60 cursor-default"
							title={[
								`输入: ${(session!.input_tokens ?? 0).toLocaleString()} tokens`,
								`输出: ${(session!.output_tokens ?? 0).toLocaleString()} tokens`,
								session?.cache_read_tokens
									? `缓存读: ${session.cache_read_tokens.toLocaleString()} tokens`
									: "",
								session?.cache_write_tokens
									? `缓存写: ${session.cache_write_tokens.toLocaleString()} tokens`
									: "",
							]
								.filter(Boolean)
								.join(" · ")}
						>
							(
							{(
								(session!.input_tokens ?? 0) + (session!.output_tokens ?? 0)
							).toLocaleString()}
							t)
						</span>
					)}
				</>
			)}
			{(session?.num_turns ?? 0) > 0 && (
				<>
					<div className="w-px h-3 bg-border" />
					<div className="flex items-center gap-1" title="对话轮次">
						<MessageSquare className="size-3" />
						<span>{session!.num_turns}</span>
					</div>
				</>
			)}

			{/* Connection status */}
			{connStatus !== "connected" && (
				<>
					<div className="w-px h-3 bg-border" />
					<div className="flex items-center gap-1 text-amber-600 dark:text-amber-400">
						<Loader2
							className={cn(
								"size-3",
								connStatus === "connecting" && "animate-spin",
							)}
						/>
						<span>{connStatus === "connecting" ? "连接中..." : "已断开"}</span>
					</div>
				</>
			)}

			{/* Stop button */}
			{status === "running" && (
				<>
					<div className="w-px h-3 bg-border" />
					<button
						type="button"
						className="flex items-center gap-1 text-destructive hover:text-destructive/80 transition-colors"
						title="中断当前任务"
						onClick={() => sendToSession(sessionId, { type: "interrupt" })}
					>
						<Ban className="size-3" />
						<span>中断</span>
					</button>
				</>
			)}

			{/* Shortcuts hint */}
			<div className="ml-auto flex items-center gap-1 text-muted-foreground/60">
				<CornerDownLeft className="size-3" />
				<span>发送</span>
				<span className="mx-0.5">/</span>
				<span>Shift+Enter 换行</span>
			</div>
		</div>
	);
}
