import type { Selection } from '@fortune-sheet/core';
import { spreadsheetChartSvgDataUrl } from './work-spreadsheet-chart-svg';
import { createWorkId } from './work-templates';
import type {
  WorkSpreadsheetChart,
  WorkSpreadsheetChartAxes,
  WorkSpreadsheetChartSeries,
  WorkSpreadsheetContent,
  WorkSpreadsheetImage,
  WorkSpreadsheetSheet,
} from './work-types';
import { spreadsheetColumnWidth, spreadsheetRowHeight } from './work-xlsx-drawing-geometry';

const CHART_PREVIEW_IMAGE_PREFIX = 'work-chart-preview-';

export interface SpreadsheetCellRangeReference {
  sheet: WorkSpreadsheetSheet;
  startRow: number;
  endRow: number;
  startColumn: number;
  endColumn: number;
}

export function spreadsheetChartCount(content: WorkSpreadsheetContent): number {
  return content.sheets.reduce((count, sheet) => count + (sheet.charts?.length ?? 0), 0);
}

export function spreadsheetSheetsWithChartPreviews(content: WorkSpreadsheetContent): WorkSpreadsheetSheet[] {
  return content.sheets.map((sheet) => {
    const chartImages = (sheet.charts ?? []).map((chart) =>
      chartPreviewImage(resolveSpreadsheetChart(content, sheet, chart))
    );
    if (!chartImages.length) return sheet;
    return {
      ...sheet,
      images: [...(sheet.images ?? []), ...chartImages],
    };
  });
}

export function reconcileSpreadsheetChartPreviews(
  content: WorkSpreadsheetContent,
  changedSheets: WorkSpreadsheetSheet[]
): WorkSpreadsheetContent {
  const sourceById = new Map(content.sheets.flatMap((sheet) => (sheet.id ? [[sheet.id, sheet] as const] : [])));
  const renamedSheets = new Map(
    changedSheets.flatMap((sheet) => {
      const sourceName = sheet.id ? sourceById.get(sheet.id)?.name : undefined;
      return sourceName && sourceName !== sheet.name ? [[sourceName, sheet.name] as const] : [];
    })
  );
  const sheets = changedSheets.map((changedSheet) => {
    const source = (changedSheet.id ? sourceById.get(changedSheet.id) : undefined) ?? changedSheet;
    const previewImages = new Map(
      (changedSheet.images ?? [])
        .filter((image) => image.id.startsWith(CHART_PREVIEW_IMAGE_PREFIX))
        .map((image) => [image.id, image])
    );
    const charts = (source.charts ?? []).flatMap((sourceChart) => {
      const chart = renameSpreadsheetChartReferences(sourceChart, renamedSheets);
      const preview = previewImages.get(chartPreviewImageId(chart.id));
      if (!preview) return [];
      return [
        {
          ...chart,
          left: finiteDimension(preview.left, chart.left),
          top: finiteDimension(preview.top, chart.top),
          width: finiteDimension(preview.width, chart.width, 1),
          height: finiteDimension(preview.height, chart.height, 1),
        },
      ];
    });
    const images = (changedSheet.images ?? []).filter((image) => !image.id.startsWith(CHART_PREVIEW_IMAGE_PREFIX));
    return {
      ...changedSheet,
      images: images.length ? images : undefined,
      charts: charts.length ? charts : undefined,
      ...((changedSheet.formulaMetadata ?? source.formulaMetadata)
        ? { formulaMetadata: changedSheet.formulaMetadata ?? source.formulaMetadata }
        : {}),
    };
  });
  return { ...content, sheets };
}

export function resolveSpreadsheetChart(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  chart: WorkSpreadsheetChart
): WorkSpreadsheetChart {
  const titleValue = chart.titleReference
    ? spreadsheetReferenceValues(content, ownerSheet, chart.titleReference)?.[0]
    : undefined;
  const categories = chart.categoryReference
    ? spreadsheetReferenceValues(content, ownerSheet, chart.categoryReference)?.map(displayCellValue)
    : undefined;
  const series = chart.series.map((item) => resolveSpreadsheetChartSeries(content, ownerSheet, item));
  const axes = resolveSpreadsheetChartAxes(content, ownerSheet, chart.axes);
  return {
    ...chart,
    title: titleValue === undefined ? chart.title : displayCellValue(titleValue),
    categories: categories ?? chart.categories,
    series,
    ...(axes ? { axes } : {}),
  };
}

export function createSpreadsheetChartFromSelection(
  content: WorkSpreadsheetContent,
  sheetId: string,
  selection: Selection | undefined
): WorkSpreadsheetChart | null {
  const sheet = content.sheets.find((candidate) => candidate.id === sheetId);
  if (!sheet) return null;
  const bounds = normalizedSelection(selection ?? sheet.luckysheet_select_save?.at(-1));
  if (!bounds) return null;
  const rowCount = bounds.endRow - bounds.startRow + 1;
  const columnCount = bounds.endColumn - bounds.startColumn + 1;
  const hasHeader = rowCount > 1 && selectionHasHeader(sheet, bounds);
  const dataStartRow = hasHeader ? bounds.startRow + 1 : bounds.startRow;
  const chartNumber = spreadsheetChartCount(content) + 1;
  const chartName = `图表 ${chartNumber}`;
  let categoryReference: string | undefined;
  let categories: string[];
  let series: WorkSpreadsheetChartSeries[];

  if (columnCount > 1) {
    categoryReference = cellRangeFormula(
      sheet.name,
      dataStartRow,
      bounds.startColumn,
      bounds.endRow,
      bounds.startColumn
    );
    categories = rangeValues(sheet, dataStartRow, bounds.startColumn, bounds.endRow, bounds.startColumn).map(
      displayCellValue
    );
    series = inclusiveNumbers(bounds.startColumn + 1, bounds.endColumn).map((column, index) => ({
      name: hasHeader
        ? displayCellValue(sheet.data?.[bounds.startRow]?.[column]?.v ?? `系列 ${index + 1}`)
        : `系列 ${index + 1}`,
      nameReference: hasHeader
        ? cellRangeFormula(sheet.name, bounds.startRow, column, bounds.startRow, column)
        : undefined,
      valuesReference: cellRangeFormula(sheet.name, dataStartRow, column, bounds.endRow, column),
      values: rangeValues(sheet, dataStartRow, column, bounds.endRow, column).map(numericCellValue),
    }));
  } else {
    categories = inclusiveNumbers(dataStartRow, bounds.endRow).map((row) => String(row + 1));
    series = [
      {
        name: hasHeader
          ? displayCellValue(sheet.data?.[bounds.startRow]?.[bounds.startColumn]?.v ?? '系列 1')
          : '系列 1',
        nameReference: hasHeader
          ? cellRangeFormula(sheet.name, bounds.startRow, bounds.startColumn, bounds.startRow, bounds.startColumn)
          : undefined,
        valuesReference: cellRangeFormula(
          sheet.name,
          dataStartRow,
          bounds.startColumn,
          bounds.endRow,
          bounds.startColumn
        ),
        values: rangeValues(sheet, dataStartRow, bounds.startColumn, bounds.endRow, bounds.startColumn).map(
          numericCellValue
        ),
      },
    ];
  }

  const left = axisPosition(bounds.endColumn + 1, (index) => spreadsheetColumnWidth(sheet.config, index)) + 16;
  const top = axisPosition(bounds.startRow, (index) => spreadsheetRowHeight(sheet.config, index));
  return {
    id: createWorkId('chart'),
    name: chartName,
    altText: `${chartName}，来源于 ${sheet.name} 的当前选区`,
    type: 'column',
    title: chartName,
    categories,
    categoryReference,
    series,
    showLegend: series.length > 1,
    legendPosition: 'right',
    legendOverlay: false,
    grouping: 'clustered',
    gapWidth: 150,
    overlap: 0,
    left,
    top,
    width: 480,
    height: 288,
  };
}

export function parseSpreadsheetChartReference(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  reference: string
): SpreadsheetCellRangeReference | null {
  const formula = reference.trim().replace(/^=/, '');
  const match = /^(?:(?:'((?:[^']|'')+)'|([^!]+))!)?(\$?[A-Z]{1,3}\$?[1-9]\d*)(?::(\$?[A-Z]{1,3}\$?[1-9]\d*))?$/i.exec(
    formula
  );
  if (!match) return null;
  const sheetName = (match[1]?.replaceAll("''", "'") ?? match[2])?.trim();
  if (sheetName?.includes('[') || sheetName?.includes(']')) return null;
  const sheet = sheetName ? content.sheets.find((candidate) => candidate.name === sheetName) : ownerSheet;
  if (!sheet) return null;
  const start = decodeCellAddress(match[3]);
  const end = decodeCellAddress(match[4] ?? match[3]);
  if (!start || !end) return null;
  return {
    sheet,
    startRow: Math.min(start.row, end.row),
    endRow: Math.max(start.row, end.row),
    startColumn: Math.min(start.column, end.column),
    endColumn: Math.max(start.column, end.column),
  };
}

export function spreadsheetReferenceValues(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  reference: string
): unknown[] | undefined {
  const range = parseSpreadsheetChartReference(content, ownerSheet, reference);
  if (!range) return undefined;
  return rangeValues(range.sheet, range.startRow, range.startColumn, range.endRow, range.endColumn);
}

export function quoteSpreadsheetSheetName(name: string): string {
  return /^[A-Za-z_][A-Za-z0-9_.]*$/.test(name) ? name : `'${name.replaceAll("'", "''")}'`;
}

function renameSpreadsheetChartReferences(
  chart: WorkSpreadsheetChart,
  renamedSheets: ReadonlyMap<string, string>
): WorkSpreadsheetChart {
  if (!renamedSheets.size) return chart;
  return {
    ...chart,
    titleReference: renameSpreadsheetReference(chart.titleReference, renamedSheets),
    axes: renameSpreadsheetChartAxesReferences(chart.axes, renamedSheets),
    categoryReference: renameSpreadsheetReference(chart.categoryReference, renamedSheets),
    series: chart.series.map((series) => ({
      ...series,
      nameReference: renameSpreadsheetReference(series.nameReference, renamedSheets),
      valuesReference: renameSpreadsheetReference(series.valuesReference, renamedSheets),
      xValuesReference: renameSpreadsheetReference(series.xValuesReference, renamedSheets),
      bubbleSizesReference: renameSpreadsheetReference(series.bubbleSizesReference, renamedSheets),
      ...(series.errorBars
        ? {
            errorBars: series.errorBars.map((errorBars) => ({
              ...errorBars,
              plusReference: renameSpreadsheetReference(errorBars.plusReference, renamedSheets),
              minusReference: renameSpreadsheetReference(errorBars.minusReference, renamedSheets),
            })),
          }
        : {}),
    })),
  };
}

function renameSpreadsheetChartAxesReferences(
  axes: WorkSpreadsheetChartAxes | undefined,
  renamedSheets: ReadonlyMap<string, string>
): WorkSpreadsheetChartAxes | undefined {
  if (!axes) return undefined;
  const bottom = renameSpreadsheetChartAxisReference(axes.bottom, renamedSheets);
  const left = renameSpreadsheetChartAxisReference(axes.left, renamedSheets);
  const top = renameSpreadsheetChartAxisReference(axes.top, renamedSheets);
  const right = renameSpreadsheetChartAxisReference(axes.right, renamedSheets);
  return {
    ...(bottom ? { bottom } : {}),
    ...(left ? { left } : {}),
    ...(top ? { top } : {}),
    ...(right ? { right } : {}),
  };
}

function renameSpreadsheetChartAxisReference(
  axis: WorkSpreadsheetChartAxes['bottom'],
  renamedSheets: ReadonlyMap<string, string>
): WorkSpreadsheetChartAxes['bottom'] {
  return axis ? { ...axis, titleReference: renameSpreadsheetReference(axis.titleReference, renamedSheets) } : undefined;
}

function renameSpreadsheetReference(
  reference: string | undefined,
  renamedSheets: ReadonlyMap<string, string>
): string | undefined {
  if (!reference) return undefined;
  const formula = reference.trim().replace(/^=/, '');
  const match = /^(?:'((?:[^']|'')+)'|([^!]+))!(.+)$/.exec(formula);
  const sheetName = match ? (match[1]?.replaceAll("''", "'") ?? match[2]) : undefined;
  const renamed = sheetName ? renamedSheets.get(sheetName) : undefined;
  return renamed && match ? `${quoteSpreadsheetSheetName(renamed)}!${match[3]}` : formula;
}

function resolveSpreadsheetChartSeries(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  series: WorkSpreadsheetChartSeries
): WorkSpreadsheetChartSeries {
  const nameValue = series.nameReference
    ? spreadsheetReferenceValues(content, ownerSheet, series.nameReference)?.[0]
    : undefined;
  const values = series.valuesReference
    ? spreadsheetReferenceValues(content, ownerSheet, series.valuesReference)?.map(numericCellValue)
    : undefined;
  const xValues = series.xValuesReference
    ? spreadsheetReferenceValues(content, ownerSheet, series.xValuesReference)?.map(numericCellValue)
    : undefined;
  const bubbleSizes = series.bubbleSizesReference
    ? spreadsheetReferenceValues(content, ownerSheet, series.bubbleSizesReference)?.map(numericCellValue)
    : undefined;
  const errorBars = series.errorBars?.map((source) => ({
    ...source,
    plusValues: source.plusReference
      ? (spreadsheetReferenceValues(content, ownerSheet, source.plusReference)?.map(numericCellValue) ??
        source.plusValues)
      : source.plusValues,
    minusValues: source.minusReference
      ? (spreadsheetReferenceValues(content, ownerSheet, source.minusReference)?.map(numericCellValue) ??
        source.minusValues)
      : source.minusValues,
  }));
  return {
    ...series,
    name: nameValue === undefined ? series.name : displayCellValue(nameValue),
    values: values ?? series.values,
    xValues: xValues ?? series.xValues,
    bubbleSizes: bubbleSizes ?? series.bubbleSizes,
    ...(errorBars ? { errorBars } : {}),
  };
}

function resolveSpreadsheetChartAxes(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  axes: WorkSpreadsheetChartAxes | undefined
): WorkSpreadsheetChartAxes | undefined {
  if (!axes) return undefined;
  const bottom = resolveSpreadsheetChartAxis(content, ownerSheet, axes.bottom);
  const left = resolveSpreadsheetChartAxis(content, ownerSheet, axes.left);
  const top = resolveSpreadsheetChartAxis(content, ownerSheet, axes.top);
  const right = resolveSpreadsheetChartAxis(content, ownerSheet, axes.right);
  return {
    ...(bottom ? { bottom } : {}),
    ...(left ? { left } : {}),
    ...(top ? { top } : {}),
    ...(right ? { right } : {}),
  };
}

function resolveSpreadsheetChartAxis(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  axis: WorkSpreadsheetChartAxes['bottom']
): WorkSpreadsheetChartAxes['bottom'] {
  if (!axis) return undefined;
  const titleValue = axis.titleReference
    ? spreadsheetReferenceValues(content, ownerSheet, axis.titleReference)?.[0]
    : undefined;
  return { ...axis, title: titleValue === undefined ? axis.title : displayCellValue(titleValue) };
}

function chartPreviewImage(chart: WorkSpreadsheetChart): WorkSpreadsheetImage {
  return {
    id: chartPreviewImageId(chart.id),
    name: chart.name,
    altText: chart.altText ?? chart.title ?? chart.name,
    contentType: 'image/svg+xml',
    src: spreadsheetChartSvgDataUrl(chart),
    left: chart.left,
    top: chart.top,
    width: chart.width,
    height: chart.height,
  };
}

function chartPreviewImageId(chartId: string): string {
  return `${CHART_PREVIEW_IMAGE_PREFIX}${chartId}`;
}

function rangeValues(
  sheet: WorkSpreadsheetSheet,
  startRow: number,
  startColumn: number,
  endRow: number,
  endColumn: number
): unknown[] {
  const values: unknown[] = [];
  for (let row = startRow; row <= endRow; row += 1) {
    for (let column = startColumn; column <= endColumn; column += 1) {
      const cell = sheet.data?.[row]?.[column];
      values.push(cell?.v ?? cell?.m ?? '');
    }
  }
  return values;
}

function selectionHasHeader(
  sheet: WorkSpreadsheetSheet,
  bounds: { startRow: number; endRow: number; startColumn: number; endColumn: number }
): boolean {
  const seriesStartColumn = bounds.endColumn > bounds.startColumn ? bounds.startColumn + 1 : bounds.startColumn;
  const firstRowValues = rangeValues(sheet, bounds.startRow, seriesStartColumn, bounds.startRow, bounds.endColumn);
  const bodyValues = rangeValues(sheet, bounds.startRow + 1, seriesStartColumn, bounds.endRow, bounds.endColumn);
  return (
    firstRowValues.some((value) => String(value ?? '').trim() && !isNumericValue(value)) &&
    bodyValues.some(isNumericValue)
  );
}

function normalizedSelection(
  selection: Selection | undefined
): { startRow: number; endRow: number; startColumn: number; endColumn: number } | null {
  if (!selection) return null;
  const startRow = Math.max(0, Math.min(selection.row[0], selection.row[1]));
  const endRow = Math.max(startRow, Math.max(selection.row[0], selection.row[1]));
  const startColumn = Math.max(0, Math.min(selection.column[0], selection.column[1]));
  const endColumn = Math.max(startColumn, Math.max(selection.column[0], selection.column[1]));
  if (![startRow, endRow, startColumn, endColumn].every(Number.isSafeInteger)) return null;
  return { startRow, endRow, startColumn, endColumn };
}

function cellRangeFormula(
  sheetName: string,
  startRow: number,
  startColumn: number,
  endRow: number,
  endColumn: number
): string {
  const start = absoluteCellAddress(startRow, startColumn);
  const end = absoluteCellAddress(endRow, endColumn);
  return `${quoteSpreadsheetSheetName(sheetName)}!${start}${start === end ? '' : `:${end}`}`;
}

function absoluteCellAddress(row: number, column: number): string {
  return `$${columnName(column)}$${row + 1}`;
}

function decodeCellAddress(address: string): { row: number; column: number } | null {
  const match = /^\$?([A-Z]{1,3})\$?([1-9]\d*)$/i.exec(address);
  if (!match) return null;
  let column = 0;
  for (const character of match[1].toUpperCase()) column = column * 26 + character.charCodeAt(0) - 64;
  const row = Number(match[2]);
  if (column < 1 || column > 16_384 || row < 1 || row > 1_048_576) return null;
  return { row: row - 1, column: column - 1 };
}

function columnName(index: number): string {
  let value = index + 1;
  let result = '';
  while (value > 0) {
    value -= 1;
    result = String.fromCharCode(65 + (value % 26)) + result;
    value = Math.floor(value / 26);
  }
  return result;
}

function axisPosition(index: number, size: (index: number) => number): number {
  let position = 0;
  for (let current = 0; current < index; current += 1) position += size(current);
  return position;
}

function inclusiveNumbers(start: number, end: number): number[] {
  return Array.from({ length: Math.max(0, end - start + 1) }, (_, index) => start + index);
}

function displayCellValue(value: unknown): string {
  if (value instanceof Date) return value.toLocaleDateString('zh-CN');
  if (value === null || value === undefined) return '';
  return String(value);
}

function numericCellValue(value: unknown): number {
  if (value instanceof Date) return value.getTime();
  const number = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(number) ? number : 0;
}

function isNumericValue(value: unknown): boolean {
  if (value instanceof Date || typeof value === 'number') return true;
  if (typeof value !== 'string' || !value.trim()) return false;
  return Number.isFinite(Number(value));
}

function finiteDimension(value: number, fallback: number, minimum = 0): number {
  return Number.isFinite(value) ? Math.max(minimum, value) : fallback;
}
