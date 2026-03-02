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
import {
	connectSession,
	disconnectSession,
	sendToSession,
} from "@/hooks/use-agent-ws";
import { agentApi, type McpServerConfig } from "@/lib/agent-api";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { FolderOpen, Pencil } from "lucide-react";
import { useState, useEffect } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";

function parseKeyValueLines(input: string): Record<string, string> | undefined {
	const lines = input
		.split("\n")
		.map((line) => line.trim())
		.filter(Boolean);
	if (lines.length === 0) return undefined;

	const out: Record<string, string> = {};
	for (const line of lines) {
		const idx = line.indexOf("=");
		if (idx <= 0) continue;
		const key = line.slice(0, idx).trim();
		const value = line.slice(idx + 1).trim();
		if (!key) continue;
		out[key] = value;
	}
	return Object.keys(out).length > 0 ? out : undefined;
}

function parseArgs(input: string): string[] | undefined {
	const args = input
		.split(/\s+/)
		.map((s) => s.trim())
		.filter(Boolean);
	return args.length > 0 ? args : undefined;
}

function toKeyValueLines(input?: Record<string, string>): string {
	if (!input) return "";
	return Object.entries(input)
		.map(([k, v]) => `${k}=${v}`)
		.join("\n");
}

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
	const [mcpLoading, setMcpLoading] = useState(false);
	const [sessionMcpServers, setSessionMcpServers] = useState<McpServerConfig[]>(
		[],
	);
	const [initialSessionMcpServers, setInitialSessionMcpServers] = useState<
		McpServerConfig[]
	>([]);
	const [mcpName, setMcpName] = useState("");
	const [mcpTransport, setMcpTransport] = useState<"stdio" | "http">("stdio");
	const [mcpCommand, setMcpCommand] = useState("");
	const [mcpArgs, setMcpArgs] = useState("");
	const [mcpUrl, setMcpUrl] = useState("");
	const [mcpHeaders, setMcpHeaders] = useState("");
	const [mcpEnv, setMcpEnv] = useState("");
	const [mcpTimeoutSecs, setMcpTimeoutSecs] = useState("60");
	const [mcpEnabled, setMcpEnabled] = useState(true);
	const [mcpEditingName, setMcpEditingName] = useState<string | null>(null);

	const resetMcpForm = () => {
		setMcpName("");
		setMcpTransport("stdio");
		setMcpCommand("");
		setMcpArgs("");
		setMcpUrl("");
		setMcpHeaders("");
		setMcpEnv("");
		setMcpTimeoutSecs("60");
		setMcpEnabled(true);
		setMcpEditingName(null);
	};

	const fillMcpForm = (mcp: McpServerConfig) => {
		setMcpEditingName(mcp.name);
		setMcpName(mcp.name);
		setMcpEnabled(mcp.enabled ?? true);
		setMcpEnv(toKeyValueLines(mcp.env));
		setMcpTimeoutSecs(String(mcp.tool_timeout_secs ?? 60));
		if (mcp.transport.type === "stdio") {
			setMcpTransport("stdio");
			setMcpCommand(mcp.transport.command);
			setMcpArgs((mcp.transport.args ?? []).join(" "));
			setMcpUrl("");
			setMcpHeaders("");
		} else {
			setMcpTransport("http");
			setMcpUrl(mcp.transport.url);
			setMcpHeaders(toKeyValueLines(mcp.transport.headers));
			setMcpCommand("");
			setMcpArgs("");
		}
	};

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

	useEffect(() => {
		if (!open) return;
		setMcpLoading(true);
		agentApi
			.getSessionMcpServers(sessionId)
			.then((configs) => {
				const list = Array.isArray(configs) ? configs : [];
				setSessionMcpServers(list);
				setInitialSessionMcpServers(list);
			})
			.catch(() => {
				setSessionMcpServers([]);
				setInitialSessionMcpServers([]);
			})
			.finally(() => setMcpLoading(false));
	}, [open, sessionId]);

	const addSessionMcp = () => {
		const timeout = Number.parseInt(mcpTimeoutSecs, 10);
		const next: McpServerConfig = {
			name: mcpName.trim(),
			transport:
				mcpTransport === "stdio"
					? {
							type: "stdio",
							command: mcpCommand.trim(),
							args: parseArgs(mcpArgs),
						}
					: {
							type: "http",
							url: mcpUrl.trim(),
							headers: parseKeyValueLines(mcpHeaders),
						},
			env: parseKeyValueLines(mcpEnv),
			tool_timeout_secs: Number.isFinite(timeout) && timeout > 0 ? timeout : 60,
			enabled: mcpEnabled,
		};

		if (!next.name) {
			toast.error("请填写 MCP 服务名");
			return;
		}
		if (next.transport.type === "stdio" && !next.transport.command.trim()) {
			toast.error("请填写 MCP Command");
			return;
		}
		if (next.transport.type === "http" && !next.transport.url.trim()) {
			toast.error("请填写 MCP URL");
			return;
		}

		setSessionMcpServers((prev) => {
			const filtered = prev.filter(
				(s) => s.name !== next.name && s.name !== mcpEditingName,
			);
			return [...filtered, next];
		});
		resetMcpForm();
	};

	const handleApply = async () => {
		const mcpChanged =
			JSON.stringify(sessionMcpServers) !==
			JSON.stringify(initialSessionMcpServers);
		const cwdChanged = cwd !== session?.cwd && !!cwd.trim();

		if (model !== session?.model)
			sendToSession(sessionId, { type: "set_model", model });
		if (permMode !== session?.permission_mode)
			sendToSession(sessionId, { type: "set_permission_mode", mode: permMode });
		if (systemPrompt !== persona?.systemPrompt)
			sendToSession(sessionId, {
				type: "set_system_prompt",
				system_prompt: systemPrompt,
			});

		if (mcpChanged) {
			try {
				await agentApi.setSessionMcpServers(sessionId, sessionMcpServers);
			} catch {
				toast.error("会话 MCP 配置保存失败");
				return;
			}
		}

		if (cwdChanged) {
			try {
				await agentApi.updateSession(sessionId, { cwd: cwd.trim() });
			} catch {
				toast.error("工作目录更新失败");
				return;
			}
		}

		if (cwdChanged || mcpChanged) {
			try {
				const result = await agentApi.relaunchSession(sessionId);
				if (result?.session_id) {
					const sessions = await agentApi.listSessions();
					if (Array.isArray(sessions)) agentModel.setSdkSessions(sessions);
					disconnectSession(sessionId);
					connectSession(result.session_id);
					agentModel.setCurrentSession(result.session_id);
					const pid = personaModel.state.sessionPersonas[sessionId];
					if (pid) personaModel.setSessionPersona(result.session_id, pid);
				}
				toast.success(
					cwdChanged
						? "工作目录 / MCP 配置已更新，会话已重启"
						: "MCP 配置已更新，会话已重启",
				);
			} catch {
				toast.error("会话重启失败，请稍后重试");
				return;
			}
		} else {
			toast.success("配置已更新");
		}
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
						<div className="flex gap-1.5">
							<Input
								value={cwd}
								readOnly
								className="h-8 text-xs font-mono flex-1 cursor-default"
								placeholder="/path/to/workspace"
							/>
							<button
								type="button"
								className="flex items-center justify-center size-8 rounded-md border text-muted-foreground hover:text-foreground hover:bg-foreground/[0.04] transition-colors shrink-0"
								aria-label="选择目录"
								onClick={async () => {
									const selected = await openDialog({
										directory: true,
										multiple: false,
									});
									if (typeof selected === "string") setCwd(selected);
								}}
							>
								<FolderOpen className="size-3.5" />
							</button>
						</div>
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

					<div className="space-y-2">
						<Label className="text-xs font-medium">会话 MCP 服务</Label>
						{mcpLoading ? (
							<div className="text-[11px] text-muted-foreground">加载中...</div>
						) : (
							<div className="space-y-1.5">
								{sessionMcpServers.length === 0 ? (
									<div className="rounded-md border border-dashed px-2.5 py-2 text-[11px] text-muted-foreground">
										未配置会话级 MCP 服务
									</div>
								) : (
									sessionMcpServers.map((mcp) => (
										<div
											key={mcp.name}
											className="rounded-md border px-2.5 py-2 text-[11px]"
										>
											<div className="flex items-center justify-between gap-2">
												<span className="font-medium font-mono">
													{mcp.name}
												</span>
												<div className="flex items-center gap-2">
													<button
														type="button"
														className="text-muted-foreground hover:text-foreground"
														onClick={() => fillMcpForm(mcp)}
													>
														<Pencil className="size-3" />
													</button>
													<button
														type="button"
														className="text-muted-foreground hover:text-destructive"
														onClick={() =>
															setSessionMcpServers((prev) =>
																prev.filter((x) => x.name !== mcp.name),
															)
														}
													>
														删除
													</button>
												</div>
											</div>
											<div className="text-muted-foreground font-mono">
												{mcp.transport.type === "stdio"
													? `${mcp.transport.command} ${(mcp.transport.args || []).join(" ")}`
													: mcp.transport.url}
												{mcp.enabled === false ? " · 已禁用" : ""}
											</div>
										</div>
									))
								)}
							</div>
						)}

						<div className="rounded-md border bg-muted/20 p-2 space-y-2">
							<div className="flex items-center justify-between gap-2">
								<span className="text-[11px] text-muted-foreground">
									{mcpEditingName ? `编辑 ${mcpEditingName}` : "新增会话 MCP"}
								</span>
								{mcpEditingName && (
									<button
										type="button"
										className="text-[11px] text-muted-foreground hover:text-foreground"
										onClick={resetMcpForm}
									>
										取消编辑
									</button>
								)}
							</div>
							<div className="grid grid-cols-2 gap-2">
								<Input
									value={mcpName}
									onChange={(e) => setMcpName(e.target.value)}
									className="h-7 text-[11px] font-mono"
									placeholder="服务名"
								/>
								<Select
									value={mcpTransport}
									onValueChange={(v) => setMcpTransport(v as "stdio" | "http")}
								>
									<SelectTrigger className="h-7 text-[11px]">
										<SelectValue />
									</SelectTrigger>
									<SelectContent>
										<SelectItem value="stdio" className="text-[11px]">
											stdio
										</SelectItem>
										<SelectItem value="http" className="text-[11px]">
											http
										</SelectItem>
									</SelectContent>
								</Select>
							</div>

							{mcpTransport === "stdio" ? (
								<div className="grid grid-cols-2 gap-2">
									<Input
										value={mcpCommand}
										onChange={(e) => setMcpCommand(e.target.value)}
										className="h-7 text-[11px] font-mono"
										placeholder="Command"
									/>
									<Input
										value={mcpArgs}
										onChange={(e) => setMcpArgs(e.target.value)}
										className="h-7 text-[11px] font-mono"
										placeholder="Args"
									/>
								</div>
							) : (
								<Input
									value={mcpUrl}
									onChange={(e) => setMcpUrl(e.target.value)}
									className="h-7 text-[11px] font-mono"
									placeholder="URL"
								/>
							)}

							<div className="grid grid-cols-2 gap-2">
								<textarea
									value={mcpHeaders}
									onChange={(e) => setMcpHeaders(e.target.value)}
									className="w-full rounded-md border bg-background px-2 py-1.5 text-[11px] font-mono min-h-[56px]"
									placeholder="Headers KEY=VALUE"
								/>
								<textarea
									value={mcpEnv}
									onChange={(e) => setMcpEnv(e.target.value)}
									className="w-full rounded-md border bg-background px-2 py-1.5 text-[11px] font-mono min-h-[56px]"
									placeholder="Env KEY=VALUE"
								/>
							</div>

							<div className="flex items-center justify-between gap-2">
								<button
									type="button"
									className="rounded-md border px-2 py-1 text-[11px] hover:bg-foreground/[0.04]"
									onClick={() => setMcpEnabled((v) => !v)}
								>
									{mcpEnabled ? "已启用" : "已禁用"}
								</button>
								<Input
									type="number"
									min={1}
									value={mcpTimeoutSecs}
									onChange={(e) => setMcpTimeoutSecs(e.target.value)}
									className="h-7 w-24 text-[11px]"
									placeholder="Timeout"
								/>
								<button
									type="button"
									className="rounded-md border px-2 py-1 text-[11px] hover:bg-foreground/[0.04]"
									onClick={addSessionMcp}
								>
									{mcpEditingName ? "保存 MCP" : "添加 MCP"}
								</button>
							</div>
						</div>
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
