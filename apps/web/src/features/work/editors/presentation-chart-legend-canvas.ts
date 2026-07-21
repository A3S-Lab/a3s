import { normalizePresentationChartLegendPosition, presentationChartShowsLegend } from '../work-presentation-charts';
import type { WorkSlideChart, WorkSlideChartLegendPosition } from '../work-types';
import type { PresentationChartRect } from './presentation-chart-axis-canvas';

interface PresentationChartLegendItem {
  color: string;
  label: string;
}

export const PRESENTATION_CHART_COLORS = ['#4472c4', '#ed7d31', '#a5a5a5', '#ffc000', '#5b9bd5', '#70ad47'];

export function presentationChartCanvasLayout(
  chart: WorkSlideChart,
  content: PresentationChartRect,
  hasLegendItems: boolean
): {
  plot: PresentationChartRect;
  legend?: PresentationChartRect;
  legendPosition: WorkSlideChartLegendPosition;
} {
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

export function presentationChartLegendItems(chart: WorkSlideChart): PresentationChartLegendItem[] {
  if (chart.type === 'pie' || chart.type === 'doughnut') {
    const itemCount = Math.max(chart.categories.length, chart.series[0]?.values.length ?? 0);
    return Array.from({ length: itemCount }, (_, index) => ({
      color: PRESENTATION_CHART_COLORS[index % PRESENTATION_CHART_COLORS.length],
      label: chart.categories[index]?.trim() || `分类 ${index + 1}`,
    }));
  }
  return chart.series.map((series, index) => ({
    color: PRESENTATION_CHART_COLORS[index % PRESENTATION_CHART_COLORS.length],
    label: series.name.trim() || `系列 ${index + 1}`,
  }));
}

export function drawPresentationChartLegend(
  context: CanvasRenderingContext2D,
  items: PresentationChartLegendItem[],
  rect: PresentationChartRect,
  position: WorkSlideChartLegendPosition
): void {
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
  item: PresentationChartLegendItem,
  x: number,
  y: number,
  swatchSize: number
): void {
  context.fillStyle = item.color;
  context.fillRect(x, y - swatchSize / 2, swatchSize, swatchSize);
  context.fillStyle = '#526078';
  context.fillText(item.label, x + swatchSize + 4, y);
}
