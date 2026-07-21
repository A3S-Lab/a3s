import { useEffect, useRef } from 'react';
import { presentationChartAxes } from '../work-presentation-chart-axes';
import {
  normalizeDoughnutHoleSize,
  normalizePresentationBubbleScale,
  normalizePresentationBubbleSizeRepresents,
  normalizePresentationChartLegendPosition,
  normalizePresentationScatterStyle,
  presentationChartBubbleSizes,
  presentationChartErrorBarCount,
  presentationChartHasDataLabels,
  presentationChartShowsLegend,
  presentationChartTrendlineCount,
  presentationChartXValues,
} from '../work-presentation-charts';
import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSmoothLines,
} from '../work-spreadsheet-chart-layout';
import type { WorkSlideChart } from '../work-types';
import {
  drawPresentationChartSeriesAnalysis,
  drawPresentationChartXyLine,
  presentationChartAnalysisBounds,
} from './presentation-chart-analysis-canvas';
import {
  drawPresentationRadarAxes,
  drawPresentationXyAxes,
  type PresentationChartRect,
} from './presentation-chart-axis-canvas';
import { drawPresentationCartesianChart } from './presentation-chart-cartesian-canvas';
import { drawPresentationChartDataLabel } from './presentation-chart-data-labels';
import {
  drawPresentationChartLegend,
  PRESENTATION_CHART_COLORS,
  presentationChartCanvasLayout,
  presentationChartLegendItems,
} from './presentation-chart-legend-canvas';
import {
  drawPresentationChartMarker,
  fillPresentationChartShape,
  presentationChartSeriesFill,
  presentationChartSeriesLine,
  strokePresentationChartPath,
} from './presentation-chart-series-canvas';

type ChartRect = PresentationChartRect;

export function SlideChart({ chart, label }: { chart: WorkSlideChart; label: string }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const legendPosition = normalizePresentationChartLegendPosition(chart.legendPosition);
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const draw = () => drawChart(canvas, chart);
    draw();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(draw);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [chart]);
  return (
    <canvas
      ref={canvasRef}
      className='work-slide-chart'
      role='img'
      aria-label={label}
      data-presentation-chart-type={chart.type}
      data-presentation-chart-legend-position={presentationChartShowsLegend(chart) ? legendPosition : 'none'}
      data-presentation-chart-data-labels={String(presentationChartHasDataLabels(chart))}
      data-presentation-chart-axes={String(Boolean(presentationChartAxes(chart)))}
      data-presentation-chart-trendlines={presentationChartTrendlineCount(chart)}
      data-presentation-chart-error-bars={presentationChartErrorBarCount(chart)}
      data-presentation-chart-legend-overlay={
        presentationChartShowsLegend(chart)
          ? String(normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay))
          : undefined
      }
      data-presentation-chart-grouping={
        workSpreadsheetChartSupportsGrouping(chart.type)
          ? normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type)
          : undefined
      }
      data-presentation-chart-gap-width={
        workSpreadsheetChartSupportsBarSpacing(chart.type)
          ? normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)
          : undefined
      }
      data-presentation-chart-overlap={
        workSpreadsheetChartSupportsBarSpacing(chart.type)
          ? normalizeWorkSpreadsheetChartOverlap(
              chart.overlap,
              normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type)
            )
          : undefined
      }
      data-presentation-chart-smooth-lines={
        workSpreadsheetChartSupportsSmoothLines(chart.type)
          ? String(normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines))
          : undefined
      }
      data-presentation-chart-custom-series-styles={chart.series.filter((series) => series.style).length}
      data-presentation-chart-scatter-style={
        chart.type === 'scatter' ? normalizePresentationScatterStyle(chart.scatterStyle) : undefined
      }
      data-presentation-chart-bubble-scale={
        chart.type === 'bubble' ? normalizePresentationBubbleScale(chart.bubbleScale) : undefined
      }
      data-presentation-chart-bubble-size-represents={
        chart.type === 'bubble' ? normalizePresentationBubbleSizeRepresents(chart.bubbleSizeRepresents) : undefined
      }
    />
  );
}

function drawChart(canvas: HTMLCanvasElement, chart: WorkSlideChart) {
  const bounds = canvas.getBoundingClientRect();
  const scale = Math.max(1, Math.min(window.devicePixelRatio || 1, 2));
  const width = Math.max(80, Math.round(bounds.width || canvas.clientWidth || 320));
  const height = Math.max(50, Math.round(bounds.height || canvas.clientHeight || 180));
  canvas.width = width * scale;
  canvas.height = height * scale;
  const context = canvas.getContext('2d');
  if (!context) return;
  context.scale(scale, scale);
  context.clearRect(0, 0, width, height);
  const titleHeight = chart.title ? Math.min(24, height * 0.16) : 0;
  if (chart.title) {
    context.fillStyle = '#172033';
    context.font = `${Math.max(8, Math.min(14, width / 22))}px sans-serif`;
    context.textAlign = 'center';
    context.textBaseline = 'alphabetic';
    context.fillText(chart.title, width / 2, Math.max(11, titleHeight * 0.72));
  }

  const content: ChartRect = {
    x: 4,
    y: titleHeight + 3,
    width: Math.max(24, width - 8),
    height: Math.max(18, height - titleHeight - 7),
  };
  const legendItems = presentationChartLegendItems(chart);
  const layout = presentationChartCanvasLayout(chart, content, legendItems.length > 0);
  if (chart.type === 'pie' || chart.type === 'doughnut') {
    drawPieChart(context, chart, layout.plot);
  } else if (chart.type === 'radar') {
    drawRadarChart(context, chart, layout.plot);
  } else if (chart.type === 'scatter' || chart.type === 'bubble') {
    drawXyChart(context, chart, layout.plot);
  } else {
    drawPresentationCartesianChart(context, chart, layout.plot);
  }
  if (layout.legend) drawPresentationChartLegend(context, legendItems, layout.legend, layout.legendPosition);
}

function drawXyChart(context: CanvasRenderingContext2D, chart: WorkSlideChart, sourceRect: ChartRect) {
  const xValues = presentationChartXValues(chart);
  const bounds = presentationChartAnalysisBounds(chart, xValues);
  const { plot, xPosition, yPosition } = drawPresentationXyAxes(context, chart, sourceRect, bounds.x, bounds.y);

  const scatterStyle = normalizePresentationScatterStyle(chart.scatterStyle);
  const drawLine = chart.type === 'scatter' && scatterStyle !== 'marker';
  const drawMarker = chart.type === 'scatter' && scatterStyle !== 'line' && scatterStyle !== 'smooth';
  const smooth = scatterStyle === 'smooth' || scatterStyle === 'smoothMarker';
  const bubbleScale = normalizePresentationBubbleScale(chart.bubbleScale) / 100;
  const sizeRepresents = normalizePresentationBubbleSizeRepresents(chart.bubbleSizeRepresents);
  const visibleBubbleSizes = chart.series.flatMap((series) =>
    presentationChartBubbleSizes(series).filter((value) => value > 0 || chart.showNegativeBubbles)
  );
  const maximumBubbleSize = Math.max(1, ...visibleBubbleSizes.map((value) => Math.abs(value)));
  const maximumBubbleRadius = Math.max(4, Math.min(plot.width, plot.height) * 0.1 * bubbleScale);

  for (const [seriesIndex, series] of chart.series.entries()) {
    const defaultColor = PRESENTATION_CHART_COLORS[seriesIndex % PRESENTATION_CHART_COLORS.length];
    const line = presentationChartSeriesLine(series, seriesIndex, 2, defaultColor);
    const fill = presentationChartSeriesFill(series, seriesIndex, 1, defaultColor);
    const points = series.values.flatMap((value, index) => {
      const x = xValues[index];
      return Number.isFinite(x) && Number.isFinite(value)
        ? [{ x: xPosition(x), y: yPosition(value), sourceIndex: index }]
        : [];
    });
    if (drawLine && points.length) {
      context.beginPath();
      drawPresentationChartXyLine(context, points, smooth);
      strokePresentationChartPath(context, line);
    }
    for (const point of points) {
      drawPresentationChartMarker(context, series, seriesIndex, point.x, point.y, drawMarker, defaultColor);
    }
    if (chart.type === 'bubble') {
      const sizes = presentationChartBubbleSizes(series);
      for (const point of points) {
        const size = sizes[point.sourceIndex] ?? 0;
        if (size === 0 || (size < 0 && !chart.showNegativeBubbles)) continue;
        const ratio = Math.abs(size) / maximumBubbleSize;
        const radius = Math.max(2, maximumBubbleRadius * (sizeRepresents === 'width' ? ratio : Math.sqrt(ratio)));
        fillPresentationChartShape(context, { ...fill, opacity: fill.opacity * (size < 0 ? 0.32 : 0.58) }, () => {
          context.beginPath();
          context.arc(point.x, point.y, radius, 0, Math.PI * 2);
          context.fill();
        });
      }
    }
    drawPresentationChartSeriesAnalysis({
      context,
      chart,
      seriesIndex,
      xValues,
      color: line.color,
      trendlinePosition: (x, y) => [xPosition(x), yPosition(y)],
      errorBarPosition: (direction, pointIndex, value) =>
        direction === 'x'
          ? [xPosition(value), yPosition(series.values[pointIndex] ?? 0)]
          : [xPosition(xValues[pointIndex] ?? pointIndex + 1), yPosition(value)],
    });
    for (const point of points) {
      if (chart.type === 'bubble') {
        const size = series.bubbleSizes?.[point.sourceIndex] ?? 1;
        if (size === 0 || (size < 0 && !chart.showNegativeBubbles)) continue;
      }
      drawPresentationChartDataLabel(context, chart, seriesIndex, point.sourceIndex, {
        kind: 'point',
        x: point.x,
        y: point.y,
      });
    }
  }
}

function drawPieChart(context: CanvasRenderingContext2D, chart: WorkSlideChart, rect: ChartRect) {
  const values = chart.series[0]?.values ?? [];
  const total = values.reduce((sum, value) => sum + Math.max(0, value), 0) || 1;
  const centerX = rect.x + rect.width / 2;
  const centerY = rect.y + rect.height / 2;
  const radius = Math.max(8, Math.min(rect.width, rect.height) * 0.42);
  const slices: Array<{ angle: number; index: number }> = [];
  let angle = -Math.PI / 2;
  for (const [index, value] of values.entries()) {
    const next = angle + (Math.max(0, value) / total) * Math.PI * 2;
    context.beginPath();
    context.moveTo(centerX, centerY);
    context.arc(centerX, centerY, radius, angle, next);
    context.closePath();
    const fill = presentationChartSeriesFill(
      chart.series[0],
      0,
      1,
      PRESENTATION_CHART_COLORS[index % PRESENTATION_CHART_COLORS.length]
    );
    fillPresentationChartShape(context, fill, () => context.fill());
    slices.push({ angle: (angle + next) / 2, index });
    angle = next;
  }
  const innerRadius =
    chart.type === 'doughnut' ? radius * (normalizeDoughnutHoleSize(chart.doughnutHoleSize) / 100) : 0;
  if (chart.type === 'doughnut') {
    context.save();
    context.globalCompositeOperation = 'destination-out';
    context.beginPath();
    context.arc(centerX, centerY, innerRadius, 0, Math.PI * 2);
    context.fill();
    context.restore();
  }
  for (const slice of slices) {
    drawPresentationChartDataLabel(context, chart, 0, slice.index, {
      kind: 'circular',
      centerX,
      centerY,
      angle: slice.angle,
      innerRadius,
      outerRadius: radius,
    });
  }
}

function drawRadarChart(context: CanvasRenderingContext2D, chart: WorkSlideChart, sourceRect: ChartRect) {
  const categoryCount = Math.max(3, chart.categories.length, ...chart.series.map((series) => series.values.length));
  const values = chart.series.flatMap((series) => series.values).filter(Number.isFinite);
  const { point, valueRatio } = drawPresentationRadarAxes(context, chart, sourceRect, categoryCount, values);
  for (const [seriesIndex, series] of chart.series.entries()) {
    const defaultColor = PRESENTATION_CHART_COLORS[seriesIndex % PRESENTATION_CHART_COLORS.length];
    const fill = presentationChartSeriesFill(
      series,
      seriesIndex,
      chart.radarStyle === 'filled' ? 0.24 : 0,
      defaultColor
    );
    const line = presentationChartSeriesLine(series, seriesIndex, 2, defaultColor);
    const points = Array.from({ length: categoryCount }, (_, index) =>
      point(index, valueRatio(series.values[index] ?? 0))
    );
    context.beginPath();
    for (const [index, current] of points.entries()) {
      if (index === 0) context.moveTo(current.x, current.y);
      else context.lineTo(current.x, current.y);
    }
    context.closePath();
    if (chart.radarStyle === 'filled') {
      fillPresentationChartShape(context, fill, () => context.fill());
    }
    strokePresentationChartPath(context, line);
    for (const current of points) {
      drawPresentationChartMarker(
        context,
        series,
        seriesIndex,
        current.x,
        current.y,
        chart.radarStyle === 'marker',
        defaultColor
      );
    }
  }
  for (const [seriesIndex, series] of chart.series.entries()) {
    for (const [index, value] of series.values.entries()) {
      const current = point(index, valueRatio(value));
      drawPresentationChartDataLabel(context, chart, seriesIndex, index, { kind: 'point', ...current });
    }
  }
}
