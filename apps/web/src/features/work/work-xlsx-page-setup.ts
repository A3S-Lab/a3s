import { attribute, directChild, directChildren } from './work-ooxml-package';
import { normalizeSpreadsheetPaperSize } from './work-spreadsheet-page-setup';
import type { WorkSpreadsheetPageMargins, WorkSpreadsheetPageSetup, WorkSpreadsheetPaperSize } from './work-types';
import { readXlsxHeaderFooter, writeXlsxHeaderFooter } from './work-xlsx-header-footer';

export type XlsxPageSetup = Omit<WorkSpreadsheetPageSetup, 'sheetId'>;

const PAPER_SIZE_CODES: Record<WorkSpreadsheetPaperSize, string> = {
  letter: '1',
  tabloid: '3',
  legal: '5',
  a3: '8',
  a4: '9',
  a5: '11',
};

const PAPER_SIZES_BY_CODE = new Map(
  Object.entries(PAPER_SIZE_CODES).map(([paperSize, code]) => [code, paperSize as WorkSpreadsheetPaperSize])
);

export function readXlsxPageSetup(document: Document): XlsxPageSetup | undefined {
  const root = document.documentElement;
  const pageSetupElement = directChild(root, 'pageSetup');
  const marginsElement = directChild(root, 'pageMargins');
  const printOptions = directChild(root, 'printOptions');
  const headerFooterElement = directChild(root, 'headerFooter');
  const sheetProperties = directChild(root, 'sheetPr');
  const setupProperties = sheetProperties ? directChild(sheetProperties, 'pageSetUpPr') : undefined;
  if (!pageSetupElement && !marginsElement && !printOptions && !setupProperties && !headerFooterElement)
    return undefined;

  const pageSetup: XlsxPageSetup = {};
  const paperSize = pageSetupElement ? attribute(pageSetupElement, 'paperSize') : null;
  const editablePaperSize = paperSize ? PAPER_SIZES_BY_CODE.get(paperSize) : undefined;
  if (editablePaperSize) pageSetup.paperSize = editablePaperSize;
  const orientation = pageSetupElement ? attribute(pageSetupElement, 'orientation')?.toLowerCase() : undefined;
  if (orientation === 'portrait' || orientation === 'landscape') pageSetup.orientation = orientation;
  const scale = boundedIntegerAttribute(pageSetupElement, 'scale', 10, 400);
  if (scale !== undefined) pageSetup.scale = scale;
  const fitToWidth = boundedIntegerAttribute(pageSetupElement, 'fitToWidth', 0, 32_767);
  if (fitToWidth !== undefined) pageSetup.fitToWidth = fitToWidth;
  const fitToHeight = boundedIntegerAttribute(pageSetupElement, 'fitToHeight', 0, 32_767);
  if (fitToHeight !== undefined) pageSetup.fitToHeight = fitToHeight;
  const pageOrder = pageSetupElement ? attribute(pageSetupElement, 'pageOrder') : null;
  if (pageOrder === 'downThenOver' || pageOrder === 'overThenDown') pageSetup.pageOrder = pageOrder;
  const pageNumberStart = boundedIntegerAttribute(pageSetupElement, 'firstPageNumber', 1, 32_767);
  const useFirstPageNumber = pageSetupElement ? attribute(pageSetupElement, 'useFirstPageNumber') : null;
  if (
    pageNumberStart !== undefined &&
    (useFirstPageNumber === null || booleanAttribute(pageSetupElement, 'useFirstPageNumber'))
  ) {
    pageSetup.pageNumberStart = pageNumberStart;
  }
  if (setupProperties && attribute(setupProperties, 'fitToPage') !== null) {
    pageSetup.fitToPage = booleanAttribute(setupProperties, 'fitToPage');
  }
  if (printOptions && attribute(printOptions, 'horizontalCentered') !== null) {
    pageSetup.horizontalCentered = booleanAttribute(printOptions, 'horizontalCentered');
  }
  if (printOptions && attribute(printOptions, 'verticalCentered') !== null) {
    pageSetup.verticalCentered = booleanAttribute(printOptions, 'verticalCentered');
  }
  const margins = readMargins(marginsElement);
  if (margins) pageSetup.margins = margins;
  Object.assign(pageSetup, readXlsxHeaderFooter(headerFooterElement));
  return pageSetup;
}

export function writeXlsxPageSetup(document: Document, pageSetup: WorkSpreadsheetPageSetup | undefined): void {
  const root = document.documentElement;
  for (const existing of directChildren(root).filter((element) =>
    ['printOptions', 'pageMargins', 'pageSetup', 'headerFooter'].includes(element.localName)
  )) {
    existing.remove();
  }
  writeFitToPage(document, pageSetup?.fitToPage);
  if (!pageSetup) return;

  const elements: Element[] = [];
  if (pageSetup.horizontalCentered !== undefined || pageSetup.verticalCentered !== undefined) {
    const printOptions = document.createElementNS(root.namespaceURI, 'printOptions');
    if (pageSetup.horizontalCentered !== undefined) {
      printOptions.setAttribute('horizontalCentered', pageSetup.horizontalCentered ? '1' : '0');
    }
    if (pageSetup.verticalCentered !== undefined) {
      printOptions.setAttribute('verticalCentered', pageSetup.verticalCentered ? '1' : '0');
    }
    elements.push(printOptions);
  }
  if (pageSetup.margins) elements.push(writeMargins(document, pageSetup.margins));

  const setup = document.createElementNS(root.namespaceURI, 'pageSetup');
  if (pageSetup.paperSize) {
    setup.setAttribute('paperSize', PAPER_SIZE_CODES[normalizeSpreadsheetPaperSize(pageSetup.paperSize)]);
  }
  if (pageSetup.orientation) setup.setAttribute('orientation', pageSetup.orientation);
  if (pageSetup.scale !== undefined) setup.setAttribute('scale', String(pageSetup.scale));
  if (pageSetup.fitToWidth !== undefined) setup.setAttribute('fitToWidth', String(pageSetup.fitToWidth));
  if (pageSetup.fitToHeight !== undefined) setup.setAttribute('fitToHeight', String(pageSetup.fitToHeight));
  if (pageSetup.pageOrder) setup.setAttribute('pageOrder', pageSetup.pageOrder);
  if (
    pageSetup.pageNumberStart !== undefined &&
    Number.isInteger(pageSetup.pageNumberStart) &&
    pageSetup.pageNumberStart >= 1 &&
    pageSetup.pageNumberStart <= 32_767
  ) {
    setup.setAttribute('firstPageNumber', String(pageSetup.pageNumberStart));
    setup.setAttribute('useFirstPageNumber', '1');
  }
  if (setup.attributes.length) elements.push(setup);
  const headerFooter = writeXlsxHeaderFooter(document, pageSetup);
  if (headerFooter) elements.push(headerFooter);

  const anchor =
    directChildren(root).find((element) =>
      [
        'rowBreaks',
        'colBreaks',
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
  for (const element of elements) root.insertBefore(element, anchor);
}

export function isEditableXlsxPaperSizeCode(value: string): boolean {
  return PAPER_SIZES_BY_CODE.has(value);
}

function writeFitToPage(document: Document, fitToPage: boolean | undefined): void {
  const root = document.documentElement;
  let sheetProperties = directChild(root, 'sheetPr');
  let setupProperties = sheetProperties ? directChild(sheetProperties, 'pageSetUpPr') : undefined;
  if (fitToPage !== undefined) {
    if (!sheetProperties) {
      sheetProperties = document.createElementNS(root.namespaceURI, 'sheetPr');
      root.insertBefore(sheetProperties, root.firstElementChild);
    }
    if (!setupProperties) {
      setupProperties = document.createElementNS(root.namespaceURI, 'pageSetUpPr');
      sheetProperties.append(setupProperties);
    }
    setupProperties.setAttribute('fitToPage', fitToPage ? '1' : '0');
    return;
  }
  setupProperties?.removeAttribute('fitToPage');
  if (setupProperties && !setupProperties.attributes.length && !setupProperties.children.length) {
    setupProperties.remove();
  }
}

function readMargins(element: Element | undefined): WorkSpreadsheetPageMargins | null {
  if (!element) return null;
  const values = ['top', 'right', 'bottom', 'left', 'header', 'footer'].map((name) =>
    nonnegativeNumberAttribute(element, name)
  );
  if (values.some((value) => value === null)) return null;
  const [top, right, bottom, left, header, footer] = values as number[];
  return {
    top: inchesToMillimeters(top),
    right: inchesToMillimeters(right),
    bottom: inchesToMillimeters(bottom),
    left: inchesToMillimeters(left),
    header: inchesToMillimeters(header),
    footer: inchesToMillimeters(footer),
  };
}

function writeMargins(document: Document, margins: WorkSpreadsheetPageMargins): Element {
  const element = document.createElementNS(document.documentElement.namespaceURI, 'pageMargins');
  for (const [name, value] of Object.entries({
    left: margins.left,
    right: margins.right,
    top: margins.top,
    bottom: margins.bottom,
    header: margins.header,
    footer: margins.footer,
  })) {
    element.setAttribute(name, millimetersToInches(value));
  }
  return element;
}

function boundedIntegerAttribute(
  element: Element | undefined,
  name: string,
  minimum: number,
  maximum: number
): number | undefined {
  if (!element) return undefined;
  const source = attribute(element, name);
  if (source === null) return undefined;
  const value = Number(source);
  return Number.isInteger(value) && value >= minimum && value <= maximum ? value : undefined;
}

function nonnegativeNumberAttribute(element: Element, name: string): number | null {
  const source = attribute(element, name);
  if (source === null) return null;
  const value = Number(source);
  return Number.isFinite(value) && value >= 0 ? value : null;
}

function booleanAttribute(element: Element | undefined, name: string): boolean {
  const value = element ? attribute(element, name)?.toLowerCase() : undefined;
  return value === '1' || value === 'true';
}

function inchesToMillimeters(value: number): number {
  return Number((value * 25.4).toFixed(4));
}

function millimetersToInches(value: number): string {
  return String(Number((value / 25.4).toFixed(4)));
}
