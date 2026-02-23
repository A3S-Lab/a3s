/**
 * Unified HTTP client with Tauri plugin-http support.
 * Replaces duplicated httpFetch/safeFetch/apiUrl across:
 * - security-api.ts
 * - users/index.tsx
 * - memory/index.tsx
 */
import constants from "@/constants";

const IS_TAURI =
	typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Fetch that auto-selects Tauri plugin-http or native fetch */
export async function httpFetch(
	url: string,
	init?: RequestInit,
): Promise<Response> {
	if (IS_TAURI) {
		const { fetch: tauriFetch } = await import("@tauri-apps/plugin-http");
		return tauriFetch(url, init);
	}
	return fetch(url, init);
}

/** Build a gateway API URL from a path like "/users" → "http://…/api/v1/users" */
export function apiUrl(path: string): string {
	return `${constants.gatewayUrl}/api/v1${path}`;
}

/**
 * Fetch JSON from the gateway API with error handling.
 * Throws descriptive errors on non-2xx responses.
 */
export async function apiFetch<T = unknown>(
	path: string,
	init?: RequestInit,
): Promise<T> {
	const res = await httpFetch(apiUrl(path), init);
	if (!res.ok) {
		const text = await res.text().catch(() => "");
		throw new Error(
			`${init?.method ?? "GET"} ${path} → ${res.status}: ${text}`,
		);
	}
	const ct = res.headers.get("content-type") ?? "";
	if (ct.includes("application/json")) return res.json();
	return null as T;
}

/** Shorthand for JSON POST/PATCH/PUT body */
export function jsonBody(method: string, body: unknown): RequestInit {
	return {
		method,
		headers: { "Content-Type": "application/json" },
		body: JSON.stringify(body),
	};
}
