import { proxy } from "valtio";

export interface Repo {
	id: string;
	name: string;
	path: string;
	description: string;
	language: string;
	pinned: boolean;
	addedAt: number;
	/** Last time the user opened this repo in an agent session */
	lastOpenedAt: number | null;
}

const STORAGE_KEY = "safeclaw-repos";

function load(): Repo[] {
	try {
		const stored = JSON.parse(
			localStorage.getItem(STORAGE_KEY) || "[]",
		) as Repo[];
		// Filter out legacy mock entries
		const filtered = stored.filter((r) => !r.id.startsWith("mock-"));
		if (filtered.length !== stored.length) {
			localStorage.setItem(STORAGE_KEY, JSON.stringify(filtered));
		}
		return filtered;
	} catch {
		return [];
	}
}

function persist() {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(state.repos));
	} catch {
		/* ignore */
	}
}

const state = proxy<{ repos: Repo[] }>({ repos: load() });

const actions = {
	add(repo: Omit<Repo, "id" | "addedAt" | "lastOpenedAt">) {
		state.repos.push({
			...repo,
			id: crypto.randomUUID(),
			addedAt: Date.now(),
			lastOpenedAt: null,
		});
		persist();
	},

	remove(id: string) {
		const idx = state.repos.findIndex((r) => r.id === id);
		if (idx >= 0) {
			state.repos.splice(idx, 1);
			persist();
		}
	},

	update(id: string, patch: Partial<Omit<Repo, "id" | "addedAt">>) {
		const repo = state.repos.find((r) => r.id === id);
		if (repo) {
			Object.assign(repo, patch);
			persist();
		}
	},

	togglePin(id: string) {
		const repo = state.repos.find((r) => r.id === id);
		if (repo) {
			repo.pinned = !repo.pinned;
			persist();
		}
	},

	markOpened(id: string) {
		const repo = state.repos.find((r) => r.id === id);
		if (repo) {
			repo.lastOpenedAt = Date.now();
			persist();
		}
	},
};

export default { state, ...actions };
