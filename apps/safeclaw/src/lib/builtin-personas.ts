import type { AgentPersona } from "@/typings/persona";

export const BUILTIN_PERSONAS: AgentPersona[] = [
	{
		id: "super-admin",
		name: "超级管理员",
		description: "全能 AI 助手，可处理任意任务",
		tags: [],
		avatar: {
			sex: "man",
			faceColor: "#F9C9B6",
			earSize: "small",
			hairColor: "#000",
			hairStyle: "thick",
			hatStyle: "none",
			eyeStyle: "oval",
			glassesStyle: "none",
			noseStyle: "short",
			mouthStyle: "smile",
			shirtStyle: "polo",
			shirtColor: "#0064FA",
			bgColor: "#E0EDFF",
		},
		systemPrompt: "",
		builtin: true,
		undeletable: true,
	},
];

export function getPersonaById(id: string): AgentPersona | undefined {
	return BUILTIN_PERSONAS.find((p) => p.id === id);
}

export const DEFAULT_PERSONA_ID = "super-admin";
