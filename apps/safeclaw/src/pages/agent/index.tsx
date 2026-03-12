import {
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
} from "@/components/ui/resizable";
import { agentApi } from "@/lib/agent-api";
import agentModel from "@/models/agent.model";
import personaModel from "@/models/persona.model";
import { connectSession } from "@/hooks/use-agent-ws";
import { useEffect, useState } from "react";
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

	// Reload sessions when page becomes visible
	useEffect(() => {
		loadSessions();

		// Also reload when window regains focus (user comes back from marketplace)
		const handleFocus = () => {
			loadSessions();
		};
		window.addEventListener("focus", handleFocus);
		return () => window.removeEventListener("focus", handleFocus);
	}, []);

	// Get current session's cwd
	const currentSession = sdkSessions.find(
		(s) => s.session_id === currentSessionId,
	);
	const [effectiveCwd, setEffectiveCwd] = useState<string | undefined>(
		currentSession?.cwd,
	);

	// Handle super-admin sessions without cwd
	useEffect(() => {
		const session = sdkSessions.find((s) => s.session_id === currentSessionId);
		if (!session) {
			setEffectiveCwd(undefined);
			return;
		}

		// If session has cwd, use it
		if (session.cwd) {
			setEffectiveCwd(session.cwd);
			return;
		}

		// If super-admin session without cwd, set to homeDir
		const personaId = personaModel.getSessionPersona(currentSessionId);
		if (personaId === "super-admin") {
			import("@tauri-apps/api/path").then(({ homeDir }) => {
				homeDir().then((home) => {
					console.log("[Agent] Setting super-admin cwd to homeDir:", home);
					setEffectiveCwd(home);
				});
			});
		} else {
			setEffectiveCwd(undefined);
		}
	}, [currentSessionId, sdkSessions]);

	return (
		<>
			<ResizablePanelGroup direction="horizontal" className="h-full w-full">
				<ResizablePanel defaultSize={22} minSize={15} maxSize={40}>
					<AgentSessionList />
				</ResizablePanel>
				<ResizableHandle withHandle />
				<ResizablePanel defaultSize={78} minSize={40}>
					{currentSessionId ? (
						<AgentChat
							key={currentSessionId}
							sessionId={currentSessionId}
							cwd={effectiveCwd}
						/>
					) : (
						<EmptyState />
					)}
				</ResizablePanel>
			</ResizablePanelGroup>
		</>
	);
}
