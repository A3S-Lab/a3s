import { mergeAttributes, Node } from '@tiptap/core';

export const DocumentPageBreak = Node.create({
  name: 'pageBreak',
  group: 'block',
  atom: true,
  selectable: true,

  parseHTML() {
    return [{ tag: '[data-page-break]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      'div',
      mergeAttributes(HTMLAttributes, {
        'data-page-break': 'true',
        class: 'work-page-break',
        role: 'separator',
        'aria-label': '分页符',
      }),
    ];
  },

  addKeyboardShortcuts() {
    return {
      'Mod-Enter': () => this.editor.chain().insertContent({ type: this.name }).run(),
    };
  },
});
