/**
 * Workflow model — thin reactive cache over the SafeClaw backend API.
 *
 * All mutations go through the API first; on success the local Valtio state
 * is updated to reflect the server response.  The `load()` action fetches the
 * full list from the backend and should be called once on app startup.
 */
import { proxy } from "valtio";
import { workflowApi, type WorkflowDoc } from "@/lib/agent-api";

export type { WorkflowDoc };

const state = proxy<{ workflows: WorkflowDoc[]; loaded: boolean }>({
	workflows: [],
	loaded: false,
});

const actions = {
	/** Fetch the full workflow list from the backend and populate local state. */
	async load(): Promise<void> {
		try {
			const list = await workflowApi.list();
			state.workflows = list;
		} catch (e) {
			console.warn("Failed to load workflows from backend:", e);
		} finally {
			state.loaded = true;
		}
	},

	/** Create a new workflow and return the server-assigned document. */
	async create(name: string, description?: string): Promise<WorkflowDoc> {
		const wf = await workflowApi.create({ name, description });
		state.workflows.unshift(wf);
		return wf;
	},

	/** Update name, description, document, or session_id. */
	async update(
		id: string,
		patch: Partial<
			Pick<WorkflowDoc, "name" | "description" | "document" | "session_id">
		>,
	): Promise<void> {
		const updated = await workflowApi.update(id, patch);
		const idx = state.workflows.findIndex((w) => w.id === id);
		if (idx >= 0) {
			state.workflows[idx] = updated;
		}
	},

	/** Delete a workflow. */
	async remove(id: string): Promise<void> {
		await workflowApi.remove(id);
		const idx = state.workflows.findIndex((w) => w.id === id);
		if (idx >= 0) {
			state.workflows.splice(idx, 1);
		}
	},

	/** Read a single workflow from the local cache (does not hit the network). */
	get(id: string): WorkflowDoc | undefined {
		return state.workflows.find((w) => w.id === id);
	},

	/** Bind a session to a workflow (persisted to backend). */
	async setSessionId(id: string, sessionId: string): Promise<void> {
		await actions.update(id, { session_id: sessionId });
	},
};

export default { state, ...actions };
