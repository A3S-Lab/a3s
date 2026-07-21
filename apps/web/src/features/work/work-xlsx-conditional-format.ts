import { attribute, directChild, directChildren, firstDescendant } from './work-ooxml-package';
import {
  isSpreadsheetConditionalComparisonOperator,
  spreadsheetConditionalComparisonNeedsUpperValue,
} from './work-spreadsheet-conditional-comparisons';
import {
  defaultSpreadsheetConditionalIconThresholds,
  isSpreadsheetConditionalIconSetName,
  spreadsheetConditionalIconSetCount,
  type SpreadsheetConditionalIconSetFormat,
  type SpreadsheetConditionalIconThreshold,
  type SpreadsheetConditionalIconThresholdType,
} from './work-spreadsheet-conditional-icons';
import {
  defaultSpreadsheetColorScaleThresholds,
  defaultSpreadsheetDataBarOptions,
  spreadsheetConditionalThresholdsEqual,
  type SpreadsheetConditionalThreshold,
  type SpreadsheetConditionalThresholdType,
  type SpreadsheetConditionalVisualOptions,
} from './work-spreadsheet-conditional-values';

export { writeXlsxConditionalFormats, XlsxDifferentialFormatWriter } from './work-xlsx-conditional-format-write';
export type {
  SpreadsheetConditionalIconSetFormat,
  SpreadsheetConditionalIconSetName,
  SpreadsheetConditionalIconThreshold,
  SpreadsheetConditionalIconThresholdType,
} from './work-spreadsheet-conditional-icons';
export type {
  SpreadsheetConditionalThreshold,
  SpreadsheetConditionalThresholdType,
  SpreadsheetConditionalVisualOptions,
} from './work-spreadsheet-conditional-values';

export interface FortuneConditionalFormatRange {
  row: [number, number];
  column: [number, number];
}

export interface FortuneConditionalFormatStyle {
  textColor: string | null;
  cellColor: string | null;
}

export interface FortuneConditionalFormatRule {
  type: 'default' | 'colorGradation' | 'dataBar' | 'icons';
  cellrange: FortuneConditionalFormatRange[];
  format: FortuneConditionalFormatStyle | SpreadsheetConditionalIconSetFormat | string[];
  visualOptions?: SpreadsheetConditionalVisualOptions;
  stopIfTrue?: boolean;
  conditionName?: string;
  conditionRange?: FortuneConditionalFormatRange[];
  conditionValue?: Array<string | number>;
}

interface DifferentialFormat {
  textColor: string | null;
  cellColor: string | null;
}

export function readXlsxDifferentialFormats(styles: Document | null): DifferentialFormat[] {
  const container = styles ? firstDescendant(styles, 'dxfs') : null;
  if (!container) return [];
  return directChildren(container, 'dxf').map((dxf) => ({
    textColor: colorFromContainer(directChild(dxf, 'font')),
    cellColor: colorFromContainer(directChild(directChild(dxf, 'fill') ?? dxf, 'patternFill')),
  }));
}

export function readXlsxConditionalFormats(
  worksheet: Document,
  differentialFormats: DifferentialFormat[]
): FortuneConditionalFormatRule[] {
  const rules: Array<{ rule: FortuneConditionalFormatRule; priority: number; sourceOrder: number }> = [];
  let sourceOrder = 0;
  for (const container of directChildren(worksheet.documentElement, 'conditionalFormatting')) {
    const cellrange = parseSqref(attribute(container, 'sqref'));
    if (!cellrange.length) continue;
    for (const element of directChildren(container, 'cfRule')) {
      const parsed = readRule(element, cellrange, differentialFormats);
      if (parsed) {
        if (booleanAttribute(element, 'stopIfTrue')) parsed.stopIfTrue = true;
        rules.push({
          rule: parsed,
          priority: conditionalRulePriority(element),
          sourceOrder,
        });
        sourceOrder += 1;
      }
    }
  }
  return rules
    .sort((left, right) => left.priority - right.priority || left.sourceOrder - right.sourceOrder)
    .map(({ rule }) => rule);
}

function readRule(
  element: Element,
  cellrange: FortuneConditionalFormatRange[],
  differentialFormats: DifferentialFormat[]
): FortuneConditionalFormatRule | null {
  const type = attribute(element, 'type');
  const dxfId = attribute(element, 'dxfId');
  const dxfIndex = dxfId !== null && /^\d+$/.test(dxfId) ? Number(dxfId) : -1;
  const style = differentialFormats[dxfIndex] ?? {
    textColor: null,
    cellColor: null,
  };
  const formulas = directChildren(element, 'formula').map((formula) => parseFormulaValue(formula.textContent ?? ''));
  if (type === 'cellIs') {
    const operator = attribute(element, 'operator');
    if (
      !isSpreadsheetConditionalComparisonOperator(operator) ||
      !formulas.length ||
      (spreadsheetConditionalComparisonNeedsUpperValue(operator) && formulas.length < 2)
    ) {
      return null;
    }
    return defaultRule(cellrange, style, operator, formulas);
  }
  if (type === 'containsText') {
    const text = attribute(element, 'text') ?? formulas[0];
    return text ? defaultRule(cellrange, style, 'textContains', [text]) : null;
  }
  if (type === 'duplicateValues' || type === 'uniqueValues') {
    return defaultRule(cellrange, style, 'duplicateValue', [type === 'duplicateValues' ? '0' : '1']);
  }
  if (type === 'top10') {
    const bottom = booleanAttribute(element, 'bottom');
    const percent = booleanAttribute(element, 'percent');
    const conditionName = `${bottom ? 'last10' : 'top10'}${percent ? '_percent' : ''}`;
    return defaultRule(cellrange, style, conditionName, [attribute(element, 'rank') ?? '10']);
  }
  if (type === 'aboveAverage') {
    return defaultRule(
      cellrange,
      style,
      booleanAttribute(element, 'aboveAverage', true) ? 'aboveAverage' : 'belowAverage',
      ['']
    );
  }
  if (type === 'expression' && formulas[0]) {
    return defaultRule(cellrange, style, 'formula', [formulas[0]]);
  }
  if (type === 'colorScale') {
    const scale = directChild(element, 'colorScale');
    if (!scale) return null;
    const colors = directChildren(scale, 'color')
      .map((color) => colorFromElement(color))
      .filter((color): color is string => Boolean(color))
      .map(cssRgbColor);
    if (colors.length < 2 || colors.length > 3) return null;
    const defaults = defaultSpreadsheetColorScaleThresholds(colors.length);
    const thresholds = readVisualThresholds(scale, defaults);
    if (!thresholds) return null;
    return {
      type: 'colorGradation',
      cellrange,
      format: colors.reverse(),
      ...(spreadsheetConditionalThresholdsEqual(thresholds, defaults) ? {} : { visualOptions: { thresholds } }),
    };
  }
  if (type === 'dataBar') {
    const dataBar = directChild(element, 'dataBar');
    if (!dataBar) return null;
    const color = colorFromElement(directChild(dataBar, 'color'));
    if (!color) return null;
    const defaults = defaultSpreadsheetDataBarOptions();
    const thresholds = readVisualThresholds(dataBar, defaults.thresholds);
    const showValue = booleanAttribute(dataBar, 'showValue', defaults.showValue);
    const minLength = boundedPercentageAttribute(dataBar, 'minLength', defaults.minLength);
    const maxLength = boundedPercentageAttribute(dataBar, 'maxLength', defaults.maxLength);
    if (!thresholds || minLength === null || maxLength === null || minLength > maxLength) return null;
    const hasCustomOptions =
      !spreadsheetConditionalThresholdsEqual(thresholds, defaults.thresholds) ||
      showValue !== defaults.showValue ||
      minLength !== defaults.minLength ||
      maxLength !== defaults.maxLength;
    return {
      type: 'dataBar',
      cellrange,
      format: { textColor: null, cellColor: color },
      ...(hasCustomOptions
        ? {
            visualOptions: {
              thresholds,
              showValue,
              minLength,
              maxLength,
            },
          }
        : {}),
    };
  }
  if (type === 'iconSet') {
    const iconSet = directChild(element, 'iconSet');
    if (!iconSet || booleanAttribute(iconSet, 'custom') || directChildren(iconSet, 'cfIcon').length) return null;
    const name = attribute(iconSet, 'iconSet') ?? '3TrafficLights1';
    if (!isSpreadsheetConditionalIconSetName(name)) return null;
    const expectedCount = spreadsheetConditionalIconSetCount(name);
    const values = directChildren(iconSet, 'cfvo');
    const thresholds = values.length
      ? values.map(readIconThreshold)
      : defaultSpreadsheetConditionalIconThresholds(name);
    if (thresholds.length !== expectedCount || thresholds.some((threshold) => !threshold)) return null;
    return {
      type: 'icons',
      cellrange,
      format: {
        iconSet: name,
        showValue: booleanAttribute(iconSet, 'showValue', true),
        reverse: booleanAttribute(iconSet, 'reverse'),
        percent: booleanAttribute(iconSet, 'percent', true),
        thresholds: thresholds as SpreadsheetConditionalIconThreshold[],
      },
    };
  }
  return null;
}

function defaultRule(
  cellrange: FortuneConditionalFormatRange[],
  format: FortuneConditionalFormatStyle,
  conditionName: string,
  conditionValue: Array<string | number>
): FortuneConditionalFormatRule {
  return {
    type: 'default',
    cellrange,
    format,
    conditionName,
    conditionRange: [],
    conditionValue,
  };
}

function parseSqref(value: string | null): FortuneConditionalFormatRange[] {
  if (!value) return [];
  return value
    .split(/\s+/)
    .map((reference) => parseRange(reference))
    .filter((range): range is FortuneConditionalFormatRange => Boolean(range));
}

function parseRange(value: string): FortuneConditionalFormatRange | null {
  const match = /^\$?([A-Z]{1,3})\$?([1-9]\d*)(?::\$?([A-Z]{1,3})\$?([1-9]\d*))?$/i.exec(value);
  if (!match) return null;
  const start = { row: Number(match[2]) - 1, column: decodeColumn(match[1]) };
  const end = match[3] ? { row: Number(match[4]) - 1, column: decodeColumn(match[3]) } : start;
  return {
    row: [Math.min(start.row, end.row), Math.max(start.row, end.row)],
    column: [Math.min(start.column, end.column), Math.max(start.column, end.column)],
  };
}

function decodeColumn(value: string): number {
  let result = 0;
  for (const character of value.toUpperCase()) result = result * 26 + character.charCodeAt(0) - 64;
  return result - 1;
}

function colorFromContainer(container: Element | undefined): string | null {
  return colorFromElement(firstDescendant(container, 'color') ?? firstDescendant(container, 'fgColor'));
}

function colorFromElement(element: Element | undefined): string | null {
  if (!element) return null;
  const rgb = attribute(element, 'rgb');
  if (!rgb || !/^[0-9a-f]{6,8}$/i.test(rgb)) return null;
  return `#${rgb.slice(-6).toLowerCase()}`;
}

function cssRgbColor(value: string): string {
  const rgb = value
    .slice(-6)
    .match(/.{2}/g)!
    .map((part) => Number.parseInt(part, 16));
  return `rgb(${rgb.join(', ')})`;
}

function parseFormulaValue(value: string): string {
  const normalized = value.trim().replace(/^=/, '');
  if (normalized.startsWith('"') && normalized.endsWith('"')) {
    return normalized.slice(1, -1).replaceAll('""', '"');
  }
  return normalized;
}

function readIconThreshold(element: Element): SpreadsheetConditionalIconThreshold | null {
  const sourceType = attribute(element, 'type');
  if (!sourceType || !['min', 'max', 'num', 'percent', 'percentile'].includes(sourceType)) return null;
  const type = sourceType as SpreadsheetConditionalIconThresholdType;
  const gte = booleanAttribute(element, 'gte', true);
  if (type === 'min' || type === 'max') return { type, gte };
  const value = Number(attribute(element, 'val'));
  return Number.isFinite(value) ? { type, value, gte } : null;
}

function readVisualThresholds(
  container: Element,
  defaults: SpreadsheetConditionalThreshold[]
): SpreadsheetConditionalThreshold[] | null {
  const elements = directChildren(container, 'cfvo');
  if (!elements.length) return defaults;
  if (elements.length !== defaults.length) return null;
  const thresholds = elements.map(readVisualThreshold);
  return thresholds.some((threshold) => !threshold) ? null : (thresholds as SpreadsheetConditionalThreshold[]);
}

function readVisualThreshold(element: Element): SpreadsheetConditionalThreshold | null {
  const sourceType = attribute(element, 'type');
  if (!sourceType || !['min', 'max', 'num', 'percent', 'percentile'].includes(sourceType)) return null;
  const type = sourceType as SpreadsheetConditionalThresholdType;
  if (type === 'min' || type === 'max') return { type };
  const sourceValue = attribute(element, 'val');
  const value = sourceValue === null ? Number.NaN : Number(sourceValue);
  return Number.isFinite(value) ? { type, value } : null;
}

function boundedPercentageAttribute(element: Element, name: string, defaultValue: number): number | null {
  const source = attribute(element, name);
  if (source === null) return defaultValue;
  const value = Number(source);
  return Number.isFinite(value) && value >= 0 && value <= 100 ? value : null;
}

function conditionalRulePriority(element: Element): number {
  const source = attribute(element, 'priority');
  if (!source || !/^[1-9]\d*$/.test(source)) return Number.MAX_SAFE_INTEGER;
  const priority = Number(source);
  return Number.isSafeInteger(priority) ? priority : Number.MAX_SAFE_INTEGER;
}

function booleanAttribute(element: Element, name: string, defaultValue = false): boolean {
  const value = attribute(element, name);
  if (value === null) return defaultValue;
  return value === '1' || value.toLowerCase() === 'true';
}
