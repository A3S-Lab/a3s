import { Button } from "@/components/ui/button";
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
	agentApi,
	type McpServerConfig,
	type McpServerStatus,
} from "@/lib/agent-api";
import { Pencil, PlugZap, Plus, RefreshCcw, Trash2 } from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { SectionHeader } from "./shared";

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

function toArgsText(input?: string[]): string {
	return input?.join(" ") ?? "";
}

export function McpSection() {
	const [loading, setLoading] = useState(false);
	const [servers, setServers] = useState<Record<string, McpServerStatus>>({});
	const [globalConfigs, setGlobalConfigs] = useState<McpServerConfig[]>([]);
	const [statusFilter, setStatusFilter] = useState<
		"all" | "enabled" | "connected"
	>("all");

	const [name, setName] = useState("");
	const [transport, setTransport] = useState<"stdio" | "http">("stdio");
	const [command, setCommand] = useState("");
	const [args, setArgs] = useState("");
	const [url, setUrl] = useState("");
	const [headersInput, setHeadersInput] = useState("");
	const [envInput, setEnvInput] = useState("");
	const [timeoutSecs, setTimeoutSecs] = useState("60");
	const [editingName, setEditingName] = useState<string | null>(null);

	const resetForm = useCallback(() => {
		setName("");
		setTransport("stdio");
		setCommand("");
		setArgs("");
		setUrl("");
		setHeadersInput("");
		setEnvInput("");
		setTimeoutSecs("60");
		setEditingName(null);
	}, []);

	const fillFormFromConfig = useCallback((config: McpServerConfig) => {
		setName(config.name);
		setEnvInput(toKeyValueLines(config.env));
		setTimeoutSecs(String(config.tool_timeout_secs ?? 60));
		if (config.transport.type === "stdio") {
			setTransport("stdio");
			setCommand(config.transport.command);
			setArgs(toArgsText(config.transport.args));
			setUrl("");
			setHeadersInput("");
		} else {
			setTransport("http");
			setUrl(config.transport.url);
			setHeadersInput(toKeyValueLines(config.transport.headers));
			setCommand("");
			setArgs("");
		}
	}, []);

	const refresh = useCallback(async () => {
		setLoading(true);
		try {
			const [status, config] = await Promise.all([
				agentApi.listMcpServers(),
				agentApi.fetchConfig().catch(() => ({})),
			]);
			setServers(status || {});
			setGlobalConfigs(
				Array.isArray(config?.mcp_servers) ? config.mcp_servers : [],
			);
		} catch {
			toast.error("加载 MCP 服务状态失败");
		} finally {
			setLoading(false);
		}
	}, []);

	useEffect(() => {
		refresh();
	}, [refresh]);

	const canAdd = useMemo(() => {
		if (!name.trim()) return false;
		if (transport === "stdio") return !!command.trim();
		return !!url.trim();
	}, [name, transport, command, url]);

	const handleSave = async () => {
		const timeout = Number.parseInt(timeoutSecs, 10);
		const config: McpServerConfig = {
			name: name.trim(),
			transport:
				transport === "stdio"
					? {
							type: "stdio",
							command: command.trim(),
							args: parseArgs(args),
						}
					: {
							type: "http",
							url: url.trim(),
							headers: parseKeyValueLines(headersInput),
						},
			env: parseKeyValueLines(envInput),
			tool_timeout_secs: Number.isFinite(timeout) && timeout > 0 ? timeout : 60,
			enabled: true,
		};

		try {
			if (editingName) {
				await agentApi.removeMcpServer(editingName);
			}
			await agentApi.addMcpServer(config);
			toast.success(
				editingName
					? `MCP 服务 ${editingName} 已更新`
					: `MCP 服务 ${config.name} 已添加`,
			);
			resetForm();
			await refresh();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : "保存 MCP 服务失败");
		}
	};

	const handleRemove = async (serverName: string) => {
		try {
			await agentApi.removeMcpServer(serverName);
			toast.success(`MCP 服务 ${serverName} 已移除`);
			await refresh();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : "移除 MCP 服务失败");
		}
	};

	const handleToggleEnabled = async (
		config: McpServerConfig,
		enabled: boolean,
	) => {
		try {
			await agentApi.addMcpServer({ ...config, enabled });
			toast.success(`MCP 服务 ${config.name} 已${enabled ? "启用" : "禁用"}`);
			if (editingName === config.name) {
				fillFormFromConfig({ ...config, enabled });
			}
			await refresh();
		} catch (e) {
			toast.error(e instanceof Error ? e.message : "更新 MCP 服务状态失败");
		}
	};

	const configByName = useMemo(
		() => Object.fromEntries(globalConfigs.map((cfg) => [cfg.name, cfg])),
		[globalConfigs],
	);
	const displayNames = useMemo(() => {
		const names = new Set<string>(globalConfigs.map((cfg) => cfg.name));
		for (const name of Object.keys(servers)) names.add(name);
		return Array.from(names);
	}, [globalConfigs, servers]);
	const filteredNames = useMemo(() => {
		if (statusFilter === "all") return displayNames;
		return displayNames.filter((name) => {
			const config = configByName[name];
			const status = servers[name];
			if (statusFilter === "enabled") return (config?.enabled ?? true) === true;
			return !!status?.connected;
		});
	}, [statusFilter, displayNames, configByName, servers]);
	const filterCounts = useMemo(() => {
		const all = displayNames.length;
		const enabled = displayNames.filter(
			(name) => (configByName[name]?.enabled ?? true) === true,
		).length;
		const connected = displayNames.filter(
			(name) => !!servers[name]?.connected,
		).length;
		return { all, enabled, connected };
	}, [displayNames, configByName, servers]);

	return (
		<div>
			<SectionHeader
				icon={PlugZap}
				title="MCP 服务"
				description="管理全局 MCP 服务，供所有会话复用。"
			/>

			<div className="rounded-xl border bg-card p-4 space-y-3 mb-4">
				<div className="flex items-center justify-between">
					<div className="flex items-center gap-2">
						<h3 className="text-sm font-semibold">全局服务状态</h3>
						<div className="flex items-center gap-1">
							<Button
								variant={statusFilter === "all" ? "default" : "outline"}
								size="sm"
								className="h-7 text-xs"
								onClick={() => setStatusFilter("all")}
							>
								全部({filterCounts.all})
							</Button>
							<Button
								variant={statusFilter === "enabled" ? "default" : "outline"}
								size="sm"
								className="h-7 text-xs"
								onClick={() => setStatusFilter("enabled")}
							>
								已启用({filterCounts.enabled})
							</Button>
							<Button
								variant={statusFilter === "connected" ? "default" : "outline"}
								size="sm"
								className="h-7 text-xs"
								onClick={() => setStatusFilter("connected")}
							>
								已连接({filterCounts.connected})
							</Button>
						</div>
					</div>
					<Button
						variant="outline"
						size="sm"
						className="h-7 text-xs"
						onClick={refresh}
						disabled={loading}
					>
						<RefreshCcw className="size-3 mr-1" />
						刷新
					</Button>
				</div>
				{filteredNames.length === 0 ? (
					<p className="text-xs text-muted-foreground">
						{statusFilter === "all"
							? "当前没有已注册的 MCP 服务。"
							: "当前筛选条件下没有匹配的 MCP 服务。"}
					</p>
				) : (
					<div className="space-y-2">
						{filteredNames.map((serverName) => {
							const status = servers[serverName];
							const config = configByName[serverName];
							return (
								<div
									key={serverName}
									className="flex items-center gap-2 rounded-lg border px-3 py-2"
								>
									<span
										className={`size-2 rounded-full ${
											status?.connected ? "bg-emerald-500" : "bg-amber-500"
										}`}
									/>
									<div className="flex-1 min-w-0">
										<p className="text-sm font-medium truncate">{serverName}</p>
										<p className="text-[11px] text-muted-foreground">
											{config?.enabled === false
												? "已禁用"
												: status?.connected
													? "已连接"
													: "未连接"}
											{typeof status?.tool_count === "number"
												? ` · ${status.tool_count} tools`
												: ""}
											{status?.error ? ` · ${status.error}` : ""}
										</p>
									</div>
									<div className="flex items-center gap-1">
										<Button
											variant="ghost"
											size="sm"
											className="h-7 text-xs"
											disabled={!config}
											onClick={() => {
												if (!config) return;
												handleToggleEnabled(config, !(config.enabled ?? true));
											}}
										>
											{config?.enabled === false ? "启用" : "禁用"}
										</Button>
										<Button
											variant="ghost"
											size="sm"
											className="h-7 text-xs"
											disabled={!config}
											onClick={() => {
												if (!config) return;
												setEditingName(serverName);
												fillFormFromConfig(config);
											}}
										>
											<Pencil className="size-3" />
										</Button>
										<Button
											variant="ghost"
											size="sm"
											className="h-7 text-xs text-destructive"
											onClick={() => handleRemove(serverName)}
										>
											<Trash2 className="size-3" />
										</Button>
									</div>
								</div>
							);
						})}
					</div>
				)}
			</div>

			<div className="rounded-xl border bg-card p-4 space-y-3">
				<div className="flex items-center justify-between gap-2">
					<h3 className="text-sm font-semibold">
						{editingName
							? `编辑全局 MCP 服务：${editingName}`
							: "添加全局 MCP 服务"}
					</h3>
					{editingName && (
						<Button
							variant="outline"
							size="sm"
							className="h-7 text-xs"
							onClick={resetForm}
						>
							取消编辑
						</Button>
					)}
				</div>
				<div className="grid grid-cols-2 gap-3">
					<div className="space-y-1.5">
						<Label className="text-xs">服务名</Label>
						<Input
							className="h-8 text-xs font-mono"
							placeholder="filesystem"
							value={name}
							onChange={(e) => setName(e.target.value)}
						/>
					</div>
					<div className="space-y-1.5">
						<Label className="text-xs">传输方式</Label>
						<Select
							value={transport}
							onValueChange={(v) => setTransport(v as "stdio" | "http")}
						>
							<SelectTrigger className="h-8 text-xs">
								<SelectValue />
							</SelectTrigger>
							<SelectContent>
								<SelectItem value="stdio" className="text-xs">
									stdio
								</SelectItem>
								<SelectItem value="http" className="text-xs">
									http
								</SelectItem>
							</SelectContent>
						</Select>
					</div>
				</div>

				{transport === "stdio" ? (
					<div className="grid grid-cols-2 gap-3">
						<div className="space-y-1.5">
							<Label className="text-xs">Command</Label>
							<Input
								className="h-8 text-xs font-mono"
								placeholder="npx"
								value={command}
								onChange={(e) => setCommand(e.target.value)}
							/>
						</div>
						<div className="space-y-1.5">
							<Label className="text-xs">Args（空格分隔）</Label>
							<Input
								className="h-8 text-xs font-mono"
								placeholder="-y @modelcontextprotocol/server-filesystem /path"
								value={args}
								onChange={(e) => setArgs(e.target.value)}
							/>
						</div>
					</div>
				) : (
					<div className="space-y-1.5">
						<Label className="text-xs">URL</Label>
						<Input
							className="h-8 text-xs font-mono"
							placeholder="http://127.0.0.1:8787/mcp"
							value={url}
							onChange={(e) => setUrl(e.target.value)}
						/>
					</div>
				)}

				<div className="grid grid-cols-2 gap-3">
					<div className="space-y-1.5">
						<Label className="text-xs">Headers（KEY=VALUE，每行一个）</Label>
						<textarea
							className="w-full min-h-[72px] rounded-md border bg-background px-3 py-2 text-xs font-mono"
							placeholder="Authorization=Bearer ..."
							value={headersInput}
							onChange={(e) => setHeadersInput(e.target.value)}
						/>
					</div>
					<div className="space-y-1.5">
						<Label className="text-xs">Env（KEY=VALUE，每行一个）</Label>
						<textarea
							className="w-full min-h-[72px] rounded-md border bg-background px-3 py-2 text-xs font-mono"
							placeholder="API_TOKEN=..."
							value={envInput}
							onChange={(e) => setEnvInput(e.target.value)}
						/>
					</div>
				</div>

				<div className="flex items-end justify-between gap-3">
					<div className="space-y-1.5 w-32">
						<Label className="text-xs">超时（秒）</Label>
						<Input
							type="number"
							min={1}
							className="h-8 text-xs"
							value={timeoutSecs}
							onChange={(e) => setTimeoutSecs(e.target.value)}
						/>
					</div>
					<Button
						size="sm"
						className="h-8 text-xs"
						disabled={!canAdd}
						onClick={handleSave}
					>
						<Plus className="size-3 mr-1" />
						{editingName ? "保存更新" : "添加服务"}
					</Button>
				</div>
			</div>
		</div>
	);
}
