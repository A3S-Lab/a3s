/**
 * A3S Box state management (Valtio).
 * Manages MicroVM boxes, images, networks, volumes, snapshots.
 */
import { proxy } from "valtio";
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
import * as boxApi from "@/lib/box-api";

interface BoxState {
	boxes: BoxInfo[];
	stats: BoxStats[];
	images: BoxImage[];
	networks: BoxNetwork[];
	volumes: BoxVolume[];
	snapshots: BoxSnapshot[];
	systemInfo: BoxSystemInfo | null;
	diskUsage: BoxDiskUsage | null;
	loading: {
		boxes: boolean;
		images: boolean;
		networks: boolean;
		volumes: boolean;
		snapshots: boolean;
		system: boolean;
	};
}

const state = proxy<BoxState>({
	boxes: [],
	stats: [],
	images: [],
	networks: [],
	volumes: [],
	snapshots: [],
	systemInfo: null,
	diskUsage: null,
	loading: {
		boxes: false,
		images: false,
		networks: false,
		volumes: false,
		snapshots: false,
		system: false,
	},
});

const actions = {
	async fetchBoxes() {
		state.loading.boxes = true;
		try {
			const [boxes, stats] = await Promise.all([
				boxApi.listBoxes(),
				boxApi.getBoxStats().catch(() => []),
			]);
			state.boxes = boxes;
			state.stats = stats;
		} catch {
			/* backend may not be available */
		} finally {
			state.loading.boxes = false;
		}
	},

	async fetchImages() {
		state.loading.images = true;
		try {
			state.images = await boxApi.listImages();
		} catch {
			/* ignore */
		} finally {
			state.loading.images = false;
		}
	},

	async fetchNetworks() {
		state.loading.networks = true;
		try {
			state.networks = await boxApi.listNetworks();
		} catch {
			/* ignore */
		} finally {
			state.loading.networks = false;
		}
	},

	async fetchVolumes() {
		state.loading.volumes = true;
		try {
			state.volumes = await boxApi.listVolumes();
		} catch {
			/* ignore */
		} finally {
			state.loading.volumes = false;
		}
	},

	async fetchSnapshots() {
		state.loading.snapshots = true;
		try {
			state.snapshots = await boxApi.listSnapshots();
		} catch {
			/* ignore */
		} finally {
			state.loading.snapshots = false;
		}
	},

	async fetchSystemInfo() {
		state.loading.system = true;
		try {
			const [info, disk] = await Promise.all([
				boxApi.getSystemInfo(),
				boxApi.getDiskUsage().catch(() => null),
			]);
			state.systemInfo = info;
			state.diskUsage = disk;
		} catch {
			/* ignore */
		} finally {
			state.loading.system = false;
		}
	},

	async stopBox(id: string) {
		await boxApi.stopBox(id);
		await actions.fetchBoxes();
	},

	async startBox(id: string) {
		await boxApi.startBox(id);
		await actions.fetchBoxes();
	},

	async restartBox(id: string) {
		await boxApi.restartBox(id);
		await actions.fetchBoxes();
	},

	async removeBox(id: string, force = false) {
		await boxApi.removeBox(id, force);
		await actions.fetchBoxes();
	},

	async pauseBox(id: string) {
		await boxApi.pauseBox(id);
		await actions.fetchBoxes();
	},

	async unpauseBox(id: string) {
		await boxApi.unpauseBox(id);
		await actions.fetchBoxes();
	},

	async removeImage(id: string, force = false) {
		await boxApi.removeImage(id, force);
		await actions.fetchImages();
	},

	async pullImage(image: string) {
		await boxApi.pullImage(image);
		await actions.fetchImages();
	},

	async pruneImages() {
		const result = await boxApi.pruneImages();
		await actions.fetchImages();
		return result;
	},

	async createNetwork(params: { name: string; driver?: string; isolation?: string }) {
		await boxApi.createNetwork(params);
		await actions.fetchNetworks();
	},

	async removeNetwork(id: string) {
		await boxApi.removeNetwork(id);
		await actions.fetchNetworks();
	},

	async createVolume(params: { name: string; driver?: string }) {
		await boxApi.createVolume(params);
		await actions.fetchVolumes();
	},

	async removeVolume(name: string) {
		await boxApi.removeVolume(name);
		await actions.fetchVolumes();
	},

	async pruneVolumes() {
		const result = await boxApi.pruneVolumes();
		await actions.fetchVolumes();
		return result;
	},

	async removeSnapshot(id: string) {
		await boxApi.removeSnapshot(id);
		await actions.fetchSnapshots();
	},

	async restoreSnapshot(id: string) {
		await boxApi.restoreSnapshot(id);
		await actions.fetchSnapshots();
	},

	async systemPrune() {
		const result = await boxApi.systemPrune();
		await Promise.all([
			actions.fetchBoxes(),
			actions.fetchImages(),
			actions.fetchVolumes(),
			actions.fetchSystemInfo(),
		]);
		return result;
	},
};

export default { state, ...actions };
