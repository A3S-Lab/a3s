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
import type { WorkSlideChart, WorkSlideChartLegendPosition } from '../work-types';
import {
  drawPresentationChartSeriesAnalysis,
  drawPresentationChartXyLine,
  presentationChartAnalysisBounds,
} from './presentation-chart-analysis-canvas';
import {
  drawPresentationCartesianAxes,
  drawPresentationRadarAxes,
  drawPresentationXyAxes,
  type PresentationCartesianAxisCanvas,
} from './presentation-chart-axis-canvas';
import { drawPresentationChartDataLabel } from './presentation-chart-data-labels';

interface ChartRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface ChartLegendItem {
  color: string;
  label: string;
}

const CHART_COLORS = ['#4472c4', '#ed7d31', '#a5a5a5', '#ffc000', '#5b9bd5', '#70ad47'];

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
  const layout = chartLayout(chart, content, legendItems.length > 0);
  if (chart.type === 'pie' || chart.type === 'doughnut') {
    drawPieChart(context, chart, layout.plot);
  } else if (chart.type === 'radar') {
    drawRadarChart(context, chart, layout.plot);
  } else if (chart.type === 'scatter' || chart.type === 'bubble') {
    drawXyChart(context, chart, layout.plot);
  } else {
    drawCartesianChart(context, chart, layout.plot);
  }
  if (layout.legend) drawChartLegend(context, legendItems, layout.legend, layout.legendPosition);
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
    const color = CHART_COLORS[seriesIndex % CHART_COLORS.length];
    const points = series.values.flatMap((value, index) => {
      const x = xValues[index];
      return Number.isFinite(x) && Number.isFinite(value)
        ? [{ x: xPosition(x), y: yPosition(value), sourceIndex: index }]
        : [];
    });
    if (drawLine && points.length) {
      context.beginPath();
      drawPresentationChartXyLine(context, points, smooth);
      context.strokeStyle = color;
      context.lineWidth = 2;
      context.stroke();
    }
    if (drawMarker) {
      context.fillStyle = color;
      for (const point of points) {
        context.beginPath();
        context.arc(point.x, point.y, 3, 0, Math.PI * 2);
        context.fill();
      }
    }
    if (chart.type === 'bubble') {
      const sizes = presentationChartBubbleSizes(series);
      context.fillStyle = color;
      for (const point of points) {
        const size = sizes[point.sourceIndex] ?? 0;
        if (size === 0 || (size < 0 && !chart.showNegativeBubbles)) continue;
        const ratio = Math.abs(size) / maximumBubbleSize;
        const radius = Math.max(2, maximumBubbleRadius * (sizeRepresents === 'width' ? ratio : Math.sqrt(ratio)));
        context.save();
        context.globalAlpha = size < 0 ? 0.32 : 0.58;
        context.beginPath();
        context.arc(point.x, point.y, radius, 0, Math.PI * 2);
        context.fill();
        context.restore();
      }
    }
    drawPresentationChartSeriesAnalysis({
      context,
      chart,
      seriesIndex,
      xValues,
      color,
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

function chartLayout(
  chart: WorkSlideChart,
  content: ChartRect,
  hasLegendItems: boolean
): { plot: ChartRect; legend?: ChartRect; legendPosition: WorkSlideChartLegendPosition } {
  const legendPosition = normalizePresentationChartLegendPosition(chart.legendPosition);
  if (!presentationChartShowsLegend(chart) || !hasLegendItems) return { plot: content, legendPosition };
  const gap = 6;
  if (legendPosition === 'top' || legendPosition === 'bottom') {
    const legendHeight = Math.min(34, Math.max(20, content.height * 0.19));
    const plotHeight = Math.max(18, content.height - legendHeight - gap);
    if (legendPosition === 'top') {
      return {
        plot: { ...content, y: content.y + legendHeight + gap, height: plotHeight },
        legend: { ...content, height: legendHeight },
        legendPosition,
      };
    }
    return {
      plot: { ...content, height: plotHeight },
      legend: { ...content, y: content.y + plotHeight + gap, height: legendHeight },
      legendPosition,
    };
  }

  const legendWidth = Math.min(104, Math.max(66, content.width * 0.24));
  const plotWidth = Math.max(24, content.width - legendWidth - gap);
  if (legendPosition === 'left') {
    return {
      plot: { ...content, x: content.x + legendWidth + gap, width: plotWidth },
      legend: { ...content, width: legendWidth },
      legendPosition,
    };
  }
  return {
    plot: { ...content, width: plotWidth },
    legend: { ...content, x: content.x + plotWidth + gap, width: legendWidth },
    legendPosition,
  };
}

function presentationChartLegendItems(chart: WorkSlideChart): ChartLegendItem[] {
  if (chart.type === 'pie' || chart.type === 'doughnut') {
    const itemCount = Math.max(chart.categories.length, chart.series[0]?.values.length ?? 0);
    return Array.from({ length: itemCount }, (_, index) => ({
      color: CHART_COLORS[index % CHART_COLORS.length],
      label: chart.categories[index]?.trim() || `分类 ${index + 1}`,
    }));
  }
  return chart.series.map((series, index) => ({
    color: CHART_COLORS[index % CHART_COLORS.length],
    label: series.name.trim() || `系列 ${index + 1}`,
  }));
}

function drawChartLegend(
  context: CanvasRenderingContext2D,
  items: ChartLegendItem[],
  rect: ChartRect,
  position: WorkSlideChartLegendPosition
) {
  context.font = `${Math.max(7, Math.min(11, rect.height / 3))}px sans-serif`;
  context.textAlign = 'left';
  context.textBaseline = 'middle';
  if (position === 'top' || position === 'bottom') {
    const itemWidth = rect.width / Math.max(1, items.length);
    const y = rect.y + rect.height / 2;
    for (const [index, item] of items.entries()) {
      const x = rect.x + index * itemWidth + 3;
      drawLegendItem(context, item, x, y, Math.max(5, Math.min(9, rect.height * 0.3)));
    }
    return;
  }
  const itemHeight = Math.max(13, Math.min(20, rect.height / Math.max(1, items.length)));
  const startY =
    position === 'topRight'
      ? rect.y + itemHeight / 2 + 2
      : rect.y + rect.height / 2 - ((items.length - 1) * itemHeight) / 2;
  for (const [index, item] of items.entries()) {
    drawLegendItem(context, item, rect.x + 3, startY + index * itemHeight, Math.max(5, Math.min(9, itemHeight * 0.46)));
  }
}

function drawLegendItem(
  context: CanvasRenderingContext2D,
  item: ChartLegendItem,
  x: number,
  y: number,
  swatchSize: number
) {
  context.fillStyle = item.color;
  context.fillRect(x, y - swatchSize / 2, swatchSize, swatchSize);
  context.fillStyle = '#526078';
  context.fillText(item.label, x + swatchSize + 4, y);
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
    context.fillStyle = CHART_COLORS[index % CHART_COLORS.length];
    context.fill();
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
    const seriesColor = CHART_COLORS[seriesIndex % CHART_COLORS.length];
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
      context.save();
      context.globalAlpha = 0.24;
      context.fillStyle = seriesColor;
      context.fill();
      context.restore();
    }
    context.strokeStyle = seriesColor;
    context.lineWidth = 2;
    context.stroke();
    if (chart.radarStyle === 'marker') {
      context.fillStyle = seriesColor;
      for (const current of points) {
        context.beginPath();
        context.arc(current.x, current.y, 2.5, 0, Math.PI * 2);
        context.fill();
      }
    }
  }
  for (const [seriesIndex, series] of chart.series.entries()) {
    for (const [index, value] of series.values.entries()) {
      const current = point(index, valueRatio(value));
      drawPresentationChartDataLabel(context, chart, seriesIndex, index, { kind: 'point', ...current });
    }
  }
}

function drawCartesianChart(context: CanvasRenderingContext2D, chart: WorkSlideChart, sourceRect: ChartRect) {
  const categoryCount = Math.max(1, chart.categories.length, ...chart.series.map((series) => series.values.length));
  const categoryValues = Array.from({ length: categoryCount }, (_, index) => index + 1);
  const values = presentationChartAnalysisBounds(chart, categoryValues).y;
  const axes = drawPresentationCartesianAxes(context, chart, sourceRect, values, categoryCount);
  if (chart.type === 'line' || chart.type === 'area') {
    drawLineChart(context, chart, axes);
    return;
  }
  drawBarChart(context, chart, axes, categoryCount);
}

function drawLineChart(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  axes: PresentationCartesianAxisCanvas
) {
  const { baseline, categoryPosition, valuePosition } = axes;
  for (const [seriesIndex, series] of chart.series.entries()) {
    context.beginPath();
    for (const [index, value] of series.values.entries()) {
      const x = categoryPosition(index);
      const y = valuePosition(value);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    }
    if (chart.type === 'area') {
      context.lineTo(categoryPosition(Math.max(0, series.values.length - 1)), baseline);
      context.lineTo(categoryPosition(0), baseline);
      context.closePath();
      context.globalAlpha = 0.2;
      context.fillStyle = CHART_COLORS[seriesIndex % CHART_COLORS.length];
      context.fill();
      context.globalAlpha = 1;
    }
    context.strokeStyle = CHART_COLORS[seriesIndex % CHART_COLORS.length];
    context.lineWidth = 2;
    context.stroke();
  }
  const xValues = Array.from(
    { length: Math.max(1, ...chart.series.map((series) => series.values.length)) },
    (_, index) => index + 1
  );
  for (const [seriesIndex] of chart.series.entries()) {
    drawPresentationChartSeriesAnalysis({
      context,
      chart,
      seriesIndex,
      xValues,
      color: CHART_COLORS[seriesIndex % CHART_COLORS.length],
      trendlinePosition: (x, y) => [categoryPosition(x - 1), valuePosition(y)],
      errorBarPosition: (_direction, pointIndex, value) => [categoryPosition(pointIndex), valuePosition(value)],
    });
  }
  for (const [seriesIndex, series] of chart.series.entries()) {
    for (const [index, value] of series.values.entries()) {
      const x = categoryPosition(index);
      const y = valuePosition(value);
      drawPresentationChartDataLabel(context, chart, seriesIndex, index, { kind: 'point', x, y });
    }
  }
}

function drawBarChart(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  axes: PresentationCartesianAxisCanvas,
  categoryCount: number
) {
  const { plot, baseline, categoryPosition, valuePosition } = axes;
  const groupSize = (chart.type === 'bar' ? plot.height : plot.width) / categoryCount;
  const seriesCount = Math.max(1, chart.series.length);
  for (const [seriesIndex, series] of chart.series.entries()) {
    for (const [index, value] of series.values.entries()) {
      const end = valuePosition(value);
      context.fillStyle = CHART_COLORS[seriesIndex % CHART_COLORS.length];
      if (chart.type === 'bar') {
        const barHeight = Math.max(2, groupSize / seriesCount - 2);
        const y = categoryPosition(index) - groupSize / 2 + seriesIndex * (barHeight + 2) + 1;
        context.fillRect(Math.min(baseline, end), y, Math.max(1, Math.abs(end - baseline)), barHeight);
      } else {
        const barWidth = Math.max(2, groupSize / seriesCount - 2);
        const x = categoryPosition(index) - groupSize / 2 + seriesIndex * (barWidth + 2) + 1;
        context.fillRect(x, Math.min(baseline, end), barWidth, Math.max(1, Math.abs(end - baseline)));
      }
    }
  }
  const xValues = Array.from(
    { length: Math.max(1, ...chart.series.map((series) => series.values.length)) },
    (_, index) => index + 1
  );
  for (const [seriesIndex] of chart.series.entries()) {
    const barSize = Math.max(2, groupSize / seriesCount - 2);
    const seriesOffset = -groupSize / 2 + seriesIndex * (barSize + 2) + 1 + barSize / 2;
    const position = (x: number, y: number): readonly [number, number] =>
      chart.type === 'bar'
        ? [valuePosition(y), categoryPosition(x - 1) + seriesOffset]
        : [categoryPosition(x - 1) + seriesOffset, valuePosition(y)];
    drawPresentationChartSeriesAnalysis({
      context,
      chart,
      seriesIndex,
      xValues,
      color: CHART_COLORS[seriesIndex % CHART_COLORS.length],
      trendlinePosition: position,
      errorBarPosition: (_direction, pointIndex, value) => position(pointIndex + 1, value),
    });
  }
  for (const [seriesIndex, series] of chart.series.entries()) {
    for (const [index, value] of series.values.entries()) {
      const end = valuePosition(value);
      if (chart.type === 'bar') {
        const barHeight = Math.max(2, groupSize / seriesCount - 2);
        const y = categoryPosition(index) - groupSize / 2 + seriesIndex * (barHeight + 2) + 1 + barHeight / 2;
        drawPresentationChartDataLabel(context, chart, seriesIndex, index, {
          kind: 'horizontalBar',
          y,
          valueX: end,
          baselineX: baseline,
          value,
        });
      } else {
        const barWidth = Math.max(2, groupSize / seriesCount - 2);
        const x = categoryPosition(index) - groupSize / 2 + seriesIndex * (barWidth + 2) + 1 + barWidth / 2;
        drawPresentationChartDataLabel(context, chart, seriesIndex, index, {
          kind: 'verticalBar',
          x,
          valueY: end,
          baselineY: baseline,
          value,
        });
      }
    }
  }
}
