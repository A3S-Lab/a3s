/**
 * TipTap file-mention extension for selecting workspace files via "@"
 * This extends the existing @ mention to support both agents and files
 */
import { Extension } from "@tiptap/core";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { type SuggestionOptions } from "@tiptap/suggestion";

export interface FileMentionItem {
	id: string;
	label: string;
	description?: string;
	icon?: string;
	type: "file" | "directory";
	path: string;
}

export type FileMentionOptions = {
	suggestion: Omit<SuggestionOptions<FileMentionItem>, "editor">;
};

export const FileMentionPluginKey = new PluginKey("fileMention");

export const FileMention = Extension.create<FileMentionOptions>({
	name: "fileMention",

	addOptions() {
		return {
			suggestion: {
				char: "#",
				pluginKey: FileMentionPluginKey,
				command: ({ editor, range, props }) => {
					// Delete the trigger text and insert a file mention
					editor
						.chain()
						.focus()
						.deleteRange(range)
						.insertContent({
							type: "text",
							marks: [
								{
									type: "code",
								},
							],
							text: `${props.path} `,
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
