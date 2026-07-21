import type { Cell, CellMatrix, Sheet } from '@fortune-sheet/core';
import {
  formatSpreadsheetCellRanges,
  parseSpreadsheetCellRanges,
  type SpreadsheetCellRange,
} from './work-spreadsheet-ranges';

export const DEFAULT_PROTECTION_HINT = '此工作表已受保护。若要更改锁定的单元格，请先取消工作表保护。';

export interface FortuneSheetEditableRange {
  name: string;
  sqref: string;
  hintText?: string;
  xlsxAttributes?: Record<string, string>;
}

export interface FortuneSheetProtectionAuthority {
  sheet: 0 | 1;
  selectLockedCells: 0 | 1;
  selectunLockedCells: 0 | 1;
  formatCells: 0 | 1;
  formatColumns: 0 | 1;
  formatRows: 0 | 1;
  insertColumns: 0 | 1;
  insertRows: 0 | 1;
  insertHyperlinks: 0 | 1;
  deleteColumns: 0 | 1;
  deleteRows: 0 | 1;
  sort: 0 | 1;
  filter: 0 | 1;
  usePivotTablereports: 0 | 1;
  editObjects: 0 | 1;
  editScenarios: 0 | 1;
  hintText: string;
  defaultSheetHintText: string;
  allowRangeList: FortuneSheetEditableRange[];
  xlsxAttributes?: Record<string, string>;
}

export interface SpreadsheetCellProtectionRange {
  range: SpreadsheetCellRange;
  locked: boolean;
  hidden: boolean;
}

type ProtectionCell = Cell & { hi?: number };

export function defaultSheetProtectionAuthority(enabled = false): FortuneSheetProtectionAuthority {
  return {
    sheet: enabled ? 1 : 0,
    selectLockedCells: 1,
    selectunLockedCells: 1,
    formatCells: 0,
    formatColumns: 0,
    formatRows: 0,
    insertColumns: 0,
    insertRows: 0,
    insertHyperlinks: 0,
    deleteColumns: 0,
    deleteRows: 0,
    sort: 0,
    filter: 0,
    usePivotTablereports: 0,
    editObjects: 0,
    editScenarios: 0,
    hintText: '',
    defaultSheetHintText: DEFAULT_PROTECTION_HINT,
    allowRangeList: [],
  };
}

export function sheetProtectionAuthority(sheet: Sheet): FortuneSheetProtectionAuthority {
  return normalizeSheetProtectionAuthority(sheet.config?.authority);
}

export function normalizeSheetProtectionAuthority(source: unknown): FortuneSheetProtectionAuthority {
  const defaults = defaultSheetProtectionAuthority();
  if (!source || typeof source !== 'object') return defaults;
  const authority = source as Record<string, unknown>;
  return {
    sheet: flag(authority.sheet, defaults.sheet),
    selectLockedCells: flag(authority.selectLockedCells, defaults.selectLockedCells),
    selectunLockedCells: flag(authority.selectunLockedCells, defaults.selectunLockedCells),
    formatCells: flag(authority.formatCells, defaults.formatCells),
    formatColumns: flag(authority.formatColumns, defaults.formatColumns),
    formatRows: flag(authority.formatRows, defaults.formatRows),
    insertColumns: flag(authority.insertColumns, defaults.insertColumns),
    insertRows: flag(authority.insertRows, defaults.insertRows),
    insertHyperlinks: flag(authority.insertHyperlinks, defaults.insertHyperlinks),
    deleteColumns: flag(authority.deleteColumns, defaults.deleteColumns),
    deleteRows: flag(authority.deleteRows, defaults.deleteRows),
    sort: flag(authority.sort, defaults.sort),
    filter: flag(authority.filter, defaults.filter),
    usePivotTablereports: flag(authority.usePivotTablereports, defaults.usePivotTablereports),
    editObjects: flag(authority.editObjects, defaults.editObjects),
    editScenarios: flag(authority.editScenarios, defaults.editScenarios),
    hintText: stringValue(authority.hintText),
    defaultSheetHintText: stringValue(authority.defaultSheetHintText) || DEFAULT_PROTECTION_HINT,
    allowRangeList: editableRangeList(authority.allowRangeList),
    xlsxAttributes: stringRecord(authority.xlsxAttributes),
  };
}

export function withSheetProtection(sheet: Sheet, enabled: boolean): Sheet {
  const authority = sheetProtectionAuthority(sheet);
  authority.sheet = enabled ? 1 : 0;
  return withAuthority(sheet, authority);
}

export function withSheetSelectionPermissions(
  sheet: Sheet,
  permissions: { selectLockedCells?: boolean; selectUnlockedCells?: boolean }
): Sheet {
  const authority = sheetProtectionAuthority(sheet);
  if (permissions.selectLockedCells !== undefined) {
    authority.selectLockedCells = permissions.selectLockedCells ? 1 : 0;
  }
  if (permissions.selectUnlockedCells !== undefined) {
    authority.selectunLockedCells = permissions.selectUnlockedCells ? 1 : 0;
  }
  return withAuthority(sheet, authority);
}

export function withEditableRange(sheet: Sheet, index: number | null, editableRange: FortuneSheetEditableRange): Sheet {
  const authority = sheetProtectionAuthority(sheet);
  const nextRange: FortuneSheetEditableRange = {
    name: editableRange.name.trim(),
    sqref: canonicalRangeReference(editableRange.sqref),
    hintText: editableRange.hintText?.trim() || undefined,
    xlsxAttributes: editableRange.xlsxAttributes,
  };
  if (index !== null && authority.allowRangeList[index]) authority.allowRangeList[index] = nextRange;
  else authority.allowRangeList.push(nextRange);
  const ranges = parseSpreadsheetCellRanges(nextRange.sqref) ?? [];
  return withAuthority(withCellProtection(sheet, ranges, false), authority);
}

export function withoutEditableRange(sheet: Sheet, index: number): Sheet {
  const authority = sheetProtectionAuthority(sheet);
  const removed = authority.allowRangeList[index];
  if (!removed) return sheet;
  authority.allowRangeList.splice(index, 1);
  const removedRanges = parseSpreadsheetCellRanges(removed.sqref) ?? [];
  let next = withCellProtection(sheet, removedRanges, true);
  for (const editableRange of authority.allowRangeList) {
    if (editableRangeRequiresCredentials(editableRange)) continue;
    next = withCellProtection(next, parseSpreadsheetCellRanges(editableRange.sqref) ?? [], false);
  }
  return withAuthority(next, authority);
}

export function withCellProtection(
  sheet: Sheet,
  ranges: SpreadsheetCellRange[],
  locked: boolean,
  hidden = false
): Sheet {
  if (!ranges.length) return sheet;
  const data = cloneMatrix(sheet.data);
  let rowCount = Math.max(sheet.row ?? 0, data.length);
  let columnCount = Math.max(sheet.column ?? 0, ...data.map((row) => row.length));
  for (const range of ranges) {
    rowCount = Math.max(rowCount, range.row[1] + 1);
    columnCount = Math.max(columnCount, range.column[1] + 1);
    for (let row = range.row[0]; row <= range.row[1]; row += 1) {
      data[row] ??= [];
      for (let column = range.column[0]; column <= range.column[1]; column += 1) {
        const cell = { ...(data[row][column] ?? {}) } as ProtectionCell;
        cell.lo = locked ? 1 : 0;
        if (hidden) cell.hi = 1;
        else if (cell.hi !== undefined) delete cell.hi;
        data[row][column] = cell;
      }
    }
  }
  normalizeMatrix(data, rowCount, columnCount);
  return { ...sheet, row: rowCount, column: columnCount, data };
}

export function applySpreadsheetCellProtectionRanges(
  data: CellMatrix,
  ranges: SpreadsheetCellProtectionRange[],
  rowCount: number,
  columnCount: number
): void {
  for (const item of ranges) {
    const lastRow = Math.min(item.range.row[1], rowCount - 1);
    const lastColumn = Math.min(item.range.column[1], columnCount - 1);
    for (let row = Math.max(0, item.range.row[0]); row <= lastRow; row += 1) {
      data[row] ??= [];
      for (let column = Math.max(0, item.range.column[0]); column <= lastColumn; column += 1) {
        const cell = { ...(data[row][column] ?? {}) } as ProtectionCell;
        cell.lo = item.locked ? 1 : 0;
        if (item.hidden) cell.hi = 1;
        else if (cell.hi !== undefined) delete cell.hi;
        data[row][column] = cell;
      }
    }
  }
}

export function applyPasswordlessEditableRanges(
  data: CellMatrix,
  ranges: FortuneSheetEditableRange[],
  rowCount: number,
  columnCount: number
): void {
  const protectionRanges = ranges
    .filter((range) => !editableRangeRequiresCredentials(range))
    .flatMap((range) => parseSpreadsheetCellRanges(range.sqref) ?? [])
    .map((range) => ({ range, locked: false, hidden: false }));
  applySpreadsheetCellProtectionRanges(data, protectionRanges, rowCount, columnCount);
}

export function editableRangeRequiresCredentials(range: FortuneSheetEditableRange): boolean {
  const attributes = range.xlsxAttributes ?? {};
  return Boolean(
    attributes.password ||
      attributes.hashValue ||
      attributes.saltValue ||
      attributes.securityDescriptor ||
      attributes.securitydescriptor
  );
}

export function sheetHasProtectionState(sheet: Sheet): boolean {
  const authority = sheetProtectionAuthority(sheet);
  if (authority.sheet === 1 || authority.allowRangeList.length || Object.keys(authority.xlsxAttributes ?? {}).length) {
    return true;
  }
  return (sheet.data ?? []).some((row) =>
    row.some((cell) => cell?.lo !== undefined || (cell as ProtectionCell | null)?.hi !== undefined)
  );
}

export function protectedSheetCount(sheets: Sheet[]): number {
  return sheets.filter((sheet) => sheetProtectionAuthority(sheet).sheet === 1).length;
}

export function unlockedCellCount(sheet: Sheet): number {
  return (sheet.data ?? []).reduce(
    (total, row) => total + row.reduce((rowTotal, cell) => rowTotal + (cell?.lo === 0 ? 1 : 0), 0),
    0
  );
}

export function spreadsheetProtectionKey(sheets: Sheet[]): string {
  return sheets
    .map((sheet) => {
      const authority = sheetProtectionAuthority(sheet);
      const cells: string[] = [];
      for (const [row, values] of (sheet.data ?? []).entries()) {
        for (const [column, cell] of values.entries()) {
          const hidden = (cell as ProtectionCell | null)?.hi;
          if (cell?.lo !== undefined || hidden !== undefined) {
            cells.push(`${row}_${column}:${cell?.lo ?? ''}:${hidden ?? ''}`);
          }
        }
      }
      return `${sheet.id ?? sheet.name}:${JSON.stringify(authority)}:${cells.join(',')}`;
    })
    .join('|');
}

export function editableRangeCellCount(ranges: SpreadsheetCellRange[]): number {
  return ranges.reduce(
    (total, range) => total + (range.row[1] - range.row[0] + 1) * (range.column[1] - range.column[0] + 1),
    0
  );
}

function withAuthority(sheet: Sheet, authority: FortuneSheetProtectionAuthority): Sheet {
  return {
    ...sheet,
    config: {
      ...(sheet.config ?? {}),
      authority,
    },
  };
}

function editableRangeList(value: unknown): FortuneSheetEditableRange[] {
  if (!Array.isArray(value)) return [];
  return value.flatMap((item, index) => {
    if (!item || typeof item !== 'object') return [];
    const range = item as Record<string, unknown>;
    const sqref = stringValue(range.sqref);
    if (!sqref || !parseSpreadsheetCellRanges(sqref)) return [];
    return [
      {
        name: stringValue(range.name) || `Range ${index + 1}`,
        sqref: canonicalRangeReference(sqref),
        hintText: stringValue(range.hintText) || undefined,
        xlsxAttributes: stringRecord(range.xlsxAttributes),
      },
    ];
  });
}

function canonicalRangeReference(value: string): string {
  const ranges = parseSpreadsheetCellRanges(value);
  return ranges ? formatSpreadsheetCellRanges(ranges) : value.trim();
}

function cloneMatrix(source: CellMatrix | undefined): CellMatrix {
  return (source ?? []).map((row) => [...row]);
}

function normalizeMatrix(data: CellMatrix, rows: number, columns: number): void {
  while (data.length < rows) data.push([]);
  for (const row of data) while (row.length < columns) row.push(null);
}

function flag(value: unknown, fallback: 0 | 1): 0 | 1 {
  if (value === 1 || value === true || value === '1') return 1;
  if (value === 0 || value === false || value === '0') return 0;
  return fallback;
}

function stringValue(value: unknown): string {
  return typeof value === 'string' ? value : '';
}

function stringRecord(value: unknown): Record<string, string> | undefined {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return undefined;
  const entries = Object.entries(value).filter((entry): entry is [string, string] => typeof entry[1] === 'string');
  return entries.length ? Object.fromEntries(entries) : undefined;
}
