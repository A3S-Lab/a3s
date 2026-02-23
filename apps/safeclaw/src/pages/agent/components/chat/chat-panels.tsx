import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import settingsModel, {
	resolveApiKey,
	resolveBaseUrl,
} from "@/models/settings.model";
import { sendToSession } from "@/hooks/use-agent-ws";
import { agentApi } from "@/lib/agent-api";
import { cn } from "@/lib/utils";
import {
	Ban,
	ChevronDown,
	ChevronRight,
	Loader2,
	MessageSquare,
	Send,
	ShieldAlert,
	ShieldCheck,
	Terminal,
	X,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useState } from "react";
import NiceAvatar, { genConfig } from "react-nice-avatar";
import { useSnapshot } from "valtio";

// =============================================================================
// AuthStatusBanner
// =============================================================================

export function AuthStatusBanner({ sessionId }: { sessionId: string }) {
	const { authStatus } = useSnapshot(agentModel.state);
	const status = authStatus[sessionId];
	const [expanded, setExpanded] = useState(false);

	if (!status?.is_authenticating) return null;

	return (
		<div className="mx-3 mb-2 rounded-lg border border-blue-500/30 bg-blue-500/10 px-3 py-2 text-xs">
			<div className="flex items-center gap-2">
				<Loader2 className="size-3.5 text-blue-500 animate-spin shrink-0" />
				<span className="font-medium text-blue-600 dark:text-blue-400">
					正在进行身份验证...
				</span>
				{status.output.length > 0 && (
					<button
						type="button"
						className="ml-auto text-muted-foreground hover:text-foreground transition-colors"
						onClick={() => setExpanded((v) => !v)}
					>
						{expanded ? (
							<ChevronDown className="size-3.5" />
						) : (
							<ChevronRight className="size-3.5" />
						)}
					</button>
				)}
			</div>
			{status.error && <p className="mt-1 text-red-500">{status.error}</p>}
			{expanded && status.output.length > 0 && (
				<pre className="mt-1.5 text-[10px] text-muted-foreground bg-muted/40 rounded p-2 overflow-x-auto whitespace-pre-wrap">
					{status.output.join("\n")}
				</pre>
			)}
		</div>
	);
}

// =============================================================================
// PermissionRequestPanel
// =============================================================================

export function PermissionRequestPanel({ sessionId }: { sessionId: string }) {
	const { pendingPermissions } = useSnapshot(agentModel.state);
	const requests = Object.values(pendingPermissions[sessionId] || {});
	const [expandedId, setExpandedId] = useState<string | null>(null);

	if (requests.length === 0) return null;

	const handleAllow = (requestId: string) => {
		sendToSession(sessionId, {
			type: "permission_response",
			request_id: requestId,
			behavior: "allow",
		});
	};

	const handleDeny = (requestId: string) => {
		sendToSession(sessionId, {
			type: "permission_response",
			request_id: requestId,
			behavior: "deny",
		});
	};

	const handleAllowAll = () => {
		for (const req of requests) {
			sendToSession(sessionId, {
				type: "permission_response",
				request_id: req.request_id,
				behavior: "allow",
			});
		}
	};

	return (
		<div className="border-t bg-amber-50/50 dark:bg-amber-950/20 shrink-0">
			<div className="flex items-center gap-2 px-3 py-1.5 border-b border-amber-200/60 dark:border-amber-800/40">
				<ShieldAlert className="size-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
				<span className="text-xs font-medium text-amber-700 dark:text-amber-300">
					等待权限确认 ({requests.length})
				</span>
				{requests.length > 1 && (
					<button
						type="button"
						className="ml-auto flex items-center gap-1 rounded-md bg-primary px-2 py-0.5 text-[10px] font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
						onClick={handleAllowAll}
					>
						<ShieldCheck className="size-3" />
						全部允许
					</button>
				)}
			</div>
			<div className="max-h-48 overflow-y-auto divide-y divide-amber-100 dark:divide-amber-900/40">
				{requests.map((req) => {
					const isExpanded = expandedId === req.request_id;
					const inputStr = JSON.stringify(req.input, null, 2);
					return (
						<div key={req.request_id} className="px-3 py-2">
							<div className="flex items-start gap-2">
								<div className="flex-1 min-w-0">
									<div className="flex items-center gap-1.5 mb-0.5">
										<Terminal className="size-3 text-amber-600 dark:text-amber-400 shrink-0" />
										<span className="text-xs font-mono font-semibold text-foreground">
											{req.tool_name}
										</span>
									</div>
									{req.description && (
										<p className="text-xs text-muted-foreground mb-1">
											{req.description}
										</p>
									)}
									<button
										type="button"
										className="flex items-center gap-1 text-[10px] text-muted-foreground/70 hover:text-muted-foreground transition-colors"
										onClick={() =>
											setExpandedId(isExpanded ? null : req.request_id)
										}
									>
										{isExpanded ? (
											<ChevronDown className="size-3" />
										) : (
											<ChevronRight className="size-3" />
										)}
										{isExpanded ? "收起参数" : "查看参数"}
									</button>
									{isExpanded && (
										<pre className="mt-1.5 rounded bg-muted/60 px-2.5 py-2 text-[11px] font-mono overflow-x-auto max-h-32 whitespace-pre-wrap text-muted-foreground">
											{inputStr}
										</pre>
									)}
								</div>
								<div className="flex items-center gap-1.5 shrink-0 mt-0.5">
									<button
										type="button"
										className="flex items-center gap-1 rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
										onClick={() => handleAllow(req.request_id)}
									>
										<ShieldCheck className="size-3" />
										允许
									</button>
									<button
										type="button"
										className="flex items-center gap-1 rounded-md border px-2.5 py-1 text-xs text-muted-foreground hover:bg-foreground/[0.04] transition-colors"
										onClick={() => handleDeny(req.request_id)}
									>
										<Ban className="size-3" />
										拒绝
									</button>
								</div>
							</div>
						</div>
					);
				})}
			</div>
		</div>
	);
}

// =============================================================================
// AgentMessageInbox
// =============================================================================

export function AgentMessageInbox({ sessionId }: { sessionId: string }) {
	const { agentMessages } = useSnapshot(agentModel.state);
	const msgs = agentMessages[sessionId] || [];

	// Configure LLM before sending — same logic as handleSend in agent-chat
	const configureAndSend = useCallback(
		async (content: string) => {
			const modelId = settingsModel.state.defaultModel;
			const providerName = settingsModel.state.defaultProvider;
			const apiKey = resolveApiKey(providerName, modelId);
			const baseUrl = resolveBaseUrl(providerName, modelId);
			const fullModel =
				providerName && modelId ? `${providerName}/${modelId}` : modelId;
			try {
				await agentApi.configureSession(sessionId, {
					model: fullModel || undefined,
					api_key: apiKey || undefined,
					base_url: baseUrl || undefined,
				});
			} catch (e) {
				console.warn(
					"Failed to configure session before agent message execute",
					e,
				);
			}
			sendToSession(sessionId, { type: "user_message", content });
		},
		[sessionId],
	);

	// Auto-execute messages marked with auto_execute
	useEffect(() => {
		const autoMsgs = msgs.filter((m) => m.auto_execute);
		for (const msg of autoMsgs) {
			configureAndSend(msg.content);
			agentModel.removeAgentMessage(sessionId, msg.message_id);
		}
	}, [msgs, sessionId, configureAndSend]);

	// Only show non-auto messages in the inbox
	const pendingMsgs = msgs.filter((m) => !m.auto_execute);
	if (pendingMsgs.length === 0) return null;

	const handleExecute = (msg: (typeof pendingMsgs)[0]) => {
		configureAndSend(msg.content);
		agentModel.removeAgentMessage(sessionId, msg.message_id);
	};

	const handleDismiss = (messageId: string) => {
		agentModel.removeAgentMessage(sessionId, messageId);
	};

	return (
		<div className="border-t bg-blue-50/50 dark:bg-blue-950/20 shrink-0">
			<div className="flex items-center gap-2 px-3 py-1.5 border-b border-blue-200/60 dark:border-blue-800/40">
				<MessageSquare className="size-3.5 text-blue-600 dark:text-blue-400 shrink-0" />
				<span className="text-xs font-medium text-blue-700 dark:text-blue-300">
					收到 Agent 消息 ({pendingMsgs.length})
				</span>
			</div>
			<div className="max-h-36 overflow-y-auto divide-y divide-blue-100 dark:divide-blue-900/40">
				{pendingMsgs.map((msg) => (
					<div
						key={msg.message_id}
						className="px-3 py-2 flex items-start gap-2"
					>
						<div className="flex-1 min-w-0">
							<div className="text-[10px] text-muted-foreground mb-0.5 truncate">
								来自 {msg.from_session_id.slice(0, 8)}… · {msg.topic}
							</div>
							<p className="text-xs text-foreground line-clamp-2">
								{msg.content}
							</p>
						</div>
						<div className="flex items-center gap-1.5 shrink-0 mt-0.5">
							<button
								type="button"
								className="flex items-center gap-1 rounded-md bg-primary px-2.5 py-1 text-xs font-medium text-primary-foreground hover:bg-primary/90 transition-colors"
								onClick={() => handleExecute(msg)}
							>
								<Send className="size-3" />
								执行
							</button>
							<button
								type="button"
								className="flex items-center justify-center size-6 rounded-md text-muted-foreground hover:bg-foreground/[0.04] transition-colors"
								onClick={() => handleDismiss(msg.message_id)}
								aria-label="忽略"
							>
								<X className="size-3" />
							</button>
						</div>
					</div>
				))}
			</div>
		</div>
	);
}

// =============================================================================
// EmptyChat
// =============================================================================

export function EmptyChat({ sessionId }: { sessionId: string }) {
	const persona = personaModel.getSessionPersona(sessionId);
	const cfg = useMemo(() => genConfig(persona.avatar), [persona.avatar]);
	return (
		<div className="flex flex-col items-center justify-center h-full gap-4 text-muted-foreground px-8">
			<NiceAvatar className="size-16" {...cfg} />
			<div className="text-center">
				<p className="text-base font-medium text-foreground">{persona.name}</p>
				<p className="text-sm mt-1 max-w-xs leading-relaxed">
					{persona.description}
				</p>
			</div>
			<p className="text-xs opacity-50">
				发送消息开始对话，/ 触发技能，@ 派发给其他 Agent
			</p>
		</div>
	);
}
