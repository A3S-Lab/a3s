import { User } from "@/typings";
import { proxy, subscribe } from "valtio";

// =============================================================================
// Profile persistence
// =============================================================================

const PROFILE_KEY = "safeclaw-profile";

interface ProfileData {
	nickname: string;
	avatar: string;
	isOnboarded: boolean;
}

const DEFAULT_PROFILE: ProfileData = {
	nickname: "",
	avatar: "/safeclaw.jpg",
	isOnboarded: false,
};

function loadProfile(): ProfileData {
	try {
		const raw = localStorage.getItem(PROFILE_KEY);
		if (raw) return { ...DEFAULT_PROFILE, ...JSON.parse(raw) };
	} catch {
		// ignore
	}
	return DEFAULT_PROFILE;
}

const saved = loadProfile();

const state = proxy<{ user: User; isOnboarded: boolean }>({
	user: {
		id: 1,
		nickname: saved.nickname,
		email: "",
		avatar: saved.avatar,
	},
	isOnboarded: saved.isOnboarded,
});

subscribe(state, () => {
	try {
		localStorage.setItem(
			PROFILE_KEY,
			JSON.stringify({
				nickname: state.user.nickname,
				avatar: state.user.avatar,
				isOnboarded: state.isOnboarded,
			}),
		);
	} catch {
		// Storage unavailable
	}
});

const actions = {
	setProfile: (nickname: string, avatar: string) => {
		state.user.nickname = nickname;
		state.user.avatar = avatar || DEFAULT_PROFILE.avatar;
		state.isOnboarded = true;
	},
	load: async () => {
		return { user: state.user };
	},
	login: async () => {
		window.location.href = "/";
	},
	logout: async () => {
		window.location.href = "/login";
	},
};

export default {
	state,
	...actions,
};
