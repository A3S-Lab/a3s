import { presentationChartAxes } from '../work-presentation-chart-axes';
import { normalizeWorkSpreadsheetChartGrouping } from '../work-spreadsheet-chart-layout';
import {
  formatSpreadsheetChartAxisNumber,
  spreadsheetChartAxisGridlinesVisible,
  spreadsheetChartAxisLabelLayout,
  spreadsheetChartAxisScale,
  spreadsheetChartAxisValueRatio,
  spreadsheetChartCategoryLabelVisible,
  spreadsheetChartCategoryVisualIndex,
  type SpreadsheetChartAxisScale,
} from '../work-spreadsheet-chart-axis';
import type { WorkSlideChart, WorkSlideChartAxes, WorkSlideChartAxis } from '../work-types';

export interface PresentationChartRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface PresentationCartesianAxisCanvas {
  plot: PresentationChartRect;
  scale: SpreadsheetChartAxisScale;
  baseline: number;
  categoryPosition: (index: number) => number;
  valuePosition: (value: number) => number;
}

export interface PresentationXyAxisCanvas {
  plot: PresentationChartRect;
  xScale: SpreadsheetChartAxisScale;
  yScale: SpreadsheetChartAxisScale;
  xPosition: (value: number) => number;
  yPosition: (value: number) => number;
}

export interface PresentationRadarAxisCanvas {
  scale: SpreadsheetChartAxisScale;
  valueRatio: (value: number) => number;
  point: (index: number, ratio: number) => { x: number; y: number };
}

const AXIS_COLOR = '#aeb6c4';
const GRID_COLOR = '#e8ebf0';
const LABEL_COLOR = '#717b8f';
const CATEGORY_COLOR = '#536078';

export function drawPresentationCartesianAxes(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  sourceRect: PresentationChartRect,
  values: readonly number[],
  categoryCount: number
): PresentationCartesianAxisCanvas {
  const { axes, plot } = presentationAxisPlot(context, chart, sourceRect);
  const horizontalBars = chart.type === 'bar';
  const sourceValueAxis = horizontalBars ? axes.bottom : axes.left;
  const valueAxis =
    normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type) === 'percentStacked' &&
    !sourceValueAxis?.numberFormat
      ? { ...sourceValueAxis, numberFormat: '0%' }
      : sourceValueAxis;
  const categoryAxis = horizontalBars ? axes.left : axes.bottom;
  const valuePositionName = horizontalBars ? 'bottom' : 'left';
  const categoryPositionName = horizontalBars ? 'left' : 'bottom';
  const scale = spreadsheetChartAxisScale(values, valueAxis, { includeZero: true });
  const valuePosition = horizontalBars
    ? (value: number) => plot.x + spreadsheetChartAxisValueRatio(value, scale, valueAxis) * plot.width
    : (value: number) => plot.y + (1 - spreadsheetChartAxisValueRatio(value, scale, valueAxis)) * plot.height;
  const baseline = valuePosition(0);
  const count = Math.max(1, categoryCount);
  const pointCategories = chart.type === 'line' || chart.type === 'area';
  const categoryPosition = (index: number) => {
    const visualIndex = spreadsheetChartCategoryVisualIndex(index, count, categoryAxis);
    if (horizontalBars) return plot.y + ((visualIndex + 0.5) / count) * plot.height;
    if (pointCategories) {
      return count === 1 ? plot.x + plot.width / 2 : plot.x + (visualIndex / (count - 1)) * plot.width;
    }
    return plot.x + ((visualIndex + 0.5) / count) * plot.width;
  };

  drawValueTicks(context, chart, plot, valueAxis, scale, valuePositionName, valuePosition);
  drawAxisLine(
    context,
    horizontalBars ? plot.x : plot.x,
    horizontalBars ? plot.y + plot.height : plot.y,
    horizontalBars ? plot.x + plot.width : plot.x,
    plot.y + plot.height
  );
  drawAxisLine(
    context,
    horizontalBars ? baseline : plot.x,
    horizontalBars ? plot.y : baseline,
    horizontalBars ? baseline : plot.x + plot.width,
    horizontalBars ? plot.y + plot.height : baseline
  );
  drawCategoryTicks(context, chart, plot, categoryAxis, categoryPositionName, baseline, count, categoryPosition);
  return { plot, scale, baseline, categoryPosition, valuePosition };
}

export function drawPresentationXyAxes(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  sourceRect: PresentationChartRect,
  xValues: readonly number[],
  yValues: readonly number[]
): PresentationXyAxisCanvas {
  const { axes, plot } = presentationAxisPlot(context, chart, sourceRect);
  const xAxis = axes.bottom;
  const yAxis = axes.left;
  const xScale = spreadsheetChartAxisScale(xValues, xAxis, { paddingRatio: 0.06 });
  const yScale = spreadsheetChartAxisScale(yValues, yAxis, { paddingRatio: 0.06 });
  const xPosition = (value: number) => plot.x + spreadsheetChartAxisValueRatio(value, xScale, xAxis) * plot.width;
  const yPosition = (value: number) =>
    plot.y + (1 - spreadsheetChartAxisValueRatio(value, yScale, yAxis)) * plot.height;
  drawValueTicks(context, chart, plot, xAxis, xScale, 'bottom', xPosition);
  drawValueTicks(context, chart, plot, yAxis, yScale, 'left', yPosition);
  drawAxisLine(context, plot.x, plot.y + plot.height, plot.x + plot.width, plot.y + plot.height);
  drawAxisLine(context, plot.x, plot.y, plot.x, plot.y + plot.height);
  return { plot, xScale, yScale, xPosition, yPosition };
}

export function drawPresentationRadarAxes(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  sourceRect: PresentationChartRect,
  categoryCount: number,
  values: readonly number[]
): PresentationRadarAxisCanvas {
  const { axes, plot } = presentationAxisPlot(context, chart, sourceRect);
  const valueAxis = axes.left;
  const categoryAxis = axes.bottom;
  const scale = spreadsheetChartAxisScale(values, valueAxis, { includeZero: true });
  const centerX = plot.x + plot.width / 2;
  const centerY = plot.y + plot.height / 2;
  const radius = Math.max(8, Math.min(plot.width, plot.height) * 0.36);
  const count = Math.max(3, categoryCount);
  const point = (index: number, ratio: number) => {
    const visualIndex = spreadsheetChartCategoryVisualIndex(index, count, categoryAxis);
    const angle = -Math.PI / 2 + (visualIndex / count) * Math.PI * 2;
    return {
      x: centerX + Math.cos(angle) * radius * ratio,
      y: centerY + Math.sin(angle) * radius * ratio,
    };
  };
  const valueRatio = (value: number) => spreadsheetChartAxisValueRatio(value, scale, valueAxis);
  const showGridlines = spreadsheetChartAxisGridlinesVisible(valueAxis, chart.type, 'left');
  for (const tick of scale.ticks) {
    const ratio = valueRatio(tick);
    if (showGridlines && ratio > 0) {
      context.beginPath();
      for (let categoryIndex = 0; categoryIndex < count; categoryIndex += 1) {
        const current = point(categoryIndex, ratio);
        if (categoryIndex === 0) context.moveTo(current.x, current.y);
        else context.lineTo(current.x, current.y);
      }
      context.closePath();
      context.strokeStyle = GRID_COLOR;
      context.lineWidth = 1;
      context.stroke();
    }
    const tickPoint = point(0, ratio);
    drawMajorTick(context, valueAxis, 'left', centerX, tickPoint.y);
    if ((valueAxis?.labelPosition ?? 'nextTo') !== 'none') {
      drawText(
        context,
        formatSpreadsheetChartAxisNumber(tick, valueAxis?.numberFormat),
        centerX + 4,
        tickPoint.y + 10,
        'left',
        LABEL_COLOR
      );
    }
  }
  for (let index = 0; index < count; index += 1) {
    const edge = point(index, 1);
    drawAxisLine(context, centerX, centerY, edge.x, edge.y, GRID_COLOR, 1);
    drawMajorTick(context, categoryAxis, 'bottom', edge.x, edge.y);
    if (
      (categoryAxis?.labelPosition ?? 'nextTo') !== 'none' &&
      spreadsheetChartCategoryLabelVisible(index, count, categoryAxis)
    ) {
      const labelPoint = point(index, 1.14);
      const horizontal = labelPoint.x - centerX;
      drawText(
        context,
        truncate(chart.categories[index] ?? String(index + 1), 13),
        labelPoint.x,
        labelPoint.y + 4,
        Math.abs(horizontal) < 8 ? 'center' : horizontal > 0 ? 'left' : 'right',
        CATEGORY_COLOR
      );
    }
  }
  return { scale, valueRatio, point };
}

function presentationAxisPlot(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  rect: PresentationChartRect
): { axes: WorkSlideChartAxes; plot: PresentationChartRect } {
  const axes = presentationChartAxes(chart) ?? {};
  const bottomAxis = axes.bottom;
  const leftAxis = axes.left;
  const bottomLabelsHigh = bottomAxis?.labelPosition === 'high';
  const leftLabelsHigh = leftAxis?.labelPosition === 'high';
  const leftTitleSpace = leftAxis?.title?.trim() ? 15 : 0;
  const bottomTitleSpace = bottomAxis?.title?.trim() ? 15 : 0;
  const leftSpace = (leftLabelsHigh ? 5 : 27) + leftTitleSpace;
  const rightSpace = leftLabelsHigh ? 30 : 5;
  const topSpace = bottomLabelsHigh ? 18 : 5;
  const bottomSpace = (bottomLabelsHigh ? 5 : 20) + bottomTitleSpace;
  const plot = {
    x: rect.x + Math.min(leftSpace, rect.width * 0.28),
    y: rect.y + Math.min(topSpace, rect.height * 0.2),
    width: Math.max(18, rect.width - Math.min(leftSpace + rightSpace, rect.width * 0.42)),
    height: Math.max(18, rect.height - Math.min(topSpace + bottomSpace, rect.height * 0.42)),
  };
  context.font = `${Math.max(7, Math.min(10, Math.min(rect.width, rect.height) / 18))}px sans-serif`;
  if (leftAxis?.title?.trim()) {
    context.save();
    context.fillStyle = CATEGORY_COLOR;
    context.textAlign = 'center';
    context.textBaseline = 'middle';
    context.translate(rect.x + 7, plot.y + plot.height / 2);
    context.rotate(-Math.PI / 2);
    context.fillText(leftAxis.title.trim(), 0, 0);
    context.restore();
  }
  if (bottomAxis?.title?.trim()) {
    drawText(
      context,
      bottomAxis.title.trim(),
      plot.x + plot.width / 2,
      rect.y + rect.height - 5,
      'center',
      CATEGORY_COLOR
    );
  }
  return { axes, plot };
}

function drawValueTicks(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  plot: PresentationChartRect,
  axis: WorkSlideChartAxis | undefined,
  scale: SpreadsheetChartAxisScale,
  position: 'bottom' | 'left',
  valuePosition: (value: number) => number
): void {
  const showGridlines = spreadsheetChartAxisGridlinesVisible(axis, chart.type, position);
  for (const tick of scale.ticks) {
    const coordinate = valuePosition(tick);
    const tickX = position === 'bottom' ? coordinate : plot.x;
    const tickY = position === 'left' ? coordinate : plot.y + plot.height;
    if (showGridlines) {
      if (position === 'bottom')
        drawAxisLine(context, coordinate, plot.y, coordinate, plot.y + plot.height, GRID_COLOR, 1);
      else drawAxisLine(context, plot.x, coordinate, plot.x + plot.width, coordinate, GRID_COLOR, 1);
    }
    drawMajorTick(context, axis, position, tickX, tickY);
    const label = spreadsheetChartAxisLabelLayout(axis, chart.type, position, plot, tickX, tickY);
    if (label) {
      drawText(
        context,
        formatSpreadsheetChartAxisNumber(tick, axis?.numberFormat),
        label.x,
        label.y,
        canvasTextAlign(label.textAnchor),
        LABEL_COLOR
      );
    }
  }
}

function drawCategoryTicks(
  context: CanvasRenderingContext2D,
  chart: WorkSlideChart,
  plot: PresentationChartRect,
  axis: WorkSlideChartAxis | undefined,
  position: 'bottom' | 'left',
  baseline: number,
  count: number,
  categoryPosition: (index: number) => number
): void {
  for (let index = 0; index < count; index += 1) {
    const coordinate = categoryPosition(index);
    const tickX = position === 'bottom' ? coordinate : baseline;
    const tickY = position === 'left' ? coordinate : baseline;
    drawMajorTick(context, axis, position, tickX, tickY);
    if (!spreadsheetChartCategoryLabelVisible(index, count, axis)) continue;
    const label = spreadsheetChartAxisLabelLayout(axis, chart.type, position, plot, tickX, tickY);
    if (!label) continue;
    drawText(
      context,
      truncate(chart.categories[index] ?? String(index + 1), 13),
      label.x,
      label.y,
      canvasTextAlign(label.textAnchor),
      CATEGORY_COLOR
    );
  }
}

function drawMajorTick(
  context: CanvasRenderingContext2D,
  axis: WorkSlideChartAxis | undefined,
  position: 'bottom' | 'left',
  x: number,
  y: number
): void {
  const mark = axis?.majorTickMark ?? 'none';
  if (mark === 'none') return;
  const inward = position === 'bottom' ? ([0, -1] as const) : ([1, 0] as const);
  const insideLength = mark === 'outside' ? 0 : mark === 'cross' ? 3 : 5;
  const outsideLength = mark === 'inside' ? 0 : mark === 'cross' ? 3 : 5;
  drawAxisLine(
    context,
    x - inward[0] * outsideLength,
    y - inward[1] * outsideLength,
    x + inward[0] * insideLength,
    y + inward[1] * insideLength,
    LABEL_COLOR,
    1
  );
}

function drawAxisLine(
  context: CanvasRenderingContext2D,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  color = AXIS_COLOR,
  width = 1.2
): void {
  context.beginPath();
  context.moveTo(x1, y1);
  context.lineTo(x2, y2);
  context.strokeStyle = color;
  context.lineWidth = width;
  context.stroke();
}

function drawText(
  context: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  align: CanvasTextAlign,
  color: string
): void {
  context.fillStyle = color;
  context.font = context.font || '9px sans-serif';
  context.textAlign = align;
  context.textBaseline = 'alphabetic';
  context.fillText(text, x, y);
}

function canvasTextAlign(anchor: 'start' | 'middle' | 'end'): CanvasTextAlign {
  return anchor === 'start' ? 'left' : anchor === 'end' ? 'right' : 'center';
}

function truncate(value: string, length: number): string {
  const text = value.trim();
  return text.length > length ? `${text.slice(0, Math.max(1, length - 1))}…` : text;
}
