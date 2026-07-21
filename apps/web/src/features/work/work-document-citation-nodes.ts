import { mergeAttributes, Node } from '@tiptap/core';
import {
  documentCitationInstruction,
  documentCitationTags,
  documentCitationTagsFromInstruction,
} from './work-document-citations';

export const DocumentCitation = Node.create({
  name: 'documentCitation',
  inline: true,
  group: 'inline',
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      id: hiddenAttribute(''),
      tags: hiddenAttribute(''),
      instruction: hiddenAttribute(''),
      display: hiddenAttribute(''),
      orphaned: hiddenAttribute(false),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'span[data-document-citation]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          const instruction = node.dataset.citationInstruction?.trim() ?? '';
          const tags = documentCitationTags(node.dataset.citationTags);
          const resolvedTags = tags.length ? tags : documentCitationTagsFromInstruction(instruction);
          return {
            id: node.dataset.citationId ?? '',
            tags: resolvedTags.join(' '),
            instruction: instruction || documentCitationInstruction(resolvedTags),
            display: node.dataset.citationDisplay?.trim() || node.textContent?.trim() || '缺失引文',
            orphaned: node.dataset.citationOrphaned === 'true',
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const tags = typeof node.attrs.tags === 'string' ? documentCitationTags(node.attrs.tags) : [];
    const instruction = typeof node.attrs.instruction === 'string' ? node.attrs.instruction.trim() : '';
    const display =
      typeof node.attrs.display === 'string' && node.attrs.display.trim() ? node.attrs.display.trim() : '缺失引文';
    const orphaned = Boolean(node.attrs.orphaned);
    return [
      'span',
      mergeAttributes(HTMLAttributes, {
        'data-document-citation': 'true',
        'data-citation-id': typeof node.attrs.id === 'string' ? node.attrs.id : '',
        'data-citation-tags': tags.join(' '),
        'data-citation-instruction': instruction || documentCitationInstruction(tags),
        'data-citation-display': display,
        'data-citation-orphaned': orphaned ? 'true' : undefined,
        class: 'work-document-citation',
        title: tags.length ? `引文：${tags.join('、')}` : '缺失引文',
      }),
      display,
    ];
  },

  renderText({ node }) {
    return typeof node.attrs.display === 'string' && node.attrs.display.trim() ? node.attrs.display.trim() : '缺失引文';
  },
});

export const DocumentBibliography = Node.create({
  name: 'documentBibliography',
  group: 'block',
  content: 'block+',
  atom: true,
  selectable: true,
  defining: true,
  isolating: true,

  addAttributes() {
    return {
      id: hiddenAttribute('document-bibliography-1'),
      style: hiddenAttribute('apa'),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'section[data-document-bibliography]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          return {
            id: node.dataset.bibliographyId || 'document-bibliography-1',
            style: node.dataset.bibliographyStyle || 'apa',
          };
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    return [
      'section',
      mergeAttributes(HTMLAttributes, {
        'data-document-bibliography': 'true',
        'data-bibliography-id': typeof node.attrs.id === 'string' ? node.attrs.id : 'document-bibliography-1',
        'data-bibliography-style': typeof node.attrs.style === 'string' ? node.attrs.style : 'apa',
        class: 'work-document-bibliography',
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
