/** A3S Box — MicroVM runtime types */

export type BoxStatus =
	| "running"
	| "stopped"
	| "paused"
	| "created"
	| "exited"
	| "error";

export interface BoxInfo {
	id: string;
	name: string;
	image: string;
	status: BoxStatus;
	cpus: number;
	memory: string;
	created_at: number;
	started_at: number | null;
	ports: string[];
	labels: Record<string, string>;
	networks: string[];
	tee: boolean;
	restart_policy: string;
	health_status: "healthy" | "unhealthy" | "starting" | "none";
}

export interface BoxStats {
	id: string;
	name: string;
	cpu_percent: number;
	memory_usage: number;
	memory_limit: number;
	network_rx: number;
	network_tx: number;
	pids: number;
}

export interface BoxImage {
	id: string;
	repository: string;
	tag: string;
	size: number;
	created_at: number;
	digest: string;
}

export interface BoxNetwork {
	id: string;
	name: string;
	driver: string;
	scope: string;
	containers: number;
	isolation: "none" | "strict" | "custom";
	created_at: number;
}

export interface BoxVolume {
	name: string;
	driver: string;
	mountpoint: string;
	size: number;
	created_at: number;
	labels: Record<string, string>;
}

export interface BoxSnapshot {
	id: string;
	box_id: string;
	box_name: string;
	description: string;
	size: number;
	created_at: number;
}

export interface BoxSystemInfo {
	version: string;
	os: string;
	arch: string;
	cpus: number;
	memory_total: number;
	boxes_running: number;
	boxes_stopped: number;
	images_count: number;
	tee_available: boolean;
	tee_backend: string;
}

export interface BoxDiskUsage {
	images_size: number;
	containers_size: number;
	volumes_size: number;
	cache_size: number;
	total: number;
}
