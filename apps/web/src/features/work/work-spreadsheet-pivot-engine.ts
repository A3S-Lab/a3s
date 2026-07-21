import type { Cell, CellMatrix } from '@fortune-sheet/core';
import {
  displaySpreadsheetPivotValue,
  finiteSpreadsheetPivotNumber,
  normalizeSpreadsheetPivotFilterValue,
  spreadsheetPivotCellValue,
  spreadsheetPivotFilterValueKey,
} from './work-spreadsheet-pivot-values';
import type {
  WorkSpreadsheetPivotAggregation,
  WorkSpreadsheetPivotFilterValue,
  WorkSpreadsheetPivotTable,
  WorkSpreadsheetPivotValue,
  WorkSpreadsheetSheet,
} from './work-types';

export interface SpreadsheetPivotField {
  index: number;
  name: string;
  numeric: boolean;
}

export interface SpreadsheetPivotFilterItem {
  value: WorkSpreadsheetPivotFilterValue;
  label: string;
}

export interface SpreadsheetPivotBounds {
  startRow: number;
  endRow: number;
  startColumn: number;
  endColumn: number;
}

interface PivotAccumulator {
  nonEmptyCount: number;
  numberCount: number;
  sum: number;
  product: number;
  minimum?: number;
  maximum?: number;
  mean: number;
  m2: number;
}

interface PivotCombination {
  key: string;
  values: unknown[];
}

export function spreadsheetPivotFilterItemsFromSource(
  sourceSheet: WorkSpreadsheetSheet,
  sourceBounds: SpreadsheetPivotBounds,
  fieldIndex: number
): SpreadsheetPivotFilterItem[] {
  if (fieldIndex < 0 || fieldIndex > sourceBounds.endColumn - sourceBounds.startColumn) return [];
  const items = new Map<string, SpreadsheetPivotFilterItem>();
  for (let row = sourceBounds.startRow + 1; row <= sourceBounds.endRow; row += 1) {
    const value = normalizeSpreadsheetPivotFilterValue(
      spreadsheetPivotCellValue(sourceSheet.data?.[row]?.[sourceBounds.startColumn + fieldIndex])
    );
    const item = { value, label: displaySpreadsheetPivotValue(value) };
    items.set(spreadsheetPivotFilterValueKey(value), item);
  }
  return Array.from(items.values()).sort((left, right) =>
    left.label.localeCompare(right.label, 'zh-CN', {
      numeric: true,
      sensitivity: 'base',
    })
  );
}

export function buildSpreadsheetPivotOutput(
  sourceSheet: WorkSpreadsheetSheet,
  sourceBounds: SpreadsheetPivotBounds,
  fields: SpreadsheetPivotField[],
  pivot: WorkSpreadsheetPivotTable
): CellMatrix {
  const rowCombinations = new Map<string, PivotCombination>();
  const columnCombinations = new Map<string, PivotCombination>();
  const cells = new Map<string, PivotAccumulator>();
  const rowTotals = new Map<string, PivotAccumulator>();
  const columnTotals = new Map<string, PivotAccumulator>();
  const totals = pivot.values.map(() => accumulator());
  const singleColumn = combination([]);
  if (!pivot.columnFields.length) columnCombinations.set(singleColumn.key, singleColumn);

  for (let row = sourceBounds.startRow + 1; row <= sourceBounds.endRow; row += 1) {
    const values = fields.map((field) =>
      spreadsheetPivotCellValue(sourceSheet.data?.[row]?.[sourceBounds.startColumn + field.index])
    );
    if (values.every((value) => value === null || value === undefined || value === '')) continue;
    if (!matchesPivotReportFilters(values, pivot)) continue;
    const rowCombination = combination(pivot.rowFields.map((fieldIndex) => values[fieldIndex]));
    const columnCombination = pivot.columnFields.length
      ? combination(pivot.columnFields.map((fieldIndex) => values[fieldIndex]))
      : singleColumn;
    rowCombinations.set(rowCombination.key, rowCombination);
    columnCombinations.set(columnCombination.key, columnCombination);
    for (const [valueIndex, valueField] of pivot.values.entries()) {
      const value = values[valueField.fieldIndex];
      updateAccumulator(mapAccumulator(cells, cellKey(rowCombination.key, columnCombination.key, valueIndex)), value);
      updateAccumulator(mapAccumulator(rowTotals, cellKey(rowCombination.key, '', valueIndex)), value);
      updateAccumulator(mapAccumulator(columnTotals, cellKey('', columnCombination.key, valueIndex)), value);
      updateAccumulator(totals[valueIndex], value);
    }
  }

  const sortedRows = Array.from(rowCombinations.values()).sort(compareCombinations);
  const sortedColumns = Array.from(columnCombinations.values()).sort(compareCombinations);
  if (!sortedRows.length) return [];
  const headers = [
    ...pivot.rowFields.map((fieldIndex) => fields[fieldIndex].name),
    ...sortedColumns.flatMap((column) =>
      pivot.values.map((value) => columnValueCaption(column, pivot.columnFields, fields, value))
    ),
    ...(pivot.rowGrandTotals && pivot.columnFields.length
      ? pivot.values.map((value) => `总计 · ${valueCaption(value, fields)}`)
      : []),
  ];
  const output: CellMatrix = [headers.map((value) => pivotHeaderCell(value))];

  for (const row of sortedRows) {
    const result: Array<Cell | null> = row.values.map((value) => pivotDimensionCell(value));
    for (const column of sortedColumns) {
      for (const [valueIndex, value] of pivot.values.entries()) {
        result.push(
          pivotValueCell(
            accumulatorValue(cells.get(cellKey(row.key, column.key, valueIndex)), value.aggregation),
            false
          )
        );
      }
    }
    if (pivot.rowGrandTotals && pivot.columnFields.length) {
      for (const [valueIndex, value] of pivot.values.entries()) {
        result.push(
          pivotValueCell(accumulatorValue(rowTotals.get(cellKey(row.key, '', valueIndex)), value.aggregation), true)
        );
      }
    }
    output.push(result);
  }
  if (pivot.columnGrandTotals) {
    const result: Array<Cell | null> = pivot.rowFields.map((_field, index) =>
      index === 0 ? pivotTotalLabelCell() : pivotValueCell(null, true)
    );
    for (const column of sortedColumns) {
      for (const [valueIndex, value] of pivot.values.entries()) {
        result.push(
          pivotValueCell(
            accumulatorValue(columnTotals.get(cellKey('', column.key, valueIndex)), value.aggregation),
            true
          )
        );
      }
    }
    if (pivot.rowGrandTotals && pivot.columnFields.length) {
      for (const [valueIndex, value] of pivot.values.entries()) {
        result.push(pivotValueCell(accumulatorValue(totals[valueIndex], value.aggregation), true));
      }
    }
    output.push(result);
  }
  const reportFilters = pivot.reportFilters ?? [];
  if (!reportFilters.length) return output;
  const width = output[0]?.length ?? 0;
  const filterRows: CellMatrix = reportFilters.map((filter) => {
    const row = Array<Cell | null>(width).fill(null);
    row[0] = pivotFilterLabelCell(fields[filter.fieldIndex].name);
    row[1] = pivotFilterSelectionCell(
      filter.selectedItem === undefined ? '(全部)' : displaySpreadsheetPivotValue(filter.selectedItem)
    );
    return row;
  });
  filterRows.push(Array<Cell | null>(width).fill(null));
  return [...filterRows, ...output];
}

function accumulator(): PivotAccumulator {
  return {
    nonEmptyCount: 0,
    numberCount: 0,
    sum: 0,
    product: 1,
    mean: 0,
    m2: 0,
  };
}

function mapAccumulator(map: Map<string, PivotAccumulator>, key: string): PivotAccumulator {
  const existing = map.get(key);
  if (existing) return existing;
  const next = accumulator();
  map.set(key, next);
  return next;
}

function updateAccumulator(state: PivotAccumulator, value: unknown): void {
  if (value !== null && value !== undefined && value !== '') state.nonEmptyCount += 1;
  const number = finiteSpreadsheetPivotNumber(value);
  if (number === null) return;
  state.numberCount += 1;
  state.sum += number;
  state.product *= number;
  state.minimum = state.minimum === undefined ? number : Math.min(state.minimum, number);
  state.maximum = state.maximum === undefined ? number : Math.max(state.maximum, number);
  const delta = number - state.mean;
  state.mean += delta / state.numberCount;
  state.m2 += delta * (number - state.mean);
}

function accumulatorValue(
  state: PivotAccumulator | undefined,
  aggregation: WorkSpreadsheetPivotAggregation
): number | null {
  if (!state) return aggregation === 'sum' || aggregation === 'count' || aggregation === 'counta' ? 0 : null;
  if (aggregation === 'count') return state.numberCount;
  if (aggregation === 'counta') return state.nonEmptyCount;
  if (aggregation === 'average') return state.numberCount ? state.sum / state.numberCount : null;
  if (aggregation === 'max') return state.maximum ?? null;
  if (aggregation === 'min') return state.minimum ?? null;
  if (aggregation === 'product') return state.numberCount ? state.product : null;
  if (aggregation === 'stdDev') return state.numberCount > 1 ? Math.sqrt(state.m2 / (state.numberCount - 1)) : null;
  if (aggregation === 'stdDevP') return state.numberCount ? Math.sqrt(state.m2 / state.numberCount) : null;
  if (aggregation === 'var') return state.numberCount > 1 ? state.m2 / (state.numberCount - 1) : null;
  if (aggregation === 'varP') return state.numberCount ? state.m2 / state.numberCount : null;
  return state.sum;
}

function combination(values: unknown[]): PivotCombination {
  return {
    key: JSON.stringify(values.map(normalizedPivotKeyValue)),
    values,
  };
}

function normalizedPivotKeyValue(value: unknown): unknown {
  if (value instanceof Date) return ['date', value.toISOString()];
  if (typeof value === 'number') return ['number', Number.isFinite(value) ? value : null];
  if (typeof value === 'boolean') return ['boolean', value];
  if (value === null || value === undefined || value === '') return ['blank'];
  return ['string', String(value)];
}

function compareCombinations(left: PivotCombination, right: PivotCombination): number {
  const count = Math.max(left.values.length, right.values.length);
  for (let index = 0; index < count; index += 1) {
    const difference = displaySpreadsheetPivotValue(left.values[index]).localeCompare(
      displaySpreadsheetPivotValue(right.values[index]),
      'zh-CN',
      {
        numeric: true,
        sensitivity: 'base',
      }
    );
    if (difference) return difference;
  }
  return 0;
}

function cellKey(rowKey: string, columnKey: string, valueIndex: number): string {
  return `${rowKey.length}:${rowKey}${columnKey.length}:${columnKey}${valueIndex}`;
}

function columnValueCaption(
  column: PivotCombination,
  columnFields: number[],
  fields: SpreadsheetPivotField[],
  value: WorkSpreadsheetPivotValue
): string {
  const dimensions = column.values.map((item, index) => {
    const label = displaySpreadsheetPivotValue(item);
    return columnFields.length > 1 ? `${fields[columnFields[index]].name}: ${label}` : label;
  });
  return [...dimensions, valueCaption(value, fields)].filter(Boolean).join(' · ');
}

function valueCaption(value: WorkSpreadsheetPivotValue, fields: SpreadsheetPivotField[]): string {
  return (
    value.caption?.trim() ||
    defaultPivotValueCaption(fields[value.fieldIndex]?.name ?? `字段 ${value.fieldIndex + 1}`, value.aggregation)
  );
}

export function defaultPivotValueCaption(fieldName: string, aggregation: WorkSpreadsheetPivotAggregation): string {
  return `${fieldName}（${spreadsheetPivotAggregationLabel(aggregation)}）`;
}

export function spreadsheetPivotAggregationLabel(aggregation: WorkSpreadsheetPivotAggregation): string {
  if (aggregation === 'count') return '计数';
  if (aggregation === 'counta') return '非空计数';
  if (aggregation === 'average') return '平均值';
  if (aggregation === 'max') return '最大值';
  if (aggregation === 'min') return '最小值';
  if (aggregation === 'product') return '乘积';
  if (aggregation === 'stdDev') return '样本标准差';
  if (aggregation === 'stdDevP') return '总体标准差';
  if (aggregation === 'var') return '样本方差';
  if (aggregation === 'varP') return '总体方差';
  return '求和';
}

function pivotHeaderCell(value: string): Cell {
  return {
    v: value,
    m: value,
    bl: 1,
    bg: '#e7f1ec',
    fc: '#173d2b',
  };
}

function pivotDimensionCell(value: unknown): Cell {
  const display = displaySpreadsheetPivotValue(value);
  return {
    v: value === null || value === undefined || value === '' ? display : (value as Cell['v']),
    m: display,
  };
}

function pivotValueCell(value: number | null, total: boolean): Cell | null {
  if (value === null) {
    return total ? { v: '', m: '', bl: 1, bg: '#f3f7f5' } : null;
  }
  return {
    v: value,
    m: formatPivotNumber(value),
    bl: total ? 1 : undefined,
    bg: total ? '#f3f7f5' : undefined,
    ht: 2,
  };
}

function pivotTotalLabelCell(): Cell {
  return {
    v: '总计',
    m: '总计',
    bl: 1,
    bg: '#f3f7f5',
  };
}

function pivotFilterLabelCell(value: string): Cell {
  return {
    v: value,
    m: value,
    bl: 1,
    bg: '#f3f7f5',
    fc: '#173d2b',
  };
}

function pivotFilterSelectionCell(value: string): Cell {
  return {
    v: value,
    m: value,
    bg: '#f3f7f5',
  };
}

function formatPivotNumber(value: number): string {
  return Number.isInteger(value) ? String(value) : String(Number(value.toPrecision(15)));
}

function matchesPivotReportFilters(values: unknown[], pivot: WorkSpreadsheetPivotTable): boolean {
  return (pivot.reportFilters ?? []).every(
    (filter) =>
      filter.selectedItem === undefined ||
      spreadsheetPivotFilterValueKey(normalizeSpreadsheetPivotFilterValue(values[filter.fieldIndex])) ===
        spreadsheetPivotFilterValueKey(filter.selectedItem)
  );
}
