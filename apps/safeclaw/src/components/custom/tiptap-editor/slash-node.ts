/**
 * ProseMirror inline atom node for slash commands.
 * Renders as a styled, non-editable tag (like @mention).
 * renderText ensures editor.getText() returns "/<id>" for message submission.
 */
import { Node, mergeAttributes } from "@tiptap/core";

export const SlashCommandNode = Node.create({
	name: "slashCommand",
	group: "inline",
	inline: true,
	atom: true,

	addAttributes() {
		return {
			id: { default: null },
			label: { default: null },
		};
	},

	parseHTML() {
		return [{ tag: "span[data-slash]" }];
	},

	renderHTML({ node, HTMLAttributes }) {
		const label = node.attrs.label ?? node.attrs.id;
		return [
			"span",
			mergeAttributes(HTMLAttributes, {
				"data-slash": "",
				class: "tiptap-slash-command",
			}),
			`/${label}`,
		];
	},

	renderText({ node }) {
		return `/${node.attrs.label ?? node.attrs.id}`;
	},
});
