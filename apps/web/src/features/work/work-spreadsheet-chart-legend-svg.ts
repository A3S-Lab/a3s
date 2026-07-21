import {
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartLegendPosition,
  type WorkSpreadsheetChartLegendPosition,
} from './work-spreadsheet-chart-layout';
import {
  compactChartNumber,
  escapeChartXml,
  roundChartNumber,
  truncateChartText,
  WORK_SPREADSHEET_CHART_COLORS,
} from './work-spreadsheet-chart-svg-utils';
import type { WorkSpreadsheetChart } from './work-types';
import {
  normalizeWorkSpreadsheetChartSeriesStyle,
  spreadsheetChartSeriesLegendColor,
} from './work-spreadsheet-chart-series-style';

export interface SpreadsheetChartPlotBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface SpreadsheetChartLegendFrame {
  width: number;
  height: number;
  titleHeight: number;
}

export interface SpreadsheetChartLegendLayout {
  plot: SpreadsheetChartPlotBounds;
  svg: string;
}

const VERTICAL_RESERVE = 118;
const HORIZONTAL_RESERVE = 40;

export function spreadsheetChartLegendLayout(
  chart: WorkSpreadsheetChart,
  sourcePlot: SpreadsheetChartPlotBounds,
  frame: SpreadsheetChartLegendFrame
): SpreadsheetChartLegendLayout {
  if (!chart.showLegend) return { plot: sourcePlot, svg: '' };
  const position = normalizeWorkSpreadsheetChartLegendPosition(chart.legendPosition);
  const overlay = normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay);
  const plot = reservedPlot(sourcePlot, position, overlay);
  const bounds = legendBounds(sourcePlot, plot, frame, position, overlay);
  const items = spreadsheetChartLegendItems(chart).slice(0, 8);
  const entries =
    position === 'top' || position === 'bottom'
      ? horizontalLegendEntries(items, bounds)
      : verticalLegendEntries(items, bounds);
  return {
    plot,
    svg: `<g data-chart-legend-position="${position}" data-chart-legend-overlay="${overlay}" aria-label="图例">${entries}</g>`,
  };
}

function reservedPlot(
  source: SpreadsheetChartPlotBounds,
  position: WorkSpreadsheetChartLegendPosition,
  overlay: boolean
): SpreadsheetChartPlotBounds {
  if (overlay) return source;
  if (position === 'left') {
    return { ...source, x: source.x + VERTICAL_RESERVE, width: Math.max(40, source.width - VERTICAL_RESERVE) };
  }
  if (position === 'right' || position === 'topRight') {
    return { ...source, width: Math.max(40, source.width - VERTICAL_RESERVE) };
  }
  if (position === 'top') {
    return { ...source, y: source.y + HORIZONTAL_RESERVE, height: Math.max(40, source.height - HORIZONTAL_RESERVE) };
  }
  return { ...source, height: Math.max(40, source.height - HORIZONTAL_RESERVE) };
}

function legendBounds(
  source: SpreadsheetChartPlotBounds,
  plot: SpreadsheetChartPlotBounds,
  frame: SpreadsheetChartLegendFrame,
  position: WorkSpreadsheetChartLegendPosition,
  overlay: boolean
): SpreadsheetChartPlotBounds {
  if (position === 'left') {
    return {
      x: 12,
      y: frame.titleHeight + 18,
      width: VERTICAL_RESERVE - 18,
      height: frame.height - frame.titleHeight - 32,
    };
  }
  if (position === 'right' || position === 'topRight') {
    return {
      x: overlay ? source.x + source.width - VERTICAL_RESERVE + 12 : plot.x + plot.width + 12,
      y: position === 'topRight' ? frame.titleHeight + 4 : frame.titleHeight + 18,
      width: VERTICAL_RESERVE - 18,
      height: frame.height - frame.titleHeight - 32,
    };
  }
  if (position === 'top') {
    return { x: plot.x, y: frame.titleHeight + 4, width: plot.width, height: HORIZONTAL_RESERVE - 4 };
  }
  return { x: plot.x, y: frame.height - HORIZONTAL_RESERVE + 6, width: plot.width, height: HORIZONTAL_RESERVE - 8 };
}

interface SpreadsheetChartLegendItem {
  label: string;
  color: string;
}

function spreadsheetChartLegendItems(chart: WorkSpreadsheetChart): SpreadsheetChartLegendItem[] {
  if (chart.type === 'pie' || chart.type === 'doughnut') {
    const values = chart.series[0]?.values ?? [];
    const seriesColor = normalizeWorkSpreadsheetChartSeriesStyle(chart.series[0]?.style)?.fillColor;
    return chart.categories.map((category, index) => ({
      color: seriesColor ?? WORK_SPREADSHEET_CHART_COLORS[index % WORK_SPREADSHEET_CHART_COLORS.length],
      label: `${category}${values[index] === undefined ? '' : ` ${compactChartNumber(values[index])}`}`,
    }));
  }
  return chart.series.map((series, index) => ({
    color: spreadsheetChartSeriesLegendColor(series, index),
    label: series.name || `系列 ${index + 1}`,
  }));
}

function verticalLegendEntries(items: SpreadsheetChartLegendItem[], bounds: SpreadsheetChartPlotBounds): string {
  return items.map((item, index) => legendEntrySvg(item, index, bounds.x, bounds.y + index * 21, 16)).join('');
}

function horizontalLegendEntries(items: SpreadsheetChartLegendItem[], bounds: SpreadsheetChartPlotBounds): string {
  if (!items.length) return '';
  const columns = Math.max(1, Math.min(items.length, Math.floor(bounds.width / 96)));
  const itemWidth = bounds.width / columns;
  return items
    .map((item, index) => {
      const column = index % columns;
      const row = Math.floor(index / columns);
      return legendEntrySvg(
        item,
        index,
        bounds.x + column * itemWidth,
        bounds.y + row * 17 + 9,
        Math.max(8, Math.floor(itemWidth / 7))
      );
    })
    .join('');
}

function legendEntrySvg(
  item: SpreadsheetChartLegendItem,
  index: number,
  x: number,
  baselineY: number,
  maximumCharacters: number
): string {
  return [
    `<rect data-chart-legend-entry="${index}" x="${roundChartNumber(x)}" y="${roundChartNumber(
      baselineY - 9
    )}" width="10" height="10" rx="2" fill="${item.color}"/>`,
    `<text x="${roundChartNumber(x + 16)}" y="${roundChartNumber(
      baselineY
    )}" fill="#536078" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10">${escapeChartXml(
      truncateChartText(item.label, maximumCharacters)
    )}</text>`,
  ].join('');
}
