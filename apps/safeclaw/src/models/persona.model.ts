import { proxy } from "valtio";
import {
	BUILTIN_PERSONAS,
	DEFAULT_PERSONA_ID,
	getPersonaById,
} from "@/lib/builtin-personas";
import { agentApi } from "@/lib/agent-api";
import type { AgentPersona } from "@/typings/persona";
import type { PersonaInfo } from "@/typings/agent";

const STORAGE_KEY = "safeclaw-session-personas";
const OVERRIDES_KEY = "safeclaw-persona-overrides";
const CUSTOM_PERSONAS_KEY = "safeclaw-custom-personas";

type PersonaOverride = Pick<
	AgentPersona,
	"defaultModel" | "defaultPermissionMode" | "systemPrompt"
>;

interface MarketState {
	items: AgentPersona[];
	total: number;
	page: number;
	pageSize: number;
	loading: boolean;
	search: string;
	tags: string[];
}

interface PersonaStoreState {
	/** Maps session_id → persona_id */
	sessionPersonas: Record<string, string>;
	/** Custom (user-created) personas */
	customPersonas: AgentPersona[];
	/** Server-side personas loaded from backend skill registry */
	serverPersonas: AgentPersona[];
	/** User overrides for builtin/server personas (persisted) */
	personaOverrides: Record<string, PersonaOverride>;
	/** Marketplace pagination state */
	market: MarketState;
}

function loadOverrides(): Record<string, PersonaOverride> {
	try {
		return JSON.parse(localStorage.getItem(OVERRIDES_KEY) || "{}");
	} catch {
		return {};
	}
}

function loadCustomPersonas(): AgentPersona[] {
	try {
		return JSON.parse(localStorage.getItem(CUSTOM_PERSONAS_KEY) || "[]");
	} catch {
		return [];
	}
}

function persistCustomPersonas() {
	try {
		localStorage.setItem(
			CUSTOM_PERSONAS_KEY,
			JSON.stringify(state.customPersonas),
		);
	} catch {
		/* ignore */
	}
}

const state = proxy<PersonaStoreState>({
	sessionPersonas: JSON.parse(localStorage.getItem(STORAGE_KEY) || "{}"),
	customPersonas: loadCustomPersonas(),
	serverPersonas: [],
	personaOverrides: loadOverrides(),
	market: {
		items: [],
		total: 0,
		page: 1,
		pageSize: 20,
		loading: false,
		search: "",
		tags: [],
	},
});

function persistSessionPersonas() {
	localStorage.setItem(STORAGE_KEY, JSON.stringify(state.sessionPersonas));
}

/** Convert a backend PersonaInfo to AgentPersona for UI use */
function serverPersonaToAgentPersona(p: PersonaInfo): AgentPersona {
	return {
		id: p.id,
		name: p.name,
		description: p.description,
		avatar: {},
		systemPrompt: "",
		builtin: false,
		tags: p.tags,
	};
}

/** Apply stored overrides on top of a persona */
function applyOverrides(persona: AgentPersona): AgentPersona {
	const ov = state.personaOverrides[persona.id];
	if (!ov) return persona;
	return { ...persona, ...ov };
}

const actions = {
	/** Assign a persona to a session */
	setSessionPersona(sessionId: string, personaId: string) {
		state.sessionPersonas[sessionId] = personaId;
		persistSessionPersonas();
	},

	/** Remove persona mapping when session is deleted */
	removeSessionPersona(sessionId: string) {
		delete state.sessionPersonas[sessionId];
		persistSessionPersonas();
	},

	/** Get the persona for a session, falling back to default */
	getSessionPersona(sessionId: string): AgentPersona {
		const personaId = state.sessionPersonas[sessionId] || DEFAULT_PERSONA_ID;
		const base =
			getPersonaById(personaId) ??
			state.customPersonas.find((p) => p.id === personaId) ??
			state.serverPersonas.find((p) => p.id === personaId) ??
			BUILTIN_PERSONAS[0];
		return applyOverrides(base);
	},

	/** Get all available personas (builtin + server + custom) with overrides applied */
	getAllPersonas(): AgentPersona[] {
		return [
			...BUILTIN_PERSONAS,
			...state.serverPersonas,
			...state.customPersonas,
		].map(applyOverrides);
	},

	/** Update persona defaults (works for all personas; persisted as overrides for builtins) */
	updatePersonaDefaults(personaId: string, patch: Partial<PersonaOverride>) {
		const custom = state.customPersonas.find((p) => p.id === personaId);
		if (custom) {
			Object.assign(custom, patch);
			persistCustomPersonas();
		} else {
			state.personaOverrides[personaId] = {
				...state.personaOverrides[personaId],
				...patch,
			};
			try {
				localStorage.setItem(
					OVERRIDES_KEY,
					JSON.stringify(state.personaOverrides),
				);
			} catch {
				/* ignore */
			}
		}
	},

	/** Add a custom persona (user-created) */
	addCustomPersona(persona: AgentPersona) {
		state.customPersonas.push({ ...persona, builtin: false });
		persistCustomPersonas();
	},

	/** Update a custom persona */
	updateCustomPersona(personaId: string, patch: Partial<AgentPersona>) {
		const idx = state.customPersonas.findIndex((p) => p.id === personaId);
		if (idx >= 0) {
			Object.assign(state.customPersonas[idx], patch);
			persistCustomPersonas();
		}
	},

	/** Delete a custom persona */
	deleteCustomPersona(personaId: string) {
		const idx = state.customPersonas.findIndex((p) => p.id === personaId);
		if (idx >= 0) {
			state.customPersonas.splice(idx, 1);
			persistCustomPersonas();
		}
	},

	/** Load server-side personas from the backend skill registry */
	async loadServerPersonas() {
		try {
			const personas: PersonaInfo[] = await agentApi.listPersonas();
			state.serverPersonas = personas.map(serverPersonaToAgentPersona);
		} catch {
			// Backend may not have personas configured — silently ignore
		}
	},

	/** Fetch marketplace personas with pagination, search, and tag filtering */
	async fetchMarketPersonas(params?: {
		page?: number;
		search?: string;
		tags?: string[];
		reset?: boolean;
	}) {
		const page = params?.page ?? 1;
		const search = params?.search ?? state.market.search;
		const tags = params?.tags ?? state.market.tags;

		state.market.loading = true;
		state.market.search = search;
		state.market.tags = tags;

		try {
			const result = await agentApi.listMarketPersonas({
				page,
				page_size: state.market.pageSize,
				search: search || undefined,
				tags: tags.length > 0 ? tags : undefined,
			});
			const items = result.items.map(serverPersonaToAgentPersona);
			if (params?.reset || page === 1) {
				state.market.items = items;
			} else {
				// Append for infinite scroll, deduplicate by id
				const existingIds = new Set(state.market.items.map((p) => p.id));
				state.market.items.push(
					...items.filter((p) => !existingIds.has(p.id)),
				);
			}
			state.market.total = result.total;
			state.market.page = result.page;
		} catch {
			// If backend doesn't support market endpoint, fall back to local personas
			if (page === 1) {
				state.market.items = actions.getAllPersonas();
				state.market.total = state.market.items.length;
				state.market.page = 1;
			}
		} finally {
			state.market.loading = false;
		}
	},

	/** Reset marketplace state */
	resetMarket() {
		state.market.items = [];
		state.market.total = 0;
		state.market.page = 1;
		state.market.loading = false;
		state.market.search = "";
		state.market.tags = [];
	},
};

export default { state, ...actions };
