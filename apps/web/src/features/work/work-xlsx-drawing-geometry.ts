import { attribute, directChild } from './work-ooxml-package';
import type { WorkSpreadsheetSheet } from './work-types';

export const XLSX_EMU_PER_PIXEL = 9_525;
const DEFAULT_COLUMN_WIDTH = 96;
const DEFAULT_ROW_HEIGHT = 24;

export interface XlsxDrawingMarker {
  column: number;
  row: number;
  columnOffsetEmu: number;
  rowOffsetEmu: number;
}

export interface XlsxDrawingAnchor {
  from?: XlsxDrawingMarker;
  to?: XlsxDrawingMarker;
  leftEmu?: number;
  topEmu?: number;
  widthEmu?: number;
  heightEmu?: number;
}

export interface SpreadsheetDrawingBounds {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function readXlsxDrawingAnchor(anchor: Element): XlsxDrawingAnchor {
  const extent = directChild(anchor, 'ext');
  const position = directChild(anchor, 'pos');
  return {
    from: readMarker(directChild(anchor, 'from')),
    to: readMarker(directChild(anchor, 'to')),
    leftEmu: numberAttribute(position, 'x'),
    topEmu: numberAttribute(position, 'y'),
    widthEmu: numberAttribute(extent, 'cx'),
    heightEmu: numberAttribute(extent, 'cy'),
  };
}

export function xlsxDrawingAnchorToBounds(
  anchor: XlsxDrawingAnchor,
  config: WorkSpreadsheetSheet['config']
): SpreadsheetDrawingBounds | null {
  const left = anchor.from
    ? markerPosition(anchor.from.column, anchor.from.columnOffsetEmu, (index) => spreadsheetColumnWidth(config, index))
    : emuToPixels(anchor.leftEmu);
  const top = anchor.from
    ? markerPosition(anchor.from.row, anchor.from.rowOffsetEmu, (index) => spreadsheetRowHeight(config, index))
    : emuToPixels(anchor.topEmu);
  const right = anchor.to
    ? markerPosition(anchor.to.column, anchor.to.columnOffsetEmu, (index) => spreadsheetColumnWidth(config, index))
    : left + emuToPixels(anchor.widthEmu);
  const bottom = anchor.to
    ? markerPosition(anchor.to.row, anchor.to.rowOffsetEmu, (index) => spreadsheetRowHeight(config, index))
    : top + emuToPixels(anchor.heightEmu);
  if (![left, top, right, bottom].every(Number.isFinite)) return null;
  return {
    left: roundPosition(Math.max(0, left)),
    top: roundPosition(Math.max(0, top)),
    width: roundPosition(Math.max(1, right - left)),
    height: roundPosition(Math.max(1, bottom - top)),
  };
}

export function xlsxTwoCellAnchorMarkers(bounds: SpreadsheetDrawingBounds, sheet: WorkSpreadsheetSheet): string {
  const fromColumn = axisMarker(bounds.left, (index) => spreadsheetColumnWidth(sheet.config, index));
  const toColumn = axisMarker(bounds.left + bounds.width, (index) => spreadsheetColumnWidth(sheet.config, index));
  const fromRow = axisMarker(bounds.top, (index) => spreadsheetRowHeight(sheet.config, index));
  const toRow = axisMarker(bounds.top + bounds.height, (index) => spreadsheetRowHeight(sheet.config, index));
  return `${markerXml('from', fromColumn, fromRow)}${markerXml('to', toColumn, toRow)}`;
}

export function spreadsheetColumnWidth(config: WorkSpreadsheetSheet['config'], index: number): number {
  const key = String(index);
  if (Object.hasOwn(config?.colhidden ?? {}, key)) return 0;
  const width = config?.columnlen?.[key];
  return typeof width === 'number' && width > 0 ? width : DEFAULT_COLUMN_WIDTH;
}

export function spreadsheetRowHeight(config: WorkSpreadsheetSheet['config'], index: number): number {
  const key = String(index);
  if (Object.hasOwn(config?.rowhidden ?? {}, key)) return 0;
  const height = config?.rowlen?.[key];
  return typeof height === 'number' && height > 0 ? height : DEFAULT_ROW_HEIGHT;
}

function readMarker(element: Element | undefined): XlsxDrawingMarker | undefined {
  if (!element) return undefined;
  return {
    column: nonNegativeInteger(directChild(element, 'col')?.textContent),
    row: nonNegativeInteger(directChild(element, 'row')?.textContent),
    columnOffsetEmu: nonNegativeNumber(directChild(element, 'colOff')?.textContent),
    rowOffsetEmu: nonNegativeNumber(directChild(element, 'rowOff')?.textContent),
  };
}

function numberAttribute(element: Element | undefined, name: string): number | undefined {
  if (!element) return undefined;
  const value = Number(attribute(element, name));
  return Number.isFinite(value) && value >= 0 ? value : undefined;
}

function nonNegativeInteger(value: string | null | undefined): number {
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : 0;
}

function nonNegativeNumber(value: string | null | undefined): number {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : 0;
}

function markerPosition(index: number, offsetEmu: number, size: (index: number) => number): number {
  let position = 0;
  for (let current = 0; current < index; current += 1) position += size(current);
  return position + emuToPixels(offsetEmu);
}

function emuToPixels(value: number | undefined): number {
  return (value ?? 0) / XLSX_EMU_PER_PIXEL;
}

interface AxisMarker {
  index: number;
  offsetEmu: number;
}

function axisMarker(position: number, size: (index: number) => number): AxisMarker {
  let remaining = Math.max(0, Number.isFinite(position) ? position : 0);
  let index = 0;
  while (index < 1_048_576) {
    const currentSize = Math.max(0, size(index));
    if (currentSize > 0 && remaining < currentSize) break;
    if (currentSize > 0) remaining -= currentSize;
    index += 1;
  }
  return { index, offsetEmu: Math.round(remaining * XLSX_EMU_PER_PIXEL) };
}

function markerXml(kind: 'from' | 'to', column: AxisMarker, row: AxisMarker): string {
  return [
    `<xdr:${kind}>`,
    `<xdr:col>${column.index}</xdr:col><xdr:colOff>${column.offsetEmu}</xdr:colOff>`,
    `<xdr:row>${row.index}</xdr:row><xdr:rowOff>${row.offsetEmu}</xdr:rowOff>`,
    `</xdr:${kind}>`,
  ].join('');
}

function roundPosition(value: number): number {
  return Math.round(value * 1_000) / 1_000;
}
