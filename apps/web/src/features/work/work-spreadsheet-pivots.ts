import type { Cell, CellMatrix, Selection } from '@fortune-sheet/core';
import {
  buildSpreadsheetPivotOutput,
  defaultPivotValueCaption,
  spreadsheetPivotAggregationLabel,
  spreadsheetPivotFilterItemsFromSource,
  type SpreadsheetPivotBounds as PivotBounds,
  type SpreadsheetPivotField,
  type SpreadsheetPivotFilterItem,
} from './work-spreadsheet-pivot-engine';
import {
  displaySpreadsheetPivotValue,
  finiteSpreadsheetPivotNumber,
  spreadsheetPivotCellValue,
  spreadsheetPivotFilterValueKey,
} from './work-spreadsheet-pivot-values';
import { formatSpreadsheetCellRanges, parseSpreadsheetCellRanges } from './work-spreadsheet-ranges';
import { createWorkId } from './work-templates';
import type { WorkSpreadsheetContent, WorkSpreadsheetPivotTable, WorkSpreadsheetSheet } from './work-types';

const MAXIMUM_SOURCE_CELLS = 100_000;
const MAXIMUM_OUTPUT_CELLS = 20_000;
const MAXIMUM_XLSX_ROW = 1_048_575;
const MAXIMUM_XLSX_COLUMN = 16_383;

export { defaultPivotValueCaption, spreadsheetPivotAggregationLabel };
export type { SpreadsheetPivotField, SpreadsheetPivotFilterItem };

export interface SpreadsheetPivotValidation {
  valid: boolean;
  code?: string;
  message?: string;
  outputReference?: string;
}

export interface CreateSpreadsheetPivotResult {
  content: WorkSpreadsheetContent;
  ownerSheetId?: string;
  pivotId?: string;
  error?: string;
}

interface ResolvedPivot {
  ownerSheet: WorkSpreadsheetSheet;
  sourceSheet: WorkSpreadsheetSheet;
  sourceBounds: PivotBounds;
  outputBounds: PivotBounds;
  outputReference: string;
  fields: SpreadsheetPivotField[];
  output: CellMatrix;
}

export function spreadsheetPivotCount(content: WorkSpreadsheetContent): number {
  return content.sheets.reduce((count, sheet) => count + (sheet.pivotTables?.length ?? 0), 0);
}

export function spreadsheetPivotFields(
  content: WorkSpreadsheetContent,
  pivot: Pick<WorkSpreadsheetPivotTable, 'sourceSheetId' | 'sourceReference'>
): SpreadsheetPivotField[] {
  const sourceSheet = content.sheets.find((sheet) => sheet.id === pivot.sourceSheetId);
  const range = singleRange(pivot.sourceReference);
  if (!sourceSheet || !range) return [];
  const fields: SpreadsheetPivotField[] = [];
  for (let column = range.startColumn; column <= range.endColumn; column += 1) {
    const header = spreadsheetPivotCellValue(sourceSheet.data?.[range.startRow]?.[column]);
    if (header === null || header === undefined || String(header).trim() === '') return [];
    const name = displaySpreadsheetPivotValue(header).trim();
    let numeric = false;
    for (let row = range.startRow + 1; row <= range.endRow; row += 1) {
      const value = spreadsheetPivotCellValue(sourceSheet.data?.[row]?.[column]);
      if (value === null || value === undefined || value === '') continue;
      if (finiteSpreadsheetPivotNumber(value) !== null) {
        numeric = true;
        break;
      }
    }
    fields.push({ index: column - range.startColumn, name, numeric });
  }
  return uniqueFieldNames(fields) ? fields : [];
}

export function spreadsheetPivotFilterItems(
  content: WorkSpreadsheetContent,
  pivot: Pick<WorkSpreadsheetPivotTable, 'sourceSheetId' | 'sourceReference'>,
  fieldIndex: number
): SpreadsheetPivotFilterItem[] {
  const sourceSheet = content.sheets.find((sheet) => sheet.id === pivot.sourceSheetId);
  const range = singleRange(pivot.sourceReference);
  if (!sourceSheet || !range || fieldIndex < 0 || fieldIndex > range.endColumn - range.startColumn) return [];
  return spreadsheetPivotFilterItemsFromSource(sourceSheet, range, fieldIndex);
}

export function createSpreadsheetPivotFromSelection(
  content: WorkSpreadsheetContent,
  sourceSheetId: string,
  selection: Pick<Selection, 'row' | 'column'>
): CreateSpreadsheetPivotResult {
  const sourceSheet = content.sheets.find((sheet) => sheet.id === sourceSheetId);
  const bounds = selectionBounds(selection);
  if (!sourceSheet || !bounds || bounds.endRow <= bounds.startRow) {
    return { content, error: '请选择包含标题行和至少一行数据的连续区域。' };
  }
  if (
    spreadsheetPivotIntersects(sourceSheet, {
      startRow: bounds.startRow,
      endRow: bounds.endRow,
      startColumn: bounds.startColumn,
      endColumn: bounds.endColumn,
    }).length
  ) {
    return { content, error: '不能把现有数据透视表的输出区域作为新的透视表源。' };
  }
  const sourceReference = formatSpreadsheetCellRanges([
    {
      row: [bounds.startRow, bounds.endRow],
      column: [bounds.startColumn, bounds.endColumn],
    },
  ]);
  const draftPivot: WorkSpreadsheetPivotTable = {
    id: createWorkId('pivot'),
    name: nextPivotName(content),
    sourceSheetId,
    sourceReference,
    anchor: 'A1',
    rowFields: [],
    columnFields: [],
    reportFilters: [],
    values: [],
    rowGrandTotals: true,
    columnGrandTotals: true,
    styleName: 'PivotStyleLight16',
    refreshOnLoad: true,
  };
  const fields = spreadsheetPivotFields(content, draftPivot);
  if (!fields.length) {
    return { content, error: '透视表源区域的标题必须非空且不重复。' };
  }
  const valueField = fields.find((field) => field.numeric) ?? fields.at(-1)!;
  const dimensions = fields.filter((field) => field.index !== valueField.index);
  const rowField = dimensions[0] ?? fields.find((field) => field.index !== valueField.index);
  if (!rowField) {
    return { content, error: '透视表至少需要一个分类字段和一个值字段。' };
  }
  const columnField = dimensions.find((field) => field.index !== rowField.index);
  draftPivot.rowFields = [rowField.index];
  draftPivot.columnFields = columnField ? [columnField.index] : [];
  draftPivot.values = [
    {
      fieldIndex: valueField.index,
      aggregation: valueField.numeric ? 'sum' : 'counta',
      caption: defaultPivotValueCaption(valueField.name, valueField.numeric ? 'sum' : 'counta'),
    },
  ];

  const ownerSheetId = createWorkId('sheet');
  const ownerSheet: WorkSpreadsheetSheet = {
    id: ownerSheetId,
    name: nextSheetName(content, '数据透视表'),
    order: content.sheets.length,
    status: 0,
    row: 40,
    column: 12,
    data: Array.from({ length: 40 }, () => Array<Cell | null>(12).fill(null)),
    config: {},
    pivotTables: [draftPivot],
  };
  const withReport = {
    ...content,
    sheets: [...content.sheets, ownerSheet],
  };
  const validation = spreadsheetPivotValidation(withReport, ownerSheetId, draftPivot);
  if (!validation.valid) return { content, error: validation.message };
  const next = refreshSpreadsheetPivotTables(withReport);
  return {
    content: next,
    ownerSheetId,
    pivotId: draftPivot.id,
  };
}

export function refreshSpreadsheetPivotTables(content: WorkSpreadsheetContent): WorkSpreadsheetContent {
  if (!spreadsheetPivotCount(content)) return content;
  const sheets = content.sheets.map((sheet) => ({
    ...sheet,
    pivotTables: sheet.pivotTables?.map((pivot) => ({
      ...pivot,
      rowFields: [...pivot.rowFields],
      columnFields: [...pivot.columnFields],
      reportFilters: pivot.reportFilters?.map((filter) => ({ ...filter })),
      values: pivot.values.map((value) => ({ ...value })),
    })),
  }));
  const next: WorkSpreadsheetContent = { ...content, sheets };

  for (const ownerSheet of sheets) {
    if (!ownerSheet.pivotTables?.length || !ownerSheet.id) continue;
    let data = ownerSheet.data;
    let dataChanged = false;
    for (const pivot of ownerSheet.pivotTables) {
      const resolved = resolvePivot(next, ownerSheet.id, pivot);
      if (!resolved) continue;
      if (!dataChanged) {
        data = cloneCellMatrix(ownerSheet.data);
        ownerSheet.data = data;
        dataChanged = true;
      }
      clearRange(data!, singleRange(pivot.outputReference ?? ''));
      writePivotOutput(data!, resolved.outputBounds, resolved.output);
      pivot.outputReference = resolved.outputReference;
      ownerSheet.row = Math.max(ownerSheet.row ?? 0, resolved.outputBounds.endRow + 1, data!.length);
      ownerSheet.column = Math.max(ownerSheet.column ?? 0, resolved.outputBounds.endColumn + 1);
    }
  }
  return next;
}

export function reconcileSpreadsheetPivots(
  content: WorkSpreadsheetContent,
  changedSheets: WorkSpreadsheetSheet[]
): WorkSpreadsheetContent {
  const sourceById = new Map(content.sheets.flatMap((sheet) => (sheet.id ? [[sheet.id, sheet] as const] : [])));
  const sheets = changedSheets.map((sheet) => {
    const source = sheet.id ? sourceById.get(sheet.id) : undefined;
    return source?.pivotTables?.length
      ? {
          ...sheet,
          pivotTables: source.pivotTables,
        }
      : sheet;
  });
  return refreshSpreadsheetPivotTables({ ...content, sheets });
}

export function deleteSpreadsheetPivotTable(
  content: WorkSpreadsheetContent,
  ownerSheetId: string,
  pivotId: string
): WorkSpreadsheetContent {
  const sheets = content.sheets.map((sheet) => {
    if (sheet.id !== ownerSheetId) return sheet;
    const pivot = sheet.pivotTables?.find((candidate) => candidate.id === pivotId);
    if (!pivot) return sheet;
    const data = cloneCellMatrix(sheet.data);
    clearRange(data, singleRange(pivot.outputReference ?? ''));
    const pivotTables = (sheet.pivotTables ?? []).filter((candidate) => candidate.id !== pivotId);
    return {
      ...sheet,
      data,
      pivotTables: pivotTables.length ? pivotTables : undefined,
    };
  });
  return { ...content, sheets };
}

export function spreadsheetPivotValidation(
  content: WorkSpreadsheetContent,
  ownerSheetId: string,
  pivot: WorkSpreadsheetPivotTable
): SpreadsheetPivotValidation {
  const failure = pivotFailure(content, ownerSheetId, pivot);
  if (failure) return failure;
  const resolved = resolvePivot(content, ownerSheetId, pivot);
  return resolved
    ? { valid: true, outputReference: resolved.outputReference }
    : {
        valid: false,
        code: 'pivot.invalid',
        message: '无法根据当前字段和源数据生成透视表。',
      };
}

export function spreadsheetPivotOutputContains(sheet: WorkSpreadsheetSheet, row: number, column: number): boolean {
  return (sheet.pivotTables ?? []).some((pivot) => {
    const bounds = singleRange(pivot.outputReference ?? '');
    return Boolean(bounds && containsCell(bounds, row, column));
  });
}

export function spreadsheetPivotIntersects(
  sheet: WorkSpreadsheetSheet,
  range: PivotBounds
): WorkSpreadsheetPivotTable[] {
  return (sheet.pivotTables ?? []).filter((pivot) => {
    const bounds = singleRange(pivot.outputReference ?? '');
    return Boolean(bounds && rangesOverlap(bounds, range));
  });
}

function resolvePivot(
  content: WorkSpreadsheetContent,
  ownerSheetId: string,
  pivot: WorkSpreadsheetPivotTable
): ResolvedPivot | null {
  const failure = pivotFailure(content, ownerSheetId, pivot);
  if (failure) return null;
  const ownerSheet = content.sheets.find((sheet) => sheet.id === ownerSheetId)!;
  const sourceSheet = content.sheets.find((sheet) => sheet.id === pivot.sourceSheetId)!;
  const sourceBounds = singleRange(pivot.sourceReference)!;
  const anchor = singleRange(pivot.anchor)!;
  const fields = spreadsheetPivotFields(content, pivot);
  const output = buildSpreadsheetPivotOutput(sourceSheet, sourceBounds, fields, pivot);
  if (!output.length || !output[0]?.length) return null;
  const outputBounds = {
    startRow: anchor.startRow,
    endRow: anchor.startRow + output.length - 1,
    startColumn: anchor.startColumn,
    endColumn: anchor.startColumn + output[0].length - 1,
  };
  const outputReference = formatBounds(outputBounds);
  if (output.length * output[0].length > MAXIMUM_OUTPUT_CELLS) return null;
  if (outputBounds.endRow > MAXIMUM_XLSX_ROW || outputBounds.endColumn > MAXIMUM_XLSX_COLUMN) return null;
  if (ownerSheet.id === sourceSheet.id && rangesOverlap(sourceBounds, outputBounds)) return null;
  const oldOutput = singleRange(pivot.outputReference ?? '');
  for (const other of ownerSheet.pivotTables ?? []) {
    if (other.id === pivot.id) continue;
    const otherOutput = singleRange(other.outputReference ?? '');
    if (otherOutput && rangesOverlap(otherOutput, outputBounds)) return null;
  }
  for (let row = outputBounds.startRow; row <= outputBounds.endRow; row += 1) {
    for (let column = outputBounds.startColumn; column <= outputBounds.endColumn; column += 1) {
      if (oldOutput && containsCell(oldOutput, row, column)) continue;
      if (ownerSheet.data?.[row]?.[column]) return null;
    }
  }
  return {
    ownerSheet,
    sourceSheet,
    sourceBounds,
    outputBounds,
    outputReference,
    fields,
    output,
  };
}

function pivotFailure(
  content: WorkSpreadsheetContent,
  ownerSheetId: string,
  pivot: WorkSpreadsheetPivotTable
): SpreadsheetPivotValidation | null {
  const ownerSheet = content.sheets.find((sheet) => sheet.id === ownerSheetId);
  const sourceSheet = content.sheets.find((sheet) => sheet.id === pivot.sourceSheetId);
  if (!ownerSheet || !sourceSheet) {
    return invalid('pivot.source-sheet-missing', '找不到透视表的源工作表或目标工作表。');
  }
  const sourceBounds = singleRange(pivot.sourceReference);
  if (!sourceBounds || sourceBounds.endRow <= sourceBounds.startRow) {
    return invalid('pivot.source-reference-invalid', '源区域必须是包含标题和数据的连续单元格范围。');
  }
  const sourceCellCount =
    (sourceBounds.endRow - sourceBounds.startRow + 1) * (sourceBounds.endColumn - sourceBounds.startColumn + 1);
  if (sourceCellCount > MAXIMUM_SOURCE_CELLS) {
    return invalid('pivot.source-too-large', `源区域不能超过 ${MAXIMUM_SOURCE_CELLS.toLocaleString()} 个单元格。`);
  }
  const anchor = singleRange(pivot.anchor);
  if (!anchor || anchor.startRow !== anchor.endRow || anchor.startColumn !== anchor.endColumn) {
    return invalid('pivot.anchor-invalid', '输出位置必须是一个 A1 单元格地址。');
  }
  const fields = spreadsheetPivotFields(content, pivot);
  if (!fields.length) {
    return invalid('pivot.headers-invalid', '源区域的标题必须非空且不重复。');
  }
  const reportFilters = pivot.reportFilters ?? [];
  const reportFilterFields = reportFilters.map((filter) => filter.fieldIndex);
  if (
    new Set(reportFilterFields).size !== reportFilterFields.length ||
    reportFilterFields.some((index) => !fields[index]) ||
    reportFilterFields.some((index) => pivot.rowFields.includes(index) || pivot.columnFields.includes(index))
  ) {
    return invalid('pivot.filters-invalid', '报表筛选字段必须存在、互不重复，且不能同时作为行或列字段。');
  }
  for (const filter of reportFilters) {
    if (
      filter.selectedItem !== undefined &&
      !spreadsheetPivotFilterItems(content, pivot, filter.fieldIndex).some(
        (item) => spreadsheetPivotFilterValueKey(item.value) === spreadsheetPivotFilterValueKey(filter.selectedItem!)
      )
    ) {
      return invalid('pivot.filter-item-invalid', '报表筛选器选择的项目已不在源数据中。');
    }
  }
  const dimensionFields = [...pivot.rowFields, ...pivot.columnFields, ...reportFilterFields];
  if (!pivot.rowFields.length) {
    return invalid('pivot.rows-empty', '至少选择一个行字段。');
  }
  if (!pivot.values.length) {
    return invalid('pivot.values-empty', '至少选择一个值字段。');
  }
  if (
    new Set(dimensionFields).size !== dimensionFields.length ||
    dimensionFields.some((index) => !fields[index]) ||
    pivot.values.some((value) => !fields[value.fieldIndex])
  ) {
    return invalid('pivot.fields-invalid', '透视表包含重复、缺失或超出源区域的字段。');
  }
  const output = buildSpreadsheetPivotOutput(sourceSheet, sourceBounds, fields, pivot);
  if (!output.length || !output[0]?.length) {
    return invalid('pivot.output-empty', '当前源数据不能生成透视表结果。');
  }
  if (output.length * output[0].length > MAXIMUM_OUTPUT_CELLS) {
    return invalid('pivot.output-too-large', `透视表结果不能超过 ${MAXIMUM_OUTPUT_CELLS.toLocaleString()} 个单元格。`);
  }
  const outputBounds = {
    startRow: anchor.startRow,
    endRow: anchor.startRow + output.length - 1,
    startColumn: anchor.startColumn,
    endColumn: anchor.startColumn + output[0].length - 1,
  };
  if (outputBounds.endRow > MAXIMUM_XLSX_ROW || outputBounds.endColumn > MAXIMUM_XLSX_COLUMN) {
    return invalid('pivot.output-out-of-bounds', '透视表结果超出 XLSX 工作表边界。');
  }
  if (ownerSheet.id === sourceSheet.id && rangesOverlap(sourceBounds, outputBounds)) {
    return invalid('pivot.output-overlaps-source', '透视表输出区域与源数据重叠，请改用其他位置或工作表。');
  }
  if (
    Object.values(ownerSheet.config?.merge ?? {}).some((merge) =>
      rangesOverlap(outputBounds, {
        startRow: merge.r,
        endRow: merge.r + merge.rs - 1,
        startColumn: merge.c,
        endColumn: merge.c + merge.cs - 1,
      })
    )
  ) {
    return invalid('pivot.output-overlaps-merge', '透视表输出区域与合并单元格重叠。');
  }
  const oldOutput = singleRange(pivot.outputReference ?? '');
  for (const other of ownerSheet.pivotTables ?? []) {
    if (other.id === pivot.id) continue;
    const otherOutput = singleRange(other.outputReference ?? '');
    if (otherOutput && rangesOverlap(otherOutput, outputBounds)) {
      return invalid('pivot.output-overlaps-pivot', '透视表输出区域与另一个透视表重叠。');
    }
  }
  for (let row = outputBounds.startRow; row <= outputBounds.endRow; row += 1) {
    for (let column = outputBounds.startColumn; column <= outputBounds.endColumn; column += 1) {
      if (oldOutput && containsCell(oldOutput, row, column)) continue;
      if (ownerSheet.data?.[row]?.[column]) {
        return invalid('pivot.output-not-empty', '透视表输出区域包含现有内容，请选择空白位置。');
      }
    }
  }
  return null;
}

function uniqueFieldNames(fields: SpreadsheetPivotField[]): boolean {
  const names = fields.map((field) => field.name.trim().toLocaleLowerCase());
  return names.every(Boolean) && new Set(names).size === names.length;
}

function singleRange(reference: string): PivotBounds | null {
  const ranges = parseSpreadsheetCellRanges(reference);
  if (ranges?.length !== 1) return null;
  return {
    startRow: ranges[0].row[0],
    endRow: ranges[0].row[1],
    startColumn: ranges[0].column[0],
    endColumn: ranges[0].column[1],
  };
}

function selectionBounds(selection: Pick<Selection, 'row' | 'column'>): PivotBounds | null {
  if (selection.row.length < 2 || selection.column.length < 2) return null;
  return {
    startRow: Math.max(0, Math.min(selection.row[0], selection.row[1])),
    endRow: Math.max(0, Math.max(selection.row[0], selection.row[1])),
    startColumn: Math.max(0, Math.min(selection.column[0], selection.column[1])),
    endColumn: Math.max(0, Math.max(selection.column[0], selection.column[1])),
  };
}

function containsCell(bounds: PivotBounds, row: number, column: number): boolean {
  return row >= bounds.startRow && row <= bounds.endRow && column >= bounds.startColumn && column <= bounds.endColumn;
}

function rangesOverlap(left: PivotBounds, right: PivotBounds): boolean {
  return (
    left.startRow <= right.endRow &&
    left.endRow >= right.startRow &&
    left.startColumn <= right.endColumn &&
    left.endColumn >= right.startColumn
  );
}

function formatBounds(bounds: PivotBounds): string {
  return formatSpreadsheetCellRanges([
    {
      row: [bounds.startRow, bounds.endRow],
      column: [bounds.startColumn, bounds.endColumn],
    },
  ]);
}

function cloneCellMatrix(source: CellMatrix | undefined): CellMatrix {
  return (source ?? []).map((row) => [...row]);
}

function clearRange(data: CellMatrix, bounds: PivotBounds | null): void {
  if (!bounds) return;
  for (let row = bounds.startRow; row <= bounds.endRow; row += 1) {
    if (!data[row]) continue;
    for (let column = bounds.startColumn; column <= bounds.endColumn; column += 1) {
      data[row][column] = null;
    }
  }
}

function writePivotOutput(data: CellMatrix, bounds: PivotBounds, output: CellMatrix): void {
  const width = output[0]?.length ?? 0;
  while (data.length <= bounds.endRow) data.push([]);
  for (let row = 0; row < output.length; row += 1) {
    const target = data[bounds.startRow + row] ?? [];
    data[bounds.startRow + row] = target;
    while (target.length < bounds.startColumn + width) target.push(null);
    for (let column = 0; column < width; column += 1) {
      target[bounds.startColumn + column] = output[row][column] ?? null;
    }
  }
}

function nextSheetName(content: WorkSpreadsheetContent, base: string): string {
  const existing = new Set(content.sheets.map((sheet) => sheet.name.toLocaleLowerCase()));
  if (!existing.has(base.toLocaleLowerCase())) return base;
  let index = 2;
  while (existing.has(`${base} ${index}`.toLocaleLowerCase())) index += 1;
  return `${base} ${index}`;
}

function nextPivotName(content: WorkSpreadsheetContent): string {
  const existing = new Set(
    content.sheets.flatMap((sheet) => (sheet.pivotTables ?? []).map((pivot) => pivot.name.toLocaleLowerCase()))
  );
  let index = 1;
  while (existing.has(`pivottable${index}`)) index += 1;
  return `PivotTable${index}`;
}

function invalid(code: string, message: string): SpreadsheetPivotValidation {
  return { valid: false, code, message };
}
