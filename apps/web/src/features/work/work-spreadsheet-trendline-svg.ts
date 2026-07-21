import { normalizeWorkSpreadsheetTrendline, type WorkSpreadsheetChartSeries } from './work-types';
import { escapeChartXml, roundChartNumber, WORK_SPREADSHEET_CHART_COLORS } from './work-spreadsheet-chart-svg-utils';
import { fitSpreadsheetTrendline, type SpreadsheetTrendlineFit } from './work-spreadsheet-trendlines';

interface SeriesTrendlineFit {
  sourceIndex: number;
  fit: SpreadsheetTrendlineFit;
}

export function spreadsheetSeriesTrendlineFits(
  series: WorkSpreadsheetChartSeries,
  xValues: readonly number[]
): SeriesTrendlineFit[] {
  return (series.trendlines ?? []).flatMap((source, sourceIndex) => {
    const fit = fitSpreadsheetTrendline(xValues, series.values, source);
    return fit ? [{ sourceIndex, fit }] : [];
  });
}

export function spreadsheetSeriesTrendlinesSvg(
  series: WorkSpreadsheetChartSeries,
  seriesIndex: number,
  xValues: readonly number[],
  position: (x: number, y: number) => readonly [number, number]
): string {
  const color = WORK_SPREADSHEET_CHART_COLORS[seriesIndex % WORK_SPREADSHEET_CHART_COLORS.length];
  return spreadsheetSeriesTrendlineFits(series, xValues)
    .map(({ sourceIndex, fit }) => {
      const trendline = normalizeWorkSpreadsheetTrendline(series.trendlines?.[sourceIndex] ?? { type: 'linear' });
      const points = fit.points.map((point) => position(point.x, point.y));
      if (points.length < 2) return '';
      const path = points
        .map(([x, y], index) => `${index ? 'L' : 'M'} ${roundChartNumber(x)} ${roundChartNumber(y)}`)
        .join(' ');
      const equation = trendline.displayEquation && fit.equation ? fit.equation : undefined;
      const rSquared =
        trendline.displayRSquared && fit.rSquared !== undefined ? roundChartNumber(fit.rSquared) : undefined;
      const label = [equation, rSquared === undefined ? '' : `R² = ${rSquared}`].filter(Boolean).join(' · ');
      const labelPoint = points.at(-1)!;
      return [
        `<g data-trendline-series="${seriesIndex}:${sourceIndex}" data-trendline-type="${trendline.type}"${
          trendline.name ? ` data-trendline-name="${escapeChartXml(trendline.name)}"` : ''
        }${equation ? ` data-trendline-equation="${escapeChartXml(equation)}"` : ''}${
          rSquared === undefined ? '' : ` data-trendline-r-squared="${rSquared}"`
        }>`,
        `<path d="${path}" fill="none" stroke="${color}" stroke-width="2" stroke-dasharray="${
          sourceIndex % 2 ? '3 3' : '7 4'
        }" stroke-linecap="round"/>`,
        label
          ? `<text x="${roundChartNumber(labelPoint[0] - 4)}" y="${roundChartNumber(
              labelPoint[1] - 7 - sourceIndex * 12
            )}" fill="${color}" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="9" text-anchor="end">${escapeChartXml(
              label
            )}</text>`
          : '',
        '</g>',
      ].join('');
    })
    .join('');
}
