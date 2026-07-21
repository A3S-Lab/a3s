import { normalizeWorkSpreadsheetChartSmoothLines } from './work-spreadsheet-chart-layout';
import type { SpreadsheetChartSeriesLayout } from './work-spreadsheet-chart-series-layout';
import { roundChartNumber } from './work-spreadsheet-chart-svg-utils';
import { spreadsheetDataLabelSvg } from './work-spreadsheet-data-label-svg';
import { spreadsheetSeriesErrorBarsSvg } from './work-spreadsheet-error-bar-svg';
import { spreadsheetChartCategoryVisualIndex } from './work-spreadsheet-chart-axis';
import type { WorkSpreadsheetChart } from './work-types';
import {
  spreadsheetChartSeriesFillStyle,
  spreadsheetChartSeriesLineStyle,
  spreadsheetChartSeriesMarkerSvg,
} from './work-spreadsheet-chart-series-style';

interface SpreadsheetLineChartPlot {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function spreadsheetLineChartSvg(
  chart: WorkSpreadsheetChart,
  plot: SpreadsheetLineChartPlot,
  valuePosition: (value: number) => number,
  layout: SpreadsheetChartSeriesLayout
): string {
  const pointX = (index: number) =>
    layout.categoryCount === 1
      ? plot.x + plot.width / 2
      : plot.x +
        ((spreadsheetChartCategoryVisualIndex(index, layout.categoryCount, chart.axes?.bottom) + 0.5) /
          layout.categoryCount) *
          plot.width;
  const smooth = chart.type === 'line' && normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines);
  return layout.series
    .map((points, seriesIndex) => {
      const positioned = points.map((point) => ({
        ...point,
        x: pointX(point.categoryIndex),
        startY: valuePosition(point.start),
        endY: valuePosition(point.end),
      }));
      if (!positioned.length) return '';
      const path = chartLinePath(
        positioned.map((point) => [point.x, point.endY] as const),
        smooth
      );
      const series = chart.series[seriesIndex];
      const fill = spreadsheetChartSeriesFillStyle(series, seriesIndex, 0.2);
      const line = spreadsheetChartSeriesLineStyle(series, seriesIndex, 2.5);
      const area =
        chart.type === 'area'
          ? `<path data-area-series="${seriesIndex}" data-area-baseline="${
              layout.stacked ? 'stacked' : 'zero'
            }" d="${stackedAreaPath(positioned)}" ${fill.attributes}/>`
          : '';
      const markers = positioned
        .map((point) =>
          spreadsheetChartSeriesMarkerSvg(series, seriesIndex, point.x, point.endY, {
            visible: true,
            defaultSize: 5,
            attributes: `data-chart-point="${seriesIndex}:${point.categoryIndex}" data-stack-start="${roundChartNumber(
              point.start
            )}" data-stack-end="${roundChartNumber(point.end)}"`,
          })
        )
        .join('');
      const dataLabels = positioned
        .map((point) =>
          spreadsheetDataLabelSvg(chart, seriesIndex, point.categoryIndex, {
            kind: 'point',
            x: point.x,
            y: point.endY,
          })
        )
        .join('');
      const errorBars = layout.stacked
        ? ''
        : spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (_direction, pointIndex, value) => [
            pointX(pointIndex),
            valuePosition(value),
          ]);
      return `${area}${errorBars}<path data-line-series="${seriesIndex}" data-smooth="${smooth}" d="${path}" fill="none" ${line.attributes} stroke-linejoin="round" stroke-linecap="round"/>${markers}${dataLabels}`;
    })
    .join('');
}

function stackedAreaPath(
  points: Array<SpreadsheetChartSeriesLayout['series'][number][number] & { x: number; startY: number; endY: number }>
): string {
  const upper = points.map(
    (point, index) => `${index ? 'L' : 'M'} ${roundChartNumber(point.x)} ${roundChartNumber(point.endY)}`
  );
  const lower = [...points]
    .reverse()
    .map((point) => `L ${roundChartNumber(point.x)} ${roundChartNumber(point.startY)}`);
  return [...upper, ...lower, 'Z'].join(' ');
}

function chartLinePath(points: ReadonlyArray<readonly [number, number]>, smooth: boolean): string {
  if (!points.length) return '';
  if (!smooth || points.length < 3) {
    return points
      .map(([x, y], index) => `${index ? 'L' : 'M'} ${roundChartNumber(x)} ${roundChartNumber(y)}`)
      .join(' ');
  }
  const commands = [`M ${roundChartNumber(points[0][0])} ${roundChartNumber(points[0][1])}`];
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[index - 1] ?? points[index];
    const current = points[index];
    const next = points[index + 1];
    const following = points[index + 2] ?? next;
    commands.push(
      `C ${roundChartNumber(current[0] + (next[0] - previous[0]) / 6)} ${roundChartNumber(
        current[1] + (next[1] - previous[1]) / 6
      )} ${roundChartNumber(next[0] - (following[0] - current[0]) / 6)} ${roundChartNumber(
        next[1] - (following[1] - current[1]) / 6
      )} ${roundChartNumber(next[0])} ${roundChartNumber(next[1])}`
    );
  }
  return commands.join(' ');
}
