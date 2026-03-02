import { getGatewayUrl } from "@/models/settings.model";

function baseUrl() {
	return `${getGatewayUrl()}/api/agent`;
}

const jsonHeaders = { "Content-Type": "application/json" };

/** Safe JSON fetch — throws on HTTP errors with descriptive message */
async function safeFetch(url: string, init?: RequestInit): Promise<any> {
	const res = await fetch(url, init);
	if (!res.ok) {
		const text = await res.text().catch(() => "");
		throw new Error(`${init?.method ?? "GET"} ${url} → ${res.status}: ${text}`);
	}
	const ct = res.headers.get("content-type") ?? "";
	if (ct.includes("application/json")) return res.json();
	// DELETE and some endpoints return no body
	return null;
}

export type McpTransportConfig =
	| { type: "stdio"; command: string; args?: string[] }
	| { type: "http"; url: string; headers?: Record<string, string> };

export interface McpServerConfig {
	name: string;
	transport: McpTransportConfig;
	enabled?: boolean;
	env?: Record<string, string>;
	tool_timeout_secs?: number;
}

export interface McpServerStatus {
	name: string;
	connected: boolean;
	enabled: boolean;
	tool_count: number;
	error?: string;
}

export const agentApi = {
	createSession: (params: {
		model?: string;
		permission_mode?: string;
		cwd?: string;
		persona_id?: string;
		base_url?: string;
		api_key?: string;
		system_prompt?: string;
		skills?: string[];
		mcp_servers?: McpServerConfig[];
	}) =>
		safeFetch(`${baseUrl()}/sessions`, {
			method: "POST",
			headers: jsonHeaders,
			body: JSON.stringify(params),
		}),

	listSessions: () => safeFetch(`${baseUrl()}/sessions`),

	getSession: (id: string) => safeFetch(`${baseUrl()}/sessions/${id}`),

	updateSession: (
		id: string,
		updates: { name?: string; archived?: boolean; cwd?: string },
	) =>
		safeFetch(`${baseUrl()}/sessions/${id}`, {
			method: "PATCH",
			headers: jsonHeaders,
			body: JSON.stringify(updates),
		}),

	/** GET /api/agent/sessions/:id/mcp — fetch per-session MCP configs */
	getSessionMcpServers: (id: string) =>
		safeFetch(`${baseUrl()}/sessions/${id}/mcp`) as Promise<McpServerConfig[]>,

	/** PUT /api/agent/sessions/:id/mcp — replace per-session MCP configs */
	setSessionMcpServers: (id: string, mcp_servers: McpServerConfig[]) =>
		safeFetch(`${baseUrl()}/sessions/${id}/mcp`, {
			method: "PUT",
			headers: jsonHeaders,
			body: JSON.stringify({ mcp_servers }),
		}),

	deleteSession: (id: string) =>
		safeFetch(`${baseUrl()}/sessions/${id}`, { method: "DELETE" }),

	relaunchSession: (id: string) =>
		safeFetch(`${baseUrl()}/sessions/${id}/relaunch`, { method: "POST" }),

	listBackends: () => safeFetch(`${baseUrl()}/backends`),

	listPersonas: () => safeFetch(`${baseUrl()}/personas`),

	/** GET /api/agent/personas/market — paginated marketplace query */
	listMarketPersonas: (params: {
		page?: number;
		page_size?: number;
		search?: string;
		tags?: string[];
	}) => {
		const qs = new URLSearchParams();
		if (params.page) qs.set("page", String(params.page));
		if (params.page_size) qs.set("page_size", String(params.page_size));
		if (params.search) qs.set("search", params.search);
		if (params.tags?.length) qs.set("tags", params.tags.join(","));
		return safeFetch(
			`${baseUrl()}/personas/market?${qs.toString()}`,
		) as Promise<{
			items: import("@/typings/agent").PersonaInfo[];
			total: number;
			page: number;
			page_size: number;
		}>;
	},

	/** GET /api/agent/commands — list available slash commands */
	listCommands: () =>
		safeFetch(`${baseUrl()}/commands`) as Promise<
			{ name: string; description: string }[]
		>,

	sendAgentMessage: (id: string, target: string, content: string) =>
		safeFetch(`${baseUrl()}/sessions/${id}/message`, {
			method: "POST",
			headers: jsonHeaders,
			body: JSON.stringify({ target, content }),
		}),

	setAutoExecute: (id: string, enabled: boolean) =>
		safeFetch(`${baseUrl()}/sessions/${id}/auto-execute`, {
			method: "PUT",
			headers: jsonHeaders,
			body: JSON.stringify({ enabled }),
		}),

	/** GET /api/agent/config — fetch current CodeConfig from backend */
	fetchConfig: () => safeFetch(`${baseUrl()}/config`),

	/** PUT /api/agent/config — persist updated CodeConfig to backend HCL file */
	updateConfig: (patch: { default_model?: string; providers?: unknown[] }) =>
		safeFetch(`${baseUrl()}/config`, {
			method: "PUT",
			headers: jsonHeaders,
			body: JSON.stringify(patch),
		}),

	/** POST /api/agent/sessions/:id/configure — configure LLM for an existing session */
	configureSession: (
		id: string,
		params: { model?: string; api_key?: string; base_url?: string },
	) =>
		safeFetch(`${baseUrl()}/sessions/${id}/configure`, {
			method: "POST",
			headers: jsonHeaders,
			body: JSON.stringify(params),
		}),

	/** POST /api/agent/sessions/:id/archive — archive a session */
	archiveSession: (id: string) =>
		safeFetch(`${baseUrl()}/sessions/${id}/archive`, { method: "POST" }),

	/** POST /api/agent/sessions/:id/unarchive — unarchive a session */
	unarchiveSession: (id: string) =>
		safeFetch(`${baseUrl()}/sessions/${id}/unarchive`, { method: "POST" }),

	/** GET /api/agent/stats — aggregate session statistics */
	getSessionStats: () => safeFetch(`${baseUrl()}/stats`),

	/** GET /api/agent/directory — agent discovery directory */
	getAgentDirectory: () => safeFetch(`${baseUrl()}/directory`),

	/** GET /api/agent/mcp — list all MCP servers and their status */
	listMcpServers: () =>
		safeFetch(`${baseUrl()}/mcp`) as Promise<Record<string, McpServerStatus>>,

	/** POST /api/agent/mcp — add and connect an MCP server */
	addMcpServer: (config: McpServerConfig) =>
		safeFetch(`${baseUrl()}/mcp`, {
			method: "POST",
			headers: jsonHeaders,
			body: JSON.stringify(config),
		}),

	/** DELETE /api/agent/mcp/:name — disconnect and remove an MCP server */
	removeMcpServer: (name: string) =>
		safeFetch(`${baseUrl()}/mcp/${name}`, { method: "DELETE" }),
};
