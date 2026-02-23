/**
 * Security & gateway API client.
 * Uses shared http utilities from @/lib/http.
 */
import { apiFetch, httpFetch, apiUrl, jsonBody } from "@/lib/http";

// ── Audit ─────────────────────────────────────────────────────────────

export async function fetchAuditEvents(params?: {
	severity?: string;
	vector?: string;
	limit?: number;
	offset?: number;
}): Promise<any[]> {
	const qs = new URLSearchParams();
	if (params?.severity) qs.set("severity", params.severity);
	if (params?.vector) qs.set("vector", params.vector);
	if (params?.limit) qs.set("limit", String(params.limit));
	if (params?.offset) qs.set("offset", String(params.offset));
	const res = await httpFetch(apiUrl(`/audit/events?${qs}`));
	if (!res.ok) throw new Error(`fetchAuditEvents failed: ${res.status}`);
	const data = await res.json();
	return Array.isArray(data) ? data : (data.events ?? []);
}

export async function fetchAuditStats(): Promise<any> {
	return apiFetch("/audit/stats");
}

export async function fetchAlerts(): Promise<any[]> {
	return apiFetch("/audit/alerts");
}

export async function queryAuditEvents(
	body: Record<string, any>,
): Promise<any[]> {
	const data = await apiFetch<any>("/audit/query", jsonBody("POST", body));
	return Array.isArray(data) ? data : (data.events ?? []);
}

export async function exportAuditEvents(
	format: "json" | "csv" = "json",
): Promise<Blob> {
	const res = await httpFetch(apiUrl(`/audit/export?format=${format}`));
	if (!res.ok) throw new Error(`exportAuditEvents failed: ${res.status}`);
	return res.blob();
}

// ── Privacy ───────────────────────────────────────────────────────────

export async function classifyText(text: string): Promise<any> {
	return apiFetch("/privacy/classify", jsonBody("POST", { text }));
}

export async function analyzePrivacy(text: string): Promise<any> {
	return apiFetch("/privacy/analyze", jsonBody("POST", { text }));
}

export async function scanPrivacy(text: string): Promise<any> {
	return apiFetch("/privacy/scan", jsonBody("POST", { text }));
}

// ── Gateway ───────────────────────────────────────────────────────────

export async function fetchGatewayStatus(): Promise<any> {
	return apiFetch("/gateway/status");
}

// ── TEE ───────────────────────────────────────────────────────────────

export async function fetchTeeStatus(): Promise<any> {
	return apiFetch("/tee/status");
}

// ── Taint ─────────────────────────────────────────────────────────────

export async function fetchTaintEntries(): Promise<any[]> {
	return apiFetch("/taint/entries");
}

// ── Channels ──────────────────────────────────────────────────────────

export async function fetchChannels(): Promise<any[]> {
	return apiFetch("/channels");
}

export async function fetchSecurityOverview(): Promise<any> {
	return apiFetch("/security/overview");
}

// ── Credential Health ──────────────────────────────────────────────────

export async function fetchCredentialHealth(): Promise<Record<string, string>> {
	try {
		const channels = await fetchChannels();
		const health: Record<string, string> = {};
		for (const ch of channels) {
			health[ch.id ?? ch.name] = ch.connected ? "ok" : "disconnected";
		}
		return health;
	} catch {
		return {};
	}
}

export async function updateChannelAgentConfig(
	channelId: string,
	config: {
		model?: string | null;
		permissionMode?: string;
		allowedTools?: string[] | null;
		blockedTools?: string[];
	},
): Promise<void> {
	await apiFetch(
		`/channels/${channelId}/agent-config`,
		jsonBody("PATCH", config),
	);
}
