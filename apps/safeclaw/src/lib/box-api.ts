/**
 * A3S Box API client.
 * Endpoints for managing MicroVM boxes, images, networks, volumes, snapshots.
 */
import { apiFetch, jsonBody } from "@/lib/http";
import type {
	BoxInfo,
	BoxStats,
	BoxImage,
	BoxNetwork,
	BoxVolume,
	BoxSnapshot,
	BoxSystemInfo,
	BoxDiskUsage,
} from "@/typings/box";

// ── Boxes ────────────────────────────────────────────────────────────

export async function listBoxes(all = true): Promise<BoxInfo[]> {
	return apiFetch(`/box/containers${all ? "?all=true" : ""}`);
}

export async function getBoxStats(): Promise<BoxStats[]> {
	return apiFetch("/box/stats");
}

export async function stopBox(id: string): Promise<void> {
	await apiFetch(`/box/containers/${id}/stop`, jsonBody("POST", {}));
}

export async function startBox(id: string): Promise<void> {
	await apiFetch(`/box/containers/${id}/start`, jsonBody("POST", {}));
}

export async function restartBox(id: string): Promise<void> {
	await apiFetch(`/box/containers/${id}/restart`, jsonBody("POST", {}));
}

export async function removeBox(id: string, force = false): Promise<void> {
	await apiFetch(
		`/box/containers/${id}${force ? "?force=true" : ""}`,
		{ method: "DELETE" },
	);
}

export async function pauseBox(id: string): Promise<void> {
	await apiFetch(`/box/containers/${id}/pause`, jsonBody("POST", {}));
}

export async function unpauseBox(id: string): Promise<void> {
	await apiFetch(`/box/containers/${id}/unpause`, jsonBody("POST", {}));
}

// ── Images ───────────────────────────────────────────────────────────

export async function listImages(): Promise<BoxImage[]> {
	return apiFetch("/box/images");
}

export async function removeImage(id: string, force = false): Promise<void> {
	await apiFetch(
		`/box/images/${id}${force ? "?force=true" : ""}`,
		{ method: "DELETE" },
	);
}

export async function pullImage(image: string): Promise<void> {
	await apiFetch("/box/images/pull", jsonBody("POST", { image }));
}

export async function pruneImages(): Promise<{ reclaimed: number }> {
	return apiFetch("/box/images/prune", jsonBody("POST", {}));
}

// ── Networks ─────────────────────────────────────────────────────────

export async function listNetworks(): Promise<BoxNetwork[]> {
	return apiFetch("/box/networks");
}

export async function createNetwork(params: {
	name: string;
	driver?: string;
	isolation?: string;
}): Promise<BoxNetwork> {
	return apiFetch("/box/networks", jsonBody("POST", params));
}

export async function removeNetwork(id: string): Promise<void> {
	await apiFetch(`/box/networks/${id}`, { method: "DELETE" });
}

// ── Volumes ──────────────────────────────────────────────────────────

export async function listVolumes(): Promise<BoxVolume[]> {
	return apiFetch("/box/volumes");
}

export async function createVolume(params: {
	name: string;
	driver?: string;
	labels?: Record<string, string>;
}): Promise<BoxVolume> {
	return apiFetch("/box/volumes", jsonBody("POST", params));
}

export async function removeVolume(name: string): Promise<void> {
	await apiFetch(`/box/volumes/${name}`, { method: "DELETE" });
}

export async function pruneVolumes(): Promise<{ reclaimed: number }> {
	return apiFetch("/box/volumes/prune", jsonBody("POST", {}));
}

// ── Snapshots ────────────────────────────────────────────────────────

export async function listSnapshots(): Promise<BoxSnapshot[]> {
	return apiFetch("/box/snapshots");
}

export async function removeSnapshot(id: string): Promise<void> {
	await apiFetch(`/box/snapshots/${id}`, { method: "DELETE" });
}

export async function restoreSnapshot(id: string): Promise<void> {
	await apiFetch(`/box/snapshots/${id}/restore`, jsonBody("POST", {}));
}

// ── System ───────────────────────────────────────────────────────────

export async function getSystemInfo(): Promise<BoxSystemInfo> {
	return apiFetch("/box/info");
}

export async function getDiskUsage(): Promise<BoxDiskUsage> {
	return apiFetch("/box/df");
}

export async function systemPrune(): Promise<{ reclaimed: number }> {
	return apiFetch("/box/system/prune", jsonBody("POST", {}));
}
