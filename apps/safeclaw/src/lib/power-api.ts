import { httpFetch } from "@/lib/http";
import type { ProviderConfig } from "@/models/settings.model";

export interface PowerModelInfo {
	id: string;
	object: string;
	created: number;
	owned_by: string;
}

export interface PowerPullEvent {
	status:
		| "already_exists"
		| "already_pulling"
		| "resuming"
		| "downloading"
		| "verifying"
		| "success";
	name?: string;
	id?: string;
	offset?: number;
	completed?: number;
	total?: number;
}

export interface PowerRegisterRequest {
	name: string;
	path: string;
	format?: "gguf" | "safetensors" | "huggingface";
}

export interface PowerDiagnosticResult {
	model: string;
	latencyMs: number;
	tokensPerSecond: number;
	ok: boolean;
}

export interface PowerHealthInfo {
	status: string;
	version?: string;
	uptimeSeconds?: number;
	loadedModels?: number;
	tee?: {
		enabled?: boolean;
		type?: string;
		modelsVerified?: boolean;
		attestationAvailable?: boolean;
	};
}

export interface PowerPullStateInfo {
	name: string;
	status: "pulling" | "done" | "failed";
	completed: number;
	total: number;
	error?: string;
	started_at?: string;
	finished_at?: string;
}

function toPowerRoot(provider?: ProviderConfig): string {
	const raw = provider?.baseUrl?.trim() || "http://127.0.0.1:11435/v1";
	const normalized = raw.replace(/\/+$/, "");
	return normalized.endsWith("/v1") ? normalized.slice(0, -3) : normalized;
}

async function safeFetch(url: string, init?: RequestInit): Promise<Response> {
	const res = await httpFetch(url, init);
	if (!res.ok) {
		const text = await res.text().catch(() => "");
		throw new Error(
			`${init?.method ?? "GET"} ${url} -> ${res.status}: ${text}`,
		);
	}
	return res;
}

export const powerApi = {
	async registerModel(
		payload: PowerRegisterRequest,
		provider?: ProviderConfig,
	): Promise<PowerModelInfo> {
		const root = toPowerRoot(provider);
		const res = await safeFetch(`${root}/v1/models`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify(payload),
		});
		return (await res.json()) as PowerModelInfo;
	},

	async health(provider?: ProviderConfig): Promise<boolean> {
		const info = await this.healthInfo(provider);
		return info.status === "ok";
	},

	async healthInfo(provider?: ProviderConfig): Promise<PowerHealthInfo> {
		const root = toPowerRoot(provider);
		const res = await safeFetch(`${root}/health`);
		const json = (await res.json()) as {
			status?: string;
			version?: string;
			uptime_seconds?: number;
			loaded_models?: number;
			tee?: {
				enabled?: boolean;
				type?: string;
				models_verified?: boolean;
				attestation_available?: boolean;
			};
		};
		return {
			status: json.status || "unknown",
			version: json.version,
			uptimeSeconds: json.uptime_seconds,
			loadedModels: json.loaded_models,
			tee: json.tee
				? {
						enabled: json.tee.enabled,
						type: json.tee.type,
						modelsVerified: json.tee.models_verified,
						attestationAvailable: json.tee.attestation_available,
					}
				: undefined,
		};
	},

	async diagnoseModel(
		modelId: string,
		provider?: ProviderConfig,
	): Promise<PowerDiagnosticResult> {
		const root = toPowerRoot(provider);
		const startedAt = Date.now();
		const res = await safeFetch(`${root}/v1/chat/completions`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({
				model: modelId,
				stream: false,
				temperature: 0,
				max_tokens: 24,
				messages: [{ role: "user", content: "Reply with OK." }],
			}),
		});
		const latencyMs = Date.now() - startedAt;
		const json = (await res.json()) as {
			usage?: { completion_tokens?: number };
			choices?: Array<{ message?: { content?: unknown } }>;
		};
		const completionTokens = Number(json.usage?.completion_tokens || 0);
		const tokensPerSecond =
			latencyMs > 0 ? (completionTokens * 1000) / latencyMs : completionTokens;
		const content = json.choices?.[0]?.message?.content;
		const ok =
			typeof content === "string"
				? content.toLowerCase().includes("ok")
				: completionTokens > 0;

		return {
			model: modelId,
			latencyMs,
			tokensPerSecond,
			ok,
		};
	},

	async listModels(provider?: ProviderConfig): Promise<PowerModelInfo[]> {
		const root = toPowerRoot(provider);
		const res = await safeFetch(`${root}/v1/models`);
		const json = (await res.json()) as { data?: PowerModelInfo[] };
		return json.data || [];
	},

	async deleteModel(modelId: string, provider?: ProviderConfig): Promise<void> {
		const root = toPowerRoot(provider);
		await safeFetch(`${root}/v1/models/${encodeURIComponent(modelId)}`, {
			method: "DELETE",
		});
	},

	async pullModel(
		modelName: string,
		onEvent: (event: PowerPullEvent) => void,
		provider?: ProviderConfig,
	): Promise<void> {
		const root = toPowerRoot(provider);
		const res = await safeFetch(`${root}/v1/models/pull`, {
			method: "POST",
			headers: { "Content-Type": "application/json" },
			body: JSON.stringify({ name: modelName }),
		});

		if (!res.body) {
			throw new Error("Power pull stream is not available");
		}

		const reader = res.body.getReader();
		const decoder = new TextDecoder();
		let buffer = "";

		while (true) {
			const { done, value } = await reader.read();
			if (done) break;
			buffer += decoder.decode(value, { stream: true });

			const frames = buffer.split("\n\n");
			buffer = frames.pop() || "";

			for (const frame of frames) {
				for (const line of frame.split("\n")) {
					if (!line.startsWith("data:")) continue;
					const payload = line.slice(5).trim();
					if (!payload) continue;
					try {
						onEvent(JSON.parse(payload) as PowerPullEvent);
					} catch {
						// Ignore malformed events and continue streaming.
					}
				}
			}
		}
	},

	async pullStatus(
		modelName: string,
		provider?: ProviderConfig,
	): Promise<PowerPullStateInfo | null> {
		const root = toPowerRoot(provider);
		const res = await httpFetch(
			`${root}/v1/models/pull/${encodeURIComponent(modelName)}/status`,
		);
		if (res.status === 404) return null;
		if (!res.ok) {
			const text = await res.text().catch(() => "");
			throw new Error(
				`GET ${root}/v1/models/pull/.../status -> ${res.status}: ${text}`,
			);
		}
		return (await res.json()) as PowerPullStateInfo;
	},

	/**
	 * Stream server log entries from GET /v1/logs (SSE).
	 *
	 * Calls `onEntry` for each log line received — first the buffered history,
	 * then live entries as they are emitted.  Returns an `AbortController` that
	 * the caller should `.abort()` to stop streaming.
	 */
	streamLogs(
		onEntry: (entry: PowerLogEntry) => void,
		provider?: ProviderConfig,
	): AbortController {
		const root = toPowerRoot(provider);
		const ctrl = new AbortController();

		(async () => {
			try {
				const res = await httpFetch(`${root}/v1/logs`, {
					signal: ctrl.signal,
				});
				if (!res.ok || !res.body) return;

				const reader = res.body.getReader();
				const decoder = new TextDecoder();
				let buf = "";

				while (true) {
					const { done, value } = await reader.read();
					if (done) break;
					buf += decoder.decode(value, { stream: true });

					const lines = buf.split("\n");
					buf = lines.pop() ?? "";

					for (const line of lines) {
						if (!line.startsWith("data:")) continue;
						const raw = line.slice(5).trim();
						if (!raw) continue;
						try {
							onEntry(JSON.parse(raw) as PowerLogEntry);
						} catch {
							// ignore malformed frames
						}
					}
				}
			} catch {
				// AbortError or connection failure — silently stop
			}
		})();

		return ctrl;
	},
};

export interface PowerLogEntry {
	/** ISO-8601 timestamp with milliseconds */
	ts: string;
	/** "ERROR" | "WARN" | "INFO" | "DEBUG" | "TRACE" */
	level: string;
	/** Tracing target (module path) */
	target: string;
	/** Log message */
	message: string;
}
