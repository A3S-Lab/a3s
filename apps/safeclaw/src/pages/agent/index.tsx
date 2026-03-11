import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import { agentApi } from "@/lib/agent-api";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { connectSession } from "@/hooks/use-agent-ws";
import { useEffect, useRef } from "react";
import { useSnapshot } from "valtio";
import AgentSessionList from "./components/agent-session-list";
import AgentChat from "./components/agent-chat";

function EmptyState() {
	return (
		<div className="flex items-center justify-center h-full text-muted-foreground">
			<div className="text-center space-y-3 max-w-xs">
				<div className="text-4xl">🤖</div>
				<p className="text-lg font-medium text-foreground">选择智能体</p>
				<p className="text-sm">
					从左侧选择一个智能体开始对话，或点击 + 自定义创建
				</p>
			</div>
		</div>
	);
}

/** Load real sessions from backend and wire up WebSocket connections */
async function loadSessions() {
	try {
		// Load server personas first so session list can map them
		await personaModel.loadServerPersonas();

		const sessions = await agentApi.listSessions();
		if (!Array.isArray(sessions)) return;

		// Set persona mappings BEFORE updating sdkSessions to avoid race condition
		// where the list renders before mappings are available
		for (const s of sessions) {
			if (s.persona_id) {
				personaModel.setSessionPersona(s.session_id, s.persona_id);
			}
			if (s.name && !agentModel.state.sessionNames[s.session_id]) {
				agentModel.setSessionName(s.session_id, s.name);
			}
		}

		agentModel.setSdkSessions(sessions);

		for (const s of sessions) {
			if (s.state !== "exited") {
				connectSession(s.session_id);
			}
		}

		// Validate stored currentSessionId — clear if it no longer exists
		const currentId = agentModel.state.currentSessionId;
		const activeSessions = sessions.filter(
			(s) => !s.archived && s.state !== "exited",
		);
		if (currentId && !sessions.find((s) => s.session_id === currentId)) {
			// Stale session — auto-select the most recent active session
			const latest = activeSessions.sort(
				(a, b) => b.created_at - a.created_at,
			)[0];
			agentModel.setCurrentSession(latest?.session_id ?? null);
		}
	} catch (e) {
		// Backend unavailable — UI still works, just empty
		console.warn("Failed to load sessions on startup", e);
	}
}

export default function AgentPage() {
	const { currentSessionId, sdkSessions } = useSnapshot(agentModel.state);
	const loaded = useRef(false);

	useEffect(() => {
		if (!loaded.current) {
			loaded.current = true;
			loadSessions();
		}
	}, []);

	// Get current session's cwd
	const currentSession = sdkSessions.find((s) => s.session_id === currentSessionId);
	const cwd = currentSession?.cwd;

	return (
		<>
			<ResizablePanelGroup direction="horizontal" className="h-full w-full">
				<ResizablePanel defaultSize={22} minSize={15} maxSize={40}>
					<AgentSessionList />
				</ResizablePanel>
				<ResizableHandle withHandle />
				<ResizablePanel defaultSize={78} minSize={50}>
					{currentSessionId ? (
						<AgentChat key={currentSessionId} sessionId={currentSessionId} cwd={cwd} />
					) : (
						<EmptyState />
					)}
				</ResizablePanel>
			</ResizablePanelGroup>
		</>
	);
}
