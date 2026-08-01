import { Extension } from '@tiptap/core';
import { Plugin, PluginKey } from '@tiptap/pm/state';
import { Decoration, DecorationSet } from '@tiptap/pm/view';

const slashCommandHighlightKey = new PluginKey('slashCommandHighlight');
const knownCommands = new Set(['goal']);

export const SlashCommandHighlight = Extension.create({
  name: 'slashCommandHighlight',

  addProseMirrorPlugins() {
    return [
      new Plugin({
        key: slashCommandHighlightKey,
        props: {
          decorations(state) {
            const decorations: Decoration[] = [];
            state.doc.descendants((node, position) => {
              if (!node.isText || !node.text || node.marks.some((mark) => mark.type.name === 'code')) return;
              for (const range of slashCommandRanges(node.text)) {
                decorations.push(
                  Decoration.inline(position + range.from, position + range.to, {
                    class: 'composer-slash-command',
                  })
                );
              }
              for (const range of skillMentionRanges(node.text)) {
                decorations.push(
                  Decoration.inline(position + range.from, position + range.to, {
                    class: 'composer-skill-mention',
                  })
                );
              }
            });
            return DecorationSet.create(state.doc, decorations);
          },
        },
      }),
    ];
  },
});

export function slashCommandRanges(text: string): Array<{ from: number; to: number }> {
  const match = text.match(/^\/([A-Za-z][\w.-]*)/);
  const command = match?.[1]?.toLowerCase();
  if (!command || !knownCommands.has(command)) return [];
  return [{ from: 0, to: command.length + 1 }];
}

export function skillMentionRanges(text: string): Array<{ from: number; to: number }> {
  const ranges: Array<{ from: number; to: number }> = [];
  const pattern = /(?:^|[\s([{'"“（【])\$([\p{L}_][\p{L}\p{N}._-]*)/gu;
  for (const match of text.matchAll(pattern)) {
    if (match.index === undefined) continue;
    const mention = `$${match[1]}`;
    const from = match.index + match[0].length - mention.length;
    ranges.push({ from, to: from + mention.length });
  }
  return ranges;
}
