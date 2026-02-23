import { User } from "./user";

export interface Message {
	id: string;
	role: "user" | "assistant";
	content: string;
	type: string;
	createdAt: string;
	parts?: Array<{ type: string; text: string }>;
	user?: User;
	agent?: {
		id: string;
		name: string;
		[key: string]: unknown;
	};
}
