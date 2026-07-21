import { attribute, directChild, directChildren } from './work-ooxml-package';
import type { WorkSpreadsheetPageBreaks } from './work-types';

export interface XlsxManualPageBreaks {
  rows: number[];
  columns: number[];
}

export function readXlsxManualPageBreaks(document: Document): XlsxManualPageBreaks {
  const root = document.documentElement;
  return {
    rows: readBreakContainer(directChild(root, 'rowBreaks')),
    columns: readBreakContainer(directChild(root, 'colBreaks')),
  };
}

export function writeXlsxManualPageBreaks(document: Document, pageBreaks: WorkSpreadsheetPageBreaks | undefined): void {
  const root = document.documentElement;
  for (const existing of directChildren(root).filter(
    (element) => element.localName === 'rowBreaks' || element.localName === 'colBreaks'
  )) {
    existing.remove();
  }

  const rows = normalizedBreaks(pageBreaks?.rows);
  const columns = normalizedBreaks(pageBreaks?.columns);
  if (!rows.length && !columns.length) return;
  const anchor =
    directChildren(root).find((element) =>
      [
        'customProperties',
        'cellWatches',
        'ignoredErrors',
        'smartTags',
        'drawing',
        'legacyDrawing',
        'legacyDrawingHF',
        'picture',
        'oleObjects',
        'controls',
        'webPublishItems',
        'tableParts',
        'extLst',
      ].includes(element.localName)
    ) ?? null;
  if (rows.length) root.insertBefore(createBreakContainer(document, 'rowBreaks', rows, 16_383), anchor);
  if (columns.length) root.insertBefore(createBreakContainer(document, 'colBreaks', columns, 1_048_575), anchor);
}

function readBreakContainer(container: Element | null | undefined): number[] {
  if (!container) return [];
  return normalizedBreaks(
    directChildren(container, 'brk').flatMap((element) => {
      if (!booleanAttribute(element, 'man')) return [];
      const id = Number(attribute(element, 'id'));
      return Number.isSafeInteger(id) && id > 0 ? [id] : [];
    })
  );
}

function createBreakContainer(
  document: Document,
  name: 'rowBreaks' | 'colBreaks',
  breaks: number[],
  maximumSpan: number
): Element {
  const container = document.createElementNS(document.documentElement.namespaceURI, name);
  container.setAttribute('count', String(breaks.length));
  container.setAttribute('manualBreakCount', String(breaks.length));
  for (const id of breaks) {
    const pageBreak = document.createElementNS(document.documentElement.namespaceURI, 'brk');
    pageBreak.setAttribute('id', String(id));
    pageBreak.setAttribute('min', '0');
    pageBreak.setAttribute('max', String(maximumSpan));
    pageBreak.setAttribute('man', '1');
    container.append(pageBreak);
  }
  return container;
}

function normalizedBreaks(breaks: number[] | undefined): number[] {
  return Array.from(new Set((breaks ?? []).filter((value) => Number.isSafeInteger(value) && value > 0))).sort(
    (left, right) => left - right
  );
}

function booleanAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name)?.toLowerCase();
  return value === '1' || value === 'true';
}
