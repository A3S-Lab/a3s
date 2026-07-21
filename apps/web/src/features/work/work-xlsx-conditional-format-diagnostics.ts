import { attribute, descendants, directChild, directChildren, firstDescendant } from './work-ooxml-package';
import {
  isSpreadsheetConditionalComparisonOperator,
  spreadsheetConditionalComparisonNeedsUpperValue,
} from './work-spreadsheet-conditional-comparisons';
import {
  isSpreadsheetConditionalIconSetName,
  spreadsheetConditionalIconSetCount,
} from './work-spreadsheet-conditional-icons';
import { DEFAULT_DATA_BAR_MAX_LENGTH, DEFAULT_DATA_BAR_MIN_LENGTH } from './work-spreadsheet-conditional-values';

export interface XlsxConditionalFormatDiagnostic {
  code: string;
  message: string;
  severity: 'info' | 'warning';
}

const SIMPLE_RULE_TYPES = new Set(['containsText', 'duplicateValues', 'uniqueValues', 'top10', 'aboveAverage']);

export function diagnoseXlsxConditionalFormatting(
  worksheet: Document,
  styles: Document | null
): XlsxConditionalFormatDiagnostic[] {
  const allContainers = descendants(worksheet, 'conditionalFormatting');
  if (!allContainers.length) return [];
  const containers = directChildren(worksheet.documentElement, 'conditionalFormatting');
  const dxfs = styles ? directChildren(firstDescendant(styles, 'dxfs') ?? styles, 'dxf') : [];
  const unsupported = new Set<string>();
  let supported = 0;
  let colorNormalization = false;
  let formulaNormalization = false;
  let optionNormalization = allContainers.length > containers.length;
  const priorities = new Set<number>();
  if (allContainers.length > containers.length) unsupported.add('extension-based rules');
  if (descendants(worksheet, 'dataBar').some(hasAdvancedDataBarVisuals)) {
    unsupported.add('x14 data-bar direction, axis, border, gradient, or negative-color options');
  }

  for (const container of containers) {
    for (const rule of directChildren(container, 'cfRule')) {
      const type = attribute(rule, 'type') ?? 'unknown';
      const formulas = directChildren(rule, 'formula').map((formula) => formula.textContent?.trim() ?? '');
      if (formulas.some(isAdvancedFormula)) formulaNormalization = true;
      const priority = positiveIntegerAttribute(rule, 'priority');
      if (priority === null || priorities.has(priority)) optionNormalization = true;
      else priorities.add(priority);

      if (type === 'cellIs') {
        const operator = attribute(rule, 'operator') ?? '';
        const supportedOperator = isSpreadsheetConditionalComparisonOperator(operator);
        const formulaCount = supportedOperator && spreadsheetConditionalComparisonNeedsUpperValue(operator) ? 2 : 1;
        if (supportedOperator && formulas.length >= formulaCount) supported += 1;
        else unsupported.add(operator ? `cellIs/${operator}` : 'cellIs');
      } else if (SIMPLE_RULE_TYPES.has(type)) {
        supported += 1;
        if (
          type === 'aboveAverage' &&
          (attribute(rule, 'equalAverage') !== null || attribute(rule, 'stdDev') !== null)
        ) {
          optionNormalization = true;
        }
      } else if (type === 'expression') {
        if (formulas.length) supported += 1;
        else unsupported.add('empty formula rule');
      } else if (type === 'colorScale') {
        const scale = directChild(rule, 'colorScale');
        const colors = scale ? directChildren(scale, 'color') : [];
        const thresholds = scale ? directChildren(scale, 'cfvo') : [];
        const supportedThresholds =
          colors.length >= 2 &&
          colors.length <= 3 &&
          (thresholds.length === 0 ||
            (thresholds.length === colors.length && thresholds.every(isSupportedVisualThreshold)));
        if (colors.length >= 2 && colors.length <= 3 && colors.every(hasRgbColor) && supportedThresholds) {
          supported += 1;
        } else unsupported.add('colorScale');
        if (colors.some(needsColorNormalization)) colorNormalization = true;
        if (!thresholds.length) optionNormalization = true;
      } else if (type === 'dataBar') {
        const dataBar = directChild(rule, 'dataBar');
        const colors = dataBar ? directChildren(dataBar, 'color') : [];
        const thresholds = dataBar ? directChildren(dataBar, 'cfvo') : [];
        const supportedThresholds =
          thresholds.length === 0 || (thresholds.length === 2 && thresholds.every(isSupportedVisualThreshold));
        const supportedOptions = Boolean(dataBar && hasSupportedDataBarOptions(dataBar));
        if (colors.length === 1 && colors.every(hasRgbColor) && supportedThresholds && supportedOptions) supported += 1;
        else unsupported.add('dataBar');
        if (colors.some(needsColorNormalization)) colorNormalization = true;
        if (!thresholds.length) optionNormalization = true;
        if (
          dataBar &&
          Array.from(dataBar.attributes).some(
            (item) => !['showValue', 'minLength', 'maxLength'].includes(item.localName)
          )
        ) {
          optionNormalization = true;
        }
      } else if (type === 'iconSet') {
        const iconSet = directChild(rule, 'iconSet');
        const name = iconSet ? (attribute(iconSet, 'iconSet') ?? '3TrafficLights1') : '';
        if (iconSet && (booleanAttribute(iconSet, 'custom') || directChildren(iconSet, 'cfIcon').length > 0)) {
          unsupported.add('custom icon sets');
        } else if (!iconSet || !isSpreadsheetConditionalIconSetName(name)) {
          unsupported.add(name ? `iconSet/${name}` : 'iconSet');
        } else {
          const thresholds = directChildren(iconSet, 'cfvo');
          const validThresholds =
            thresholds.length === 0 ||
            (thresholds.length === spreadsheetConditionalIconSetCount(name) &&
              thresholds.every(isSupportedIconThreshold));
          if (validThresholds) supported += 1;
          else unsupported.add('iconSet thresholds');
          if (!thresholds.length) optionNormalization = true;
          if (
            Array.from(iconSet.attributes).some(
              (item) => !['iconSet', 'showValue', 'reverse', 'percent'].includes(item.localName)
            ) ||
            ['showValue', 'reverse', 'percent'].some((name) => !validBooleanAttribute(iconSet, name))
          ) {
            optionNormalization = true;
          }
        }
      } else {
        unsupported.add(type);
      }

      const dxfId = attribute(rule, 'dxfId');
      if (dxfId !== null) {
        const dxfIndex = /^\d+$/.test(dxfId) ? Number(dxfId) : -1;
        const dxf = dxfs[dxfIndex];
        if (!dxf) {
          unsupported.add('missing differential style');
        } else {
          const colors = [...descendants(dxf, 'color'), ...descendants(dxf, 'fgColor')];
          if (colors.some(needsColorNormalization)) colorNormalization = true;
          if (hasAdvancedDifferentialStyle(dxf)) optionNormalization = true;
        }
      }
    }
  }

  const diagnostics: XlsxConditionalFormatDiagnostic[] = [
    {
      code: 'xlsx.conditional-formatting',
      message: supported
        ? `${supported} supported conditional-formatting rule(s) are preserved in editable sheet state; common cell rules can be changed in Work.`
        : 'Conditional formatting was detected, but no rule could be converted to editable sheet state.',
      severity: supported ? 'info' : 'warning',
    },
  ];
  if (unsupported.size) {
    diagnostics.push({
      code: 'xlsx.conditional-formatting.unsupported',
      message: `Unsupported conditional-formatting features remain in the original workbook only: ${Array.from(unsupported).join(', ')}.`,
      severity: 'warning',
    });
  }
  if (colorNormalization) {
    diagnostics.push({
      code: 'xlsx.conditional-formatting.colors',
      message: 'Theme, indexed, automatic, or tinted conditional-format colors are normalized to supported RGB colors.',
      severity: 'warning',
    });
  }
  if (formulaNormalization) {
    diagnostics.push({
      code: 'xlsx.conditional-formatting.formulas',
      message: 'External, structured, cross-sheet, or advanced conditional formulas may be recalculated differently.',
      severity: 'warning',
    });
  }
  if (optionNormalization) {
    diagnostics.push({
      code: 'xlsx.conditional-formatting.options',
      message:
        'Invalid or duplicate priorities, advanced differential styles, omitted thresholds, or x14 visual extensions may be normalized.',
      severity: 'warning',
    });
  }
  return diagnostics;
}

function hasRgbColor(element: Element): boolean {
  return /^[0-9a-f]{6,8}$/i.test(attribute(element, 'rgb') ?? '');
}

function needsColorNormalization(element: Element): boolean {
  return (
    !hasRgbColor(element) && ['theme', 'indexed', 'auto', 'tint'].some((name) => attribute(element, name) !== null)
  );
}

function isAdvancedFormula(value: string): boolean {
  return (
    /(?:^|[^A-Z0-9_.])(?:INDIRECT|OFFSET|LET|LAMBDA|_XLFN\.)\s*\(/i.test(value) || /(?:\[[^\]]+\]|!|[#@])/.test(value)
  );
}

function hasAdvancedDifferentialStyle(dxf: Element): boolean {
  return directChildren(dxf).some((child) => {
    if (child.localName === 'font') return directChildren(child).some((item) => item.localName !== 'color');
    if (child.localName !== 'fill') return true;
    const pattern = directChild(child, 'patternFill');
    return !pattern || attribute(pattern, 'patternType') !== 'solid';
  });
}

function booleanAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name)?.toLowerCase();
  return value === '1' || value === 'true';
}

function validBooleanAttribute(element: Element, name: string): boolean {
  const value = attribute(element, name);
  return value === null || ['0', '1', 'false', 'true'].includes(value.toLowerCase());
}

function isSupportedIconThreshold(element: Element): boolean {
  const type = attribute(element, 'type');
  if (type === 'min' || type === 'max') return true;
  if (!['num', 'percent', 'percentile'].includes(type ?? '')) return false;
  const value = attribute(element, 'val');
  return value !== null && Number.isFinite(Number(value));
}

function isSupportedVisualThreshold(element: Element): boolean {
  const type = attribute(element, 'type');
  if (type === 'min' || type === 'max') return true;
  if (!['num', 'percent', 'percentile'].includes(type ?? '')) return false;
  const value = attribute(element, 'val');
  return value !== null && Number.isFinite(Number(value));
}

function hasSupportedDataBarOptions(element: Element): boolean {
  if (!validBooleanAttribute(element, 'showValue')) return false;
  const minimum = percentageAttribute(element, 'minLength', DEFAULT_DATA_BAR_MIN_LENGTH);
  const maximum = percentageAttribute(element, 'maxLength', DEFAULT_DATA_BAR_MAX_LENGTH);
  return minimum !== null && maximum !== null && minimum <= maximum;
}

function percentageAttribute(element: Element, name: string, defaultValue: number): number | null {
  const source = attribute(element, name);
  if (source === null) return defaultValue;
  const value = Number(source);
  return Number.isFinite(value) && value >= 0 && value <= 100 ? value : null;
}

function positiveIntegerAttribute(element: Element, name: string): number | null {
  const source = attribute(element, name);
  if (!source || !/^[1-9]\d*$/.test(source)) return null;
  const value = Number(source);
  return Number.isSafeInteger(value) ? value : null;
}

function hasAdvancedDataBarVisuals(element: Element): boolean {
  return (
    ['direction', 'axisPosition', 'border', 'gradient', 'negativeBarColorSameAsPositive'].some(
      (name) => attribute(element, name) !== null
    ) ||
    ['borderColor', 'negativeFillColor', 'negativeBorderColor', 'axisColor'].some(
      (name) => descendants(element, name).length > 0
    )
  );
}
