import {
  type WorkSpreadsheetChartAxes,
  type WorkSpreadsheetChartAxis,
  type WorkSpreadsheetChartAxisPosition,
  type WorkSpreadsheetChartType,
  workSpreadsheetChartSupportsAxes,
  workSpreadsheetChartUsesNumericXAxis,
} from './work-types';
import {
  compactChartNumber,
  escapeChartXml,
  finiteChartNumber,
  roundChartNumber,
} from './work-spreadsheet-chart-svg-utils';

const DEFAULT_TICK_COUNT = 5;
const MAX_RENDERED_TICKS = 64;

export function workSpreadsheetChartAxisIsValueAxis(
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): boolean {
  if (!workSpreadsheetChartSupportsAxes(chartType) || position === 'top') return false;
  if (workSpreadsheetChartUsesNumericXAxis(chartType)) return position === 'bottom' || position === 'left';
  if (chartType === 'bar') return position === 'bottom';
  if (chartType === 'combination') return position === 'left' || position === 'right';
  return position === 'left';
}

export function workSpreadsheetChartAxisIsCategoryAxis(
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): boolean {
  if (!workSpreadsheetChartSupportsAxes(chartType) || workSpreadsheetChartUsesNumericXAxis(chartType)) return false;
  if (chartType === 'bar') return position === 'left';
  if (chartType === 'combination') return position === 'bottom' || position === 'top';
  return position === 'bottom';
}

export function workSpreadsheetChartAxisShowsMajorGridlinesByDefault(
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): boolean {
  if (!workSpreadsheetChartAxisIsValueAxis(chartType, position)) return false;
  if (workSpreadsheetChartUsesNumericXAxis(chartType)) return position === 'left';
  if (chartType === 'bar') return position === 'bottom';
  return position === 'left';
}

export function workSpreadsheetChartAxisDefaultLabelPosition(
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): NonNullable<WorkSpreadsheetChartAxis['labelPosition']> {
  return chartType === 'combination' && position === 'top' ? 'none' : 'nextTo';
}

export function workSpreadsheetChartAxisPositionLabel(position: WorkSpreadsheetChartAxisPosition): string {
  if (position === 'left') return '纵轴';
  if (position === 'top') return '次横轴';
  if (position === 'right') return '次纵轴';
  return '横轴';
}

export function workSpreadsheetChartAxisLabelPositionLabel(
  position: NonNullable<WorkSpreadsheetChartAxis['labelPosition']>
): string {
  if (position === 'high') return '高位';
  if (position === 'low') return '低位';
  if (position === 'none') return '不显示';
  return '轴旁';
}

export function workSpreadsheetChartAxisTickMarkLabel(
  tickMark: NonNullable<WorkSpreadsheetChartAxis['majorTickMark']>
): string {
  if (tickMark === 'inside') return '向内';
  if (tickMark === 'outside') return '向外';
  if (tickMark === 'cross') return '交叉';
  return '无';
}

export function normalizeWorkSpreadsheetChartAxes(
  source: WorkSpreadsheetChartAxes | undefined,
  chartType: WorkSpreadsheetChartType,
  hasSecondaryAxes = false
): WorkSpreadsheetChartAxes | undefined {
  if (!source || !workSpreadsheetChartSupportsAxes(chartType)) return undefined;
  const bottom = normalizeWorkSpreadsheetChartAxis(source.bottom, chartType, 'bottom');
  const left = normalizeWorkSpreadsheetChartAxis(source.left, chartType, 'left');
  const top =
    chartType === 'combination' && hasSecondaryAxes
      ? normalizeWorkSpreadsheetChartAxis(source.top, chartType, 'top')
      : undefined;
  const right =
    chartType === 'combination' && hasSecondaryAxes
      ? normalizeWorkSpreadsheetChartAxis(source.right, chartType, 'right')
      : undefined;
  return bottom || left || top || right
    ? {
        ...(bottom ? { bottom } : {}),
        ...(left ? { left } : {}),
        ...(top ? { top } : {}),
        ...(right ? { right } : {}),
      }
    : undefined;
}

function normalizeWorkSpreadsheetChartAxis(
  source: WorkSpreadsheetChartAxis | undefined,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): WorkSpreadsheetChartAxis | undefined {
  if (!source) return undefined;
  const title = source.title?.trim();
  const titleReference = source.titleReference?.trim().replace(/^=/, '');
  const valueAxis = workSpreadsheetChartAxisIsValueAxis(chartType, position);
  const categoryAxis = workSpreadsheetChartAxisIsCategoryAxis(chartType, position);
  const minimum = valueAxis && Number.isFinite(source.minimum) ? Number(source.minimum) : undefined;
  const maximum = valueAxis && Number.isFinite(source.maximum) ? Number(source.maximum) : undefined;
  const validRange = minimum === undefined || maximum === undefined || minimum < maximum;
  const majorUnit =
    valueAxis && Number.isFinite(source.majorUnit) && Number(source.majorUnit) > 0
      ? Number(source.majorUnit)
      : undefined;
  const showMajorGridlines =
    valueAxis && typeof source.showMajorGridlines === 'boolean' ? source.showMajorGridlines : undefined;
  const numberFormat = valueAxis ? source.numberFormat?.trim().slice(0, 255) : undefined;
  const numberFormatSourceLinked =
    valueAxis && typeof source.numberFormatSourceLinked === 'boolean' ? source.numberFormatSourceLinked : undefined;
  const labelPosition = normalizedAxisLabelPosition(source.labelPosition);
  const defaultLabelPosition = workSpreadsheetChartAxisDefaultLabelPosition(chartType, position);
  const majorTickMark = normalizedAxisTickMark(source.majorTickMark);
  const labelInterval = categoryAxis ? normalizedLabelInterval(source.labelInterval) : undefined;
  const normalized: WorkSpreadsheetChartAxis = {
    ...(title ? { title } : {}),
    ...(titleReference ? { titleReference } : {}),
    ...(source.reverseOrder === true ? { reverseOrder: true } : {}),
    ...(labelPosition && labelPosition !== defaultLabelPosition ? { labelPosition } : {}),
    ...(majorTickMark && majorTickMark !== 'none' ? { majorTickMark } : {}),
    ...(labelInterval !== undefined ? { labelInterval } : {}),
    ...(validRange && minimum !== undefined ? { minimum } : {}),
    ...(validRange && maximum !== undefined ? { maximum } : {}),
    ...(majorUnit !== undefined ? { majorUnit } : {}),
    ...(showMajorGridlines !== undefined ? { showMajorGridlines } : {}),
    ...(numberFormat ? { numberFormat } : {}),
    ...(numberFormatSourceLinked !== undefined ? { numberFormatSourceLinked } : {}),
  };
  return Object.keys(normalized).length ? normalized : undefined;
}

function normalizedAxisLabelPosition(value: unknown): WorkSpreadsheetChartAxis['labelPosition'] {
  return value === 'nextTo' || value === 'high' || value === 'low' || value === 'none' ? value : undefined;
}

function normalizedAxisTickMark(value: unknown): WorkSpreadsheetChartAxis['majorTickMark'] {
  return value === 'none' || value === 'inside' || value === 'outside' || value === 'cross' ? value : undefined;
}

function normalizedLabelInterval(value: unknown): number | undefined {
  const interval = Number(value);
  return Number.isInteger(interval) && interval >= 1 && interval <= 31_999 ? interval : undefined;
}

export interface SpreadsheetChartAxisScale {
  minimum: number;
  maximum: number;
  span: number;
  majorUnit?: number;
  ticks: number[];
}

interface SpreadsheetChartAxisScaleOptions {
  includeZero?: boolean;
  paddingRatio?: number;
}

interface SpreadsheetChartPlotBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SpreadsheetChartAxisLabelLayout {
  x: number;
  y: number;
  textAnchor: 'start' | 'middle' | 'end';
}

export function spreadsheetChartAxisDisplayAttributes(
  axis: WorkSpreadsheetChartAxis | undefined,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): string {
  const labelPosition = axis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chartType, position);
  const majorTickMark = axis?.majorTickMark ?? 'none';
  return [
    `data-axis-display="${position}"`,
    `data-axis-reverse-order="${axis?.reverseOrder === true}"`,
    `data-axis-label-position="${labelPosition}"`,
    `data-axis-major-tick-mark="${majorTickMark}"`,
    workSpreadsheetChartAxisIsCategoryAxis(chartType, position)
      ? `data-axis-label-interval="${axis?.labelInterval ?? 'auto'}"`
      : '',
  ]
    .filter(Boolean)
    .join(' ');
}

export function spreadsheetChartAxisValueRatio(
  value: number,
  scale: SpreadsheetChartAxisScale,
  axis: WorkSpreadsheetChartAxis | undefined
): number {
  const ratio = Math.min(1, Math.max(0, (finiteChartNumber(value) - scale.minimum) / scale.span));
  return axis?.reverseOrder === true ? 1 - ratio : ratio;
}

export function spreadsheetChartCategoryVisualIndex(
  index: number,
  count: number,
  axis: WorkSpreadsheetChartAxis | undefined
): number {
  return axis?.reverseOrder === true ? count - 1 - index : index;
}

export function spreadsheetChartCategoryLabelVisible(
  index: number,
  count: number,
  axis: WorkSpreadsheetChartAxis | undefined
): boolean {
  const explicitInterval = normalizedLabelInterval(axis?.labelInterval);
  const interval = explicitInterval ?? Math.max(1, Math.ceil(count / 7));
  return index % interval === 0 || (explicitInterval === undefined && index === count - 1);
}

export function spreadsheetChartAxisLabelLayout(
  axis: WorkSpreadsheetChartAxis | undefined,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition,
  plot: SpreadsheetChartPlotBounds,
  tickX: number,
  tickY: number
): SpreadsheetChartAxisLabelLayout | null {
  const labelPosition = axis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chartType, position);
  if (labelPosition === 'none') return null;
  if (position === 'bottom') {
    return {
      x: tickX,
      y: labelPosition === 'high' ? plot.y - 7 : plot.y + plot.height + 18,
      textAnchor: 'middle',
    };
  }
  if (position === 'top') {
    return {
      x: tickX,
      y: labelPosition === 'low' ? plot.y + plot.height + 18 : plot.y - 7,
      textAnchor: 'middle',
    };
  }
  if (position === 'left') {
    const high = labelPosition === 'high';
    return {
      x: high ? plot.x + plot.width + 8 : plot.x - 8,
      y: tickY + 4,
      textAnchor: high ? 'start' : 'end',
    };
  }
  const low = labelPosition === 'low';
  return {
    x: low ? plot.x - 8 : plot.x + plot.width + 8,
    y: tickY + 4,
    textAnchor: low ? 'end' : 'start',
  };
}

export function spreadsheetChartMajorTickSvg(
  axis: WorkSpreadsheetChartAxis | undefined,
  position: WorkSpreadsheetChartAxisPosition,
  index: number,
  x: number,
  y: number
): string {
  const mark = axis?.majorTickMark ?? 'none';
  if (mark === 'none') return '';
  const inward =
    position === 'bottom'
      ? ([0, -1] as const)
      : position === 'top'
        ? ([0, 1] as const)
        : position === 'left'
          ? ([1, 0] as const)
          : ([-1, 0] as const);
  const insideLength = mark === 'outside' ? 0 : mark === 'cross' ? 3 : 5;
  const outsideLength = mark === 'inside' ? 0 : mark === 'cross' ? 3 : 5;
  const x1 = x - inward[0] * outsideLength;
  const y1 = y - inward[1] * outsideLength;
  const x2 = x + inward[0] * insideLength;
  const y2 = y + inward[1] * insideLength;
  return `<line data-axis-major-tick="${position}:${index}" x1="${roundChartNumber(
    x1
  )}" y1="${roundChartNumber(y1)}" x2="${roundChartNumber(x2)}" y2="${roundChartNumber(
    y2
  )}" stroke="#7d8798" stroke-width="1"/>`;
}

export function spreadsheetChartAxisScale(
  values: readonly number[],
  axis: WorkSpreadsheetChartAxis | undefined,
  options: SpreadsheetChartAxisScaleOptions = {}
): SpreadsheetChartAxisScale {
  const finiteValues = values.filter(Number.isFinite).map(finiteChartNumber);
  const sourceValues = finiteValues.length ? finiteValues : [0, 1];
  let automaticMinimum = Math.min(...sourceValues);
  let automaticMaximum = Math.max(...sourceValues);
  if (options.includeZero) {
    automaticMinimum = Math.min(0, automaticMinimum);
    automaticMaximum = Math.max(0, automaticMaximum);
  }
  const sourceSpan = automaticMaximum - automaticMinimum;
  const paddingRatio = Math.max(0, options.paddingRatio ?? 0);
  const padding =
    sourceSpan > 0 ? sourceSpan * paddingRatio : Math.max(1, Math.abs(automaticMinimum) * Math.max(paddingRatio, 0.05));
  automaticMinimum -= padding;
  automaticMaximum += padding;

  let minimum = Number.isFinite(axis?.minimum) ? Number(axis?.minimum) : automaticMinimum;
  let maximum = Number.isFinite(axis?.maximum) ? Number(axis?.maximum) : automaticMaximum;
  const fallbackSpan = Math.max(
    Number.isFinite(axis?.majorUnit) && Number(axis?.majorUnit) > 0 ? Number(axis?.majorUnit) : 0,
    Math.abs(minimum) * 0.1,
    Math.abs(maximum) * 0.1,
    1
  );
  if (!(minimum < maximum)) {
    if (axis?.minimum !== undefined && axis.maximum === undefined) maximum = minimum + fallbackSpan;
    else if (axis?.maximum !== undefined && axis.minimum === undefined) minimum = maximum - fallbackSpan;
    else {
      minimum = automaticMinimum;
      maximum = automaticMaximum;
    }
  }
  if (!(minimum < maximum)) maximum = minimum + fallbackSpan;
  const span = Math.max(Number.EPSILON, maximum - minimum);
  const majorUnit =
    Number.isFinite(axis?.majorUnit) && Number(axis?.majorUnit) > 0 ? Number(axis?.majorUnit) : undefined;
  return {
    minimum,
    maximum,
    span,
    ...(majorUnit !== undefined ? { majorUnit } : {}),
    ticks: axisTicks(minimum, maximum, majorUnit),
  };
}

export function spreadsheetChartAxisGridlinesVisible(
  axis: WorkSpreadsheetChartAxis | undefined,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): boolean {
  return axis?.showMajorGridlines ?? workSpreadsheetChartAxisShowsMajorGridlinesByDefault(chartType, position);
}

export function spreadsheetChartAxisScaleAttributes(
  axis: WorkSpreadsheetChartAxis | undefined,
  scale: SpreadsheetChartAxisScale,
  chartType: WorkSpreadsheetChartType,
  position: WorkSpreadsheetChartAxisPosition
): string {
  return [
    `data-axis-scale="${position}"`,
    `data-axis-minimum="${roundChartNumber(scale.minimum)}"`,
    `data-axis-maximum="${roundChartNumber(scale.maximum)}"`,
    scale.majorUnit === undefined ? '' : `data-axis-major-unit="${roundChartNumber(scale.majorUnit)}"`,
    `data-axis-gridlines="${spreadsheetChartAxisGridlinesVisible(axis, chartType, position)}"`,
    axis?.numberFormat ? `data-axis-number-format="${escapeChartXml(axis.numberFormat)}"` : '',
    axis?.numberFormatSourceLinked === undefined
      ? ''
      : `data-axis-number-format-source-linked="${axis.numberFormatSourceLinked}"`,
    spreadsheetChartAxisDisplayAttributes(axis, chartType, position),
  ]
    .filter(Boolean)
    .join(' ');
}

export function formatSpreadsheetChartAxisNumber(value: number, formatCode: string | undefined): string {
  const code = formatCode?.trim();
  if (!code || /^general$/i.test(code)) return compactChartNumber(value);
  const section = code.split(';')[value < 0 ? 1 : 0] || code.split(';')[0] || code;
  const percent = section.includes('%');
  const scientific = /e[+-]/i.test(section);
  const decimalPattern = /[0#]+\.([0#]+)/.exec(section);
  const decimals = Math.min(12, decimalPattern?.[1].length ?? 0);
  const scaled = percent ? value * 100 : value;
  let formatted: string;
  if (scientific) {
    const [mantissa, exponentText] = scaled.toExponential(decimals).split('e');
    const exponent = Number(exponentText);
    const exponentWidth = /e[+-](0+)/i.exec(section)?.[1].length ?? 1;
    formatted = `${mantissa}E${exponent >= 0 ? '+' : '-'}${String(Math.abs(exponent)).padStart(exponentWidth, '0')}`;
  } else {
    formatted = new Intl.NumberFormat('en-US', {
      useGrouping: section.includes(','),
      minimumFractionDigits: decimals,
      maximumFractionDigits: decimals,
    }).format(scaled);
  }
  const currency = /[$¥€£]/.exec(section)?.[0] ?? '';
  return `${currency}${formatted}${percent ? '%' : ''}`;
}

function axisTicks(minimum: number, maximum: number, majorUnit: number | undefined): number[] {
  if (majorUnit === undefined) {
    return Array.from({ length: DEFAULT_TICK_COUNT }, (_, index) =>
      normalizedTick(minimum + (index / (DEFAULT_TICK_COUNT - 1)) * (maximum - minimum))
    );
  }
  const epsilon = Math.max(Number.EPSILON, Math.abs(majorUnit) * 1e-9);
  const first = Math.ceil((minimum - epsilon) / majorUnit) * majorUnit;
  const count = Math.floor((maximum - first + epsilon) / majorUnit) + 1;
  if (count <= 0) return [normalizedTick(minimum), normalizedTick(maximum)];
  if (count > MAX_RENDERED_TICKS) {
    const step = Math.ceil(count / MAX_RENDERED_TICKS);
    return Array.from({ length: Math.ceil(count / step) }, (_, index) =>
      normalizedTick(first + index * step * majorUnit)
    ).filter((value) => value <= maximum + epsilon);
  }
  return Array.from({ length: count }, (_, index) => normalizedTick(first + index * majorUnit));
}

function normalizedTick(value: number): number {
  return Math.abs(value) < 1e-12 ? 0 : Math.round(value * 1e12) / 1e12;
}
