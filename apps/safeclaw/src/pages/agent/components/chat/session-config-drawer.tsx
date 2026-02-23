import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@/components/ui/select";
import {
	Sheet,
	SheetContent,
	SheetHeader,
	SheetTitle,
} from "@/components/ui/sheet";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { sendToSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import { useState, useEffect } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

export function SessionConfigDrawer({
	sessionId,
	open,
	onClose,
}: { sessionId: string; open: boolean; onClose: () => void }) {
	const { sessions } = useSnapshot(agentModel.state);
	const session = sessions[sessionId];
	const persona = personaModel.getSessionPersona(sessionId);
	const [model, setModel] = useState(session?.model ?? "");
	const [cwd, setCwd] = useState(session?.cwd ?? "");
	const [permMode, setPermMode] = useState(
		session?.permission_mode ?? "default",
	);
	const [systemPrompt, setSystemPrompt] = useState(persona?.systemPrompt ?? "");
	const [backends, setBackends] = useState<{ id: string; name: string }[]>([]);

	useEffect(() => {
		if (open && backends.length === 0) {
			agentApi
				.listBackends()
				.then((r) => {
					if (Array.isArray(r)) setBackends(r);
				})
				.catch(() => {});
		}
	}, [open]);

	useEffect(() => {
		if (session) {
			setModel(session.model ?? "");
			setCwd(session.cwd ?? "");
			setPermMode(session.permission_mode ?? "default");
		}
	}, [session?.model, session?.cwd, session?.permission_mode]);

	const handleApply = () => {
		if (model !== session?.model)
			sendToSession(sessionId, { type: "set_model", model });
		if (permMode !== session?.permission_mode)
			sendToSession(sessionId, { type: "set_permission_mode", mode: permMode });
		if (systemPrompt !== persona?.systemPrompt)
			sendToSession(sessionId, {
				type: "set_system_prompt",
				system_prompt: systemPrompt,
			});
		toast.success("配置已更新");
		onClose();
	};

	return (
		<Sheet
			open={open}
			onOpenChange={(v) => {
				if (!v) onClose();
			}}
		>
			<SheetContent side="right" className="w-80 flex flex-col gap-0 p-0">
				<SheetHeader className="px-5 py-4 border-b">
					<SheetTitle className="text-sm font-semibold">会话配置</SheetTitle>
				</SheetHeader>
				<div className="flex-1 overflow-y-auto px-5 py-4 space-y-5">
					<div className="space-y-1.5">
						<Label className="text-xs font-medium">模型</Label>
						{backends.length > 0 ? (
							<Select value={model} onValueChange={setModel}>
								<SelectTrigger className="h-8 text-xs">
									<SelectValue placeholder="选择模型" />
								</SelectTrigger>
								<SelectContent>
									{backends.map((b) => (
										<SelectItem key={b.id} value={b.id} className="text-xs">
											{b.name || b.id}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						) : (
							<Input
								value={model}
								onChange={(e) => setModel(e.target.value)}
								className="h-8 text-xs font-mono"
								placeholder="模型 ID"
							/>
						)}
					</div>

					<div className="space-y-1.5">
						<Label className="text-xs font-medium">权限模式</Label>
						<Select value={permMode} onValueChange={setPermMode}>
							<SelectTrigger className="h-8 text-xs">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="default" className="text-xs">
									Agent（默认）
								</SelectItem>
								<SelectItem value="plan" className="text-xs">
									计划模式
								</SelectItem>
								<SelectItem value="bypassPermissions" className="text-xs">
									跳过权限
								</SelectItem>
							</SelectContent>
						</Select>
					</div>

					<div className="space-y-1.5">
						<Label className="text-xs font-medium">工作目录</Label>
						<Input
							value={cwd}
							onChange={(e) => setCwd(e.target.value)}
							className="h-8 text-xs font-mono"
							placeholder="/path/to/workspace"
						/>
						<p className="text-[10px] text-muted-foreground">
							工作目录变更需重启会话后生效
						</p>
					</div>

					<div className="space-y-1.5">
						<Label className="text-xs font-medium">系统提示词</Label>
						<textarea
							value={systemPrompt}
							onChange={(e) => setSystemPrompt(e.target.value)}
							className="w-full rounded-md border bg-background px-3 py-2 text-xs font-mono min-h-[80px] resize-y focus:outline-none focus:ring-1 focus:ring-ring"
							placeholder="输入系统提示词..."
						/>
					</div>

					<div className="space-y-1.5">
						<Label className="text-xs font-medium text-muted-foreground">
							会话信息
						</Label>
						<div className="rounded-lg bg-muted/40 px-3 py-2.5 space-y-1.5 text-[11px] text-muted-foreground font-mono">
							<div className="flex justify-between">
								<span>ID</span>
								<span className="text-foreground/70">
									{sessionId.slice(0, 12)}…
								</span>
							</div>
							{(session?.num_turns ?? 0) > 0 && (
								<div className="flex justify-between">
									<span>轮次</span>
									<span className="text-foreground/70">
										{session!.num_turns}
									</span>
								</div>
							)}
							{(session?.total_cost_usd ?? 0) > 0 && (
								<div className="flex justify-between">
									<span>成本</span>
									<span className="text-foreground/70">
										${session!.total_cost_usd.toFixed(4)}
									</span>
								</div>
							)}
							{(session?.context_used_percent ?? 0) > 0 && (
								<div className="flex justify-between">
									<span>上下文</span>
									<span className="text-foreground/70">
										{Math.round(session!.context_used_percent)}%
									</span>
								</div>
							)}
						</div>
					</div>

					{session?.tools && session.tools.length > 0 && (
						<div className="space-y-1.5">
							<Label className="text-xs font-medium text-muted-foreground">
								可用工具 ({session.tools.length})
							</Label>
							<div className="rounded-lg bg-muted/40 px-3 py-2.5 flex flex-wrap gap-1">
								{session.tools.map((tool) => (
									<span
										key={tool}
										className="inline-block rounded border bg-background px-1.5 py-0.5 text-[10px] font-mono text-muted-foreground"
									>
										{tool}
									</span>
								))}
							</div>
						</div>
					)}
				</div>

				<div className="px-5 py-3 border-t flex items-center gap-2">
					<button
						type="button"
						className="flex-1 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
						onClick={handleApply}
					>
						应用
					</button>
					<button
						type="button"
						className="rounded-md border px-3 py-1.5 text-xs text-muted-foreground hover:bg-foreground/[0.04] transition-colors"
						onClick={onClose}
					>
						取消
					</button>
				</div>
			</SheetContent>
		</Sheet>
	);
}
