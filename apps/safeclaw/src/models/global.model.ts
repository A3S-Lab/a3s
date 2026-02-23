import { User } from "@/typings";
import { proxy } from "valtio";

const state = proxy<{ user: User }>({
	user: {
		id: 1,
		nickname: "Roy Lin",
		email: "admin@elljs.com",
		avatar: "https://avatars.githubusercontent.com/u/19965768?v=4",
	},
});

const actions = {
	load: async () => {
		const user = state.user;
		return { user };
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
