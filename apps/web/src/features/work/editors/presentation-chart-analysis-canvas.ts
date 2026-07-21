import { roundChartNumber } from '../work-spreadsheet-chart-svg-utils';
import { errorBarSourceValues, spreadsheetErrorBarAmounts } from '../work-spreadsheet-error-bars';
import { fitSpreadsheetTrendline } from '../work-spreadsheet-trendlines';
import {
  normalizeWorkSpreadsheetErrorBars,
  normalizeWorkSpreadsheetTrendline,
  type WorkSlideChart,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetErrorBarDirection,
} from '../work-types';

type CanvasPoint = readonly [number, number];

export interface PresentationChartAnalysisBounds {
  x: number[];
  y: number[];
}

export function presentationChartAnalysisBounds(
  chart: WorkSlideChart,
  xValues: readonly number[]
): PresentationChartAnalysisBounds {
  const x = xValues.filter(Number.isFinite);
  const y = chart.series.flatMap((series) => series.values).filter(Number.isFinite);
  for (const series of chart.series) {
    const analysisSeries = spreadsheetSeries(series, xValues);
    for (const source of series.errorBars ?? []) {
      const errorBars = normalizeWorkSpreadsheetErrorBars(source, chart.type);
      const values = errorBarSourceValues(analysisSeries, errorBars);
      const amounts = spreadsheetErrorBarAmounts(analysisSeries, errorBars, chart.type);
      const target = errorBars.direction === 'x' ? x : y;
      for (const [index, value] of values.entries()) {
        if (!Number.isFinite(value)) continue;
        const amount = amounts[index] ?? { minus: 0, plus: 0 };
        target.push(value - amount.minus, value + amount.plus);
      }
    }
    for (const source of series.trendlines ?? []) {
      const fit = fitSpreadsheetTrendline(xValues, series.values, source);
      if (!fit) continue;
      x.push(...fit.points.map((point) => point.x));
      y.push(...fit.points.map((point) => point.y));
    }
  }
  return { x: x.filter(Number.isFinite), y: y.filter(Number.isFinite) };
}

export function drawPresentationChartSeriesAnalysis({
  context,
  chart,
  seriesIndex,
  xValues,
  color,
  trendlinePosition,
  errorBarPosition,
}: {
  context: CanvasRenderingContext2D;
  chart: WorkSlideChart;
  seriesIndex: number;
  xValues: readonly number[];
  color: string;
  trendlinePosition: (x: number, y: number) => CanvasPoint;
  errorBarPosition: (direction: WorkSpreadsheetErrorBarDirection, pointIndex: number, value: number) => CanvasPoint;
}): void {
  const series = chart.series[seriesIndex];
  if (!series) return;
  const analysisSeries = spreadsheetSeries(series, xValues);
  for (const source of series.errorBars ?? []) {
    const errorBars = normalizeWorkSpreadsheetErrorBars(source, chart.type);
    const values = errorBarSourceValues(analysisSeries, errorBars);
    const amounts = spreadsheetErrorBarAmounts(analysisSeries, errorBars, chart.type);
    for (const [pointIndex, value] of values.entries()) {
      if (!Number.isFinite(value)) continue;
      const amount = amounts[pointIndex] ?? { minus: 0, plus: 0 };
      const start = errorBarPosition(errorBars.direction, pointIndex, value - amount.minus);
      const end = errorBarPosition(errorBars.direction, pointIndex, value + amount.plus);
      drawLine(context, start, end, color, 1.3);
      if (errorBars.showEndCaps === false) continue;
      const horizontal = Math.abs(end[0] - start[0]) >= Math.abs(end[1] - start[1]);
      if (amount.minus > 0) drawErrorBarCap(context, start, horizontal, color);
      if (amount.plus > 0) drawErrorBarCap(context, end, horizontal, color);
    }
  }

  for (const [trendlineIndex, source] of (series.trendlines ?? []).entries()) {
    const trendline = normalizeWorkSpreadsheetTrendline(source);
    const fit = fitSpreadsheetTrendline(xValues, series.values, trendline);
    if (!fit) continue;
    const points = fit.points.map((point) => trendlinePosition(point.x, point.y));
    const first = points[0];
    if (!first || points.length < 2) continue;
    context.save();
    context.beginPath();
    context.moveTo(first[0], first[1]);
    for (const point of points.slice(1)) context.lineTo(point[0], point[1]);
    context.strokeStyle = color;
    context.lineWidth = 1.7;
    context.setLineDash?.(trendlineIndex % 2 ? [3, 3] : [7, 4]);
    context.stroke();
    context.restore();
    const labelPoint = points.at(-1)!;
    const labels = [
      trendline.displayEquation ? fit.equation : undefined,
      trendline.displayRSquared && fit.rSquared !== undefined ? `R² = ${roundChartNumber(fit.rSquared)}` : undefined,
    ].filter((label): label is string => Boolean(label));
    if (!labels.length) continue;
    context.fillStyle = color;
    context.font = '8px sans-serif';
    context.textAlign = 'right';
    context.textBaseline = 'alphabetic';
    for (const [labelIndex, label] of labels.entries()) {
      context.fillText(label, labelPoint[0] - 3, labelPoint[1] - 6 - (labels.length - labelIndex - 1) * 10);
    }
  }
}

export function drawPresentationChartXyLine(
  context: CanvasRenderingContext2D,
  points: Array<{ x: number; y: number }>,
  smooth: boolean
): void {
  const first = points[0];
  if (!first) return;
  context.moveTo(first.x, first.y);
  if (!smooth || points.length < 3) {
    for (const point of points.slice(1)) context.lineTo(point.x, point.y);
    return;
  }
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[Math.max(0, index - 1)];
    const current = points[index];
    const next = points[index + 1];
    const following = points[Math.min(points.length - 1, index + 2)];
    context.bezierCurveTo(
      current.x + (next.x - previous.x) / 6,
      current.y + (next.y - previous.y) / 6,
      next.x - (following.x - current.x) / 6,
      next.y - (following.y - current.y) / 6,
      next.x,
      next.y
    );
  }
}

function spreadsheetSeries(
  series: WorkSlideChart['series'][number],
  xValues: readonly number[]
): WorkSpreadsheetChartSeries {
  return { ...series, xValues: [...xValues] };
}

function drawErrorBarCap(
  context: CanvasRenderingContext2D,
  point: CanvasPoint,
  horizontal: boolean,
  color: string
): void {
  const start: CanvasPoint = horizontal ? [point[0], point[1] - 4] : [point[0] - 4, point[1]];
  const end: CanvasPoint = horizontal ? [point[0], point[1] + 4] : [point[0] + 4, point[1]];
  drawLine(context, start, end, color, 1.3);
}

function drawLine(
  context: CanvasRenderingContext2D,
  start: CanvasPoint,
  end: CanvasPoint,
  color: string,
  width: number
): void {
  context.beginPath();
  context.moveTo(start[0], start[1]);
  context.lineTo(end[0], end[1]);
  context.strokeStyle = color;
  context.lineWidth = width;
  context.stroke();
}
