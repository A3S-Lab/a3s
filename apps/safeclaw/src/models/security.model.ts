import { proxy } from "valtio";

// =============================================================================
// Types
// =============================================================================

export type TeeLevel = "TeeHardware" | "VmIsolation" | "ProcessOnly";
export type Severity = "info" | "warning" | "high" | "critical";
export type LeakageVector =
	| "OutputChannel"
	| "ToolCall"
	| "DangerousCommand"
	| "NetworkExfil"
	| "FileExfil"
	| "AuthFailure";
export type SensitivityLevel =
	| "Public"
	| "Normal"
	| "Sensitive"
	| "HighlySensitive"
	| "Critical";

export interface TeeStatus {
	level: TeeLevel;
	backend: string;
	attestationExpiry: string | null;
	healthy: boolean;
}

export interface AuditEvent {
	id: string;
	severity: Severity;
	vector: LeakageVector;
	sessionId: string | null;
	summary: string;
	detail?: string;
	timestamp: number;
}

export interface AuditStats {
	total: number;
	bySeverity: Record<Severity, number>;
	byVector: Record<string, number>;
}

export interface AlertItem {
	id: string;
	severity: Severity;
	message: string;
	count: number;
	firstSeen: number;
	lastSeen: number;
}

export interface PrivacyDetection {
	type: string;
	sensitivity: SensitivityLevel;
	redacted: boolean;
	snippet: string;
}

export interface TaintEntry {
	id: string;
	source: string;
	label: string;
	propagations: number;
	createdAt: number;
	path: string[];
}

export type ChannelKind =
	| "telegram"
	| "slack"
	| "discord"
	| "feishu"
	| "dingtalk"
	| "wecom"
	| "webchat"
	| "whatsapp"
	| "teams"
	| "google_chat"
	| "signal";

export type ChannelStatus = "running" | "stopped" | "error" | "reconnecting";

export interface ChannelAgentConfig {
	model: string | null;
	permissionMode: "default" | "strict" | "trust";
	allowedTools: string[] | null; // null = all tools allowed
	blockedTools: string[];
}

export interface ChannelConfig {
	id: string;
	kind: ChannelKind;
	name: string;
	status: ChannelStatus;
	enabled: boolean;
	messagesTotal: number;
	messagesLast24h: number;
	lastActivity: number | null;
	// Credential fields (masked)
	credentials: Record<string, string>;
	dmPolicy: string;
	// Per-channel agent config (Phase 17)
	agentConfig: ChannelAgentConfig;
}

export interface CredentialHealth {
	[key: string]: "ok" | "expired" | "missing" | "unknown";
}

export interface SecurityState {
	tee: TeeStatus;
	auditEvents: AuditEvent[];
	auditStats: AuditStats;
	alerts: AlertItem[];
	taintEntries: TaintEntry[];
	todayBlocked: number;
	activeSessions: number;
	riskScore: number;
	firewallRules: string[];
	channels: ChannelConfig[];
	credentialHealth: CredentialHealth;
}

// =============================================================================
// State
// =============================================================================

const state = proxy<SecurityState>({
	tee: {
		level: "TeeHardware",
		backend: "—",
		attestationExpiry: null,
		healthy: false,
	},
	auditEvents: [],
	auditStats: {
		total: 0,
		bySeverity: { info: 0, warning: 0, high: 0, critical: 0 },
		byVector: {
			OutputChannel: 0,
			ToolCall: 0,
			DangerousCommand: 0,
			NetworkExfil: 0,
			FileExfil: 0,
			AuthFailure: 0,
		},
	},
	alerts: [],
	taintEntries: [],
	todayBlocked: 0,
	activeSessions: 0,
	riskScore: 0,
	firewallRules: [
		"api.anthropic.com",
		"api.openai.com",
		"generativelanguage.googleapis.com",
		"api.deepseek.com",
		"*.azure.com",
		"bedrock-runtime.*.amazonaws.com",
	],
	channels: [],
	credentialHealth: {},
});

// =============================================================================
// Actions
// =============================================================================

function addAuditEvent(event: AuditEvent) {
	state.auditEvents.unshift(event);
	state.auditStats.total += 1;
	state.auditStats.bySeverity[event.severity] += 1;
	state.auditStats.byVector[event.vector] =
		(state.auditStats.byVector[event.vector] || 0) + 1;
}

function clearAlerts() {
	state.alerts = [];
}

function dismissAlert(id: string) {
	const idx = state.alerts.findIndex((a) => a.id === id);
	if (idx >= 0) state.alerts.splice(idx, 1);
}

function addFirewallRule(domain: string) {
	if (!state.firewallRules.includes(domain)) {
		state.firewallRules.push(domain);
	}
}

function removeFirewallRule(domain: string) {
	const idx = state.firewallRules.indexOf(domain);
	if (idx >= 0) state.firewallRules.splice(idx, 1);
}

function toggleChannel(id: string) {
	const ch = state.channels.find((c) => c.id === id);
	if (!ch) return;
	ch.enabled = !ch.enabled;
	ch.status = ch.enabled ? "running" : "stopped";
}

function updateChannelCredential(id: string, key: string, value: string) {
	const ch = state.channels.find((c) => c.id === id);
	if (ch) ch.credentials[key] = value;
}

function updateChannelAgentConfig(
	id: string,
	config: Partial<ChannelAgentConfig>,
) {
	const ch = state.channels.find((c) => c.id === id);
	if (ch) Object.assign(ch.agentConfig, config);
}

function addChannel(channel: ChannelConfig) {
	state.channels.push(channel);
}

function removeChannel(id: string) {
	const idx = state.channels.findIndex((c) => c.id === id);
	if (idx >= 0) state.channels.splice(idx, 1);
}

export default {
	state,
	addAuditEvent,
	setAuditEvents(events: AuditEvent[]) {
		state.auditEvents = events;
	},
	setAuditStats(stats: AuditStats) {
		Object.assign(state.auditStats, stats);
	},
	setAlerts(alerts: AlertItem[]) {
		state.alerts = alerts;
	},
	clearAlerts,
	dismissAlert,
	addFirewallRule,
	removeFirewallRule,
	toggleChannel,
	updateChannelCredential,
	updateChannelAgentConfig,
	addChannel,
	removeChannel,
	setCredentialHealth(health: CredentialHealth) {
		Object.assign(state.credentialHealth, health);
	},
};
