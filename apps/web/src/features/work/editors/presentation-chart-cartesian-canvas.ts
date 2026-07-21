import { normalizeWorkSpreadsheetChartSmoothLines } from '../work-spreadsheet-chart-layout';
import {
  spreadsheetChartBarGeometry,
  spreadsheetChartSeriesLayout,
  type SpreadsheetChartSeriesLayout,
} from '../work-spreadsheet-chart-series-layout';
import type { WorkSlideChart } from '../work-types';
import {
  drawPresentationChartSeriesAnalysis,
  drawPresentationChartXyLine,
  presentationChartAnalysisBounds,
} from './presentation-chart-analysis-canvas';
import {
  drawPresentationCartesianAxes,
  type PresentationCartesianAxisCanvas,
  type PresentationChartRect,
} from './presentation-chart-axis-canvas';
import { drawPresentationChartDataLabel } from './presentation-chart-data-labels';
import { PRESENTATION_CHART_COLORS } from './presentation-chart-legend-canvas';
import {
  drawPresentationChartMarker,
  fillPresentationChartShape,
  presentationChartSeriesFill,
  presentationChartSeriesLine,
  strokePresentationChartPath,
} from './presentation-chart-series-canvas';

export function drawPresentationCartesianChart(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  sourceRect: PresentationChartRect
): void {
  const layout = spreadsheetChartSeriesLayout(chart);
  const categoryValues = Array.from({ length: layout.categoryCount }, (_, index) => index + 1);
  const values = layout.stacked ? layout.scaleValues : presentationChartAnalysisBounds(chart, categoryValues).y;
  const axes = drawPresentationCartesianAxes(context, chart, sourceRect, values, layout.categoryCount);
  if (chart.type === 'line' || chart.type === 'area') {
    drawLineChart(context, chart, axes, layout);
    return;
  }
  drawBarChart(context, chart, axes, layout);
}

function drawLineChart(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  axes: PresentationCartesianAxisCanvas,
  layout: SpreadsheetChartSeriesLayout
): void {
  const { categoryPosition, valuePosition } = axes;
  const smooth = chart.type === 'line' && normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines);
  for (const [seriesIndex, series] of chart.series.entries()) {
    const defaultColor = PRESENTATION_CHART_COLORS[seriesIndex % PRESENTATION_CHART_COLORS.length];
    const points = (layout.series[seriesIndex] ?? []).map((point) => ({
      ...point,
      x: categoryPosition(point.categoryIndex),
      startY: valuePosition(point.start),
      endY: valuePosition(point.end),
    }));
    if (!points.length) continue;
    if (chart.type === 'area') {
      context.beginPath();
      context.moveTo(points[0].x, points[0].endY);
      for (const point of points.slice(1)) context.lineTo(point.x, point.endY);
      for (const point of [...points].reverse()) context.lineTo(point.x, point.startY);
      context.closePath();
      fillPresentationChartShape(context, presentationChartSeriesFill(series, seriesIndex, 0.2, defaultColor), () =>
        context.fill()
      );
    }
    context.beginPath();
    drawPresentationChartXyLine(
      context,
      points.map((point) => ({ x: point.x, y: point.endY })),
      smooth
    );
    strokePresentationChartPath(context, presentationChartSeriesLine(series, seriesIndex, 2, defaultColor));
    for (const point of points) {
      drawPresentationChartMarker(
        context,
        series,
        seriesIndex,
        point.x,
        point.endY,
        chart.type === 'line',
        defaultColor
      );
      drawPresentationChartDataLabel(context, chart, seriesIndex, point.categoryIndex, {
        kind: 'point',
        x: point.x,
        y: point.endY,
      });
    }
  }
  if (layout.stacked) return;
  const xValues = Array.from({ length: layout.categoryCount }, (_, index) => index + 1);
  for (const [seriesIndex, series] of chart.series.entries()) {
    const line = presentationChartSeriesLine(
      series,
      seriesIndex,
      2,
      PRESENTATION_CHART_COLORS[seriesIndex % PRESENTATION_CHART_COLORS.length]
    );
    drawPresentationChartSeriesAnalysis({
      context,
      chart,
      seriesIndex,
      xValues,
      color: line.color,
      trendlinePosition: (x, y) => [categoryPosition(x - 1), valuePosition(y)],
      errorBarPosition: (_direction, pointIndex, value) => [categoryPosition(pointIndex), valuePosition(value)],
    });
  }
}

function drawBarChart(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  axes: PresentationCartesianAxisCanvas,
  layout: SpreadsheetChartSeriesLayout
): void {
  const { plot, baseline, categoryPosition, valuePosition } = axes;
  const groupSize = (chart.type === 'bar' ? plot.height : plot.width) / layout.categoryCount;
  const seriesCount = Math.max(1, chart.series.length);
  const geometry = spreadsheetChartBarGeometry(groupSize, seriesCount, chart, chart.type === 'bar' ? 30 : 44);
  for (const [seriesIndex, points] of layout.series.entries()) {
    const series = chart.series[seriesIndex];
    if (!series) continue;
    const defaultColor = PRESENTATION_CHART_COLORS[seriesIndex % PRESENTATION_CHART_COLORS.length];
    const fill = presentationChartSeriesFill(series, seriesIndex, 1, defaultColor);
    const line = presentationChartSeriesLine(series, seriesIndex, 0, defaultColor);
    for (const point of points) {
      const start = layout.stacked ? valuePosition(point.start) : baseline;
      const end = valuePosition(point.end);
      const offset = -groupSize / 2 + geometry.offset(seriesIndex);
      if (chart.type === 'bar') {
        const y = categoryPosition(point.categoryIndex) + offset;
        drawPresentationBar(
          context,
          Math.min(start, end),
          y,
          Math.max(1, Math.abs(end - start)),
          geometry.renderedSize,
          fill,
          line
        );
        drawPresentationChartDataLabel(context, chart, seriesIndex, point.categoryIndex, {
          kind: 'horizontalBar',
          y: y + geometry.renderedSize / 2,
          valueX: end,
          baselineX: start,
          value: point.rawValue,
        });
      } else {
        const x = categoryPosition(point.categoryIndex) + offset;
        drawPresentationBar(
          context,
          x,
          Math.min(start, end),
          geometry.renderedSize,
          Math.max(1, Math.abs(end - start)),
          fill,
          line
        );
        drawPresentationChartDataLabel(context, chart, seriesIndex, point.categoryIndex, {
          kind: 'verticalBar',
          x: x + geometry.renderedSize / 2,
          valueY: end,
          baselineY: start,
          value: point.rawValue,
        });
      }
    }
  }
  if (layout.stacked) return;
  const xValues = Array.from({ length: layout.categoryCount }, (_, index) => index + 1);
  for (const [seriesIndex, series] of chart.series.entries()) {
    const seriesOffset = -groupSize / 2 + geometry.offset(seriesIndex) + geometry.renderedSize / 2;
    const position = (x: number, y: number): readonly [number, number] =>
      chart.type === 'bar'
        ? [valuePosition(y), categoryPosition(x - 1) + seriesOffset]
        : [categoryPosition(x - 1) + seriesOffset, valuePosition(y)];
    drawPresentationChartSeriesAnalysis({
      context,
      chart,
      seriesIndex,
      xValues,
      color: presentationChartSeriesLine(
        series,
        seriesIndex,
        2,
        PRESENTATION_CHART_COLORS[seriesIndex % PRESENTATION_CHART_COLORS.length]
      ).color,
      trendlinePosition: position,
      errorBarPosition: (_direction, pointIndex, value) => position(pointIndex + 1, value),
    });
  }
}

function drawPresentationBar(
  context: CanvasRenderingContext2D,
  x: number,
  y: number,
  width: number,
  height: number,
  fill: ReturnType<typeof presentationChartSeriesFill>,
  line: ReturnType<typeof presentationChartSeriesLine>
): void {
  fillPresentationChartShape(context, fill, () => context.fillRect(x, y, width, height));
  if (line.width <= 0) return;
  context.beginPath();
  context.moveTo(x, y);
  context.lineTo(x + width, y);
  context.lineTo(x + width, y + height);
  context.lineTo(x, y + height);
  context.closePath();
  strokePresentationChartPath(context, line);
}
