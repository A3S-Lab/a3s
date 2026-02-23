import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { timeAgo } from "@/lib/time";
import securityModel from "@/models/security.model";
import type {
	ChannelAgentConfig,
	ChannelConfig,
	ChannelKind,
	ChannelStatus,
} from "@/models/security.model";
import {
	Check,
	ChevronDown,
	ChevronRight,
	Lock,
	MessageSquare,
	Plus,
	ShieldAlert,
	Wifi,
	WifiOff,
	X,
} from "lucide-react";
import { useState, useEffect } from "react";
import { useSnapshot } from "valtio";
import { toast } from "sonner";
import {
	fetchChannels,
	fetchCredentialHealth,
	updateChannelAgentConfig,
} from "@/lib/security-api";
import { SectionHeader, StatCard } from "./shared";

const CHANNEL_META: Record<
	ChannelKind,
	{
		label: string;
		color: string;
		credentialFields: { key: string; label: string; secret?: boolean }[];
	}
> = {
	telegram: {
		label: "Telegram",
		color: "bg-sky-500/10 text-sky-600 dark:text-sky-400 border-sky-500/20",
		credentialFields: [
			{ key: "bot_token", label: "Bot Token", secret: true },
			{ key: "allowed_users", label: "允许用户 ID (逗号分隔)" },
		],
	},
	slack: {
		label: "Slack",
		color:
			"bg-purple-500/10 text-purple-600 dark:text-purple-400 border-purple-500/20",
		credentialFields: [
			{ key: "bot_token", label: "Bot Token", secret: true },
			{ key: "app_token", label: "App Token", secret: true },
			{ key: "allowed_workspaces", label: "允许 Workspace ID" },
		],
	},
	discord: {
		label: "Discord",
		color:
			"bg-indigo-500/10 text-indigo-600 dark:text-indigo-400 border-indigo-500/20",
		credentialFields: [
			{ key: "bot_token", label: "Bot Token", secret: true },
			{ key: "allowed_guilds", label: "允许 Guild ID (逗号分隔)" },
		],
	},
	feishu: {
		label: "飞书",
		color: "bg-blue-500/10 text-blue-600 dark:text-blue-400 border-blue-500/20",
		credentialFields: [
			{ key: "app_id", label: "App ID" },
			{ key: "app_secret", label: "App Secret", secret: true },
			{ key: "encrypt_key", label: "Encrypt Key", secret: true },
			{ key: "verification_token", label: "Verification Token", secret: true },
		],
	},
	dingtalk: {
		label: "钉钉",
		color:
			"bg-orange-500/10 text-orange-600 dark:text-orange-400 border-orange-500/20",
		credentialFields: [
			{ key: "app_key", label: "App Key" },
			{ key: "app_secret", label: "App Secret", secret: true },
			{ key: "robot_code", label: "Robot Code" },
		],
	},
	wecom: {
		label: "企业微信",
		color:
			"bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20",
		credentialFields: [
			{ key: "corp_id", label: "Corp ID" },
			{ key: "agent_id", label: "Agent ID" },
			{ key: "secret", label: "Secret", secret: true },
			{ key: "token", label: "Token", secret: true },
			{ key: "encoding_aes_key", label: "AES Key", secret: true },
		],
	},
	webchat: {
		label: "Web Chat",
		color: "bg-slate-500/10 text-slate-600 dark:text-slate-400 border-slate-500/20",
		credentialFields: [
			{ key: "require_auth", label: "需要认证 (true/false)" },
			{ key: "allowed_origins", label: "允许来源 (逗号分隔)" },
		],
	},
	whatsapp: {
		label: "WhatsApp",
		color:
			"bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20",
		credentialFields: [
			{ key: "phone_number_id", label: "Phone Number ID" },
			{ key: "access_token", label: "Access Token", secret: true },
			{ key: "verify_token", label: "Webhook Verify Token", secret: true },
		],
	},
	teams: {
		label: "Microsoft Teams",
		color: "bg-blue-600/10 text-blue-700 dark:text-blue-400 border-blue-600/20",
		credentialFields: [
			{ key: "app_id", label: "App ID" },
			{ key: "app_password", label: "App Password", secret: true },
			{ key: "tenant_id", label: "Tenant ID" },
		],
	},
	google_chat: {
		label: "Google Chat",
		color: "bg-red-500/10 text-red-600 dark:text-red-400 border-red-500/20",
		credentialFields: [
			{
				key: "service_account_key",
				label: "Service Account Key (JSON)",
				secret: true,
			},
			{ key: "allowed_spaces", label: "允许 Space ID (逗号分隔)" },
		],
	},
	signal: {
		label: "Signal",
		color:
			"bg-slate-500/10 text-slate-600 dark:text-slate-400 border-slate-500/20",
		credentialFields: [
			{ key: "phone_number", label: "注册手机号" },
			{ key: "signal_cli_url", label: "signal-cli REST URL" },
		],
	},
};

const STATUS_STYLES: Record<
	ChannelStatus,
	{ dot: string; label: string; text: string }
> = {
	running: {
		dot: "bg-green-500",
		label: "运行中",
		text: "text-green-600 dark:text-green-400",
	},
	stopped: {
		dot: "bg-gray-400",
		label: "已停止",
		text: "text-muted-foreground",
	},
	error: { dot: "bg-destructive", label: "错误", text: "text-destructive" },
	reconnecting: {
		dot: "bg-amber-500",
		label: "重连中",
		text: "text-amber-600 dark:text-amber-400",
	},
};

function ChannelCard({ channel }: { channel: ChannelConfig }) {
	const [expanded, setExpanded] = useState(false);
	const [editingKey, setEditingKey] = useState<string | null>(null);
	const [draft, setDraft] = useState("");
	const [agentTab, setAgentTab] = useState<"credentials" | "agent">(
		"credentials",
	);
	const [agentDraft, setAgentDraft] = useState<ChannelAgentConfig>({
		...channel.agentConfig,
	});
	const meta = CHANNEL_META[channel.kind];
	const status = STATUS_STYLES[channel.status];

	return (
		<div
			className={cn(
				"rounded-xl border bg-card transition-all",
				!channel.enabled && "opacity-60",
			)}
		>
			<div className="flex items-center gap-3 px-4 py-3 border-b">
				<div className="relative">
					<span
						className={cn(
							"size-2 rounded-full shrink-0 inline-block",
							status.dot,
							channel.status === "running" && "animate-pulse",
						)}
					/>
				</div>
				<span
					className={cn(
						"inline-flex items-center rounded-md border px-2 py-0.5 text-[10px] font-bold",
						meta.color,
					)}
				>
					{meta.label}
				</span>
				<span className="text-sm font-medium">{channel.name}</span>
				<span className={cn("text-[11px] font-medium", status.text)}>
					{status.label}
				</span>
				<div className="flex-1" />
				<div className="flex items-center gap-2">
					<Button
						variant="ghost"
						size="sm"
						className="h-6 text-[10px]"
						onClick={() => {
							securityModel.toggleChannel(channel.id);
							toast.success(
								channel.enabled
									? `已停用 ${channel.name}`
									: `已启用 ${channel.name}`,
							);
						}}
					>
						{channel.enabled ? (
							<WifiOff className="size-3 mr-1" />
						) : (
							<Wifi className="size-3 mr-1" />
						)}
						{channel.enabled ? "停用" : "启用"}
					</Button>
					<button
						type="button"
						className="text-muted-foreground hover:text-foreground p-1"
						onClick={() => setExpanded(!expanded)}
					>
						{expanded ? (
							<ChevronDown className="size-4" />
						) : (
							<ChevronRight className="size-4" />
						)}
					</button>
				</div>
			</div>

			<div className="flex items-center gap-6 px-4 py-2.5 bg-muted/20 border-b text-[11px] text-muted-foreground">
				<span>
					总消息{" "}
					<span className="font-medium text-foreground tabular-nums">
						{channel.messagesTotal.toLocaleString()}
					</span>
				</span>
				<span>
					24h{" "}
					<span className="font-medium text-foreground tabular-nums">
						{channel.messagesLast24h}
					</span>
				</span>
				<span>
					DM 策略{" "}
					<span
						className={cn(
							"font-medium",
							channel.dmPolicy === "open"
								? "text-green-600 dark:text-green-400"
								: "text-amber-600 dark:text-amber-400",
						)}
					>
						{channel.dmPolicy === "open" ? "开放" : "配对"}
					</span>
				</span>
				{channel.lastActivity && (
					<span>
						最近活动{" "}
						<span className="font-medium text-foreground">
							{timeAgo(channel.lastActivity)}
						</span>
					</span>
				)}
			</div>

			{expanded && (
				<div className="px-4 py-3">
					<div className="flex gap-1 mb-3 border-b">
						{(["credentials", "agent"] as const).map((tab) => (
							<button
								key={tab}
								type="button"
								className={cn(
									"px-3 py-1.5 text-[11px] font-medium border-b-2 -mb-px transition-colors",
									agentTab === tab
										? "border-primary text-foreground"
										: "border-transparent text-muted-foreground hover:text-foreground",
								)}
								onClick={() => setAgentTab(tab)}
							>
								{tab === "credentials" ? "凭证配置" : "Agent 策略"}
							</button>
						))}
					</div>

					{agentTab === "credentials" && (
						<div className="space-y-2">
							{meta.credentialFields.map((field) => {
								const val = channel.credentials[field.key] || "";
								const isEditing = editingKey === field.key;
								return (
									<div key={field.key} className="flex items-center gap-2">
										<span className="text-[11px] text-muted-foreground w-32 shrink-0">
											{field.label}
										</span>
										{isEditing ? (
											<div className="flex-1 flex items-center gap-1.5">
												<Input
													className="h-6 text-[11px] font-mono flex-1"
													type={field.secret ? "password" : "text"}
													value={draft}
													onChange={(e) => setDraft(e.target.value)}
												/>
												<button
													type="button"
													className="text-primary hover:text-primary/80 p-0.5"
													onClick={() => {
														securityModel.updateChannelCredential(
															channel.id,
															field.key,
															draft,
														);
														setEditingKey(null);
														toast.success("已更新");
													}}
												>
													<Check className="size-3.5" />
												</button>
												<button
													type="button"
													className="text-muted-foreground hover:text-foreground p-0.5"
													onClick={() => setEditingKey(null)}
												>
													<X className="size-3.5" />
												</button>
											</div>
										) : (
											<button
												type="button"
												className="flex-1 text-left text-[11px] font-mono text-muted-foreground hover:text-foreground truncate"
												onClick={() => {
													setEditingKey(field.key);
													setDraft(val);
												}}
											>
												{val
													? field.secret
														? `${val.slice(0, 6)}${"•".repeat(8)}`
														: val
													: "点击设置..."}
											</button>
										)}
									</div>
								);
							})}
						</div>
					)}

					{agentTab === "agent" && (
						<div className="space-y-3">
							<div className="flex items-center gap-2">
								<span className="text-[11px] text-muted-foreground w-32 shrink-0">
									模型覆盖
								</span>
								<Input
									className="h-6 text-[11px] flex-1"
									placeholder="留空使用全局默认"
									value={agentDraft.model ?? ""}
									onChange={(e) =>
										setAgentDraft({
											...agentDraft,
											model: e.target.value || null,
										})
									}
								/>
							</div>
							<div className="flex items-center gap-2">
								<span className="text-[11px] text-muted-foreground w-32 shrink-0">
									权限模式
								</span>
								<select
									className="h-6 text-[11px] flex-1 rounded border bg-background px-2"
									value={agentDraft.permissionMode}
									onChange={(e) =>
										setAgentDraft({
											...agentDraft,
											permissionMode: e.target
												.value as ChannelAgentConfig["permissionMode"],
										})
									}
								>
									<option value="default">默认</option>
									<option value="strict">严格（需确认所有工具）</option>
									<option value="trust">信任（自动批准）</option>
								</select>
							</div>
							<div className="flex items-start gap-2">
								<span className="text-[11px] text-muted-foreground w-32 shrink-0 pt-0.5">
									工具白名单
								</span>
								<div className="flex-1">
									<Input
										className="h-6 text-[11px] font-mono"
										placeholder="留空=全部允许，逗号分隔工具名"
										value={agentDraft.allowedTools?.join(", ") ?? ""}
										onChange={(e) => {
											const v = e.target.value.trim();
											setAgentDraft({
												...agentDraft,
												allowedTools: v
													? v
															.split(",")
															.map((s) => s.trim())
															.filter(Boolean)
													: null,
											});
										}}
									/>
									<p className="text-[10px] text-muted-foreground mt-1">
										例：read_file, web_search, web_fetch
									</p>
								</div>
							</div>
							<div className="flex items-start gap-2">
								<span className="text-[11px] text-muted-foreground w-32 shrink-0 pt-0.5">
									工具黑名单
								</span>
								<Input
									className="h-6 text-[11px] font-mono flex-1"
									placeholder="逗号分隔，始终阻止"
									value={agentDraft.blockedTools.join(", ")}
									onChange={(e) =>
										setAgentDraft({
											...agentDraft,
											blockedTools: e.target.value
												.split(",")
												.map((s) => s.trim())
												.filter(Boolean),
										})
									}
								/>
							</div>
							<div className="flex justify-end gap-2 pt-1">
								<Button
									variant="ghost"
									size="sm"
									className="h-6 text-[11px]"
									onClick={() => setAgentDraft({ ...channel.agentConfig })}
								>
									重置
								</Button>
								<Button
									size="sm"
									className="h-6 text-[11px]"
									onClick={async () => {
										securityModel.updateChannelAgentConfig(
											channel.id,
											agentDraft,
										);
										try {
											await updateChannelAgentConfig(channel.id, agentDraft);
											toast.success("Agent 策略已保存");
										} catch (e) {
											console.warn(
												"Failed to save agent config to backend:",
												e,
											);
											toast.success("Agent 策略已保存（本地）");
										}
									}}
								>
									保存
								</Button>
							</div>
						</div>
					)}
				</div>
			)}
		</div>
	);
}

export function ChannelsSection() {
	const snap = useSnapshot(securityModel.state);
	const running = snap.channels.filter((c) => c.status === "running").length;
	const errored = snap.channels.filter((c) => c.status === "error").length;

	useEffect(() => {
		fetchChannels()
			.then((channels) => {
				if (Array.isArray(channels)) securityModel.state.channels = channels;
			})
			.catch((e) => console.warn("Channels unavailable:", e));
		fetchCredentialHealth()
			.then((health) => {
				securityModel.setCredentialHealth(health);
			})
			.catch((e) => console.warn("Credential health unavailable:", e));
	}, []);

	const credHealth = snap.credentialHealth as Record<string, string>;
	const credEntries = Object.entries(credHealth);
	const hasCredIssues = credEntries.some(([, v]) => v !== "ok");

	return (
		<div>
			<SectionHeader
				icon={MessageSquare}
				title="渠道管理"
				description="管理 AI 助手的消息渠道接入，支持 11 种平台。"
			/>

			{credEntries.length > 0 && (
				<div
					className={cn(
						"flex flex-wrap items-center gap-2 rounded-lg border px-3 py-2 mb-4 text-[11px]",
						hasCredIssues
							? "bg-destructive/5 border-destructive/20"
							: "bg-green-500/5 border-green-500/20",
					)}
				>
					<span
						className={cn(
							"font-medium",
							hasCredIssues
								? "text-destructive"
								: "text-green-600 dark:text-green-400",
						)}
					>
						凭证状态
					</span>
					{credEntries.map(([key, status]) => (
						<span
							key={key}
							className={cn(
								"inline-flex items-center gap-1 rounded px-1.5 py-0.5 border font-mono",
								status === "ok"
									? "bg-green-500/10 text-green-600 dark:text-green-400 border-green-500/20"
									: status === "expired"
										? "bg-destructive/10 text-destructive border-destructive/20"
										: "bg-amber-500/10 text-amber-600 dark:text-amber-400 border-amber-500/20",
							)}
						>
							<span
								className={cn(
									"size-1.5 rounded-full",
									status === "ok"
										? "bg-green-500"
										: status === "expired"
											? "bg-destructive"
											: "bg-amber-500",
								)}
							/>
							{key}: {status}
						</span>
					))}
				</div>
			)}

			<div className="flex items-center gap-2 rounded-lg bg-muted/40 border px-3 py-2 mb-4 text-[11px] text-muted-foreground">
				<Lock className="size-3 shrink-0" />
				<span>
					渠道配置通过 <code className="font-mono">safeclaw.hcl</code>{" "}
					管理，此处仅展示运行时状态。
				</span>
			</div>

			<div className="grid grid-cols-3 gap-4 mb-6">
				<StatCard
					icon={Wifi}
					label="运行中渠道"
					value={running}
					sub={`共 ${snap.channels.length} 个渠道`}
					color="text-green-600 dark:text-green-400"
				/>
				<StatCard
					icon={MessageSquare}
					label="今日消息"
					value={snap.channels.reduce((s, c) => s + c.messagesLast24h, 0)}
					sub="所有渠道合计"
					color="text-primary"
				/>
				<StatCard
					icon={ShieldAlert}
					label="异常渠道"
					value={errored}
					sub={errored > 0 ? "需要检查" : "全部正常"}
					color={
						errored > 0
							? "text-destructive"
							: "text-green-600 dark:text-green-400"
					}
				/>
			</div>

			<div className="space-y-3">
				{(snap.channels as ChannelConfig[]).map((ch) => (
					<ChannelCard key={ch.id} channel={ch} />
				))}
			</div>

			<div className="mt-4 flex items-center justify-center gap-2 rounded-xl border-2 border-dashed px-4 py-4 text-sm text-muted-foreground hover:text-foreground hover:border-primary/30 transition-colors cursor-default">
				<Plus className="size-4" />
				<span>通过修改 safeclaw.hcl 配置文件添加新渠道</span>
			</div>
		</div>
	);
}
