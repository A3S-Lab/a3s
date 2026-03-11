import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import { sendToSession } from "@/hooks/use-agent-ws";
import { cn } from "@/lib/utils";
import agentModel from "@/models/agent.model";
import {
	Circle,
	Gauge,
	Loader2,
	Sparkles,
} from "lucide-react";
import { useSnapshot } from "valtio";

export function SessionStatusBar({
	sessionId,
	readonlyCwd,
}: { sessionId: string; readonlyCwd?: boolean }) {
	const { sessionStatus, sessions, connectionStatus } = useSnapshot(
		agentModel.state,
	);
	const session = sessions[sessionId];
	const pct = Math.round(session?.context_used_percent ?? 0);
	const status = sessionStatus[sessionId] || "idle";
	const connStatus = connectionStatus[sessionId];

	return (
		<div className="flex items-center gap-3 px-3 py-1.5 border-t bg-muted/30 text-[11px] text-muted-foreground shrink-0 select-none">
			{/* Permission mode */}
			{!readonlyCwd && (
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
			)}

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
		</div>
	);
}
