export const SPREADSHEET_CONDITIONAL_ICON_SETS = [
  { name: '3Arrows', label: '三向彩色箭头', count: 3 },
  { name: '3ArrowsGray', label: '三向灰色箭头', count: 3 },
  { name: '3Flags', label: '三色旗帜', count: 3 },
  { name: '3TrafficLights1', label: '三色交通灯（实心）', count: 3 },
  { name: '3TrafficLights2', label: '三色交通灯（边框）', count: 3 },
  { name: '3Signs', label: '三色标志', count: 3 },
  { name: '3Symbols', label: '三色符号（圆形）', count: 3 },
  { name: '3Symbols2', label: '三色符号', count: 3 },
  { name: '4Arrows', label: '四向彩色箭头', count: 4 },
  { name: '4ArrowsGray', label: '四向灰色箭头', count: 4 },
  { name: '4RedToBlack', label: '红到黑圆点', count: 4 },
  { name: '4Rating', label: '四级评分', count: 4 },
  { name: '4TrafficLights', label: '四色交通灯', count: 4 },
  { name: '5Arrows', label: '五向彩色箭头', count: 5 },
  { name: '5ArrowsGray', label: '五向灰色箭头', count: 5 },
  { name: '5Rating', label: '五级评分', count: 5 },
  { name: '5Quarters', label: '五级圆饼', count: 5 },
] as const;

export type SpreadsheetConditionalIconSetName = (typeof SPREADSHEET_CONDITIONAL_ICON_SETS)[number]['name'];
export type SpreadsheetConditionalIconCount = 3 | 4 | 5;
export type SpreadsheetConditionalIconThresholdType = SpreadsheetConditionalThresholdType;

export interface SpreadsheetConditionalIconThreshold extends SpreadsheetConditionalThreshold {
  gte: boolean;
}

export interface SpreadsheetConditionalIconSetFormat {
  iconSet: SpreadsheetConditionalIconSetName;
  showValue: boolean;
  reverse: boolean;
  percent: boolean;
  thresholds: SpreadsheetConditionalIconThreshold[];
}

export interface SpreadsheetConditionalIcon {
  iconSet: SpreadsheetConditionalIconSetName;
  index: number;
  count: SpreadsheetConditionalIconCount;
  showValue: boolean;
}

export interface SpreadsheetConditionalIconAppearance {
  glyph: string;
  color: string;
  label: string;
}

export interface SpreadsheetConditionalIconBounds {
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

const COLOR_3 = ['#c62828', '#f9a825', '#2e7d32'];
const COLOR_4 = ['#c62828', '#ef6c00', '#7cb342', '#2e7d32'];
const COLOR_5 = ['#c62828', '#ef6c00', '#f9a825', '#7cb342', '#2e7d32'];

export function isSpreadsheetConditionalIconSetName(value: unknown): value is SpreadsheetConditionalIconSetName {
  return SPREADSHEET_CONDITIONAL_ICON_SETS.some((item) => item.name === value);
}

export function spreadsheetConditionalIconSetCount(
  iconSet: SpreadsheetConditionalIconSetName
): SpreadsheetConditionalIconCount {
  return SPREADSHEET_CONDITIONAL_ICON_SETS.find((item) => item.name === iconSet)!.count;
}

export function defaultSpreadsheetConditionalIconThresholds(
  iconSet: SpreadsheetConditionalIconSetName
): SpreadsheetConditionalIconThreshold[] {
  const count = spreadsheetConditionalIconSetCount(iconSet);
  return Array.from({ length: count }, (_, index) =>
    index === 0
      ? { type: 'min', gte: true }
      : {
          type: 'percent',
          value: Math.round((index * 100) / count),
          gte: true,
        }
  );
}

export function normalizeSpreadsheetConditionalIconSetFormat(
  value: unknown
): SpreadsheetConditionalIconSetFormat | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const source = value as Partial<SpreadsheetConditionalIconSetFormat>;
  if (!isSpreadsheetConditionalIconSetName(source.iconSet) || !Array.isArray(source.thresholds)) return null;
  const count = spreadsheetConditionalIconSetCount(source.iconSet);
  if (source.thresholds.length !== count) return null;
  const thresholds = source.thresholds.map(normalizeThreshold);
  if (thresholds.some((threshold) => !threshold)) return null;
  return {
    iconSet: source.iconSet,
    showValue: source.showValue !== false,
    reverse: source.reverse === true,
    percent: source.percent !== false,
    thresholds: thresholds as SpreadsheetConditionalIconThreshold[],
  };
}

export function spreadsheetConditionalIconForValue(
  sourceFormat: SpreadsheetConditionalIconSetFormat,
  value: number,
  sourceValues: number[]
): SpreadsheetConditionalIcon | null {
  const format = normalizeSpreadsheetConditionalIconSetFormat(sourceFormat);
  const values = sourceValues.filter(Number.isFinite);
  if (!format || !Number.isFinite(value) || !values.length) return null;
  const count = spreadsheetConditionalIconSetCount(format.iconSet);
  let level = 0;
  for (let index = 1; index < count; index += 1) {
    const threshold = format.thresholds[index];
    const cutoff = spreadsheetConditionalThresholdValue(threshold, values);
    if (cutoff === null) return null;
    if (threshold.gte ? value >= cutoff : value > cutoff) level = index;
  }
  return {
    iconSet: format.iconSet,
    index: format.reverse ? count - 1 - level : level,
    count,
    showValue: format.showValue,
  };
}

export function spreadsheetConditionalIconAppearance(
  icon: SpreadsheetConditionalIcon
): SpreadsheetConditionalIconAppearance {
  const index = Math.max(0, Math.min(icon.count - 1, icon.index));
  const definition = SPREADSHEET_CONDITIONAL_ICON_SETS.find((item) => item.name === icon.iconSet)!;
  const label = `${definition.label} ${index + 1}/${icon.count}`;
  if (icon.iconSet.includes('Arrows')) {
    const glyphs =
      icon.count === 3 ? ['↓', '→', '↑'] : icon.count === 4 ? ['↓', '↘', '↗', '↑'] : ['↓', '↘', '→', '↗', '↑'];
    return {
      glyph: glyphs[index],
      color: icon.iconSet.includes('Gray') ? '#606a78' : palette(icon.count)[index],
      label,
    };
  }
  if (icon.iconSet === '3Flags') return { glyph: '⚑', color: COLOR_3[index], label };
  if (icon.iconSet.includes('TrafficLights')) {
    return { glyph: icon.iconSet === '3TrafficLights2' ? '◉' : '●', color: palette(icon.count)[index], label };
  }
  if (icon.iconSet === '3Signs') {
    return { glyph: ['◆', '▲', '●'][index], color: COLOR_3[index], label };
  }
  if (icon.iconSet === '3Symbols' || icon.iconSet === '3Symbols2') {
    return { glyph: ['✕', '!', '✓'][index], color: COLOR_3[index], label };
  }
  if (icon.iconSet === '4RedToBlack') {
    return { glyph: '●', color: ['#c62828', '#e45b4f', '#697386', '#111827'][index], label };
  }
  if (icon.iconSet === '4Rating') {
    return { glyph: ['▁', '▃', '▆', '█'][index], color: '#54708f', label };
  }
  if (icon.iconSet === '5Rating') {
    return { glyph: ['▁', '▂', '▄', '▆', '█'][index], color: '#54708f', label };
  }
  return { glyph: ['○', '◔', '◑', '◕', '●'][index], color: '#54708f', label };
}

export function drawSpreadsheetConditionalIcon(
  context: CanvasRenderingContext2D,
  bounds: SpreadsheetConditionalIconBounds,
  icon: SpreadsheetConditionalIcon,
  background: string,
  maskValue = !icon.showValue
): void {
  const width = bounds.endX - bounds.startX;
  const height = bounds.endY - bounds.startY;
  if (width < 6 || height < 6) return;
  const appearance = spreadsheetConditionalIconAppearance(icon);
  context.save();
  context.beginPath();
  context.rect(bounds.startX + 1, bounds.startY + 1, Math.max(0, width - 3), Math.max(0, height - 3));
  context.clip();
  if (maskValue) {
    context.fillStyle = background;
    context.fillRect(bounds.startX + 1, bounds.startY + 1, Math.max(0, width - 3), Math.max(0, height - 3));
  }
  const size = Math.max(9, Math.min(16, height - 5));
  context.fillStyle = appearance.color;
  context.font = `700 ${size}px "Arial Unicode MS", "Segoe UI Symbol", sans-serif`;
  context.textAlign = 'left';
  context.textBaseline = 'middle';
  context.fillText(appearance.glyph, bounds.startX + 4, bounds.startY + height / 2);
  context.restore();
}

function normalizeThreshold(value: unknown): SpreadsheetConditionalIconThreshold | null {
  if (!value || typeof value !== 'object' || Array.isArray(value)) return null;
  const source = value as Partial<SpreadsheetConditionalIconThreshold>;
  const threshold = normalizeSpreadsheetConditionalThreshold(source);
  return threshold ? { ...threshold, gte: source.gte !== false } : null;
}

function palette(count: SpreadsheetConditionalIconCount): string[] {
  if (count === 3) return COLOR_3;
  if (count === 4) return COLOR_4;
  return COLOR_5;
}
import {
  normalizeSpreadsheetConditionalThreshold,
  spreadsheetConditionalThresholdValue,
  type SpreadsheetConditionalThreshold,
  type SpreadsheetConditionalThresholdType,
} from './work-spreadsheet-conditional-values';
