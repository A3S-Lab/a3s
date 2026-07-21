import {
  normalizeWorkSpreadsheetErrorBars,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetErrorBarDirection,
} from './work-types';
import { roundChartNumber, WORK_SPREADSHEET_CHART_COLORS } from './work-spreadsheet-chart-svg-utils';
import { errorBarSourceValues, spreadsheetErrorBarAmounts } from './work-spreadsheet-error-bars';

type ErrorBarPoint = readonly [number, number];
type ErrorBarPosition = (
  direction: WorkSpreadsheetErrorBarDirection,
  pointIndex: number,
  value: number
) => ErrorBarPoint;

export function spreadsheetSeriesErrorBarsSvg(
  chart: WorkSpreadsheetChart,
  seriesIndex: number,
  position: ErrorBarPosition
): string {
  const series = chart.series[seriesIndex];
  if (!series?.errorBars?.length) return '';
  const color = WORK_SPREADSHEET_CHART_COLORS[seriesIndex % WORK_SPREADSHEET_CHART_COLORS.length];
  return series.errorBars
    .map((source, errorBarIndex) => {
      const errorBars = normalizeWorkSpreadsheetErrorBars(source, chart.type);
      const values = errorBarSourceValues(series, errorBars);
      const amounts = spreadsheetErrorBarAmounts(series, errorBars, chart.type);
      const points = values
        .map((value, pointIndex) => {
          if (!Number.isFinite(value)) return '';
          const amount = amounts[pointIndex] ?? { minus: 0, plus: 0 };
          const start = position(errorBars.direction, pointIndex, value - amount.minus);
          const end = position(errorBars.direction, pointIndex, value + amount.plus);
          const horizontal = Math.abs(end[0] - start[0]) >= Math.abs(end[1] - start[1]);
          const caps =
            errorBars.showEndCaps === false
              ? ''
              : `${amount.minus > 0 ? errorBarCap(start, horizontal) : ''}${
                  amount.plus > 0 ? errorBarCap(end, horizontal) : ''
                }`;
          return `<g data-error-bar-point="${seriesIndex}:${errorBarIndex}:${pointIndex}" data-error-minus="${roundChartNumber(
            amount.minus
          )}" data-error-plus="${roundChartNumber(amount.plus)}"><path d="M ${roundChartNumber(
            start[0]
          )} ${roundChartNumber(start[1])} L ${roundChartNumber(end[0])} ${roundChartNumber(
            end[1]
          )}" fill="none" stroke="${color}" stroke-width="1.4"/>${caps}</g>`;
        })
        .join('');
      return `<g data-error-bars-series="${seriesIndex}:${errorBarIndex}" data-error-bars-direction="${
        errorBars.direction
      }" data-error-bars-type="${errorBars.barType}" data-error-bars-value-type="${
        errorBars.valueType
      }" data-error-bars-end-caps="${errorBars.showEndCaps !== false}" color="${color}">${points}</g>`;
    })
    .join('');
}

function errorBarCap(point: ErrorBarPoint, horizontal: boolean): string {
  return horizontal
    ? `<path d="M ${roundChartNumber(point[0])} ${roundChartNumber(point[1] - 4)} L ${roundChartNumber(
        point[0]
      )} ${roundChartNumber(point[1] + 4)}" fill="none" stroke="currentColor" stroke-width="1.4"/>`
    : `<path d="M ${roundChartNumber(point[0] - 4)} ${roundChartNumber(point[1])} L ${roundChartNumber(
        point[0] + 4
      )} ${roundChartNumber(point[1])}" fill="none" stroke="currentColor" stroke-width="1.4"/>`;
}
