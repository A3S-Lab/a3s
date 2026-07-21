import {
  formatSpreadsheetChartAxisNumber,
  spreadsheetChartAxisGridlinesVisible,
  spreadsheetChartAxisLabelLayout,
  spreadsheetChartAxisScaleAttributes,
  spreadsheetChartAxisValueRatio,
  spreadsheetChartMajorTickSvg,
  type SpreadsheetChartAxisScale,
} from './work-spreadsheet-chart-axis';
import { escapeChartXml, roundChartNumber } from './work-spreadsheet-chart-svg-utils';
import type { WorkSpreadsheetChart } from './work-types';

interface SpreadsheetXyPlot {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function spreadsheetXyChartAxesSvg(
  chart: WorkSpreadsheetChart,
  plot: SpreadsheetXyPlot,
  xBounds: SpreadsheetChartAxisScale,
  yBounds: SpreadsheetChartAxisScale
): string {
  const xAxis = chart.axes?.bottom;
  const yAxis = chart.axes?.left;
  const xGridlines = spreadsheetChartAxisGridlinesVisible(xAxis, chart.type, 'bottom');
  const yGridlines = spreadsheetChartAxisGridlinesVisible(yAxis, chart.type, 'left');
  const xTicks = xBounds.ticks
    .map((value, index) => {
      const x = plot.x + spreadsheetChartAxisValueRatio(value, xBounds, xAxis) * plot.width;
      const label = spreadsheetChartAxisLabelLayout(xAxis, chart.type, 'bottom', plot, x, plot.y + plot.height);
      return `${
        xGridlines
          ? `<line x1="${roundChartNumber(x)}" y1="${plot.y}" x2="${roundChartNumber(
              x
            )}" y2="${roundChartNumber(plot.y + plot.height)}" stroke="#eef0f4" stroke-width="1"/>`
          : ''
      }${spreadsheetChartMajorTickSvg(xAxis, 'bottom', index, x, plot.y + plot.height)}${
        label
          ? `<text data-axis-tick="bottom" x="${roundChartNumber(label.x)}" y="${roundChartNumber(
              label.y
            )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${label.textAnchor}">${escapeChartXml(
              formatSpreadsheetChartAxisNumber(value, xAxis?.numberFormat)
            )}</text>`
          : ''
      }`;
    })
    .join('');
  const yTicks = yBounds.ticks
    .map((value, index) => {
      const y = plot.y + (1 - spreadsheetChartAxisValueRatio(value, yBounds, yAxis)) * plot.height;
      const label = spreadsheetChartAxisLabelLayout(yAxis, chart.type, 'left', plot, plot.x, y);
      return `${
        yGridlines
          ? `<line x1="${plot.x}" y1="${roundChartNumber(y)}" x2="${roundChartNumber(
              plot.x + plot.width
            )}" y2="${roundChartNumber(y)}" stroke="#e8ebf0" stroke-width="1"/>`
          : ''
      }${spreadsheetChartMajorTickSvg(yAxis, 'left', index, plot.x, y)}${
        label
          ? `<text data-axis-tick="left" x="${roundChartNumber(label.x)}" y="${roundChartNumber(
              label.y
            )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${label.textAnchor}">${escapeChartXml(
              formatSpreadsheetChartAxisNumber(value, yAxis?.numberFormat)
            )}</text>`
          : ''
      }`;
    })
    .join('');
  return `<g ${spreadsheetChartAxisScaleAttributes(
    xAxis,
    xBounds,
    chart.type,
    'bottom'
  )}>${xTicks}<line x1="${plot.x}" y1="${roundChartNumber(
    plot.y + plot.height
  )}" x2="${roundChartNumber(plot.x + plot.width)}" y2="${roundChartNumber(
    plot.y + plot.height
  )}" stroke="#aeb6c4" stroke-width="1.2"/></g><g ${spreadsheetChartAxisScaleAttributes(
    yAxis,
    yBounds,
    chart.type,
    'left'
  )}>${yTicks}<line x1="${plot.x}" y1="${plot.y}" x2="${plot.x}" y2="${roundChartNumber(
    plot.y + plot.height
  )}" stroke="#aeb6c4" stroke-width="1.2"/></g>`;
}
