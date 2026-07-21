import { mergeAttributes, Node } from '@tiptap/core';
import {
  documentFieldInstruction,
  documentFieldKind,
  documentFieldLabel,
  docxDocumentFieldKind,
} from './work-document-fields';

export const DocumentField = Node.create({
  name: 'documentField',
  inline: true,
  group: 'inline',
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      id: hiddenAttribute(''),
      kind: hiddenAttribute('page'),
      instruction: hiddenAttribute('PAGE'),
      display: hiddenAttribute('1'),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-document-field]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          const instruction = node.dataset.fieldInstruction?.trim() ?? '';
          const kind = documentFieldKind(node.dataset.fieldKind) ?? docxDocumentFieldKind(instruction) ?? 'page';
          return {
            id: node.dataset.fieldId ?? '',
            kind,
            instruction: instruction || documentFieldInstruction(kind),
            display: node.dataset.fieldDisplay?.trim() || node.textContent?.trim() || documentFieldLabel(kind),
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const instruction = typeof node.attrs.instruction === 'string' ? node.attrs.instruction.trim() : '';
    const kind = documentFieldKind(node.attrs.kind) ?? docxDocumentFieldKind(instruction) ?? 'page';
    const display =
      typeof node.attrs.display === 'string' && node.attrs.display.trim()
        ? node.attrs.display.trim()
        : documentFieldLabel(kind);
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-document-field': 'true',
        'data-field-id': typeof node.attrs.id === 'string' ? node.attrs.id : '',
        'data-field-kind': kind,
        'data-field-instruction': instruction || documentFieldInstruction(kind),
        'data-field-display': display,
        class: 'work-document-field',
        title: documentFieldLabel(kind),
      }),
      display,
    ];
  },

  renderText({ node }) {
    const kind = documentFieldKind(node.attrs.kind) ?? 'page';
    return typeof node.attrs.display === 'string' && node.attrs.display.trim()
      ? node.attrs.display.trim()
      : documentFieldLabel(kind);
  },
});

function hiddenAttribute(defaultValue: unknown) {
  return {
    default: defaultValue,
    rendered: false,
  };
}
