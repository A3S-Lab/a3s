import {
  normalizeWorkSpreadsheetDataLabelPosition,
  normalizeWorkSpreadsheetDataLabels,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetDataLabelPosition,
} from './work-types';
import {
  compactChartNumber,
  escapeChartXml,
  finiteChartNumber,
  roundChartNumber,
  truncateChartText,
} from './work-spreadsheet-chart-svg-utils';

export type SpreadsheetDataLabelGeometry =
  | { kind: 'point'; x: number; y: number }
  | { kind: 'verticalBar'; x: number; valueY: number; baselineY: number; value: number }
  | { kind: 'horizontalBar'; y: number; valueX: number; baselineX: number; value: number }
  | {
      kind: 'circular';
      centerX: number;
      centerY: number;
      angle: number;
      innerRadius: number;
      outerRadius: number;
    };

export function spreadsheetDataLabelSvg(
  chart: WorkSpreadsheetChart,
  seriesIndex: number,
  pointIndex: number,
  geometry: SpreadsheetDataLabelGeometry
): string {
  const series = chart.series[seriesIndex];
  if (!series?.dataLabels) return '';
  const labels = normalizeWorkSpreadsheetDataLabels(series.dataLabels, chart.type);
  const text = spreadsheetDataLabelText(chart, seriesIndex, pointIndex);
  if (!text) return '';
  const position = normalizeWorkSpreadsheetDataLabelPosition(labels.position);
  const placed = placeDataLabel(geometry, position);
  const visualText = truncateChartText(text.replaceAll(/\r?\n/g, ' '), 32);
  return `<text data-data-label-series="${seriesIndex}:${pointIndex}" data-data-label-position="${position}" data-data-label-text="${escapeChartXml(
    text
  )}" x="${roundChartNumber(placed.x)}" y="${roundChartNumber(
    placed.y
  )}" fill="#27344d" stroke="#ffffff" stroke-width="3" stroke-linejoin="round" paint-order="stroke" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="9" font-weight="500" text-anchor="${placed.anchor}">${escapeChartXml(
    visualText
  )}</text>`;
}

export function spreadsheetDataLabelText(chart: WorkSpreadsheetChart, seriesIndex: number, pointIndex: number): string {
  const series = chart.series[seriesIndex];
  if (!series?.dataLabels) return '';
  const labels = normalizeWorkSpreadsheetDataLabels(series.dataLabels, chart.type);
  const parts: string[] = [];
  if (labels.showSeriesName) parts.push(series.name || `系列 ${seriesIndex + 1}`);
  if (labels.showCategoryName) {
    const category =
      chart.type === 'scatter' || chart.type === 'bubble'
        ? compactChartNumber(series.xValues?.[pointIndex] ?? pointIndex + 1)
        : (chart.categories[pointIndex] ?? String(pointIndex + 1));
    parts.push(category);
  }
  if (labels.showValue) parts.push(compactChartNumber(series.values[pointIndex]));
  if (labels.showPercentage) {
    const values = series.values.map((value) => Math.max(0, finiteChartNumber(value)));
    const total = values.reduce((sum, value) => sum + value, 0);
    if (total > 0) parts.push(`${compactChartNumber(Math.round((values[pointIndex] / total) * 1_000) / 10)}%`);
  }
  if (labels.showBubbleSize) parts.push(compactChartNumber(series.bubbleSizes?.[pointIndex] ?? 0));
  return parts.join(labels.separator ?? ', ');
}

function placeDataLabel(
  geometry: SpreadsheetDataLabelGeometry,
  position: WorkSpreadsheetDataLabelPosition
): { x: number; y: number; anchor: 'start' | 'middle' | 'end' } {
  if (geometry.kind === 'verticalBar') return placeVerticalBarLabel(geometry, position);
  if (geometry.kind === 'horizontalBar') return placeHorizontalBarLabel(geometry, position);
  if (geometry.kind === 'circular') return placeCircularLabel(geometry, position);
  if (position === 'left') return { x: geometry.x - 6, y: geometry.y + 3, anchor: 'end' };
  if (position === 'right') return { x: geometry.x + 6, y: geometry.y + 3, anchor: 'start' };
  if (position === 'below') return { x: geometry.x, y: geometry.y + 13, anchor: 'middle' };
  if (position === 'center' || position === 'insideBase' || position === 'insideEnd') {
    return { x: geometry.x, y: geometry.y + 3, anchor: 'middle' };
  }
  return { x: geometry.x, y: geometry.y - 7, anchor: 'middle' };
}

function placeVerticalBarLabel(
  geometry: Extract<SpreadsheetDataLabelGeometry, { kind: 'verticalBar' }>,
  position: WorkSpreadsheetDataLabelPosition
): { x: number; y: number; anchor: 'start' | 'middle' | 'end' } {
  const positive = geometry.value >= 0;
  if (position === 'left') return { x: geometry.x - 5, y: geometry.valueY + 3, anchor: 'end' };
  if (position === 'right') return { x: geometry.x + 5, y: geometry.valueY + 3, anchor: 'start' };
  if (position === 'center') {
    return { x: geometry.x, y: (geometry.valueY + geometry.baselineY) / 2 + 3, anchor: 'middle' };
  }
  if (position === 'insideBase') {
    return { x: geometry.x, y: geometry.baselineY + (positive ? -5 : 12), anchor: 'middle' };
  }
  if (position === 'insideEnd') {
    return { x: geometry.x, y: geometry.valueY + (positive ? 11 : -5), anchor: 'middle' };
  }
  if (position === 'below') return { x: geometry.x, y: geometry.valueY + 13, anchor: 'middle' };
  return { x: geometry.x, y: geometry.valueY + (positive ? -6 : 13), anchor: 'middle' };
}

function placeHorizontalBarLabel(
  geometry: Extract<SpreadsheetDataLabelGeometry, { kind: 'horizontalBar' }>,
  position: WorkSpreadsheetDataLabelPosition
): { x: number; y: number; anchor: 'start' | 'middle' | 'end' } {
  const positive = geometry.value >= 0;
  if (position === 'above') return { x: geometry.valueX, y: geometry.y - 5, anchor: 'middle' };
  if (position === 'below') return { x: geometry.valueX, y: geometry.y + 12, anchor: 'middle' };
  if (position === 'center') {
    return { x: (geometry.valueX + geometry.baselineX) / 2, y: geometry.y + 3, anchor: 'middle' };
  }
  if (position === 'insideBase') {
    return {
      x: geometry.baselineX + (positive ? 5 : -5),
      y: geometry.y + 3,
      anchor: positive ? 'start' : 'end',
    };
  }
  if (position === 'insideEnd') {
    return {
      x: geometry.valueX + (positive ? -5 : 5),
      y: geometry.y + 3,
      anchor: positive ? 'end' : 'start',
    };
  }
  if (position === 'left') return { x: geometry.valueX - 6, y: geometry.y + 3, anchor: 'end' };
  if (position === 'right') return { x: geometry.valueX + 6, y: geometry.y + 3, anchor: 'start' };
  return {
    x: geometry.valueX + (positive ? 6 : -6),
    y: geometry.y + 3,
    anchor: positive ? 'start' : 'end',
  };
}

function placeCircularLabel(
  geometry: Extract<SpreadsheetDataLabelGeometry, { kind: 'circular' }>,
  position: WorkSpreadsheetDataLabelPosition
): { x: number; y: number; anchor: 'start' | 'middle' | 'end' } {
  const thickness = geometry.outerRadius - geometry.innerRadius;
  const radius =
    position === 'outsideEnd'
      ? geometry.outerRadius + 14
      : position === 'insideBase'
        ? geometry.innerRadius + thickness * 0.28
        : position === 'insideEnd'
          ? geometry.innerRadius + thickness * 0.76
          : geometry.innerRadius + thickness * 0.58;
  const x = geometry.centerX + Math.cos(geometry.angle) * radius;
  const y = geometry.centerY + Math.sin(geometry.angle) * radius;
  if (position === 'left') return { x: x - 8, y: y + 3, anchor: 'end' };
  if (position === 'right') return { x: x + 8, y: y + 3, anchor: 'start' };
  if (position === 'above') return { x, y: y - 7, anchor: 'middle' };
  if (position === 'below') return { x, y: y + 13, anchor: 'middle' };
  const anchor = position === 'outsideEnd' ? (Math.cos(geometry.angle) >= 0 ? 'start' : 'end') : 'middle';
  return { x, y: y + 3, anchor };
}
