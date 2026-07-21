import { mergeAttributes, Node } from '@tiptap/core';
import {
  documentSectionDomAttributes,
  documentSectionLayoutFromNodeAttributes,
  type DocumentSectionNodeAttributes,
} from './work-document-section';
import type { WorkDocumentSectionBreakType } from './work-types';

export const DocumentSection = Node.create({
  name: 'documentSection',
  group: 'block',
  content: 'block+',
  defining: true,

  addAttributes() {
    return {
      id: hiddenAttribute(''),
      pageSize: hiddenAttribute('a4'),
      orientation: hiddenAttribute('portrait'),
      marginTop: hiddenAttribute(25),
      marginRight: hiddenAttribute(23),
      marginBottom: hiddenAttribute(25),
      marginLeft: hiddenAttribute(23),
      columnCount: hiddenAttribute(1),
      columnSpacing: hiddenAttribute(12),
      columnSeparator: hiddenAttribute(false),
      columnLayout: hiddenAttribute(''),
      breakAfter: hiddenAttribute('nextPage'),
      headerText: hiddenAttribute(''),
      footerText: hiddenAttribute(''),
      showPageNumbers: hiddenAttribute(false),
      pageNumberStart: hiddenAttribute(null),
      pageChrome: hiddenAttribute(''),
    };
  },

  parseHTML() {
    return [
      {
        tag: 'section[data-document-section]',
        getAttrs: (node) => {
          if (!(node instanceof HTMLElement)) return false;
          return {
            id: node.dataset.sectionId ?? '',
            pageSize: node.dataset.sectionPageSize === 'letter' ? 'letter' : 'a4',
            orientation: node.dataset.sectionOrientation === 'landscape' ? 'landscape' : 'portrait',
            marginTop: numberAttribute(node, 'sectionMarginTop', 25),
            marginRight: numberAttribute(node, 'sectionMarginRight', 23),
            marginBottom: numberAttribute(node, 'sectionMarginBottom', 25),
            marginLeft: numberAttribute(node, 'sectionMarginLeft', 23),
            columnCount: numberAttribute(node, 'sectionColumnCount', 1),
            columnSpacing: numberAttribute(node, 'sectionColumnSpacing', 12),
            columnSeparator: node.dataset.sectionColumnSeparator === 'true',
            columnLayout: node.dataset.sectionColumnLayout ?? '',
            breakAfter: sectionBreakAttribute(node.dataset.sectionBreakAfter),
            headerText: node.dataset.sectionHeaderText ?? '',
            footerText: node.dataset.sectionFooterText ?? '',
            showPageNumbers: node.dataset.sectionShowPageNumbers === 'true',
            pageNumberStart: nullableNumberAttribute(node, 'sectionPageNumberStart'),
            pageChrome: node.dataset.sectionPageChrome ?? '',
          } satisfies DocumentSectionNodeAttributes;
        },
      },
    ];
  },

  renderHTML({ node, HTMLAttributes }) {
    const layout = documentSectionLayoutFromNodeAttributes(node.attrs);
    const id = typeof node.attrs.id === 'string' && node.attrs.id ? node.attrs.id : 'document-section';
    const columns = layout.columns;
    const style = [
      `--work-document-column-count:${columns.count}`,
      `--work-document-column-gap:${columns.spacing}mm`,
      `--work-document-column-rule:${columns.separator ? '1px solid var(--a3s-line-strong)' : 'none'}`,
    ].join(';');
    return [
      'section',
      mergeAttributes(HTMLAttributes, documentSectionDomAttributes(layout, id), {
        class: 'work-document-section',
        style,
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

function numberAttribute(element: HTMLElement, name: keyof DOMStringMap, fallback: number): number {
  const value = Number(element.dataset[name]);
  return Number.isFinite(value) ? value : fallback;
}

function nullableNumberAttribute(element: HTMLElement, name: keyof DOMStringMap): number | null {
  const source = element.dataset[name];
  if (!source?.trim()) return null;
  const value = Number(source);
  return Number.isFinite(value) ? value : null;
}

function sectionBreakAttribute(value: string | undefined): WorkDocumentSectionBreakType {
  if (value === 'continuous' || value === 'evenPage' || value === 'oddPage' || value === 'nextColumn') return value;
  return 'nextPage';
}
