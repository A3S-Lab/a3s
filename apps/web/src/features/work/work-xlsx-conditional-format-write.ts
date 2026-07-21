import { directChild, directChildren } from './work-ooxml-package';
import {
  isSpreadsheetConditionalComparisonOperator,
  spreadsheetConditionalComparisonNeedsUpperValue,
} from './work-spreadsheet-conditional-comparisons';
import { normalizeSpreadsheetConditionalIconSetFormat } from './work-spreadsheet-conditional-icons';
import {
  DEFAULT_DATA_BAR_MAX_LENGTH,
  DEFAULT_DATA_BAR_MIN_LENGTH,
  defaultSpreadsheetColorScaleThresholds,
  defaultSpreadsheetDataBarOptions,
  normalizeSpreadsheetConditionalVisualOptions,
  type SpreadsheetConditionalThreshold,
} from './work-spreadsheet-conditional-values';
import type {
  FortuneConditionalFormatRange,
  FortuneConditionalFormatRule,
  FortuneConditionalFormatStyle,
} from './work-xlsx-conditional-format';

export class XlsxDifferentialFormatWriter {
  private readonly container: Element;
  changed = false;

  constructor(private readonly styles: Document) {
    const root = styles.documentElement;
    const existing = directChild(root, 'dxfs');
    if (existing) {
      this.container = existing;
    } else {
      this.container = styles.createElementNS(root.namespaceURI, 'dxfs');
      const anchor = directChildren(root).find((child) =>
        ['tableStyles', 'colors', 'extLst'].includes(child.localName)
      );
      root.insertBefore(this.container, anchor ?? null);
    }
    this.updateCount();
  }

  append(style: FortuneConditionalFormatStyle): number | null {
    const textColor = xlsxRgb(style.textColor);
    const cellColor = xlsxRgb(style.cellColor);
    if (!textColor && !cellColor) return null;
    const document = this.styles;
    const namespace = document.documentElement.namespaceURI;
    const dxf = document.createElementNS(namespace, 'dxf');
    if (textColor) {
      const font = document.createElementNS(namespace, 'font');
      const color = document.createElementNS(namespace, 'color');
      color.setAttribute('rgb', textColor);
      font.append(color);
      dxf.append(font);
    }
    if (cellColor) {
      const fill = document.createElementNS(namespace, 'fill');
      const pattern = document.createElementNS(namespace, 'patternFill');
      pattern.setAttribute('patternType', 'solid');
      const foreground = document.createElementNS(namespace, 'fgColor');
      foreground.setAttribute('rgb', cellColor);
      const background = document.createElementNS(namespace, 'bgColor');
      background.setAttribute('indexed', '64');
      pattern.append(foreground, background);
      fill.append(pattern);
      dxf.append(fill);
    }
    const index = directChildren(this.container, 'dxf').length;
    this.container.append(dxf);
    this.changed = true;
    this.updateCount();
    return index;
  }

  serialize(): string {
    return new XMLSerializer().serializeToString(this.styles);
  }

  private updateCount() {
    this.container.setAttribute('count', String(directChildren(this.container, 'dxf').length));
  }
}

export function writeXlsxConditionalFormats(
  worksheet: Document,
  source: unknown,
  differentialFormats?: XlsxDifferentialFormatWriter
): void {
  const root = worksheet.documentElement;
  for (const existing of directChildren(root, 'conditionalFormatting')) existing.remove();
  if (!Array.isArray(source)) return;
  const namespace = root.namespaceURI;
  let priority = 1;
  for (const value of source) {
    const rule = conditionalFormatRule(value);
    if (!rule || !rule.cellrange.length) continue;
    const ruleElement = writeRule(worksheet, rule, priority, differentialFormats);
    if (!ruleElement) continue;
    const container = worksheet.createElementNS(namespace, 'conditionalFormatting');
    container.setAttribute('sqref', rule.cellrange.map(encodeRange).join(' '));
    container.append(ruleElement);
    const anchor = directChildren(root).find((child) =>
      [
        'dataValidations',
        'hyperlinks',
        'printOptions',
        'pageMargins',
        'pageSetup',
        'headerFooter',
        'drawing',
        'legacyDrawing',
        'ignoredErrors',
        'extLst',
      ].includes(child.localName)
    );
    root.insertBefore(container, anchor ?? null);
    priority += 1;
  }
}

function writeRule(
  document: Document,
  rule: FortuneConditionalFormatRule,
  priority: number,
  differentialFormats?: XlsxDifferentialFormatWriter
): Element | null {
  const namespace = document.documentElement.namespaceURI;
  const element = document.createElementNS(namespace, 'cfRule');
  element.setAttribute('priority', String(priority));
  if (rule.stopIfTrue) element.setAttribute('stopIfTrue', '1');
  if (rule.type === 'colorGradation') {
    const colors = Array.isArray(rule.format) ? rule.format.slice(0, 3).reverse() : [];
    if (colors.length < 2) return null;
    const options = rule.visualOptions
      ? normalizeSpreadsheetConditionalVisualOptions(rule.visualOptions, colors.length, 'colorScale')
      : { thresholds: defaultSpreadsheetColorScaleThresholds(colors.length) };
    if (!options) return null;
    element.setAttribute('type', 'colorScale');
    const scale = document.createElementNS(namespace, 'colorScale');
    for (const threshold of options.thresholds) scale.append(writeThreshold(document, threshold));
    for (const sourceColor of colors) {
      const rgb = xlsxRgb(sourceColor);
      if (!rgb) return null;
      const color = document.createElementNS(namespace, 'color');
      color.setAttribute('rgb', rgb);
      scale.append(color);
    }
    element.append(scale);
    return element;
  }
  if (rule.type === 'dataBar') {
    const style = conditionalFormatStyle(rule.format);
    const rgb = xlsxRgb(style?.cellColor ?? null);
    if (!rgb) return null;
    const options = rule.visualOptions
      ? normalizeSpreadsheetConditionalVisualOptions(rule.visualOptions, 2, 'dataBar')
      : defaultSpreadsheetDataBarOptions();
    if (!options) return null;
    element.setAttribute('type', 'dataBar');
    const dataBar = document.createElementNS(namespace, 'dataBar');
    if (options.showValue === false) dataBar.setAttribute('showValue', '0');
    if (options.minLength !== undefined && options.minLength !== DEFAULT_DATA_BAR_MIN_LENGTH)
      dataBar.setAttribute('minLength', String(options.minLength));
    if (options.maxLength !== undefined && options.maxLength !== DEFAULT_DATA_BAR_MAX_LENGTH)
      dataBar.setAttribute('maxLength', String(options.maxLength));
    for (const threshold of options.thresholds) dataBar.append(writeThreshold(document, threshold));
    const color = document.createElementNS(namespace, 'color');
    color.setAttribute('rgb', rgb);
    dataBar.append(color);
    element.append(dataBar);
    return element;
  }
  if (rule.type === 'icons') {
    const format = Array.isArray(rule.format) ? null : normalizeSpreadsheetConditionalIconSetFormat(rule.format);
    if (!format) return null;
    element.setAttribute('type', 'iconSet');
    const iconSet = document.createElementNS(namespace, 'iconSet');
    iconSet.setAttribute('iconSet', format.iconSet);
    if (!format.showValue) iconSet.setAttribute('showValue', '0');
    if (format.reverse) iconSet.setAttribute('reverse', '1');
    if (!format.percent) iconSet.setAttribute('percent', '0');
    for (const threshold of format.thresholds) {
      const value = document.createElementNS(namespace, 'cfvo');
      value.setAttribute('type', threshold.type);
      if (threshold.value !== undefined) value.setAttribute('val', String(threshold.value));
      if (!threshold.gte) value.setAttribute('gte', '0');
      iconSet.append(value);
    }
    element.append(iconSet);
    return element;
  }
  if (rule.type !== 'default' || Array.isArray(rule.format) || !rule.conditionName) return null;

  const finish = () => {
    const dxfId = differentialFormats?.append(rule.format as FortuneConditionalFormatStyle);
    if (dxfId !== null && dxfId !== undefined) element.setAttribute('dxfId', String(dxfId));
    return element;
  };
  const values = (rule.conditionValue ?? []).map(String);
  if (isSpreadsheetConditionalComparisonOperator(rule.conditionName)) {
    const needsUpperValue = spreadsheetConditionalComparisonNeedsUpperValue(rule.conditionName);
    if (!values.length || (needsUpperValue && values[1] === undefined)) return null;
    element.setAttribute('type', 'cellIs');
    element.setAttribute('operator', rule.conditionName);
    appendFormula(document, element, formulaForCellRule(values[0]));
    if (needsUpperValue) appendFormula(document, element, formulaForCellRule(values[1]));
    return finish();
  }
  if (rule.conditionName === 'textContains' && values[0]) {
    element.setAttribute('type', 'containsText');
    element.setAttribute('text', values[0]);
    const firstCell = encodeCell(rule.cellrange[0].row[0], rule.cellrange[0].column[0]);
    appendFormula(document, element, `NOT(ISERROR(SEARCH("${values[0].replaceAll('"', '""')}",${firstCell})))`);
    return finish();
  }
  if (rule.conditionName === 'duplicateValue') {
    element.setAttribute('type', values[0] === '1' ? 'uniqueValues' : 'duplicateValues');
    return finish();
  }
  if (['top10', 'top10_percent', 'last10', 'last10_percent'].includes(rule.conditionName)) {
    element.setAttribute('type', 'top10');
    element.setAttribute('rank', values[0] || '10');
    if (rule.conditionName.startsWith('last')) element.setAttribute('bottom', '1');
    if (rule.conditionName.endsWith('_percent')) element.setAttribute('percent', '1');
    return finish();
  }
  if (rule.conditionName === 'aboveAverage' || rule.conditionName === 'belowAverage') {
    element.setAttribute('type', 'aboveAverage');
    if (rule.conditionName === 'belowAverage') element.setAttribute('aboveAverage', '0');
    return finish();
  }
  if (rule.conditionName === 'formula' && values[0]) {
    element.setAttribute('type', 'expression');
    appendFormula(document, element, values[0].replace(/^=/, ''));
    return finish();
  }
  return null;
}

function conditionalFormatRule(value: unknown): FortuneConditionalFormatRule | null {
  if (!value || typeof value !== 'object') return null;
  const rule = value as Partial<FortuneConditionalFormatRule>;
  if (
    !['default', 'colorGradation', 'dataBar', 'icons'].includes(String(rule.type)) ||
    !Array.isArray(rule.cellrange)
  ) {
    return null;
  }
  const cellrange = rule.cellrange.filter(validRange).map((range) => ({
    row: [Math.min(...range.row), Math.max(...range.row)] as [number, number],
    column: [Math.min(...range.column), Math.max(...range.column)] as [number, number],
  }));
  if (!cellrange.length) return null;
  const metadata = rule.stopIfTrue === true ? { stopIfTrue: true } : {};
  if (rule.type === 'colorGradation') {
    if (!Array.isArray(rule.format)) return null;
    const format = rule.format.map(String);
    if (format.length < 2 || format.length > 3) return null;
    const visualOptions = optionalVisualOptions(rule.visualOptions, format.length, 'colorScale');
    if (rule.visualOptions !== undefined && !visualOptions) return null;
    return {
      type: 'colorGradation',
      cellrange,
      format,
      ...metadata,
      ...(visualOptions ? { visualOptions } : {}),
    };
  }
  if (rule.type === 'icons') {
    const format = normalizeSpreadsheetConditionalIconSetFormat(rule.format);
    return format ? { type: 'icons', cellrange, format, ...metadata } : null;
  }
  const format = conditionalFormatStyle(rule.format);
  if (!format) return null;
  const visualOptions = rule.type === 'dataBar' ? optionalVisualOptions(rule.visualOptions, 2, 'dataBar') : undefined;
  if (rule.type === 'dataBar' && rule.visualOptions !== undefined && !visualOptions) return null;
  return {
    type: rule.type as 'default' | 'dataBar',
    cellrange,
    format,
    ...metadata,
    conditionName: typeof rule.conditionName === 'string' ? rule.conditionName : undefined,
    conditionRange: Array.isArray(rule.conditionRange) ? rule.conditionRange.filter(validRange) : [],
    conditionValue: Array.isArray(rule.conditionValue)
      ? rule.conditionValue.filter((item): item is string | number => ['string', 'number'].includes(typeof item))
      : [],
    ...(visualOptions ? { visualOptions } : {}),
  };
}

function conditionalFormatStyle(value: unknown): FortuneConditionalFormatStyle | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const style = value as Partial<FortuneConditionalFormatStyle>;
  return {
    textColor: typeof style.textColor === 'string' ? style.textColor : null,
    cellColor: typeof style.cellColor === 'string' ? style.cellColor : null,
  };
}

function validRange(value: unknown): value is FortuneConditionalFormatRange {
  if (!value || typeof value !== 'object') return false;
  const range = value as Partial<FortuneConditionalFormatRange>;
  return (
    Array.isArray(range.row) &&
    range.row.length === 2 &&
    range.row.every(validIndex) &&
    Array.isArray(range.column) &&
    range.column.length === 2 &&
    range.column.every(validIndex)
  );
}

function validIndex(value: unknown): value is number {
  return typeof value === 'number' && Number.isInteger(value) && value >= 0;
}

function encodeRange(range: FortuneConditionalFormatRange): string {
  const start = encodeCell(range.row[0], range.column[0]);
  const end = encodeCell(range.row[1], range.column[1]);
  return start === end ? start : `${start}:${end}`;
}

function encodeCell(row: number, column: number): string {
  let value = column + 1;
  let label = '';
  while (value > 0) {
    value -= 1;
    label = String.fromCharCode(65 + (value % 26)) + label;
    value = Math.floor(value / 26);
  }
  return `${label}${row + 1}`;
}

function xlsxRgb(value: string | null): string | null {
  if (!value) return null;
  const hexadecimal = /^#?([0-9a-f]{6})$/i.exec(value.trim());
  if (hexadecimal) return `FF${hexadecimal[1].toUpperCase()}`;
  const short = /^#?([0-9a-f])([0-9a-f])([0-9a-f])$/i.exec(value.trim());
  if (short)
    return `FF${short
      .slice(1)
      .map((part) => part.repeat(2).toUpperCase())
      .join('')}`;
  const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(value);
  if (!rgb) return null;
  return `FF${rgb
    .slice(1, 4)
    .map((part) =>
      Math.max(0, Math.min(255, Number(part)))
        .toString(16)
        .padStart(2, '0')
        .toUpperCase()
    )
    .join('')}`;
}

function formulaForCellRule(value: string): string {
  const normalized = value.trim().replace(/^=/, '');
  if (/^-?(?:\d+\.?\d*|\.\d+)$/.test(normalized) || /^[A-Z]+\d+(?::[A-Z]+\d+)?$/i.test(normalized)) {
    return normalized;
  }
  if (normalized.startsWith('"') && normalized.endsWith('"')) return normalized;
  return `"${normalized.replaceAll('"', '""')}"`;
}

function appendFormula(document: Document, parent: Element, value: string): void {
  const formula = document.createElementNS(document.documentElement.namespaceURI, 'formula');
  formula.textContent = value;
  parent.append(formula);
}

function writeThreshold(document: Document, threshold: SpreadsheetConditionalThreshold): Element {
  const value = document.createElementNS(document.documentElement.namespaceURI, 'cfvo');
  value.setAttribute('type', threshold.type);
  if (threshold.value !== undefined) value.setAttribute('val', String(threshold.value));
  return value;
}

function optionalVisualOptions(value: unknown, thresholdCount: number, kind: 'colorScale' | 'dataBar') {
  return value === undefined ? undefined : normalizeSpreadsheetConditionalVisualOptions(value, thresholdCount, kind);
}
