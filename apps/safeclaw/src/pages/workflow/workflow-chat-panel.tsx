/**
 * WorkflowChatPanel — a dedicated agent chat session bound to a workflow.
 *
 * Lifecycle:
 *   1. On mount, check if the workflow already has a session and it still exists.
 *   2. If yes → connect and render AgentChat.
 *   3. If no session (or session was deleted) → auto-create one using the
 *      defaultCwd from settings.  If defaultCwd is not configured, show a
 *      prompt to go to settings.
 */
import AgentChat from "@/pages/agent/components/agent-chat";
import { agentApi } from "@/lib/agent-api";
import agentModel from "@/models/agent.model";
import type { AgentProcessInfo } from "@/typings/agent";
import settingsModel, {
	getPreferredSessionModel,
	resolveApiKey,
	resolveBaseUrl,
} from "@/models/settings.model";
import workflowModel from "@/models/workflow.model";
import { connectSession } from "@/hooks/use-agent-ws";
import { AlertCircle, Loader2, MessageSquare } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";

type PanelState = "initializing" | "ready" | "no-workspace" | "error";

interface WorkflowChatPanelProps {
	workflowId: string;
}

export default function WorkflowChatPanel({
	workflowId,
}: WorkflowChatPanelProps) {
	const navigate = useNavigate();

	const [panelState, setPanelState] = useState<PanelState>("initializing");
	const [sessionId, setSessionId] = useState<string | null>(
		workflowModel.get(workflowId)?.session_id ?? null,
	);
	const [errorMsg, setErrorMsg] = useState<string | null>(null);
	const initRef = useRef(false);

	useEffect(() => {
		if (initRef.current) return;
		initRef.current = true;

		bootstrap();
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [workflowId]);

	async function bootstrap() {
		const wf = workflowModel.get(workflowId);
		if (!wf) {
			setErrorMsg("工作流不存在");
			setPanelState("error");
			return;
		}

		// Check if the stored session still exists on the backend
		if (wf.session_id) {
			try {
				const sessions = (await agentApi.listSessions()) as
					| AgentProcessInfo[]
					| null;
				if (Array.isArray(sessions)) {
					agentModel.setSdkSessions(sessions);
				}
				const found =
					Array.isArray(sessions) &&
					sessions.some((s) => s.session_id === wf.session_id);
				if (found) {
					connectSession(wf.session_id);
					agentModel.setCurrentSession(wf.session_id);
					agentModel.clearUnread(wf.session_id);
					setSessionId(wf.session_id);
					setPanelState("ready");
					return;
				}
				// Session was deleted — fall through to create a new one
			} catch {
				// Backend unreachable — fall through
			}
		}

		// No valid session — auto-create if workspace is configured
		const defaultCwd = settingsModel.state.agentDefaults.workspaceRoot.trim();
		if (!defaultCwd) {
			setPanelState("no-workspace");
			return;
		}

		await createSession(workflowId, defaultCwd);
	}

	async function createSession(wfId: string, workspaceRoot: string) {
		setPanelState("initializing");
		setErrorMsg(null);
		try {
			const wfCwd = [workspaceRoot.replace(/\/$/, ""), "workflows", wfId].join(
				"/",
			);
			const preferred = getPreferredSessionModel();
			const apiKey = preferred.modelId
				? resolveApiKey(preferred.providerName, preferred.modelId)
				: "";
			const baseUrl = preferred.modelId
				? resolveBaseUrl(preferred.providerName, preferred.modelId)
				: "";

			const result = (await agentApi.createSession({
				model: preferred.modelId || undefined,
				cwd: wfCwd,
				api_key: apiKey || undefined,
				base_url: baseUrl || undefined,
				system_prompt: `你是工作流助手，协助用户理解和改进当前工作流。`,
			})) as { session_id?: string; error?: string } | null;

			if (!result || result.error) {
				setErrorMsg(result?.error ?? "创建会话失败");
				setPanelState("error");
				return;
			}

			const sid = result.session_id!;
			const updated = (await agentApi.listSessions()) as
				| AgentProcessInfo[]
				| null;
			if (Array.isArray(updated)) {
				agentModel.setSdkSessions(updated);
			}

			workflowModel.setSessionId(wfId, sid);
			connectSession(sid);
			agentModel.setCurrentSession(sid);
			agentModel.clearUnread(sid);
			agentModel.setMessages(sid, []);

			setSessionId(sid);
			setPanelState("ready");
		} catch (e) {
			setErrorMsg(e instanceof Error ? e.message : "无法连接到网关");
			setPanelState("error");
		}
	}

	// ── Render ───────────────────────────────────────────────────────────────

	if (panelState === "initializing") {
		return (
			<div className="flex flex-col items-center justify-center h-full gap-2 text-muted-foreground">
				<Loader2 className="size-5 animate-spin" />
				<span className="text-xs">正在初始化会话…</span>
			</div>
		);
	}

	if (panelState === "no-workspace") {
		return (
			<div className="flex flex-col items-center justify-center h-full gap-4 px-6 text-center">
				<div className="flex items-center justify-center size-12 rounded-xl bg-muted">
					<MessageSquare className="size-5 text-muted-foreground" />
				</div>
				<div className="space-y-1">
					<p className="text-sm font-medium">需要配置工作区</p>
					<p className="text-xs text-muted-foreground">
						请先在设置中配置默认工作区目录，工作流会在其下自动创建独立的会话目录。
					</p>
				</div>
				<button
					type="button"
					onClick={() => navigate("/settings")}
					className="px-4 py-1.5 text-sm rounded-lg bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
				>
					前往设置
				</button>
			</div>
		);
	}

	if (panelState === "error") {
		return (
			<div className="flex flex-col items-center justify-center h-full gap-4 px-6 text-center">
				<AlertCircle className="size-8 text-destructive/60" />
				<div className="space-y-1">
					<p className="text-sm font-medium text-destructive">会话初始化失败</p>
					<p className="text-xs text-muted-foreground">{errorMsg}</p>
				</div>
				<button
					type="button"
					onClick={() => {
						initRef.current = false;
						setPanelState("initializing");
						bootstrap();
					}}
					className="px-4 py-1.5 text-sm rounded-lg border hover:bg-muted transition-colors"
				>
					重试
				</button>
			</div>
		);
	}

	// panelState === "ready"
	if (!sessionId) return null;

	return <AgentChat key={sessionId} sessionId={sessionId} disableMention />;
}
