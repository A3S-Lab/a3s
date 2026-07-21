import {
  normalizeWorkSpreadsheetChartAxisGroup,
  normalizeWorkSpreadsheetCombinationSeriesType,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetChartAxis,
} from './work-types';
import {
  escapeChartXml,
  finiteChartNumber,
  roundChartNumber,
  truncateChartText,
} from './work-spreadsheet-chart-svg-utils';
import {
  formatSpreadsheetChartAxisNumber,
  spreadsheetChartAxisDisplayAttributes,
  spreadsheetChartAxisGridlinesVisible,
  spreadsheetChartAxisLabelLayout,
  spreadsheetChartAxisScale,
  spreadsheetChartAxisScaleAttributes,
  spreadsheetChartAxisValueRatio,
  spreadsheetChartCategoryLabelVisible,
  spreadsheetChartCategoryVisualIndex,
  spreadsheetChartMajorTickSvg,
  type SpreadsheetChartAxisScale,
} from './work-spreadsheet-chart-axis';
import { spreadsheetDataLabelSvg } from './work-spreadsheet-data-label-svg';
import { spreadsheetSeriesErrorBarsSvg } from './work-spreadsheet-error-bar-svg';
import { spreadsheetSeriesErrorBarBounds } from './work-spreadsheet-error-bars';
import { spreadsheetSeriesTrendlineFits, spreadsheetSeriesTrendlinesSvg } from './work-spreadsheet-trendline-svg';
import {
  spreadsheetChartSeriesFillStyle,
  spreadsheetChartSeriesLineStyle,
  spreadsheetChartSeriesMarkerSvg,
} from './work-spreadsheet-chart-series-style';

interface CombinationPlot {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface CombinationAxis extends SpreadsheetChartAxisScale {
  position: (value: number) => number;
}

export function spreadsheetCombinationChartSvg(chart: WorkSpreadsheetChart, plot: CombinationPlot): string {
  const values = chart.series.flatMap((series) => combinationSeriesValues(series));
  if (!values.length) return emptyCombinationChartSvg(plot);
  const hasSecondaryAxis = chart.series.some(
    (series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary'
  );
  const primaryValues = chart.series
    .filter((series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'primary')
    .flatMap((series) => combinationSeriesValues(series));
  const secondaryValues = chart.series
    .filter((series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary')
    .flatMap((series) => combinationSeriesValues(series));
  const primaryAxis = combinationAxis(primaryValues.length ? primaryValues : values, plot, chart.axes?.left);
  const secondaryAxis = combinationAxis(secondaryValues.length ? secondaryValues : values, plot, chart.axes?.right);
  const categoryCount = Math.max(1, chart.categories.length, ...chart.series.map((series) => series.values.length));
  const pointX = (index: number, axis: WorkSpreadsheetChartAxis | undefined) =>
    categoryCount === 1
      ? plot.x + plot.width / 2
      : plot.x + ((spreadsheetChartCategoryVisualIndex(index, categoryCount, axis) + 0.5) / categoryCount) * plot.width;
  const axisForSeries = (seriesIndex: number) =>
    normalizeWorkSpreadsheetChartAxisGroup(chart.series[seriesIndex]?.axisGroup) === 'secondary'
      ? secondaryAxis
      : primaryAxis;
  const pointXForSeries = (seriesIndex: number, index: number) =>
    pointX(
      index,
      normalizeWorkSpreadsheetChartAxisGroup(chart.series[seriesIndex]?.axisGroup) === 'secondary'
        ? chart.axes?.top
        : chart.axes?.bottom
    );
  const areas = chart.series
    .map((series, seriesIndex) =>
      normalizeWorkSpreadsheetCombinationSeriesType(series.chartType) === 'area'
        ? combinationAreaSeriesSvg(
            chart,
            seriesIndex,
            (index) => pointXForSeries(seriesIndex, index),
            axisForSeries(seriesIndex)
          )
        : ''
    )
    .join('');
  const columns = combinationColumnSeriesSvg(chart, plot, categoryCount, axisForSeries, pointXForSeries);
  const lines = chart.series
    .map((series, seriesIndex) =>
      normalizeWorkSpreadsheetCombinationSeriesType(series.chartType) === 'line'
        ? combinationLineSeriesSvg(
            chart,
            seriesIndex,
            (index) => pointXForSeries(seriesIndex, index),
            axisForSeries(seriesIndex)
          )
        : ''
    )
    .join('');
  const trendlines = chart.series
    .map((series, seriesIndex) =>
      spreadsheetSeriesTrendlinesSvg(series, seriesIndex, combinationTrendlineXValues(series), (x, y) => [
        pointXForSeries(seriesIndex, x - 1),
        axisForSeries(seriesIndex).position(y),
      ])
    )
    .join('');
  return `<g data-combination-chart="true">${combinationAxesSvg(
    chart,
    plot,
    primaryAxis,
    secondaryAxis,
    hasSecondaryAxis
  )}${areas}${columns}${lines}${trendlines}${combinationCategoryLabelsSvg(chart, plot, categoryCount)}</g>`;
}

function combinationTrendlineXValues(series: WorkSpreadsheetChart['series'][number]): number[] {
  return series.values.map((_, index) => index + 1);
}

function combinationTrendlineValues(series: WorkSpreadsheetChart['series'][number]): number[] {
  return spreadsheetSeriesTrendlineFits(series, combinationTrendlineXValues(series)).flatMap((item) =>
    item.fit.points.map((point) => point.y)
  );
}

function combinationSeriesValues(series: WorkSpreadsheetChart['series'][number]): number[] {
  return [
    ...series.values.map(finiteChartNumber),
    ...spreadsheetSeriesErrorBarBounds(series, 'combination', 'y'),
    ...combinationTrendlineValues(series),
  ];
}

function combinationAxis(
  values: number[],
  plot: CombinationPlot,
  axis: WorkSpreadsheetChartAxis | undefined
): CombinationAxis {
  const scale = spreadsheetChartAxisScale(values, axis, { includeZero: true });
  return {
    ...scale,
    position: (value) => plot.y + (1 - spreadsheetChartAxisValueRatio(value, scale, axis)) * plot.height,
  };
}

function combinationAxesSvg(
  chart: WorkSpreadsheetChart,
  plot: CombinationPlot,
  primary: CombinationAxis,
  secondary: CombinationAxis,
  hasSecondary: boolean
): string {
  const primaryAxis = chart.axes?.left;
  const secondaryAxis = chart.axes?.right;
  const primaryGridlines = spreadsheetChartAxisGridlinesVisible(primaryAxis, chart.type, 'left');
  const secondaryGridlines = spreadsheetChartAxisGridlinesVisible(secondaryAxis, chart.type, 'right');
  const grid = primary.ticks
    .map((value, index) => {
      const y = primary.position(value);
      const label = spreadsheetChartAxisLabelLayout(primaryAxis, chart.type, 'left', plot, plot.x, y);
      return `${
        primaryGridlines
          ? `<line x1="${plot.x}" y1="${roundChartNumber(y)}" x2="${roundChartNumber(
              plot.x + plot.width
            )}" y2="${roundChartNumber(y)}" stroke="#e8ebf0" stroke-width="1"/>`
          : ''
      }${spreadsheetChartMajorTickSvg(primaryAxis, 'left', index, plot.x, y)}${
        label
          ? `<text data-axis-tick="left" x="${roundChartNumber(label.x)}" y="${roundChartNumber(
              label.y
            )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${label.textAnchor}">${escapeChartXml(
              formatSpreadsheetChartAxisNumber(value, primaryAxis?.numberFormat)
            )}</text>`
          : ''
      }`;
    })
    .join('');
  const secondaryLabels = hasSecondary
    ? `<g data-secondary-axis="true" ${spreadsheetChartAxisScaleAttributes(
        secondaryAxis,
        secondary,
        chart.type,
        'right'
      )}>${secondary.ticks
        .map((value, index) => {
          const y = secondary.position(value);
          const label = spreadsheetChartAxisLabelLayout(
            secondaryAxis,
            chart.type,
            'right',
            plot,
            plot.x + plot.width,
            y
          );
          return `${
            secondaryGridlines
              ? `<line x1="${plot.x}" y1="${roundChartNumber(y)}" x2="${roundChartNumber(
                  plot.x + plot.width
                )}" y2="${roundChartNumber(y)}" stroke="#eef0f4" stroke-width="1" stroke-dasharray="2 2"/>`
              : ''
          }${spreadsheetChartMajorTickSvg(secondaryAxis, 'right', index, plot.x + plot.width, y)}${
            label
              ? `<text data-axis-tick="right" x="${roundChartNumber(label.x)}" y="${roundChartNumber(
                  label.y
                )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${label.textAnchor}">${escapeChartXml(
                  formatSpreadsheetChartAxisNumber(value, secondaryAxis?.numberFormat)
                )}</text>`
              : ''
          }`;
        })
        .join('')}<line x1="${roundChartNumber(plot.x + plot.width)}" y1="${plot.y}" x2="${roundChartNumber(
        plot.x + plot.width
      )}" y2="${roundChartNumber(plot.y + plot.height)}" stroke="#aeb6c4" stroke-width="1.2"/></g>`
    : '';
  return `<g ${spreadsheetChartAxisScaleAttributes(
    primaryAxis,
    primary,
    chart.type,
    'left'
  )}>${grid}<line x1="${plot.x}" y1="${plot.y}" x2="${plot.x}" y2="${roundChartNumber(
    plot.y + plot.height
  )}" stroke="#aeb6c4" stroke-width="1.2"/></g><g ${spreadsheetChartAxisDisplayAttributes(
    chart.axes?.bottom,
    chart.type,
    'bottom'
  )}><line x1="${plot.x}" y1="${roundChartNumber(
    plot.y + plot.height
  )}" x2="${roundChartNumber(plot.x + plot.width)}" y2="${roundChartNumber(
    plot.y + plot.height
  )}" stroke="#aeb6c4" stroke-width="1.2"/></g>${secondaryLabels}`;
}

function combinationAreaSeriesSvg(
  chart: WorkSpreadsheetChart,
  seriesIndex: number,
  pointX: (index: number) => number,
  axis: CombinationAxis
): string {
  const series = chart.series[seriesIndex];
  const points = series.values.map((value, index) => [pointX(index), axis.position(value)] as const);
  if (!points.length) return '';
  const fill = spreadsheetChartSeriesFillStyle(series, seriesIndex, 0.2);
  const lineStyle = spreadsheetChartSeriesLineStyle(series, seriesIndex, 2);
  const line = points
    .map(([x, y], index) => `${index ? 'L' : 'M'} ${roundChartNumber(x)} ${roundChartNumber(y)}`)
    .join(' ');
  const baseline = axis.position(0);
  const path = `${line} L ${roundChartNumber(points.at(-1)?.[0] ?? pointX(0))} ${roundChartNumber(
    baseline
  )} L ${roundChartNumber(points[0][0])} ${roundChartNumber(baseline)} Z`;
  const dataLabels = points
    .map(([x, y], pointIndex) => spreadsheetDataLabelSvg(chart, seriesIndex, pointIndex, { kind: 'point', x, y }))
    .join('');
  const errorBars = spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (_direction, pointIndex, value) => [
    pointX(pointIndex),
    axis.position(value),
  ]);
  return `<g data-combination-series="${seriesIndex}" data-series-chart-type="area" data-axis-group="${normalizeWorkSpreadsheetChartAxisGroup(
    series.axisGroup
  )}"><path data-combination-area="${seriesIndex}" d="${path}" ${fill.attributes} ${
    lineStyle.attributes
  } stroke-linejoin="round"/>${errorBars}${dataLabels}</g>`;
}

function combinationColumnSeriesSvg(
  chart: WorkSpreadsheetChart,
  plot: CombinationPlot,
  categoryCount: number,
  axisForSeries: (seriesIndex: number) => CombinationAxis,
  pointXForSeries: (seriesIndex: number, pointIndex: number) => number
): string {
  const columnSeries = chart.series.flatMap((series, seriesIndex) =>
    normalizeWorkSpreadsheetCombinationSeriesType(series.chartType) === 'column' ? [{ series, seriesIndex }] : []
  );
  if (!columnSeries.length) return '';
  const groupWidth = plot.width / categoryCount;
  const barWidth = Math.max(2, Math.min(36, (groupWidth * 0.72) / columnSeries.length));
  return columnSeries
    .map(({ series, seriesIndex }, columnIndex) => {
      const axis = axisForSeries(seriesIndex);
      const baseline = axis.position(0);
      const fill = spreadsheetChartSeriesFillStyle(series, seriesIndex);
      const line = spreadsheetChartSeriesLineStyle(series, seriesIndex, 0);
      const seriesX = (pointIndex: number) =>
        pointXForSeries(seriesIndex, pointIndex) - (barWidth * columnSeries.length) / 2 + columnIndex * barWidth;
      const renderedWidth = Math.max(1, barWidth - 1.5);
      const bars = series.values
        .map((value, index) => {
          const y = axis.position(value);
          const x = seriesX(index);
          return `<rect data-combination-column="${seriesIndex}:${index}" x="${roundChartNumber(
            x
          )}" y="${roundChartNumber(Math.min(y, baseline))}" width="${roundChartNumber(
            renderedWidth
          )}" height="${roundChartNumber(
            Math.max(1, Math.abs(y - baseline))
          )}" rx="1.5" ${fill.attributes} ${line.attributes}/>${spreadsheetDataLabelSvg(chart, seriesIndex, index, {
            kind: 'verticalBar',
            x: x + renderedWidth / 2,
            valueY: y,
            baselineY: baseline,
            value,
          })}`;
        })
        .join('');
      const errorBars = spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (_direction, pointIndex, value) => [
        seriesX(pointIndex) + renderedWidth / 2,
        axis.position(value),
      ]);
      return `<g data-combination-series="${seriesIndex}" data-series-chart-type="column" data-axis-group="${normalizeWorkSpreadsheetChartAxisGroup(
        series.axisGroup
      )}">${bars}${errorBars}</g>`;
    })
    .join('');
}

function combinationLineSeriesSvg(
  chart: WorkSpreadsheetChart,
  seriesIndex: number,
  pointX: (index: number) => number,
  axis: CombinationAxis
): string {
  const series = chart.series[seriesIndex];
  const points = series.values.map((value, index) => [pointX(index), axis.position(value)] as const);
  if (!points.length) return '';
  const line = spreadsheetChartSeriesLineStyle(series, seriesIndex, 2.5);
  const path = points
    .map(([x, y], index) => `${index ? 'L' : 'M'} ${roundChartNumber(x)} ${roundChartNumber(y)}`)
    .join(' ');
  const markers = points
    .map(([x, y], pointIndex) =>
      spreadsheetChartSeriesMarkerSvg(series, seriesIndex, x, y, {
        visible: true,
        defaultSize: 5,
        attributes: `data-combination-marker="${seriesIndex}:${pointIndex}"`,
      })
    )
    .join('');
  const dataLabels = points
    .map(([x, y], pointIndex) => spreadsheetDataLabelSvg(chart, seriesIndex, pointIndex, { kind: 'point', x, y }))
    .join('');
  const errorBars = spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (_direction, pointIndex, value) => [
    pointX(pointIndex),
    axis.position(value),
  ]);
  return `<g data-combination-series="${seriesIndex}" data-series-chart-type="line" data-axis-group="${normalizeWorkSpreadsheetChartAxisGroup(
    series.axisGroup
  )}">${errorBars}<path data-combination-line="${seriesIndex}" d="${path}" fill="none" ${
    line.attributes
  } stroke-linejoin="round" stroke-linecap="round"/>${markers}${dataLabels}</g>`;
}

function combinationCategoryLabelsSvg(
  chart: WorkSpreadsheetChart,
  plot: CombinationPlot,
  categoryCount: number
): string {
  const labels = (position: 'bottom' | 'top', axis: WorkSpreadsheetChartAxis | undefined) => {
    const axisY = position === 'bottom' ? plot.y + plot.height : plot.y;
    return Array.from({ length: categoryCount }, (_, index) => {
      const visible = spreadsheetChartCategoryLabelVisible(index, categoryCount, axis);
      const label = chart.categories[index] ?? String(index + 1);
      const visualIndex = spreadsheetChartCategoryVisualIndex(index, categoryCount, axis);
      const x = plot.x + ((visualIndex + 0.5) / categoryCount) * plot.width;
      const labelLayout = spreadsheetChartAxisLabelLayout(axis, chart.type, position, plot, x, axisY);
      return `${spreadsheetChartMajorTickSvg(axis, position, index, x, axisY)}${
        visible && labelLayout
          ? `<text data-axis-category-label="${position}:${index}" x="${roundChartNumber(
              labelLayout.x
            )}" y="${roundChartNumber(
              labelLayout.y
            )}" fill="#536078" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${labelLayout.textAnchor}">${escapeChartXml(
              truncateChartText(label, 13)
            )}</text>`
          : ''
      }`;
    }).join('');
  };
  const bottom = `<g ${spreadsheetChartAxisDisplayAttributes(
    chart.axes?.bottom,
    chart.type,
    'bottom'
  )}>${labels('bottom', chart.axes?.bottom)}</g>`;
  const topAxis = chart.axes?.top;
  const showTopAxis = (topAxis?.labelPosition ?? 'none') !== 'none' || (topAxis?.majorTickMark ?? 'none') !== 'none';
  const top = showTopAxis
    ? `<g ${spreadsheetChartAxisDisplayAttributes(topAxis, chart.type, 'top')}><line x1="${plot.x}" y1="${plot.y}" x2="${roundChartNumber(
        plot.x + plot.width
      )}" y2="${plot.y}" stroke="#aeb6c4" stroke-width="1.2"/>${labels('top', topAxis)}</g>`
    : '';
  return `${bottom}${top}`;
}

function emptyCombinationChartSvg(plot: CombinationPlot): string {
  return `<text x="${roundChartNumber(plot.x + plot.width / 2)}" y="${roundChartNumber(
    plot.y + plot.height / 2
  )}" fill="#8791a3" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="13" text-anchor="middle">没有可绘制的数据</text>`;
}
