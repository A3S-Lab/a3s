import type { Cell } from '@fortune-sheet/core';
import type { WorkSpreadsheetPivotFilterValue } from './work-types';

export function spreadsheetPivotCellValue(cell: Cell | null | undefined): unknown {
  return cell?.v ?? cell?.m ?? null;
}

export function normalizeSpreadsheetPivotFilterValue(value: unknown): WorkSpreadsheetPivotFilterValue {
  if (value === null || value === undefined || value === '') return null;
  if (value instanceof Date) return value.toISOString();
  if (typeof value === 'number') return Number.isFinite(value) ? value : String(value);
  if (typeof value === 'boolean') return value;
  return String(value);
}

export function spreadsheetPivotFilterValueKey(value: WorkSpreadsheetPivotFilterValue): string {
  if (value === null) return 'blank:';
  return `${typeof value}:${String(value)}`;
}

export function displaySpreadsheetPivotValue(value: unknown): string {
  if (value === null || value === undefined || value === '') return '(空白)';
  if (value instanceof Date) return value.toISOString().slice(0, 10);
  if (typeof value === 'boolean') return value ? 'TRUE' : 'FALSE';
  return String(value);
}

export function finiteSpreadsheetPivotNumber(value: unknown): number | null {
  if (typeof value === 'number') return Number.isFinite(value) ? value : null;
  if (value instanceof Date) return value.getTime();
  return null;
}
