const CELL_REFERENCE = /\$?([A-Z]{1,3})\$?([1-9]\d*)/i;
const CELL_OR_RANGE = new RegExp(`^${CELL_REFERENCE.source}(?::${CELL_REFERENCE.source})?$`, 'i');
const COLUMN_RANGE = /^\$?([A-Z]{1,3}):\$?([A-Z]{1,3})$/i;
const ROW_RANGE = /^\$?([1-9]\d*):\$?([1-9]\d*)$/;
const DEFINED_NAME = /^[\p{L}_\\][\p{L}\p{N}_.\\]*$/u;

export interface SpreadsheetPrintBounds {
  startRow: number;
  endRow: number;
  startColumn: number;
  endColumn: number;
}

export interface SpreadsheetCellRange {
  row: [number, number];
  column: [number, number];
}

export interface SpreadsheetPrintTitleBounds {
  rows?: [number, number];
  columns?: [number, number];
}

export function isValidSpreadsheetDefinedName(value: string): boolean {
  const name = value.trim();
  if (!name || name.length > 255 || !DEFINED_NAME.test(name) || /^_xlnm\./i.test(name)) return false;
  if (/^[A-Z]{1,3}[1-9]\d*$/i.test(name) || /^R\d+C\d+$/i.test(name)) return false;
  return true;
}

export function normalizeSpreadsheetPrintArea(value: string): string | null {
  const parts = splitRangeList(value.trim().replace(/^=/, ''));
  if (!parts.length) return null;
  const normalized = parts.map((part) => {
    const reference = unqualifiedReference(part);
    if (!CELL_OR_RANGE.test(reference) && !COLUMN_RANGE.test(reference) && !ROW_RANGE.test(reference)) return null;
    return reference.replace(/[A-Z]+/gi, (column) => column.toUpperCase());
  });
  return normalized.every((part): part is string => Boolean(part)) ? normalized.join(',') : null;
}

export function normalizeSpreadsheetPrintTitleRows(value: string): string | null {
  const match = ROW_RANGE.exec(unqualifiedReference(value.trim().replace(/^=/, '')));
  if (!match) return null;
  const start = Number(match[1]);
  const end = Number(match[2]);
  return `$${Math.min(start, end)}:$${Math.max(start, end)}`;
}

export function normalizeSpreadsheetPrintTitleColumns(value: string): string | null {
  const match = COLUMN_RANGE.exec(unqualifiedReference(value.trim().replace(/^=/, '')));
  if (!match) return null;
  const start = decodeColumn(match[1]);
  const end = decodeColumn(match[2]);
  return `$${encodeColumn(Math.min(start, end))}:$${encodeColumn(Math.max(start, end))}`;
}

export function parseSpreadsheetPrintTitles(value: string): SpreadsheetPrintTitleBounds | null {
  let rows: [number, number] | undefined;
  let columns: [number, number] | undefined;
  for (const part of splitRangeList(value.trim().replace(/^=/, ''))) {
    const rowReference = normalizeSpreadsheetPrintTitleRows(part);
    if (rowReference) {
      if (rows) return null;
      const match = ROW_RANGE.exec(rowReference)!;
      rows = [Number(match[1]) - 1, Number(match[2]) - 1];
      continue;
    }
    const columnReference = normalizeSpreadsheetPrintTitleColumns(part);
    if (!columnReference || columns) return null;
    const match = COLUMN_RANGE.exec(columnReference)!;
    columns = [decodeColumn(match[1]), decodeColumn(match[2])];
  }
  return rows || columns ? { rows, columns } : null;
}

export function spreadsheetPrintTitleBounds(
  rowReference: string | undefined,
  columnReference: string | undefined,
  maximumRow: number,
  maximumColumn: number
): SpreadsheetPrintTitleBounds | null {
  const rows = rowReference ? normalizeSpreadsheetPrintTitleRows(rowReference) : null;
  const columns = columnReference ? normalizeSpreadsheetPrintTitleColumns(columnReference) : null;
  if ((rowReference && !rows) || (columnReference && !columns) || (!rows && !columns)) return null;
  const rowMatch = rows ? ROW_RANGE.exec(rows) : null;
  const columnMatch = columns ? COLUMN_RANGE.exec(columns) : null;
  return {
    rows: rowMatch
      ? [clamp(Number(rowMatch[1]) - 1, maximumRow), clamp(Number(rowMatch[2]) - 1, maximumRow)]
      : undefined,
    columns: columnMatch
      ? [clamp(decodeColumn(columnMatch[1]), maximumColumn), clamp(decodeColumn(columnMatch[2]), maximumColumn)]
      : undefined,
  };
}

export function parseSpreadsheetCellRanges(value: string): SpreadsheetCellRange[] | null {
  const parts = splitRangeList(value.trim().replace(/^=/, ''));
  if (!parts.length) return null;
  const ranges = parts.map((part) => {
    const cells = CELL_OR_RANGE.exec(unqualifiedReference(part));
    if (!cells) return null;
    const first = decodeCell(cells[1], cells[2]);
    const second = cells[3] && cells[4] ? decodeCell(cells[3], cells[4]) : first;
    return {
      row: [Math.min(first.row, second.row), Math.max(first.row, second.row)] as [number, number],
      column: [Math.min(first.column, second.column), Math.max(first.column, second.column)] as [number, number],
    };
  });
  return ranges.every((range): range is SpreadsheetCellRange => Boolean(range)) ? ranges : null;
}

export function formatSpreadsheetCellRanges(ranges: SpreadsheetCellRange[]): string {
  return ranges
    .map((range) => {
      const start = encodeCell(range.row[0], range.column[0]);
      const end = encodeCell(range.row[1], range.column[1]);
      return start === end ? start : `${start}:${end}`;
    })
    .join(',');
}

export function qualifySpreadsheetRange(value: string, sheetName: string): string {
  const prefix = `'${sheetName.replaceAll("'", "''")}'!`;
  return splitRangeList(value)
    .map((part) => (part.includes('!') ? part.trim() : `${prefix}${part.trim()}`))
    .join(',');
}

export function stripSpreadsheetSheetQualifier(value: string, sheetName: string): string {
  const quotedPrefix = `'${sheetName.replaceAll("'", "''")}'!`;
  const plainPrefix = `${sheetName}!`;
  return splitRangeList(value.trim().replace(/^=/, ''))
    .map((part) => {
      const reference = part.trim();
      if (reference.toLowerCase().startsWith(quotedPrefix.toLowerCase())) {
        return reference.slice(quotedPrefix.length);
      }
      if (reference.toLowerCase().startsWith(plainPrefix.toLowerCase())) {
        return reference.slice(plainPrefix.length);
      }
      return reference;
    })
    .join(',');
}

export function spreadsheetPrintBounds(
  value: string,
  maximumRow: number,
  maximumColumn: number
): SpreadsheetPrintBounds | null {
  const ranges: SpreadsheetPrintBounds[] = [];
  for (const part of splitRangeList(value)) {
    const reference = unqualifiedReference(part);
    const cells = CELL_OR_RANGE.exec(reference);
    if (cells) {
      const first = decodeCell(cells[1], cells[2]);
      const second = cells[3] && cells[4] ? decodeCell(cells[3], cells[4]) : first;
      ranges.push({
        startRow: Math.min(first.row, second.row),
        endRow: Math.max(first.row, second.row),
        startColumn: Math.min(first.column, second.column),
        endColumn: Math.max(first.column, second.column),
      });
      continue;
    }
    const columns = COLUMN_RANGE.exec(reference);
    if (columns) {
      ranges.push({
        startRow: 0,
        endRow: maximumRow,
        startColumn: Math.min(decodeColumn(columns[1]), decodeColumn(columns[2])),
        endColumn: Math.max(decodeColumn(columns[1]), decodeColumn(columns[2])),
      });
      continue;
    }
    const rows = ROW_RANGE.exec(reference);
    if (rows) {
      ranges.push({
        startRow: Math.min(Number(rows[1]), Number(rows[2])) - 1,
        endRow: Math.max(Number(rows[1]), Number(rows[2])) - 1,
        startColumn: 0,
        endColumn: maximumColumn,
      });
    }
  }
  if (!ranges.length) return null;
  return {
    startRow: clamp(Math.min(...ranges.map((range) => range.startRow)), maximumRow),
    endRow: clamp(Math.max(...ranges.map((range) => range.endRow)), maximumRow),
    startColumn: clamp(Math.min(...ranges.map((range) => range.startColumn)), maximumColumn),
    endColumn: clamp(Math.max(...ranges.map((range) => range.endColumn)), maximumColumn),
  };
}

function splitRangeList(value: string): string[] {
  const parts: string[] = [];
  let current = '';
  let quoted = false;
  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (character === "'") {
      if (quoted && value[index + 1] === "'") {
        current += "''";
        index += 1;
        continue;
      }
      quoted = !quoted;
    }
    if (character === ',' && !quoted) {
      if (current.trim()) parts.push(current.trim());
      current = '';
    } else {
      current += character;
    }
  }
  if (current.trim()) parts.push(current.trim());
  return parts;
}

function unqualifiedReference(value: string): string {
  const reference = value.trim().replace(/^=/, '');
  const separator = reference.lastIndexOf('!');
  return separator >= 0 ? reference.slice(separator + 1) : reference;
}

function decodeCell(column: string, row: string): { row: number; column: number } {
  return {
    row: Math.max(0, Number(row) - 1),
    column: decodeColumn(column),
  };
}

function decodeColumn(value: string): number {
  let result = 0;
  for (const character of value.toUpperCase()) result = result * 26 + character.charCodeAt(0) - 64;
  return Math.max(0, result - 1);
}

function encodeCell(row: number, column: number): string {
  return `${encodeColumn(column)}${Math.max(0, row) + 1}`;
}

function encodeColumn(column: number): string {
  let value = Math.max(0, column) + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return label;
}

function clamp(value: number, maximum: number): number {
  return Math.max(0, Math.min(Math.max(0, maximum), value));
}
