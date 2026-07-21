import { mergeAttributes, Node } from '@tiptap/core';
import { documentCaptionDisplay, documentCaptionKind, documentCaptionLabel } from './work-document-captions';

export const DocumentCaption = Node.create({
  name: 'documentCaption',
  group: 'block',
  content: 'inline*',
  defining: true,

  addAttributes() {
    return {
      id: hiddenAttribute(''),
      kind: hiddenAttribute('figure'),
      number: hiddenAttribute(1),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'figcaption[data-document-caption]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          return {
            id: node.dataset.captionId ?? '',
            kind: documentCaptionKind(node.dataset.captionKind) ?? 'figure',
            number: positiveInteger(node.dataset.captionNumber),
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const kind = documentCaptionKind(node.attrs.kind) ?? 'figure';
    const number = positiveInteger(node.attrs.number);
    return [
      'figcaption',
      mergeAttributes(HTMLAttributes, {
        'data-document-caption': 'true',
        'data-caption-id': typeof node.attrs.id === 'string' ? node.attrs.id : '',
        'data-caption-kind': kind,
        'data-caption-number': String(number),
        'data-caption-label': documentCaptionLabel(kind),
        class: 'work-document-caption',
      }),
      0,
    ];
  },

  renderText({ node }) {
    const kind = documentCaptionKind(node.attrs.kind) ?? 'figure';
    return `${documentCaptionDisplay(kind, positiveInteger(node.attrs.number))} ${node.textContent}`.trim();
  },
});

export const DocumentCrossReference = Node.create({
  name: 'documentCrossReference',
  inline: true,
  group: 'inline',
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      targetId: hiddenAttribute(''),
      kind: hiddenAttribute('figure'),
      number: hiddenAttribute(1),
      orphaned: hiddenAttribute(false),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-document-cross-reference]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          return {
            targetId: node.dataset.referenceTargetId ?? '',
            kind: documentCaptionKind(node.dataset.captionKind) ?? 'figure',
            number: positiveInteger(node.dataset.captionNumber),
            orphaned: node.dataset.referenceOrphaned === 'true',
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const kind = documentCaptionKind(node.attrs.kind) ?? 'figure';
    const number = positiveInteger(node.attrs.number);
    const orphaned = Boolean(node.attrs.orphaned);
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-document-cross-reference': 'true',
        'data-reference-target-id': typeof node.attrs.targetId === 'string' ? node.attrs.targetId : '',
        'data-caption-kind': kind,
        'data-caption-number': String(number),
        'data-caption-label': documentCaptionLabel(kind),
        'data-reference-orphaned': orphaned ? 'true' : undefined,
        class: 'work-document-cross-reference',
      }),
      orphaned ? '引用缺失' : documentCaptionDisplay(kind, number),
    ];
  },

  renderText({ node }) {
    if (node.attrs.orphaned) return '引用缺失';
    const kind = documentCaptionKind(node.attrs.kind) ?? 'figure';
    return documentCaptionDisplay(kind, positiveInteger(node.attrs.number));
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
