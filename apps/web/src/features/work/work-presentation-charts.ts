import { presentationChartAxesForType } from './work-presentation-chart-axes';
import { createWorkId } from './work-templates';
import type {
  WorkSlideBubbleSizeRepresents,
  WorkSlideChart,
  WorkSlideChartDataLabelPosition,
  WorkSlideChartDataLabels,
  WorkSlideChartLegendPosition,
  WorkSlideChartSeries,
  WorkSlideChartType,
  WorkSlideElement,
  WorkSlideRadarStyle,
  WorkSlideScatterStyle,
} from './work-types';
import {
  normalizeWorkSpreadsheetErrorBars,
  normalizeWorkSpreadsheetTrendline,
  workSpreadsheetChartSupportsErrorBars,
  workSpreadsheetChartSupportsTrendlines,
} from './work-types';

const MAX_CHART_ITEMS = 256;
const chartNumberFormatter = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 6 });
const chartPercentageFormatter = new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 1 });

export function createPresentationChartElement(): WorkSlideElement {
  return {
    id: createWorkId('element'),
    type: 'chart',
    x: 18,
    y: 20,
    width: 64,
    height: 52,
    text: '',
    fontSize: 14,
    color: '#172033',
    fill: '#ffffff',
    bold: false,
    align: 'center',
    borderColor: '#d9dee8',
    borderWidth: 1,
    altText: '季度数据图表',
    chart: {
      type: 'column',
      title: '季度数据',
      categories: ['第一季度', '第二季度', '第三季度'],
      series: [{ name: '系列 1', values: [32, 48, 61] }],
      showLegend: true,
      legendPosition: 'right',
    },
  };
}

export function presentationChartTypeLabel(type: WorkSlideChartType): string {
  if (type === 'bar') return '条形图';
  if (type === 'line') return '折线图';
  if (type === 'pie') return '饼图';
  if (type === 'doughnut') return '圆环图';
  if (type === 'area') return '面积图';
  if (type === 'radar') return '雷达图';
  if (type === 'scatter') return '散点图';
  if (type === 'bubble') return '气泡图';
  return '柱形图';
}

export function withPresentationChartType(chart: WorkSlideChart, type: WorkSlideChartType): WorkSlideChart {
  const {
    axes,
    bubbleScale,
    bubbleSizeRepresents,
    categoryAxisTitle,
    dataLabels,
    doughnutHoleSize,
    radarStyle,
    scatterStyle,
    series,
    showNegativeBubbles,
    valueAxisTitle,
    ...base
  } = chart;
  const nextAxes = presentationChartAxesForType({ ...chart, axes, categoryAxisTitle, valueAxisTitle }, type);
  const nextDataLabels = dataLabels ? normalizePresentationChartDataLabels(dataLabels, type) : undefined;
  if (nextDataLabels && !presentationChartDataLabelsHaveContent(nextDataLabels)) nextDataLabels.showValue = true;
  const nextCategories =
    presentationChartUsesNumericXAxis(type) && !presentationChartUsesNumericXAxis(chart.type)
      ? chart.categories.map((_, index) => String(index + 1))
      : chart.categories;
  const nextSeries = series.map(({ bubbleSizes, errorBars, trendlines, ...current }) => ({
    ...current,
    ...(type === 'bubble'
      ? { bubbleSizes: bubbleSizes?.map(finitePresentationChartNumber) ?? current.values.map(() => 1) }
      : {}),
    ...(workSpreadsheetChartSupportsErrorBars(type) && errorBars?.length
      ? { errorBars: errorBars.map((item) => normalizeWorkSpreadsheetErrorBars(item, type)) }
      : {}),
    ...(workSpreadsheetChartSupportsTrendlines(type) && trendlines?.length
      ? { trendlines: trendlines.map(normalizeWorkSpreadsheetTrendline) }
      : {}),
  }));
  return {
    ...base,
    categories: nextCategories,
    series: nextSeries,
    type,
    ...(nextAxes ? { axes: nextAxes } : {}),
    ...(nextDataLabels ? { dataLabels: nextDataLabels } : {}),
    ...(type === 'doughnut' ? { doughnutHoleSize: normalizeDoughnutHoleSize(doughnutHoleSize) } : {}),
    ...(type === 'radar' ? { radarStyle: normalizeRadarStyle(radarStyle) } : {}),
    ...(type === 'scatter' ? { scatterStyle: normalizePresentationScatterStyle(scatterStyle) } : {}),
    ...(type === 'bubble'
      ? {
          bubbleScale: normalizePresentationBubbleScale(bubbleScale),
          showNegativeBubbles: showNegativeBubbles === true,
          bubbleSizeRepresents: normalizePresentationBubbleSizeRepresents(bubbleSizeRepresents),
        }
      : {}),
  };
}

export function parsePresentationChartCategories(value: string): string[] {
  return value
    .split(/\r?\n/)
    .map((item) => item.trim().slice(0, 255))
    .filter(Boolean)
    .slice(0, MAX_CHART_ITEMS);
}

export function parsePresentationChartValues(value: string): number[] {
  return value
    .split(/[\s,;，；]+/)
    .map((item) => item.trim())
    .filter(Boolean)
    .slice(0, MAX_CHART_ITEMS)
    .map((item) => {
      const number = Number(item);
      return Number.isFinite(number) ? number : 0;
    });
}

export function parsePresentationChartXValues(value: string): string[] {
  return parsePresentationChartValues(value).map((item) => String(item));
}

export function createPresentationChartSeries(chart: WorkSlideChart): WorkSlideChartSeries {
  const values = Array.from({ length: Math.max(1, chart.categories.length) }, () => 0);
  return {
    name: `系列 ${chart.series.length + 1}`,
    values,
    ...(chart.type === 'bubble' ? { bubbleSizes: values.map(() => 1) } : {}),
  };
}

export function withPresentationChartSeriesAnalysis(
  chart: WorkSlideChart,
  seriesIndex: number,
  analysis: Pick<WorkSlideChartSeries, 'errorBars' | 'trendlines'>
): WorkSlideChart {
  return {
    ...chart,
    series: chart.series.map((series, index) => {
      if (index !== seriesIndex) return series;
      const { errorBars: _errorBars, trendlines: _trendlines, ...base } = series;
      const errorBars = workSpreadsheetChartSupportsErrorBars(chart.type)
        ? analysis.errorBars?.map((item) => normalizeWorkSpreadsheetErrorBars(item, chart.type))
        : undefined;
      const trendlines = workSpreadsheetChartSupportsTrendlines(chart.type)
        ? analysis.trendlines?.map(normalizeWorkSpreadsheetTrendline)
        : undefined;
      return {
        ...base,
        ...(errorBars?.length ? { errorBars } : {}),
        ...(trendlines?.length ? { trendlines } : {}),
      };
    }),
  };
}

export function presentationChartTrendlineCount(chart: WorkSlideChart): number {
  return chart.series.reduce((count, series) => count + (series.trendlines?.length ?? 0), 0);
}

export function presentationChartErrorBarCount(chart: WorkSlideChart): number {
  return chart.series.reduce((count, series) => count + (series.errorBars?.length ?? 0), 0);
}

export function normalizeDoughnutHoleSize(value: number | undefined): number {
  const number = Number(value);
  return Number.isFinite(number) ? Math.min(90, Math.max(10, Math.round(number))) : 50;
}

export function normalizeRadarStyle(value: WorkSlideRadarStyle | undefined): WorkSlideRadarStyle {
  return value === 'marker' || value === 'filled' ? value : 'standard';
}

export function normalizePresentationScatterStyle(value: unknown): WorkSlideScatterStyle {
  return value === 'marker' ||
    value === 'line' ||
    value === 'smooth' ||
    value === 'smoothMarker' ||
    value === 'lineMarker'
    ? value
    : 'lineMarker';
}

export function normalizePresentationBubbleScale(value: unknown): number {
  const number = Number(value);
  return Number.isFinite(number) ? Math.min(300, Math.max(5, Math.round(number))) : 100;
}

export function normalizePresentationBubbleSizeRepresents(value: unknown): WorkSlideBubbleSizeRepresents {
  return value === 'width' ? 'width' : 'area';
}

export function presentationChartUsesNumericXAxis(type: WorkSlideChartType): boolean {
  return type === 'scatter' || type === 'bubble';
}

export function presentationChartXValues(chart: WorkSlideChart): number[] {
  const length = Math.max(chart.categories.length, ...chart.series.map((series) => series.values.length));
  return Array.from({ length }, (_, index) => {
    const value = Number(chart.categories[index]);
    return Number.isFinite(value) ? value : index + 1;
  });
}

export function presentationChartBubbleSizes(series: WorkSlideChartSeries): number[] {
  return series.values.map((_, index) => finitePresentationChartNumber(series.bubbleSizes?.[index] ?? 1));
}

export function normalizePresentationChartLegendPosition(value: unknown): WorkSlideChartLegendPosition {
  return value === 'left' || value === 'top' || value === 'bottom' || value === 'topRight' || value === 'right'
    ? value
    : 'right';
}

export function presentationChartLegendPositionLabel(position: WorkSlideChartLegendPosition): string {
  if (position === 'left') return '左侧';
  if (position === 'top') return '顶部';
  if (position === 'bottom') return '底部';
  if (position === 'topRight') return '右上角';
  return '右侧';
}

export function presentationChartShowsLegend(chart: WorkSlideChart): boolean {
  return chart.showLegend ?? chart.series.length > 1;
}

export function presentationChartSupportsAxisTitles(chart: Pick<WorkSlideChart, 'type'>): boolean {
  return chart.type !== 'pie' && chart.type !== 'doughnut';
}

export function presentationChartDataLabelPositions(
  type: WorkSlideChartType
): readonly WorkSlideChartDataLabelPosition[] {
  if (type === 'pie' || type === 'doughnut') return ['bestFit', 'center', 'insideEnd', 'outsideEnd'];
  if (type === 'line' || type === 'radar' || type === 'scatter' || type === 'bubble') {
    return ['above', 'below', 'left', 'right', 'center'];
  }
  return ['outsideEnd', 'insideEnd', 'insideBase', 'center'];
}

export function normalizePresentationChartDataLabelPosition(
  value: unknown,
  type: WorkSlideChartType
): WorkSlideChartDataLabelPosition {
  const positions = presentationChartDataLabelPositions(type);
  if (typeof value === 'string' && positions.includes(value as WorkSlideChartDataLabelPosition)) {
    return value as WorkSlideChartDataLabelPosition;
  }
  if (type === 'pie' || type === 'doughnut') return 'bestFit';
  if (type === 'line' || type === 'radar' || type === 'scatter' || type === 'bubble') return 'above';
  return 'outsideEnd';
}

export function normalizePresentationChartDataLabels(
  source: WorkSlideChartDataLabels,
  type: WorkSlideChartType
): WorkSlideChartDataLabels {
  const separator = typeof source.separator === 'string' ? source.separator.slice(0, 64) : undefined;
  return {
    ...(source.showValue === true ? { showValue: true } : {}),
    ...(source.showCategoryName === true ? { showCategoryName: true } : {}),
    ...(source.showSeriesName === true ? { showSeriesName: true } : {}),
    ...(source.showPercentage === true && (type === 'pie' || type === 'doughnut') ? { showPercentage: true } : {}),
    ...(source.showBubbleSize === true && type === 'bubble' ? { showBubbleSize: true } : {}),
    ...(separator !== undefined ? { separator } : {}),
    position: normalizePresentationChartDataLabelPosition(source.position, type),
  };
}

export function withPresentationChartDataLabels(
  chart: WorkSlideChart,
  dataLabels: WorkSlideChartDataLabels | undefined
): WorkSlideChart {
  const { dataLabels: _current, ...base } = chart;
  return dataLabels ? { ...base, dataLabels: normalizePresentationChartDataLabels(dataLabels, chart.type) } : base;
}

export function presentationChartHasDataLabels(chart: WorkSlideChart): boolean {
  return chart.dataLabels !== undefined;
}

export function presentationChartDataLabelPositionLabel(position: WorkSlideChartDataLabelPosition): string {
  if (position === 'center') return '居中';
  if (position === 'insideBase') return '内侧基部';
  if (position === 'insideEnd') return '内侧末端';
  if (position === 'outsideEnd') return '外侧末端';
  if (position === 'left') return '左侧';
  if (position === 'right') return '右侧';
  if (position === 'above') return '上方';
  if (position === 'below') return '下方';
  return '最佳匹配';
}

export function presentationChartDataLabelText(chart: WorkSlideChart, seriesIndex: number, pointIndex: number): string {
  const series = chart.series[seriesIndex];
  if (!series || !chart.dataLabels) return '';
  const labels = normalizePresentationChartDataLabels(chart.dataLabels, chart.type);
  const parts: string[] = [];
  if (labels.showSeriesName) parts.push(series.name.trim() || `系列 ${seriesIndex + 1}`);
  if (labels.showCategoryName) parts.push(chart.categories[pointIndex]?.trim() || `分类 ${pointIndex + 1}`);
  if (labels.showValue) parts.push(formatPresentationChartNumber(series.values[pointIndex]));
  if (labels.showPercentage) {
    const values = series.values.map((value) => Math.max(0, finitePresentationChartNumber(value)));
    const total = values.reduce((sum, value) => sum + value, 0);
    if (total > 0) parts.push(`${chartPercentageFormatter.format((values[pointIndex] / total) * 100)}%`);
  }
  if (labels.showBubbleSize) parts.push(formatPresentationChartNumber(series.bubbleSizes?.[pointIndex]));
  return parts.join(labels.separator ?? ', ');
}

function presentationChartDataLabelsHaveContent(labels: WorkSlideChartDataLabels): boolean {
  return Boolean(
    labels.showValue ||
      labels.showCategoryName ||
      labels.showSeriesName ||
      labels.showPercentage ||
      labels.showBubbleSize
  );
}

function formatPresentationChartNumber(value: number | undefined): string {
  return chartNumberFormatter.format(finitePresentationChartNumber(value));
}

function finitePresentationChartNumber(value: number | undefined): number {
  return Number.isFinite(value) ? Number(value) : 0;
}
