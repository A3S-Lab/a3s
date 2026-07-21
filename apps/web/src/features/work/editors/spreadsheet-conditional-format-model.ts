import type { Sheet } from '@fortune-sheet/core';
import {
  isSpreadsheetConditionalComparisonOperator,
  SPREADSHEET_CONDITIONAL_COMPARISON_OPERATORS,
  spreadsheetConditionalComparisonNeedsUpperValue,
  type SpreadsheetConditionalComparisonOperator,
} from '../work-spreadsheet-conditional-comparisons';
import {
  defaultSpreadsheetConditionalIconThresholds,
  normalizeSpreadsheetConditionalIconSetFormat,
  SPREADSHEET_CONDITIONAL_ICON_SETS,
  type SpreadsheetConditionalIconSetName,
  type SpreadsheetConditionalIconThreshold,
} from '../work-spreadsheet-conditional-icons';
import {
  DEFAULT_DATA_BAR_MAX_LENGTH,
  DEFAULT_DATA_BAR_MIN_LENGTH,
  defaultSpreadsheetColorScaleThresholds,
  defaultSpreadsheetDataBarOptions,
  normalizeSpreadsheetConditionalVisualOptions,
  spreadsheetConditionalThresholdsEqual,
  type SpreadsheetConditionalThreshold,
} from '../work-spreadsheet-conditional-values';
import { formatSpreadsheetCellRanges, parseSpreadsheetCellRanges } from '../work-spreadsheet-ranges';
import type { FortuneConditionalFormatRule, FortuneConditionalFormatStyle } from '../work-xlsx-conditional-format';
import type { WorkSpreadsheetContent } from '../work-types';

export interface SpreadsheetConditionalThresholdDraft {
  type: SpreadsheetConditionalThreshold['type'];
  value: string;
  gte: boolean;
}

const COMPARISON_LABELS: Record<SpreadsheetConditionalComparisonOperator, string> = {
  greaterThan: '大于',
  greaterThanOrEqual: '大于或等于',
  lessThan: '小于',
  lessThanOrEqual: '小于或等于',
  equal: '等于',
  notEqual: '不等于',
  between: '介于',
  notBetween: '不介于',
};

export const SPREADSHEET_CONDITIONAL_COMPARISONS = SPREADSHEET_CONDITIONAL_COMPARISON_OPERATORS.map((name) => ({
  name,
  label: COMPARISON_LABELS[name],
}));

export interface ConditionalRuleDraft {
  sheetId: string;
  type: 'cellComparison' | 'colorGradation' | 'dataBar' | 'icons' | 'toolbarRule';
  reference: string;
  stopIfTrue: boolean;
  preservedRule: FortuneConditionalFormatRule | null;
  comparisonOperator: SpreadsheetConditionalComparisonOperator;
  comparisonValue: string;
  comparisonUpperValue: string;
  comparisonTextColor: string;
  comparisonCellColor: string;
  comparisonUseTextColor: boolean;
  comparisonUseCellColor: boolean;
  scaleSize: '2' | '3';
  minimumColor: string;
  midpointColor: string;
  maximumColor: string;
  scaleThresholds: SpreadsheetConditionalThresholdDraft[];
  barColor: string;
  barThresholds: SpreadsheetConditionalThresholdDraft[];
  barShowValue: boolean;
  barMinLength: string;
  barMaxLength: string;
  iconSet: SpreadsheetConditionalIconSetName;
  iconShowValue: boolean;
  iconReverse: boolean;
  iconThresholds: SpreadsheetConditionalThresholdDraft[];
}

export interface ConditionalRuleEntry {
  sheet: Sheet & { id: string };
  rule: FortuneConditionalFormatRule;
  index: number;
}

const DEFAULT_COLORS = {
  minimum: '#f8696b',
  midpoint: '#ffeb84',
  maximum: '#63be7b',
  bar: '#5b9bd5',
  comparisonText: '#9c0006',
  comparisonCell: '#ffc7ce',
};

const TOOLBAR_CONDITIONAL_RULE_NAMES = [
  'textContains',
  'duplicateValue',
  'top10',
  'top10_percent',
  'last10',
  'last10_percent',
  'aboveAverage',
  'belowAverage',
  'formula',
] as const;

type ToolbarConditionalRuleName = (typeof TOOLBAR_CONDITIONAL_RULE_NAMES)[number];

const TOOLBAR_CONDITIONAL_RULE_LABELS: Record<ToolbarConditionalRuleName, string> = {
  textContains: '包含文本',
  duplicateValue: '重复值',
  top10: '前几项',
  top10_percent: '前百分比',
  last10: '后几项',
  last10_percent: '后百分比',
  aboveAverage: '高于平均值',
  belowAverage: '低于平均值',
  formula: '公式',
};

export function managedConditionalFormatCount(content: WorkSpreadsheetContent): number {
  return content.sheets.reduce(
    (total, sheet) => total + sheetConditionalRules(sheet).filter(isManagedConditionalRule).length,
    0
  );
}

export function managedConditionalRuleEntries(sheets: Array<Sheet & { id: string }>): ConditionalRuleEntry[] {
  return sheets.flatMap((sheet) =>
    sheetConditionalRules(sheet).flatMap((rule, index) =>
      isManagedConditionalRule(rule) ? [{ sheet, rule, index }] : []
    )
  );
}

export function sheetConditionalRules(sheet: Sheet): unknown[] {
  return Array.isArray(sheet.luckysheet_conditionformat_save) ? sheet.luckysheet_conditionformat_save : [];
}

export function withConditionalRules(sheet: Sheet, rules: unknown[]): Sheet {
  return {
    ...sheet,
    luckysheet_conditionformat_save: rules.length ? rules : undefined,
  };
}

export function isManagedConditionalRule(value: unknown): value is FortuneConditionalFormatRule {
  if (!value || typeof value !== 'object') return false;
  const rule = value as Partial<FortuneConditionalFormatRule>;
  if (!Array.isArray(rule.cellrange)) return false;
  if (['colorGradation', 'dataBar', 'icons'].includes(String(rule.type))) return true;
  return (
    rule.type === 'default' &&
    (isSpreadsheetConditionalComparisonOperator(rule.conditionName) ||
      isToolbarConditionalRuleName(rule.conditionName)) &&
    Boolean(rule.format) &&
    !Array.isArray(rule.format)
  );
}

export function newConditionalRuleDraft(sheetId: string): ConditionalRuleDraft {
  const dataBar = defaultSpreadsheetDataBarOptions();
  return {
    sheetId,
    type: 'colorGradation',
    reference: 'A1:A10',
    stopIfTrue: false,
    preservedRule: null,
    comparisonOperator: 'greaterThan',
    comparisonValue: '0',
    comparisonUpperValue: '100',
    comparisonTextColor: DEFAULT_COLORS.comparisonText,
    comparisonCellColor: DEFAULT_COLORS.comparisonCell,
    comparisonUseTextColor: true,
    comparisonUseCellColor: true,
    scaleSize: '3',
    minimumColor: DEFAULT_COLORS.minimum,
    midpointColor: DEFAULT_COLORS.midpoint,
    maximumColor: DEFAULT_COLORS.maximum,
    scaleThresholds: conditionalThresholdDrafts(defaultSpreadsheetColorScaleThresholds(3)),
    barColor: DEFAULT_COLORS.bar,
    barThresholds: conditionalThresholdDrafts(dataBar.thresholds),
    barShowValue: dataBar.showValue,
    barMinLength: String(dataBar.minLength),
    barMaxLength: String(dataBar.maxLength),
    iconSet: '3TrafficLights1',
    iconShowValue: true,
    iconReverse: false,
    iconThresholds: conditionalThresholdDrafts(defaultSpreadsheetConditionalIconThresholds('3TrafficLights1')),
  };
}

export function conditionalRuleDraftForRule(sheetId: string, rule: FortuneConditionalFormatRule): ConditionalRuleDraft {
  const draft = newConditionalRuleDraft(sheetId);
  draft.reference = formatSpreadsheetCellRanges(rule.cellrange);
  draft.stopIfTrue = rule.stopIfTrue === true;
  if (
    rule.type === 'default' &&
    isSpreadsheetConditionalComparisonOperator(rule.conditionName) &&
    !Array.isArray(rule.format)
  ) {
    const style = rule.format as FortuneConditionalFormatStyle;
    const values = (rule.conditionValue ?? []).map(String);
    draft.type = 'cellComparison';
    draft.comparisonOperator = rule.conditionName;
    draft.comparisonValue = values[0] ?? '';
    draft.comparisonUpperValue = values[1] ?? '';
    draft.comparisonUseTextColor = Boolean(style.textColor);
    draft.comparisonUseCellColor = Boolean(style.cellColor);
    draft.comparisonTextColor = colorInputValue(style.textColor, DEFAULT_COLORS.comparisonText);
    draft.comparisonCellColor = colorInputValue(style.cellColor, DEFAULT_COLORS.comparisonCell);
  } else if (isToolbarConditionalRule(rule)) {
    draft.type = 'toolbarRule';
    draft.preservedRule = rule;
  } else if (rule.type === 'colorGradation' && Array.isArray(rule.format)) {
    draft.type = 'colorGradation';
    draft.scaleSize = rule.format.length >= 3 ? '3' : '2';
    draft.maximumColor = colorInputValue(rule.format[0], DEFAULT_COLORS.maximum);
    draft.midpointColor = colorInputValue(rule.format.length >= 3 ? rule.format[1] : null, DEFAULT_COLORS.midpoint);
    draft.minimumColor = colorInputValue(rule.format.at(-1), DEFAULT_COLORS.minimum);
    const options = rule.visualOptions
      ? normalizeSpreadsheetConditionalVisualOptions(rule.visualOptions, Number(draft.scaleSize), 'colorScale')
      : { thresholds: defaultSpreadsheetColorScaleThresholds(Number(draft.scaleSize)) };
    if (options) draft.scaleThresholds = conditionalThresholdDrafts(options.thresholds);
  } else if (rule.type === 'dataBar' && !Array.isArray(rule.format)) {
    draft.type = 'dataBar';
    draft.barColor = colorInputValue((rule.format as FortuneConditionalFormatStyle).cellColor, DEFAULT_COLORS.bar);
    const options = rule.visualOptions
      ? normalizeSpreadsheetConditionalVisualOptions(rule.visualOptions, 2, 'dataBar')
      : defaultSpreadsheetDataBarOptions();
    if (options) {
      draft.barThresholds = conditionalThresholdDrafts(options.thresholds);
      draft.barShowValue = options.showValue !== false;
      draft.barMinLength = String(options.minLength ?? DEFAULT_DATA_BAR_MIN_LENGTH);
      draft.barMaxLength = String(options.maxLength ?? DEFAULT_DATA_BAR_MAX_LENGTH);
    }
  } else if (rule.type === 'icons') {
    draft.type = 'icons';
    const format = normalizeSpreadsheetConditionalIconSetFormat(rule.format);
    if (format) {
      draft.iconSet = format.iconSet;
      draft.iconShowValue = format.showValue;
      draft.iconReverse = format.reverse;
      draft.iconThresholds = conditionalThresholdDrafts(format.thresholds);
    }
  }
  return draft;
}

export type ConditionalRuleBuildResult = { rule: FortuneConditionalFormatRule } | { error: string };

export function buildConditionalRule(draft: ConditionalRuleDraft): ConditionalRuleBuildResult {
  const cellrange = parseSpreadsheetCellRanges(draft.reference);
  if (!cellrange) {
    return { error: '请输入有效的单元格范围，例如 A2:A20 或 A2:A20,C2:C20。' };
  }
  let rule: FortuneConditionalFormatRule;
  if (draft.type === 'toolbarRule') {
    if (!draft.preservedRule || !isToolbarConditionalRule(draft.preservedRule)) {
      return { error: '当前工具栏条件格式规则已失效，请重新选择规则。' };
    }
    rule = { ...draft.preservedRule, cellrange };
    delete rule.stopIfTrue;
  } else if (draft.type === 'cellComparison') {
    const value = draft.comparisonValue.trim();
    const upperValue = draft.comparisonUpperValue.trim();
    const needsUpperValue = spreadsheetConditionalComparisonNeedsUpperValue(draft.comparisonOperator);
    if (
      !value ||
      (needsUpperValue && !upperValue) ||
      (!draft.comparisonUseTextColor && !draft.comparisonUseCellColor)
    ) {
      return { error: '请输入完整的比较值，并至少启用一种文字或填充颜色。' };
    }
    rule = {
      type: 'default',
      cellrange,
      format: {
        textColor: draft.comparisonUseTextColor ? draft.comparisonTextColor : null,
        cellColor: draft.comparisonUseCellColor ? draft.comparisonCellColor : null,
      },
      conditionName: draft.comparisonOperator,
      conditionRange: [],
      conditionValue: needsUpperValue ? [value, upperValue] : [value],
    };
  } else if (draft.type === 'dataBar') {
    const thresholds = savedVisualThresholds(draft.barThresholds);
    const minLength = Number(draft.barMinLength);
    const maxLength = Number(draft.barMaxLength);
    if (
      !thresholds ||
      !Number.isFinite(minLength) ||
      !Number.isFinite(maxLength) ||
      minLength < 0 ||
      minLength > 100 ||
      maxLength < 0 ||
      maxLength > 100 ||
      minLength > maxLength
    ) {
      return {
        error: '数据条阈值和长度必须有效；长度必须介于 0 与 100 之间，且最短长度不能超过最长长度。',
      };
    }
    rule = {
      type: 'dataBar',
      cellrange,
      format: { textColor: null, cellColor: draft.barColor },
      visualOptions: {
        thresholds,
        showValue: draft.barShowValue,
        minLength,
        maxLength,
      },
    };
  } else if (draft.type === 'icons') {
    const thresholds = savedIconThresholds(draft.iconThresholds);
    if (!thresholds) {
      return { error: '图标阈值必须是有效数字；百分比和百分位阈值必须介于 0 与 100 之间。' };
    }
    rule = {
      type: 'icons',
      cellrange,
      format: {
        iconSet: draft.iconSet,
        showValue: draft.iconShowValue,
        reverse: draft.iconReverse,
        percent: !thresholds.some((threshold) => threshold.type === 'num'),
        thresholds,
      },
    };
  } else {
    const thresholds = savedVisualThresholds(draft.scaleThresholds);
    if (!thresholds) {
      return { error: '色阶阈值必须是有效数字；百分比和百分位阈值必须介于 0 与 100 之间。' };
    }
    const defaultThresholds = defaultSpreadsheetColorScaleThresholds(Number(draft.scaleSize));
    rule = {
      type: 'colorGradation',
      cellrange,
      format:
        draft.scaleSize === '3'
          ? [draft.maximumColor, draft.midpointColor, draft.minimumColor]
          : [draft.maximumColor, draft.minimumColor],
      ...(spreadsheetConditionalThresholdsEqual(thresholds, defaultThresholds)
        ? {}
        : { visualOptions: { thresholds } }),
    };
  }
  if (draft.stopIfTrue) rule.stopIfTrue = true;
  return { rule };
}

export function conditionalRuleLabel(rule: FortuneConditionalFormatRule): string {
  if (rule.type === 'default' && isSpreadsheetConditionalComparisonOperator(rule.conditionName)) {
    return (
      SPREADSHEET_CONDITIONAL_COMPARISONS.find((comparison) => comparison.name === rule.conditionName)?.label ??
      '单元格比较'
    );
  }
  if (rule.type === 'default' && isToolbarConditionalRuleName(rule.conditionName)) {
    if (rule.conditionName === 'duplicateValue') {
      return String(rule.conditionValue?.[0] ?? '0') === '1' ? '唯一值' : '重复值';
    }
    const rank = String(rule.conditionValue?.[0] ?? '10');
    if (rule.conditionName === 'top10') return `前 ${rank} 项`;
    if (rule.conditionName === 'top10_percent') return `前 ${rank}%`;
    if (rule.conditionName === 'last10') return `后 ${rank} 项`;
    if (rule.conditionName === 'last10_percent') return `后 ${rank}%`;
    return TOOLBAR_CONDITIONAL_RULE_LABELS[rule.conditionName];
  }
  if (rule.type === 'dataBar') return '数据条';
  if (rule.type === 'icons') {
    const format = normalizeSpreadsheetConditionalIconSetFormat(rule.format);
    return SPREADSHEET_CONDITIONAL_ICON_SETS.find((iconSet) => iconSet.name === format?.iconSet)?.label ?? '图标集';
  }
  return Array.isArray(rule.format) && rule.format.length >= 3 ? '三色阶' : '双色阶';
}

export function conditionalToolbarRuleSummary(rule: FortuneConditionalFormatRule | null): string {
  if (!rule || !isToolbarConditionalRule(rule)) return '工具栏条件格式';
  const label = conditionalRuleLabel(rule);
  const value = String(rule.conditionValue?.[0] ?? '');
  if (rule.conditionName === 'textContains') return value ? `${label}：“${value}”` : label;
  if (rule.conditionName === 'formula') return value ? `${label}：${value}` : label;
  return label;
}

export function conditionalThresholdDrafts(
  thresholds: Array<SpreadsheetConditionalThreshold & { gte?: boolean }>
): SpreadsheetConditionalThresholdDraft[] {
  return thresholds.map((threshold) => ({
    type: threshold.type,
    value: threshold.value === undefined ? '' : String(threshold.value),
    gte: threshold.gte !== false,
  }));
}

export function savedIconThresholds(
  thresholds: SpreadsheetConditionalThresholdDraft[]
): SpreadsheetConditionalIconThreshold[] | null {
  const saved: SpreadsheetConditionalIconThreshold[] = [];
  for (const threshold of thresholds) {
    if (threshold.type === 'min' || threshold.type === 'max') {
      saved.push({ type: threshold.type, gte: threshold.gte });
      continue;
    }
    const value = Number(threshold.value);
    if (
      !threshold.value.trim() ||
      !Number.isFinite(value) ||
      (threshold.type !== 'num' && (value < 0 || value > 100))
    ) {
      return null;
    }
    saved.push({ type: threshold.type, value, gte: threshold.gte });
  }
  return saved;
}

export function savedVisualThresholds(
  thresholds: SpreadsheetConditionalThresholdDraft[]
): SpreadsheetConditionalThreshold[] | null {
  const saved = savedIconThresholds(thresholds);
  return saved?.map(({ type, value }) => (value === undefined ? { type } : { type, value })) ?? null;
}

function colorInputValue(value: string | null | undefined, fallback: string): string {
  if (!value) return fallback;
  const hexadecimal = /^#?([0-9a-f]{6})$/i.exec(value.trim());
  if (hexadecimal) return `#${hexadecimal[1].toLowerCase()}`;
  const rgb = /^rgba?\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)/i.exec(value);
  if (!rgb) return fallback;
  return `#${rgb
    .slice(1, 4)
    .map((part) =>
      Math.max(0, Math.min(255, Number(part)))
        .toString(16)
        .padStart(2, '0')
    )
    .join('')}`;
}

function isToolbarConditionalRule(value: FortuneConditionalFormatRule): boolean {
  return (
    value.type === 'default' &&
    isToolbarConditionalRuleName(value.conditionName) &&
    Boolean(value.format) &&
    !Array.isArray(value.format)
  );
}

function isToolbarConditionalRuleName(value: unknown): value is ToolbarConditionalRuleName {
  return TOOLBAR_CONDITIONAL_RULE_NAMES.includes(value as ToolbarConditionalRuleName);
}
