import { locale } from '@fortune-sheet/core';
import type {
  WorkSpreadsheetCalculationSettings,
  WorkSpreadsheetFormulaRange,
  WorkSpreadsheetSheet,
} from './work-types';

export const DEFAULT_SPREADSHEET_CALCULATION_SETTINGS: WorkSpreadsheetCalculationSettings = {
  mode: 'automatic',
  fullCalculationOnLoad: false,
  forceFullCalculation: false,
  iterativeCalculation: false,
  maximumIterations: 100,
  maximumChange: 0.001,
  fullPrecision: true,
};

const VOLATILE_FUNCTIONS = new Set(['CELL', 'INFO', 'INDIRECT', 'NOW', 'OFFSET', 'RAND', 'RANDBETWEEN', 'TODAY']);

const FUTURE_FUNCTIONS = new Set([
  'ARRAYTOTEXT',
  'BYCOL',
  'BYROW',
  'CHOOSECOLS',
  'CHOOSEROWS',
  'DROP',
  'EXPAND',
  'FIELDVALUE',
  'FILTER',
  'HSTACK',
  'IMAGE',
  'ISOMITTED',
  'LAMBDA',
  'LET',
  'MAKEARRAY',
  'MAP',
  'RANDARRAY',
  'REDUCE',
  'SCAN',
  'SEQUENCE',
  'SORT',
  'SORTBY',
  'STOCKHISTORY',
  'TAKE',
  'TEXTAFTER',
  'TEXTBEFORE',
  'TEXTSPLIT',
  'TOCOL',
  'TOROW',
  'UNIQUE',
  'VALUETOTEXT',
  'VSTACK',
  'WRAPCOLS',
  'WRAPROWS',
  'XLOOKUP',
  'XMATCH',
]);

const FORTUNE_SUPPORTED_FUNCTIONS = new Set(
  locale({ lang: 'en' } as Parameters<typeof locale>[0]).functionlist.map((item) => item.n.toUpperCase())
);

const FUTURE_FUNCTION_PREFIXES = new Map<string, string>([
  ['FILTER', '_xlfn._xlws.'],
  ['SORT', '_xlfn._xlws.'],
  ['SORTBY', '_xlfn._xlws.'],
  ['UNIQUE', '_xlfn.'],
  ...Array.from(FUTURE_FUNCTIONS)
    .filter((name) => !['FILTER', 'SORT', 'SORTBY', 'UNIQUE'].includes(name))
    .map((name) => [name, '_xlfn.'] as const),
]);

export interface SpreadsheetRangeBounds {
  startRow: number;
  endRow: number;
  startColumn: number;
  endColumn: number;
}

export function effectiveSpreadsheetCalculationSettings(
  settings: WorkSpreadsheetCalculationSettings | undefined
): WorkSpreadsheetCalculationSettings {
  if (!settings) return { ...DEFAULT_SPREADSHEET_CALCULATION_SETTINGS };
  return {
    mode: settings.mode,
    fullCalculationOnLoad: Boolean(settings.fullCalculationOnLoad),
    forceFullCalculation: Boolean(settings.forceFullCalculation),
    iterativeCalculation: Boolean(settings.iterativeCalculation),
    maximumIterations: clampedInteger(settings.maximumIterations, 1, 10_000, 100),
    maximumChange: positiveNumber(settings.maximumChange, 0.001),
    fullPrecision: settings.fullPrecision !== false,
  };
}

export function editableSpreadsheetFormula(source: string): string {
  return transformFormulaOutsideStrings(source, (segment) =>
    segment
      .replace(/_xlfn\._xlws\.(?=[A-Z][A-Z0-9_.]*\s*\()/gi, '')
      .replace(/_xlfn\.(?=[A-Z][A-Z0-9_.]*\s*\()/gi, '')
      .replace(/_xlws\.(?=[A-Z][A-Z0-9_.]*\s*\()/gi, '')
  );
}

export function spreadsheetFormulaForXlsx(currentFormula: string, sourceFormula?: string): string {
  const current = currentFormula.trim().replace(/^=/, '');
  const source = sourceFormula?.trim().replace(/^=/, '');
  if (source && comparableFormula(source) === comparableFormula(current)) return source;
  return futureFunctionPrefixes(current, source);
}

export function spreadsheetFormulaRangeForCell(
  sheet: WorkSpreadsheetSheet,
  row: number,
  column: number
): WorkSpreadsheetFormulaRange | undefined {
  const address = spreadsheetCellAddress(row, column);
  return (sheet.formulaMetadata?.ranges ?? []).find(
    (range) => range.anchor.toUpperCase().replaceAll('$', '') === address
  );
}

export function spreadsheetFormulaRangesForSelection(
  sheet: WorkSpreadsheetSheet,
  selection: SpreadsheetRangeBounds
): WorkSpreadsheetFormulaRange[] {
  return (sheet.formulaMetadata?.ranges ?? []).filter((range) =>
    rangesOverlap(selection, parseSpreadsheetFormulaRange(range.reference))
  );
}

export function spreadsheetFormulaRangeConflict(
  sheet: WorkSpreadsheetSheet,
  range: WorkSpreadsheetFormulaRange
): string | null {
  const bounds = parseSpreadsheetFormulaRange(range.reference);
  if (!bounds) return '引用范围无效';
  const anchor = parseSpreadsheetCellAddress(range.anchor);
  if (!anchor || !containsCell(bounds, anchor.row, anchor.column)) return '锚点不在引用范围内';
  const anchorCell = sheet.data?.[anchor.row]?.[anchor.column];
  if (!anchorCell) return '锚点单元格不存在';
  const overlaps = (sheet.formulaMetadata?.ranges ?? []).filter(
    (candidate) => candidate !== range && rangesOverlap(bounds, parseSpreadsheetFormulaRange(candidate.reference))
  );
  if (overlaps.length) return '与其他公式范围重叠';
  for (const [row, cells] of (sheet.data ?? []).entries()) {
    if (row < bounds.startRow || row > bounds.endRow) continue;
    for (const [column, cell] of cells.entries()) {
      if (column < bounds.startColumn || column > bounds.endColumn) continue;
      if (row === anchor.row && column === anchor.column) continue;
      if (cell?.f) return `${spreadsheetCellAddress(row, column)} 包含独立公式`;
    }
  }
  if (
    Object.values(sheet.config?.merge ?? {}).some((merge) =>
      rangesOverlap(bounds, {
        startRow: merge.r,
        endRow: merge.r + merge.rs - 1,
        startColumn: merge.c,
        endColumn: merge.c + merge.cs - 1,
      })
    )
  ) {
    return '范围内包含合并单元格';
  }
  if (range.type !== 'data-table' && !anchorCell.f) {
    return '锚点公式已被删除';
  }
  return null;
}

export function spreadsheetFormulaFunctions(formula: string): string[] {
  const withoutStrings = formula.replace(/"(?:[^"]|"")*"/g, '""');
  const functions = new Set<string>();
  for (const match of withoutStrings.matchAll(/(?:_xlfn\.)?(?:_xlws\.)?([A-Z][A-Z0-9_.]*)\s*\(/gi)) {
    if (match[1]) functions.add(match[1].toUpperCase());
  }
  return Array.from(functions);
}

export function formulaHasExternalReference(formula: string): boolean {
  return /\[[^\]]+\][^!]*!/i.test(formula.replace(/"(?:[^"]|"")*"/g, '""'));
}

export function formulaHasStructuredReference(formula: string): boolean {
  const withoutExternalReferences = formula.replace(/"(?:[^"]|"")*"/g, '""').replace(/\[[^\]]+\][^!]*!/gi, '');
  return (
    /\[(?:@|#)[^\]]+\]/i.test(withoutExternalReferences) ||
    /\b[A-Z_\\][A-Z0-9_.]*\s*\[[^\]]+\]/i.test(withoutExternalReferences)
  );
}

export function unsupportedSpreadsheetFormulaFunctions(formula: string): string[] {
  return spreadsheetFormulaFunctions(formula).filter((name) => !FORTUNE_SUPPORTED_FUNCTIONS.has(name));
}

export function volatileSpreadsheetFormulaFunctions(formula: string): string[] {
  return spreadsheetFormulaFunctions(formula).filter((name) => VOLATILE_FUNCTIONS.has(name));
}

export function spreadsheetCellAddress(row: number, column: number): string {
  let value = column + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return `${label}${row + 1}`;
}

function parseSpreadsheetFormulaRange(reference: string): SpreadsheetRangeBounds | null {
  const normalized = reference.trim().replaceAll('$', '').split('!').at(-1) ?? '';
  const [startText, endText = startText] = normalized.split(':');
  const start = parseSpreadsheetCellAddress(startText);
  const end = parseSpreadsheetCellAddress(endText);
  if (!start || !end) return null;
  return {
    startRow: Math.min(start.row, end.row),
    endRow: Math.max(start.row, end.row),
    startColumn: Math.min(start.column, end.column),
    endColumn: Math.max(start.column, end.column),
  };
}

function parseSpreadsheetCellAddress(address: string): { row: number; column: number } | null {
  const match = /^([A-Z]{1,3})([1-9]\d*)$/i.exec(address.trim().replaceAll('$', ''));
  if (!match) return null;
  let column = 0;
  for (const character of match[1].toUpperCase()) column = column * 26 + character.charCodeAt(0) - 64;
  const row = Number(match[2]) - 1;
  if (row > 1_048_575 || column > 16_384) return null;
  return { row, column: column - 1 };
}

function containsCell(bounds: SpreadsheetRangeBounds, row: number, column: number): boolean {
  return row >= bounds.startRow && row <= bounds.endRow && column >= bounds.startColumn && column <= bounds.endColumn;
}

function rangesOverlap(first: SpreadsheetRangeBounds, second: SpreadsheetRangeBounds | null): boolean {
  if (!second) return false;
  return (
    first.startRow <= second.endRow &&
    first.endRow >= second.startRow &&
    first.startColumn <= second.endColumn &&
    first.endColumn >= second.startColumn
  );
}

function comparableFormula(formula: string): string {
  return editableSpreadsheetFormula(formula).trim();
}

function futureFunctionPrefixes(formula: string, sourceFormula?: string): string {
  const sourcePrefixes = new Map<string, string>();
  const sourceFunctions = (sourceFormula ?? '').replace(/"(?:[^"]|"")*"/g, '""');
  for (const match of sourceFunctions.matchAll(/(_xlfn\.(?:_xlws\.)?)([A-Z][A-Z0-9_.]*)\s*(?=\()/gi)) {
    if (match[1] && match[2]) sourcePrefixes.set(match[2].toUpperCase(), match[1]);
  }
  return transformFormulaOutsideStrings(formula, (segment) =>
    segment.replace(/(?<![A-Z0-9_.])([A-Z][A-Z0-9_.]*)\s*(?=\()/gi, (match, functionName: string) => {
      const normalized = functionName.toUpperCase();
      const prefix = sourcePrefixes.get(normalized) ?? FUTURE_FUNCTION_PREFIXES.get(normalized);
      return prefix ? `${prefix}${match}` : match;
    })
  );
}

function clampedInteger(value: number, minimum: number, maximum: number, fallback: number): number {
  if (!Number.isFinite(value)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.round(value)));
}

function positiveNumber(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function transformFormulaOutsideStrings(formula: string, transform: (segment: string) => string): string {
  return formula
    .split(/("(?:[^"]|"")*")/)
    .map((segment, index) => (index % 2 ? segment : transform(segment)))
    .join('');
}
