import {
  normalizeWorkSpreadsheetChartSeriesStyle,
  spreadsheetChartSeriesFillStyle,
  spreadsheetChartSeriesLineStyle,
} from '../work-spreadsheet-chart-series-style';
import type { WorkSlideChartSeries, WorkSpreadsheetChartLineDash } from '../work-types';

export interface PresentationChartSeriesFill {
  color: string;
  opacity: number;
}

export interface PresentationChartSeriesLine {
  color: string;
  width: number;
  dash: WorkSpreadsheetChartLineDash;
}

export function presentationChartSeriesFill(
  series: WorkSlideChartSeries,
  seriesIndex: number,
  defaultOpacity = 1,
  defaultColor?: string
): PresentationChartSeriesFill {
  const fill = spreadsheetChartSeriesFillStyle(series, seriesIndex, defaultOpacity, defaultColor);
  return { color: fill.color, opacity: fill.opacity };
}

export function presentationChartSeriesLine(
  series: WorkSlideChartSeries,
  seriesIndex: number,
  defaultWidth: number,
  defaultColor?: string
): PresentationChartSeriesLine {
  const line = spreadsheetChartSeriesLineStyle(series, seriesIndex, defaultWidth, defaultColor);
  return { color: line.color, width: line.width, dash: line.dash };
}

export function fillPresentationChartShape(
  context: CanvasRenderingContext2D,
  fill: PresentationChartSeriesFill,
  draw: () => void
): void {
  context.save();
  context.fillStyle = fill.color;
  context.globalAlpha = fill.opacity;
  draw();
  context.restore();
}

export function strokePresentationChartPath(
  context: CanvasRenderingContext2D,
  line: PresentationChartSeriesLine
): void {
  if (line.width <= 0) return;
  context.strokeStyle = line.color;
  context.lineWidth = line.width;
  context.setLineDash?.(presentationChartLineDash(line.dash));
  context.stroke();
  context.setLineDash?.([]);
}

export function drawPresentationChartMarker(
  context: CanvasRenderingContext2D,
  series: WorkSlideChartSeries,
  seriesIndex: number,
  x: number,
  y: number,
  defaultVisible: boolean,
  defaultColor?: string
): void {
  const style = normalizeWorkSpreadsheetChartSeriesStyle(series.style);
  const marker = style?.marker;
  const visible = marker ? marker.symbol !== 'none' : defaultVisible;
  if (!visible) return;
  const symbol = marker?.symbol && marker.symbol !== 'none' ? marker.symbol : 'circle';
  const size = marker?.size ?? 5;
  const radius = Math.max(1.5, size * 0.6);
  const line = presentationChartSeriesLine(series, seriesIndex, 2, defaultColor);
  const fillColor = marker?.fillColor ?? '#FFFFFF';
  const lineColor = marker?.lineColor ?? line.color;

  context.save();
  context.fillStyle = fillColor;
  context.strokeStyle = lineColor;
  context.lineWidth = 1.5;
  context.setLineDash?.([]);
  context.beginPath();
  if (symbol === 'circle') {
    context.arc(x, y, radius, 0, Math.PI * 2);
    context.fill();
    context.stroke();
    context.restore();
    return;
  }
  if (symbol === 'plus' || symbol === 'x') {
    if (symbol === 'plus') {
      context.moveTo(x - radius, y);
      context.lineTo(x + radius, y);
      context.moveTo(x, y - radius);
      context.lineTo(x, y + radius);
    } else {
      context.moveTo(x - radius, y - radius);
      context.lineTo(x + radius, y + radius);
      context.moveTo(x + radius, y - radius);
      context.lineTo(x - radius, y + radius);
    }
    context.stroke();
    context.restore();
    return;
  }
  const points = markerPoints(symbol, x, y, radius);
  const first = points[0];
  if (first) {
    context.moveTo(first[0], first[1]);
    for (const point of points.slice(1)) context.lineTo(point[0], point[1]);
    context.closePath();
    context.fill();
    context.stroke();
  }
  context.restore();
}

function markerPoints(
  symbol: 'square' | 'diamond' | 'triangle' | 'star',
  x: number,
  y: number,
  radius: number
): Array<readonly [number, number]> {
  if (symbol === 'square') {
    const side = radius * 0.85;
    return [
      [x - side, y - side],
      [x + side, y - side],
      [x + side, y + side],
      [x - side, y + side],
    ];
  }
  if (symbol === 'diamond') {
    return [
      [x, y - radius],
      [x + radius, y],
      [x, y + radius],
      [x - radius, y],
    ];
  }
  if (symbol === 'triangle') {
    return [
      [x, y - radius],
      [x + radius * 0.9, y + radius * 0.75],
      [x - radius * 0.9, y + radius * 0.75],
    ];
  }
  return Array.from({ length: 10 }, (_, index) => {
    const angle = -Math.PI / 2 + (index * Math.PI) / 5;
    const pointRadius = index % 2 ? radius * 0.42 : radius;
    return [x + Math.cos(angle) * pointRadius, y + Math.sin(angle) * pointRadius] as const;
  });
}

function presentationChartLineDash(dash: WorkSpreadsheetChartLineDash): number[] {
  if (dash === 'dash') return [8, 4];
  if (dash === 'dot') return [2, 3];
  if (dash === 'dashDot') return [8, 4, 2, 4];
  return [];
}
