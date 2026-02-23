/**
 * TipTap slash-command extension for triggering skills via "/"
 */
import { Extension } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { type SuggestionOptions } from "@tiptap/suggestion";

export interface SlashCommandItem {
	id: string;
	label: string;
	description: string;
	icon?: string;
}

export type SlashCommandOptions = {
	suggestion: Omit<SuggestionOptions<SlashCommandItem>, "editor">;
};

export const SlashCommandPluginKey = new PluginKey("slashCommand");

export const SlashCommand = Extension.create<SlashCommandOptions>({
	name: "slashCommand",

	addOptions() {
		return {
			suggestion: {
				char: "/",
				pluginKey: SlashCommandPluginKey,
				command: ({ editor, range, props }) => {
					// Delete the slash trigger text and insert a skill tag
					editor
						.chain()
						.focus()
						.deleteRange(range)
						.insertContent({
							type: "text",
							marks: [
								{
									type: "bold",
								},
							],
							text: `/${props.id} `,
						})
						.run();
				},
				allow: ({ state, range }) => {
					const text = state.doc.textBetween(
						range.from - 1,
						range.from,
						"\0",
						"\0",
					);
					// Only trigger at start of line or after whitespace
					return text === "" || text === "\0" || /\s/.test(text);
				},
			},
		};
	},

	addProseMirrorPlugins() {
		return [
			Suggestion({
				editor: this.editor,
				...this.options.suggestion,
			}),
		];
	},
});
