import { proxy } from "valtio";
import { nanoid } from "nanoid";

export interface WorkflowDoc {
	id: string;
	name: string;
	description?: string;
	createdAt: number;
	updatedAt: number;
	document: Record<string, unknown>;
}

const STORAGE_KEY = "safeclaw-workflows";

function load(): WorkflowDoc[] {
	try {
		return JSON.parse(localStorage.getItem(STORAGE_KEY) || "[]");
	} catch {
		return [];
	}
}

function persist(list: WorkflowDoc[]) {
	localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
}

const state = proxy<{ workflows: WorkflowDoc[] }>({
	workflows: load(),
});

const actions = {
	create(name: string, description?: string): WorkflowDoc {
		const wf: WorkflowDoc = {
			id: nanoid(),
			name,
			description,
			createdAt: Date.now(),
			updatedAt: Date.now(),
			document: {
				nodes: [
					{
						id: "start_0",
						type: "start",
						meta: { position: { x: 180, y: 300 } },
						data: { title: "开始" },
					},
					{
						id: "end_0",
						type: "end",
						meta: { position: { x: 680, y: 300 } },
						data: { title: "结束" },
					},
				],
				edges: [{ sourceNodeID: "start_0", targetNodeID: "end_0" }],
			},
		};
		state.workflows.unshift(wf);
		persist([...state.workflows]);
		return wf;
	},

	update(
		id: string,
		patch: Partial<Pick<WorkflowDoc, "name" | "description" | "document">>,
	) {
		const wf = state.workflows.find((w) => w.id === id);
		if (!wf) return;
		Object.assign(wf, { ...patch, updatedAt: Date.now() });
		persist([...state.workflows]);
	},

	remove(id: string) {
		const idx = state.workflows.findIndex((w) => w.id === id);
		if (idx >= 0) {
			state.workflows.splice(idx, 1);
			persist([...state.workflows]);
		}
	},

	get(id: string): WorkflowDoc | undefined {
		return state.workflows.find((w) => w.id === id);
	},
};

export default { state, ...actions };
