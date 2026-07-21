import { mergeAttributes, Node } from '@tiptap/core';
import { documentNoteKind } from './work-document-notes';

export const DocumentNoteReference = Node.create({
  name: 'documentNoteReference',
  inline: true,
  group: 'inline',
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      id: hiddenAttribute(''),
      kind: hiddenAttribute('footnote'),
      number: hiddenAttribute(1),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'sup[data-document-note-reference]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          return {
            id: node.dataset.noteId ?? '',
            kind: documentNoteKind(node.dataset.noteKind) ?? 'footnote',
            number: positiveInteger(node.dataset.noteNumber),
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const kind = documentNoteKind(node.attrs.kind) ?? 'footnote';
    const number = positiveInteger(node.attrs.number);
    return [
      'sup',
      mergeAttributes(HTMLAttributes, {
        'data-document-note-reference': 'true',
        'data-note-kind': kind,
        'data-note-id': typeof node.attrs.id === 'string' ? node.attrs.id : '',
        'data-note-number': String(number),
        class: 'work-document-note-reference',
      }),
      String(number),
    ];
  },

  renderText({ node }) {
    return String(positiveInteger(node.attrs.number));
  },
});

export const DocumentNote = Node.create({
  name: 'documentNote',
  group: 'block',
  content: 'block+',
  defining: true,
  isolating: true,

  addAttributes() {
    return {
      id: hiddenAttribute(''),
      kind: hiddenAttribute('footnote'),
      number: hiddenAttribute(1),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'aside[data-document-note]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          return {
            id: node.dataset.noteId ?? '',
            kind: documentNoteKind(node.dataset.noteKind) ?? 'footnote',
            number: positiveInteger(node.dataset.noteNumber),
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const kind = documentNoteKind(node.attrs.kind) ?? 'footnote';
    const number = positiveInteger(node.attrs.number);
    return [
      'aside',
      mergeAttributes(HTMLAttributes, {
        'data-document-note': 'true',
        'data-note-kind': kind,
        'data-note-id': typeof node.attrs.id === 'string' ? node.attrs.id : '',
        'data-note-number': String(number),
        class: 'work-document-note',
      }),
      0,
    ];
  },
});

function hiddenAttribute(defaultValue: unknown) {
  return {
    default: defaultValue,
    rendered: false,
  };
}

function positiveInteger(value: unknown): number {
  const number = Number(value);
  return Number.isSafeInteger(number) && number > 0 ? number : 1;
}
