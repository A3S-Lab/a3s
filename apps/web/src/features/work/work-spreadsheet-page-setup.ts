import {
  effectiveSpreadsheetHeaderFooterSections,
  type EffectiveSpreadsheetHeaderFooterSections,
} from './work-spreadsheet-header-footer';
import type { WorkSpreadsheetPageMargins, WorkSpreadsheetPageSetup, WorkSpreadsheetPaperSize } from './work-types';

export interface EffectiveSpreadsheetPageSetup {
  paperSize: WorkSpreadsheetPaperSize;
  orientation: 'portrait' | 'landscape';
  scale: number;
  fitToPage: boolean;
  fitToWidth: number;
  fitToHeight: number;
  horizontalCentered: boolean;
  verticalCentered: boolean;
  header: EffectiveSpreadsheetHeaderFooterSections;
  footer: EffectiveSpreadsheetHeaderFooterSections;
  pageNumberStart: number;
  pageOrder: 'downThenOver' | 'overThenDown';
  scaleWithDocument: boolean;
  alignWithMargins: boolean;
  margins: WorkSpreadsheetPageMargins;
}

const DEFAULT_MARGINS: WorkSpreadsheetPageMargins = {
  top: 19.05,
  right: 17.78,
  bottom: 19.05,
  left: 17.78,
  header: 7.62,
  footer: 7.62,
};

const PAPER_DIMENSIONS: Record<WorkSpreadsheetPaperSize, { width: number; height: number }> = {
  a3: { width: 297, height: 420 },
  a4: { width: 210, height: 297 },
  a5: { width: 148, height: 210 },
  letter: { width: 215.9, height: 279.4 },
  legal: { width: 215.9, height: 355.6 },
  tabloid: { width: 279.4, height: 431.8 },
};

const BASE_PRINTABLE_WIDTH = PAPER_DIMENSIONS.a4.height - DEFAULT_MARGINS.left - DEFAULT_MARGINS.right;
const BASE_PRINTABLE_HEIGHT = PAPER_DIMENSIONS.a4.width - DEFAULT_MARGINS.top - DEFAULT_MARGINS.bottom;

export function effectiveSpreadsheetPageSetup(
  pageSetup: WorkSpreadsheetPageSetup | undefined
): EffectiveSpreadsheetPageSetup {
  return {
    paperSize: normalizeSpreadsheetPaperSize(pageSetup?.paperSize),
    orientation: pageSetup?.orientation === 'portrait' ? 'portrait' : 'landscape',
    scale: boundedInteger(pageSetup?.scale, 10, 400, 100),
    fitToPage: Boolean(pageSetup?.fitToPage),
    fitToWidth: boundedInteger(pageSetup?.fitToWidth, 0, 32_767, 1),
    fitToHeight: boundedInteger(pageSetup?.fitToHeight, 0, 32_767, 0),
    horizontalCentered: Boolean(pageSetup?.horizontalCentered),
    verticalCentered: Boolean(pageSetup?.verticalCentered),
    header: effectiveSpreadsheetHeaderFooterSections(pageSetup?.header),
    footer: effectiveSpreadsheetHeaderFooterSections(pageSetup?.footer),
    pageNumberStart: boundedInteger(pageSetup?.pageNumberStart, 1, 32_767, 1),
    pageOrder: pageSetup?.pageOrder === 'downThenOver' ? 'downThenOver' : 'overThenDown',
    scaleWithDocument: pageSetup?.scaleWithDocument !== false,
    alignWithMargins: pageSetup?.alignWithMargins !== false,
    margins: {
      top: boundedNumber(pageSetup?.margins?.top, 0, 100, DEFAULT_MARGINS.top),
      right: boundedNumber(pageSetup?.margins?.right, 0, 100, DEFAULT_MARGINS.right),
      bottom: boundedNumber(pageSetup?.margins?.bottom, 0, 100, DEFAULT_MARGINS.bottom),
      left: boundedNumber(pageSetup?.margins?.left, 0, 100, DEFAULT_MARGINS.left),
      header: boundedNumber(pageSetup?.margins?.header, 0, 100, DEFAULT_MARGINS.header),
      footer: boundedNumber(pageSetup?.margins?.footer, 0, 100, DEFAULT_MARGINS.footer),
    },
  };
}

export function normalizeSpreadsheetPaperSize(value: unknown): WorkSpreadsheetPaperSize {
  switch (value) {
    case 'a3':
    case 'a4':
    case 'a5':
    case 'letter':
    case 'legal':
    case 'tabloid':
      return value;
    default:
      return 'a4';
  }
}

export function spreadsheetPageCapacity(pageSetup: EffectiveSpreadsheetPageSetup): {
  rows: number;
  columns: number;
} {
  const paper = PAPER_DIMENSIONS[pageSetup.paperSize];
  const pageWidth = pageSetup.orientation === 'landscape' ? paper.height : paper.width;
  const pageHeight = pageSetup.orientation === 'landscape' ? paper.width : paper.height;
  const printableWidth = Math.max(10, pageWidth - pageSetup.margins.left - pageSetup.margins.right);
  const printableHeight = Math.max(10, pageHeight - pageSetup.margins.top - pageSetup.margins.bottom);
  const scaleFactor = pageSetup.fitToPage ? 1 : 100 / pageSetup.scale;
  return {
    rows: Math.max(1, Math.floor(34 * (printableHeight / BASE_PRINTABLE_HEIGHT) * scaleFactor)),
    columns: Math.max(1, Math.floor(10 * (printableWidth / BASE_PRINTABLE_WIDTH) * scaleFactor)),
  };
}

export function fitSpreadsheetAxisCapacity(
  start: number,
  end: number,
  titleRange: [number, number] | undefined,
  targetPages: number,
  fallbackCapacity: number
): number {
  if (targetPages <= 0) return fallbackCapacity;
  const bodyLength = Math.max(1, end - start + 1);
  const titleLength = titleRange ? Math.max(0, titleRange[1] - titleRange[0] + 1) : 0;
  return Math.max(1, Math.ceil(bodyLength / targetPages) + titleLength);
}

function boundedInteger(value: number | undefined, minimum: number, maximum: number, fallback: number): number {
  return Math.trunc(boundedNumber(value, minimum, maximum, fallback));
}

function boundedNumber(value: number | undefined, minimum: number, maximum: number, fallback: number): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= minimum && value <= maximum ? value : fallback;
}
