import type { CellMatrix, Sheet } from '@fortune-sheet/core';
import {
  interpolateSpreadsheetConditionalColor,
  parseSpreadsheetConditionalColor,
  spreadsheetConditionalCssColor,
  spreadsheetConditionalRatio,
  type SpreadsheetConditionalRgbColor,
} from './work-spreadsheet-conditional-colors';
import {
  spreadsheetConditionalDataBar,
  type SpreadsheetConditionalDataBar,
} from './work-spreadsheet-conditional-data-bar';
import {
  normalizeSpreadsheetConditionalIconSetFormat,
  spreadsheetConditionalIconForValue,
  type SpreadsheetConditionalIcon,
} from './work-spreadsheet-conditional-icons';
import {
  DEFAULT_DATA_BAR_MAX_LENGTH,
  DEFAULT_DATA_BAR_MIN_LENGTH,
  defaultSpreadsheetColorScaleThresholds,
  defaultSpreadsheetDataBarOptions,
  normalizeSpreadsheetConditionalVisualOptions,
  spreadsheetConditionalThresholdValue,
} from './work-spreadsheet-conditional-values';
import type {
  FortuneConditionalFormatRange,
  FortuneConditionalFormatRule,
  FortuneConditionalFormatStyle,
} from './work-xlsx-conditional-format';

export interface SpreadsheetConditionalCellStyle {
  textColor?: string;
  cellColor?: string;
  dataBar?: SpreadsheetConditionalDataBar;
  icon?: SpreadsheetConditionalIcon;
}

interface CellPosition {
  row: number;
  column: number;
}

export function spreadsheetConditionalFormatStyles(sheet: Sheet): Map<string, SpreadsheetConditionalCellStyle> {
  const styles = new Map<string, SpreadsheetConditionalCellStyle>();
  const data = sheet.data ?? [];
  if (!data.length) return styles;
  const blocked = new Set<string>();
  for (const rule of conditionalRules(sheet)) {
    const matched = new Set<string>();
    if (rule.type === 'colorGradation') applyColorScale(styles, data, rule, blocked, matched);
    else if (rule.type === 'dataBar') applyDataBars(styles, data, rule, blocked, matched);
    else if (rule.type === 'icons') applyIconSet(styles, data, rule, blocked, matched);
    else applyDefaultRule(styles, data, rule, blocked, matched);
    if (rule.stopIfTrue) for (const key of matched) blocked.add(key);
  }
  return styles;
}

function applyDefaultRule(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  data: CellMatrix,
  rule: FortuneConditionalFormatRule,
  blocked: Set<string>,
  matched: Set<string>
) {
  if (rule.type !== 'default' || !isConditionalFormatStyle(rule.format) || !rule.conditionName) return;
  const values = (rule.conditionValue ?? []).map(String);
  const style = rule.format;
  if (rule.conditionName === 'formula' && values[0]) {
    for (const range of rule.cellrange) {
      forEachCell(data, [range], ({ row, column }) => {
        if (evaluateFormula(values[0], row, column, range, data)) {
          applyDifferentialStyle(styles, blocked, matched, row, column, style);
        }
      });
    }
    return;
  }
  if (rule.conditionName === 'duplicateValue') {
    for (const range of rule.cellrange) {
      const positions = cellPositions(data, [range]).filter(({ row, column }) => cellValue(data, row, column) != null);
      const occurrences = new Map<string, number>();
      for (const { row, column } of positions) {
        const key = comparableKey(cellValue(data, row, column));
        occurrences.set(key, (occurrences.get(key) ?? 0) + 1);
      }
      const unique = values[0] === '1';
      for (const { row, column } of positions) {
        const count = occurrences.get(comparableKey(cellValue(data, row, column))) ?? 0;
        if ((unique && count === 1) || (!unique && count > 1)) {
          applyDifferentialStyle(styles, blocked, matched, row, column, style);
        }
      }
    }
    return;
  }
  if (['top10', 'top10_percent', 'last10', 'last10_percent'].includes(rule.conditionName)) {
    for (const range of rule.cellrange) {
      applyRankRule(styles, data, range, rule.conditionName, values[0], style, blocked, matched);
    }
    return;
  }
  if (rule.conditionName === 'aboveAverage' || rule.conditionName === 'belowAverage') {
    for (const range of rule.cellrange) {
      applyAverageRule(styles, data, range, rule.conditionName, style, blocked, matched);
    }
    return;
  }
  forEachCell(data, rule.cellrange, ({ row, column }) => {
    const current = cellValue(data, row, column);
    if (matchesCondition(current, rule.conditionName!, values)) {
      applyDifferentialStyle(styles, blocked, matched, row, column, style);
    }
  });
}

function applyRankRule(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  data: CellMatrix,
  range: FortuneConditionalFormatRange,
  conditionName: string,
  sourceRank: string | undefined,
  style: FortuneConditionalFormatStyle,
  blocked: Set<string>,
  matched: Set<string>
) {
  const positions = cellPositions(data, [range]);
  const numbers = positions.flatMap(({ row, column }) => {
    const value = numericCellValue(data, row, column);
    return value === null ? [] : [value];
  });
  if (!numbers.length) return;
  numbers.sort((left, right) => right - left);
  const requested = Math.max(1, Math.trunc(Number(sourceRank) || 10));
  const count = conditionName.endsWith('_percent')
    ? Math.max(1, Math.ceil((Math.min(100, requested) / 100) * numbers.length))
    : Math.min(numbers.length, requested);
  const bottom = conditionName.startsWith('last');
  const threshold = bottom ? numbers[numbers.length - count] : numbers[count - 1];
  for (const { row, column } of positions) {
    const value = numericCellValue(data, row, column);
    if (value !== null && (bottom ? value <= threshold : value >= threshold)) {
      applyDifferentialStyle(styles, blocked, matched, row, column, style);
    }
  }
}

function applyAverageRule(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  data: CellMatrix,
  range: FortuneConditionalFormatRange,
  conditionName: string,
  style: FortuneConditionalFormatStyle,
  blocked: Set<string>,
  matched: Set<string>
) {
  const positions = cellPositions(data, [range]);
  const values = positions.flatMap(({ row, column }) => {
    const value = numericCellValue(data, row, column);
    return value === null ? [] : [value];
  });
  if (!values.length) return;
  const average = values.reduce((sum, value) => sum + value, 0) / values.length;
  for (const { row, column } of positions) {
    const value = numericCellValue(data, row, column);
    if (value !== null && (conditionName === 'aboveAverage' ? value > average : value < average)) {
      applyDifferentialStyle(styles, blocked, matched, row, column, style);
    }
  }
}

function applyColorScale(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  data: CellMatrix,
  rule: FortuneConditionalFormatRule,
  blocked: Set<string>,
  matched: Set<string>
) {
  if (!Array.isArray(rule.format) || rule.format.length < 2) return;
  const positions = cellPositions(data, rule.cellrange);
  const numericPositions = positions.flatMap((position) => {
    const value = numericCellValue(data, position.row, position.column);
    return value === null ? [] : [{ ...position, value }];
  });
  if (!numericPositions.length) return;
  const colors = rule.format.slice(0, 3).map(parseSpreadsheetConditionalColor);
  if (colors.some((color) => !color)) return;
  const values = numericPositions.map(({ value }) => value);
  const options = rule.visualOptions
    ? normalizeSpreadsheetConditionalVisualOptions(rule.visualOptions, colors.length, 'colorScale')
    : { thresholds: defaultSpreadsheetColorScaleThresholds(colors.length) };
  if (!options) return;
  const points = options.thresholds.map((threshold) => spreadsheetConditionalThresholdValue(threshold, values));
  if (points.some((point) => point === null)) return;
  const minimum = points[0]!;
  const midpoint = points.length === 3 ? points[1]! : null;
  const maximum = points.at(-1)!;
  const maximumColor = colors[0]!;
  const midpointColor = colors.length === 3 ? colors[1]! : null;
  const minimumColor = colors.at(-1)!;
  for (const { row, column, value } of numericPositions) {
    let color: SpreadsheetConditionalRgbColor;
    if (maximum === minimum) {
      color = midpointColor ?? minimumColor;
    } else if (midpointColor && midpoint !== null && value <= midpoint) {
      color = interpolateSpreadsheetConditionalColor(
        minimumColor,
        midpointColor,
        spreadsheetConditionalRatio(value, minimum, midpoint)
      );
    } else if (midpointColor && midpoint !== null) {
      color = interpolateSpreadsheetConditionalColor(
        midpointColor,
        maximumColor,
        spreadsheetConditionalRatio(value, midpoint, maximum)
      );
    } else {
      color = interpolateSpreadsheetConditionalColor(
        minimumColor,
        maximumColor,
        spreadsheetConditionalRatio(value, minimum, maximum)
      );
    }
    applyConditionalStyle(styles, blocked, matched, row, column, {
      cellColor: spreadsheetConditionalCssColor(color),
    });
  }
}

function applyDataBars(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  data: CellMatrix,
  rule: FortuneConditionalFormatRule,
  blocked: Set<string>,
  matched: Set<string>
) {
  if (!isConditionalFormatStyle(rule.format)) return;
  const color = rule.format.cellColor;
  if (!color) return;
  const positions = cellPositions(data, rule.cellrange);
  const numericPositions = positions.flatMap((position) => {
    const value = numericCellValue(data, position.row, position.column);
    return value === null ? [] : [{ ...position, value }];
  });
  if (!numericPositions.length) return;
  const values = numericPositions.map(({ value }) => value);
  const options = rule.visualOptions
    ? normalizeSpreadsheetConditionalVisualOptions(rule.visualOptions, 2, 'dataBar')
    : defaultSpreadsheetDataBarOptions();
  if (!options) return;
  const points = options.thresholds.map((threshold) => spreadsheetConditionalThresholdValue(threshold, values));
  if (points.some((point) => point === null)) return;
  const minimum = points[0]!;
  const maximum = points[1]!;
  for (const { row, column, value } of numericPositions) {
    applyConditionalStyle(styles, blocked, matched, row, column, {
      dataBar: spreadsheetConditionalDataBar(
        color,
        value,
        minimum,
        maximum,
        options.minLength ?? DEFAULT_DATA_BAR_MIN_LENGTH,
        options.maxLength ?? DEFAULT_DATA_BAR_MAX_LENGTH,
        options.showValue !== false
      ),
    });
  }
}

function applyIconSet(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  data: CellMatrix,
  rule: FortuneConditionalFormatRule,
  blocked: Set<string>,
  matched: Set<string>
) {
  if (rule.type !== 'icons') return;
  const format = normalizeSpreadsheetConditionalIconSetFormat(rule.format);
  if (!format) return;
  const numericPositions = cellPositions(data, rule.cellrange).flatMap((position) => {
    const value = numericCellValue(data, position.row, position.column);
    return value === null ? [] : [{ ...position, value }];
  });
  const values = numericPositions.map(({ value }) => value);
  for (const { row, column, value } of numericPositions) {
    const icon = spreadsheetConditionalIconForValue(format, value, values);
    if (icon) applyConditionalStyle(styles, blocked, matched, row, column, { icon });
  }
}

function matchesCondition(value: unknown, name: string, conditions: string[]): boolean {
  if (value === null || value === undefined) return false;
  if (name === 'textContains')
    return String(value)
      .toLowerCase()
      .includes((conditions[0] ?? '').toLowerCase());
  if (name === 'equal') return compareValues(value, conditions[0]) === 0;
  if (name === 'notEqual') return compareValues(value, conditions[0]) !== 0;
  if (name === 'greaterThan') return compareValues(value, conditions[0]) > 0;
  if (name === 'greaterThanOrEqual') return compareValues(value, conditions[0]) >= 0;
  if (name === 'lessThan') return compareValues(value, conditions[0]) < 0;
  if (name === 'lessThanOrEqual') return compareValues(value, conditions[0]) <= 0;
  if (name === 'between' || name === 'notBetween') {
    const lower = Math.min(Number(conditions[0]), Number(conditions[1]));
    const upper = Math.max(Number(conditions[0]), Number(conditions[1]));
    if (!Number.isFinite(lower) || !Number.isFinite(upper)) return false;
    const between = Number(value) >= lower && Number(value) <= upper;
    return name === 'between' ? between : !between;
  }
  return false;
}

function evaluateFormula(
  source: string,
  row: number,
  column: number,
  baseRange: FortuneConditionalFormatRange,
  data: CellMatrix
): boolean {
  const formula = source.trim().replace(/^=/, '').replace(/\s+/g, '');
  if (/^TRUE\(\)?$/i.test(formula)) return true;
  if (/^FALSE\(\)?$/i.test(formula)) return false;
  const parity = /^MOD\((ROW|COLUMN)\(\),(\d+)\)(=|<>|>=|<=|>|<)(-?\d+)$/i.exec(formula);
  if (parity) {
    const value = parity[1].toUpperCase() === 'ROW' ? row + 1 : column + 1;
    return compareWithOperator(value % Number(parity[2]), parity[3], Number(parity[4]));
  }
  const blank = /^(NOT\()?ISBLANK\((\$?)([A-Z]{1,3})(\$?)([1-9]\d*)\)\)?$/i.exec(formula);
  if (blank) {
    const target = formulaReference(blank[2], blank[3], blank[4], blank[5], row, column, baseRange);
    const empty =
      cellValue(data, target.row, target.column) == null || cellValue(data, target.row, target.column) === '';
    return Boolean(blank[1]) ? !empty : empty;
  }
  const comparison = /^(\$?)([A-Z]{1,3})(\$?)([1-9]\d*)(=|<>|>=|<=|>|<)(?:"((?:[^"]|"")*)"|(-?\d+(?:\.\d+)?))$/i.exec(
    formula
  );
  if (!comparison) return false;
  const target = formulaReference(comparison[1], comparison[2], comparison[3], comparison[4], row, column, baseRange);
  const left = cellValue(data, target.row, target.column);
  const right = comparison[6] !== undefined ? comparison[6].replaceAll('""', '"') : Number(comparison[7]);
  return compareWithOperator(left, comparison[5], right);
}

function formulaReference(
  absoluteColumn: string,
  columnName: string,
  absoluteRow: string,
  rowNumber: string,
  row: number,
  column: number,
  baseRange: FortuneConditionalFormatRange
): CellPosition {
  const sourceColumn = decodeColumn(columnName);
  const sourceRow = Number(rowNumber) - 1;
  return {
    row: absoluteRow ? sourceRow : sourceRow + row - baseRange.row[0],
    column: absoluteColumn ? sourceColumn : sourceColumn + column - baseRange.column[0],
  };
}

function applyDifferentialStyle(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  blocked: Set<string>,
  matched: Set<string>,
  row: number,
  column: number,
  style: FortuneConditionalFormatStyle
) {
  const patch: SpreadsheetConditionalCellStyle = {};
  if (style.textColor) patch.textColor = style.textColor;
  if (style.cellColor) patch.cellColor = style.cellColor;
  applyConditionalStyle(styles, blocked, matched, row, column, patch);
}

function applyConditionalStyle(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  blocked: Set<string>,
  matched: Set<string>,
  row: number,
  column: number,
  patch: SpreadsheetConditionalCellStyle
) {
  const key = cellKey(row, column);
  if (blocked.has(key)) return;
  matched.add(key);
  if (Object.keys(patch).length) patchCellStyle(styles, row, column, patch);
}

function patchCellStyle(
  styles: Map<string, SpreadsheetConditionalCellStyle>,
  row: number,
  column: number,
  patch: SpreadsheetConditionalCellStyle
) {
  const key = cellKey(row, column);
  styles.set(key, { ...patch, ...styles.get(key) });
}

function conditionalRules(sheet: Sheet): FortuneConditionalFormatRule[] {
  if (!Array.isArray(sheet.luckysheet_conditionformat_save)) return [];
  return sheet.luckysheet_conditionformat_save.filter(isConditionalRule);
}

function isConditionalRule(value: unknown): value is FortuneConditionalFormatRule {
  if (!value || typeof value !== 'object') return false;
  const rule = value as Partial<FortuneConditionalFormatRule>;
  return ['default', 'colorGradation', 'dataBar', 'icons'].includes(String(rule.type)) && Array.isArray(rule.cellrange);
}

function isConditionalFormatStyle(
  value: FortuneConditionalFormatRule['format']
): value is FortuneConditionalFormatStyle {
  return (
    Boolean(value) &&
    !Array.isArray(value) &&
    typeof value === 'object' &&
    !('iconSet' in value) &&
    'textColor' in value &&
    'cellColor' in value
  );
}

function cellPositions(data: CellMatrix, ranges: FortuneConditionalFormatRange[]): CellPosition[] {
  const positions: CellPosition[] = [];
  const seen = new Set<string>();
  forEachCell(data, ranges, (position) => {
    const key = cellKey(position.row, position.column);
    if (!seen.has(key)) {
      seen.add(key);
      positions.push(position);
    }
  });
  return positions;
}

function cellKey(row: number, column: number): string {
  return `${row}_${column}`;
}

function forEachCell(
  data: CellMatrix,
  ranges: FortuneConditionalFormatRange[],
  visit: (position: CellPosition) => void
) {
  const maximumRow = data.length - 1;
  const maximumColumn = Math.max(0, ...data.map((cells) => cells.length - 1));
  for (const range of ranges) {
    const startRow = Math.max(0, Math.min(maximumRow, range.row[0]));
    const endRow = Math.max(0, Math.min(maximumRow, range.row[1]));
    const startColumn = Math.max(0, Math.min(maximumColumn, range.column[0]));
    const endColumn = Math.max(0, Math.min(maximumColumn, range.column[1]));
    for (let row = startRow; row <= endRow; row += 1) {
      for (let column = startColumn; column <= endColumn; column += 1) visit({ row, column });
    }
  }
}

function cellValue(data: CellMatrix, row: number, column: number): unknown {
  return data[row]?.[column]?.v ?? null;
}

function numericCellValue(data: CellMatrix, row: number, column: number): number | null {
  const value = cellValue(data, row, column);
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function comparableKey(value: unknown): string {
  return `${typeof value}:${String(value)}`;
}

function compareValues(left: unknown, right: unknown): number {
  const leftNumber = Number(left);
  const rightNumber = Number(right);
  if (String(left).trim() && String(right).trim() && Number.isFinite(leftNumber) && Number.isFinite(rightNumber)) {
    return leftNumber === rightNumber ? 0 : leftNumber > rightNumber ? 1 : -1;
  }
  return String(left).localeCompare(String(right));
}

function compareWithOperator(left: unknown, operator: string, right: unknown): boolean {
  const comparison = compareValues(left, right);
  if (operator === '=') return comparison === 0;
  if (operator === '<>') return comparison !== 0;
  if (operator === '>') return comparison > 0;
  if (operator === '<') return comparison < 0;
  if (operator === '>=') return comparison >= 0;
  return operator === '<=' && comparison <= 0;
}

function decodeColumn(value: string): number {
  let result = 0;
  for (const character of value.toUpperCase()) result = result * 26 + character.charCodeAt(0) - 64;
  return result - 1;
}
