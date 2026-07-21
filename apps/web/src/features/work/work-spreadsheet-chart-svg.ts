import {
  normalizeWorkSpreadsheetChartAxisGroup,
  normalizeWorkSpreadsheetBubbleScale,
  normalizeWorkSpreadsheetBubbleSizeRepresents,
  normalizeWorkSpreadsheetDoughnutHoleSize,
  normalizeWorkSpreadsheetRadarStyle,
  normalizeWorkSpreadsheetScatterStyle,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetChartAxes,
  type WorkSpreadsheetChartSeries,
} from './work-types';
import {
  escapeChartXml as escapeXml,
  finiteChartNumber as finiteNumber,
  roundChartNumber as round,
  truncateChartText as truncate,
  WORK_SPREADSHEET_CHART_COLORS as CHART_COLORS,
} from './work-spreadsheet-chart-svg-utils';
import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSmoothLines,
} from './work-spreadsheet-chart-layout';
import { spreadsheetChartLegendLayout, type SpreadsheetChartPlotBounds } from './work-spreadsheet-chart-legend-svg';
import {
  spreadsheetChartBarGeometry,
  spreadsheetChartSeriesLayout,
  type SpreadsheetChartSeriesLayout,
} from './work-spreadsheet-chart-series-layout';
import { spreadsheetLineChartSvg } from './work-spreadsheet-line-chart-svg';
import { spreadsheetCombinationChartSvg } from './work-spreadsheet-combination-chart-svg';
import { spreadsheetXyChartAxesSvg } from './work-spreadsheet-xy-chart-axes-svg';
import {
  formatSpreadsheetChartAxisNumber,
  normalizeWorkSpreadsheetChartAxes,
  spreadsheetChartAxisDisplayAttributes,
  spreadsheetChartAxisGridlinesVisible,
  spreadsheetChartAxisLabelLayout,
  spreadsheetChartAxisScale,
  spreadsheetChartAxisScaleAttributes,
  spreadsheetChartAxisValueRatio,
  spreadsheetChartCategoryLabelVisible,
  spreadsheetChartCategoryVisualIndex,
  spreadsheetChartMajorTickSvg,
} from './work-spreadsheet-chart-axis';
import { spreadsheetDataLabelSvg } from './work-spreadsheet-data-label-svg';
import { spreadsheetSeriesErrorBarsSvg } from './work-spreadsheet-error-bar-svg';
import { spreadsheetSeriesErrorBarBounds } from './work-spreadsheet-error-bars';
import { spreadsheetSeriesTrendlineFits, spreadsheetSeriesTrendlinesSvg } from './work-spreadsheet-trendline-svg';
import {
  normalizeWorkSpreadsheetChartSeriesStyle,
  spreadsheetChartSeriesFillStyle,
  spreadsheetChartSeriesLineStyle,
  spreadsheetChartSeriesMarkerSvg,
} from './work-spreadsheet-chart-series-style';

const WIDTH = 640;
const HEIGHT = 360;

export function spreadsheetChartSvgDataUrl(chart: WorkSpreadsheetChart): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(spreadsheetChartSvg(chart))}`;
}

export function spreadsheetChartSvg(chart: WorkSpreadsheetChart): string {
  const title = chart.title?.trim();
  const titleHeight = title ? 42 : 16;
  const circular = chart.type === 'pie' || chart.type === 'doughnut';
  const polar = chart.type === 'radar';
  const xy = chart.type === 'scatter' || chart.type === 'bubble';
  const hasSecondaryAxis =
    chart.type === 'combination' &&
    chart.series.some((series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary');
  const axes = normalizeWorkSpreadsheetChartAxes(chart.axes, chart.type, hasSecondaryAxis);
  const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
  const renderedChart: WorkSpreadsheetChart = {
    ...chart,
    axes,
    ...(workSpreadsheetChartSupportsGrouping(chart.type) ? { grouping } : {}),
    ...(workSpreadsheetChartSupportsBarSpacing(chart.type)
      ? {
          gapWidth: normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth),
          overlap: normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping),
        }
      : {}),
    ...(workSpreadsheetChartSupportsSmoothLines(chart.type)
      ? { smoothLines: normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines) }
      : {}),
  };
  const topAxisTitleHeight = axes?.top?.title ? 18 : 0;
  const secondaryAxisWidth = hasSecondaryAxis ? (axes?.right?.title ? 58 : 44) : 0;
  const plotLeft = chart.type === 'bar' ? 88 : circular || polar ? 24 : xy ? 64 : 56;
  const basePlot = {
    x: plotLeft,
    y: titleHeight + 12 + topAxisTitleHeight,
    width: WIDTH - plotLeft - 18 - secondaryAxisWidth,
    height: HEIGHT - titleHeight - 54 - topAxisTitleHeight,
  };
  const legend = spreadsheetChartLegendLayout(renderedChart, basePlot, { width: WIDTH, height: HEIGHT, titleHeight });
  const plot = legend.plot;
  const plotAttributes = spreadsheetChartPlotAttributes(renderedChart, plot);
  const plotSvg =
    chart.type === 'pie'
      ? pieChartSvg(renderedChart, plot)
      : chart.type === 'doughnut'
        ? doughnutChartSvg(renderedChart, plot)
        : chart.type === 'radar'
          ? radarChartSvg(renderedChart, plot)
          : xy
            ? xyChartSvg(renderedChart, plot)
            : chart.type === 'combination'
              ? spreadsheetCombinationChartSvg(renderedChart, plot)
              : cartesianChartSvg(renderedChart, plot);
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${WIDTH} ${HEIGHT}" preserveAspectRatio="none" data-chart-type="${escapeXml(String(chart.type))}">`,
    '<rect width="640" height="360" rx="8" fill="#ffffff"/>',
    '<rect x="0.75" y="0.75" width="638.5" height="358.5" rx="7.25" fill="none" stroke="#d9dee8" stroke-width="1.5"/>',
    title
      ? `<text x="${WIDTH / 2}" y="27" fill="#172033" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="17" font-weight="600" text-anchor="middle">${escapeXml(title)}</text>`
      : '',
    `<g ${plotAttributes}>${plotSvg}</g>`,
    axes ? chartAxisTitlesSvg(axes, plot) : '',
    legend.svg,
    '</svg>',
  ].join('');
}

function spreadsheetChartPlotAttributes(chart: WorkSpreadsheetChart, plot: SpreadsheetChartPlotBounds): string {
  const attributes = [
    'data-plot-area="true"',
    `data-plot-x="${round(plot.x)}"`,
    `data-plot-y="${round(plot.y)}"`,
    `data-plot-width="${round(plot.width)}"`,
    `data-plot-height="${round(plot.height)}"`,
  ];
  if (workSpreadsheetChartSupportsGrouping(chart.type)) {
    attributes.push(`data-chart-grouping="${normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type)}"`);
  }
  if (workSpreadsheetChartSupportsBarSpacing(chart.type)) {
    const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
    attributes.push(`data-chart-gap-width="${normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)}"`);
    attributes.push(`data-chart-overlap="${normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping)}"`);
  }
  if (workSpreadsheetChartSupportsSmoothLines(chart.type)) {
    attributes.push(`data-chart-smooth-lines="${normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines)}"`);
  }
  return attributes.join(' ');
}

function chartAxisTitlesSvg(
  axes: WorkSpreadsheetChartAxes,
  plot: { x: number; y: number; width: number; height: number }
): string {
  const bottom = chartAxisTitleText(axes.bottom?.title);
  const left = chartAxisTitleText(axes.left?.title);
  const top = chartAxisTitleText(axes.top?.title);
  const right = chartAxisTitleText(axes.right?.title);
  const font = `font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="11" font-weight="600"`;
  const bottomY = Math.min(HEIGHT - 7, plot.y + plot.height + 35);
  const leftX = Math.max(13, plot.x - 43);
  const rightX = plot.x + plot.width + 45;
  return [
    bottom
      ? `<text data-axis-title="bottom" x="${round(plot.x + plot.width / 2)}" y="${round(
          bottomY
        )}" fill="#3f4a60" ${font} text-anchor="middle">${escapeXml(bottom)}</text>`
      : '',
    left
      ? `<text data-axis-title="left" x="${round(leftX)}" y="${round(
          plot.y + plot.height / 2
        )}" fill="#3f4a60" ${font} text-anchor="middle" transform="rotate(-90 ${round(leftX)} ${round(
          plot.y + plot.height / 2
        )})">${escapeXml(left)}</text>`
      : '',
    top
      ? `<text data-axis-title="top" x="${round(plot.x + plot.width / 2)}" y="${round(
          plot.y - 7
        )}" fill="#3f4a60" ${font} text-anchor="middle">${escapeXml(top)}</text>`
      : '',
    right
      ? `<text data-axis-title="right" x="${round(rightX)}" y="${round(
          plot.y + plot.height / 2
        )}" fill="#3f4a60" ${font} text-anchor="middle" transform="rotate(90 ${round(rightX)} ${round(
          plot.y + plot.height / 2
        )})">${escapeXml(right)}</text>`
      : '',
  ].join('');
}

function chartAxisTitleText(title: string | undefined): string {
  return title?.trim() ? truncate(title.trim(), 54) : '';
}

function doughnutChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number }
): string {
  const hasValues = chart.series.some((series) => series.values.some((value) => Math.max(0, finiteNumber(value)) > 0));
  if (!hasValues) return emptyChartSvg(plot, '没有可绘制的数据');
  const centerX = plot.x + plot.width / 2;
  const centerY = plot.y + plot.height / 2;
  const radius = Math.max(24, Math.min(plot.width, plot.height) * 0.4);
  const holeSize = normalizeWorkSpreadsheetDoughnutHoleSize(chart.doughnutHoleSize);
  const holeRadius = radius * (holeSize / 100);
  const ringWidth = (radius - holeRadius) / Math.max(1, chart.series.length);
  const ringGap = Math.min(1, ringWidth * 0.18);
  const rings = chart.series
    .map((_, seriesIndex) => {
      const outerRadius = radius - seriesIndex * ringWidth;
      const innerRadius =
        holeRadius +
        (chart.series.length - seriesIndex - 1) * ringWidth +
        (seriesIndex < chart.series.length - 1 ? ringGap : 0);
      return `<g data-doughnut-series="${seriesIndex}">${doughnutRingSvg(
        chart,
        seriesIndex,
        centerX,
        centerY,
        innerRadius,
        outerRadius
      )}</g>`;
    })
    .join('');
  return `<g data-hole-size="${holeSize}">${rings}</g>`;
}

function doughnutRingSvg(
  chart: WorkSpreadsheetChart,
  seriesIndex: number,
  centerX: number,
  centerY: number,
  innerRadius: number,
  outerRadius: number
): string {
  const values = chart.series[seriesIndex]?.values ?? [];
  const usableValues = values.map((value) => Math.max(0, finiteNumber(value)));
  const total = usableValues.reduce((sum, value) => sum + value, 0);
  if (!total) return '';
  const positiveIndices = usableValues.flatMap((value, index) => (value > 0 ? [index] : []));
  if (positiveIndices.length === 1) {
    const index = positiveIndices[0];
    const middleRadius = (innerRadius + outerRadius) / 2;
    const fill = spreadsheetChartSeriesFillStyle(
      chart.series[seriesIndex],
      seriesIndex,
      1,
      CHART_COLORS[index % CHART_COLORS.length]
    );
    return `<circle cx="${round(centerX)}" cy="${round(centerY)}" r="${round(
      middleRadius
    )}" fill="none" stroke="${fill.color}" stroke-opacity="${fill.opacity}" stroke-width="${round(
      outerRadius - innerRadius
    )}" data-chart-slice="${seriesIndex}:${index}"/>${spreadsheetDataLabelSvg(chart, seriesIndex, index, {
      kind: 'circular',
      centerX,
      centerY,
      angle: 0,
      innerRadius,
      outerRadius,
    })}`;
  }
  let angle = -Math.PI / 2;
  return usableValues
    .map((value, index) => {
      const nextAngle = angle + (value / total) * Math.PI * 2;
      const middleAngle = angle + (nextAngle - angle) / 2;
      if (!value) {
        angle = nextAngle;
        return '';
      }
      const largeArc = nextAngle - angle > Math.PI ? 1 : 0;
      const outerStartX = centerX + Math.cos(angle) * outerRadius;
      const outerStartY = centerY + Math.sin(angle) * outerRadius;
      const outerEndX = centerX + Math.cos(nextAngle) * outerRadius;
      const outerEndY = centerY + Math.sin(nextAngle) * outerRadius;
      const innerEndX = centerX + Math.cos(nextAngle) * innerRadius;
      const innerEndY = centerY + Math.sin(nextAngle) * innerRadius;
      const innerStartX = centerX + Math.cos(angle) * innerRadius;
      const innerStartY = centerY + Math.sin(angle) * innerRadius;
      const path = [
        `M ${round(outerStartX)} ${round(outerStartY)}`,
        `A ${round(outerRadius)} ${round(outerRadius)} 0 ${largeArc} 1 ${round(outerEndX)} ${round(outerEndY)}`,
        `L ${round(innerEndX)} ${round(innerEndY)}`,
        `A ${round(innerRadius)} ${round(innerRadius)} 0 ${largeArc} 0 ${round(innerStartX)} ${round(innerStartY)}`,
        'Z',
      ].join(' ');
      angle = nextAngle;
      const fill = spreadsheetChartSeriesFillStyle(
        chart.series[seriesIndex],
        seriesIndex,
        1,
        CHART_COLORS[index % CHART_COLORS.length]
      );
      const line = spreadsheetChartSeriesLineStyle(chart.series[seriesIndex], seriesIndex, 1, '#FFFFFF');
      return `<path data-chart-slice="${seriesIndex}:${index}" d="${path}" ${fill.attributes} ${line.attributes}/>${spreadsheetDataLabelSvg(
        chart,
        seriesIndex,
        index,
        { kind: 'circular', centerX, centerY, angle: middleAngle, innerRadius, outerRadius }
      )}`;
    })
    .join('');
}

function radarChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number }
): string {
  const categoryCount = Math.max(0, chart.categories.length, ...chart.series.map((series) => series.values.length));
  if (categoryCount < 3) return emptyChartSvg(plot, '雷达图至少需要三个分类');
  const values = chart.series.flatMap((series) => series.values).map(finiteNumber);
  if (!values.length) return emptyChartSvg(plot, '没有可绘制的数据');
  const axis = chart.axes?.left;
  const categoryAxis = chart.axes?.bottom;
  const scale = spreadsheetChartAxisScale(values, axis, { includeZero: true });
  const centerX = plot.x + plot.width / 2;
  const centerY = plot.y + plot.height / 2;
  const radius = Math.max(24, Math.min(plot.width, plot.height) * 0.36);
  const point = (index: number, ratio: number) => {
    const visualIndex = spreadsheetChartCategoryVisualIndex(index, categoryCount, categoryAxis);
    const angle = -Math.PI / 2 + (visualIndex / categoryCount) * Math.PI * 2;
    return [centerX + Math.cos(angle) * radius * ratio, centerY + Math.sin(angle) * radius * ratio] as const;
  };
  const polygon = (ratio: number) =>
    Array.from({ length: categoryCount }, (_, index) => point(index, ratio).map(round).join(',')).join(' ');
  const showGridlines = spreadsheetChartAxisGridlinesVisible(axis, chart.type, 'left');
  const grid = scale.ticks
    .filter((tick) => tick > scale.minimum)
    .map((tick, index) => {
      const ratio = spreadsheetChartAxisValueRatio(tick, scale, axis);
      const labelVisible = (axis?.labelPosition ?? 'nextTo') !== 'none';
      const tickY = centerY - radius * ratio;
      return `${
        showGridlines ? `<polygon points="${polygon(ratio)}" fill="none" stroke="#dfe4ec" stroke-width="1"/>` : ''
      }${spreadsheetChartMajorTickSvg(axis, 'left', index, centerX, tickY)}${
        labelVisible
          ? `<text data-axis-tick="left" x="${round(centerX + 4)}" y="${round(
              tickY + 10
            )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="9" text-anchor="start">${escapeXml(
              formatSpreadsheetChartAxisNumber(tick, axis?.numberFormat)
            )}</text>`
          : ''
      }`;
    })
    .join('');
  const spokes = Array.from({ length: categoryCount }, (_, index) => {
    const [x, y] = point(index, 1);
    return `<line x1="${round(centerX)}" y1="${round(centerY)}" x2="${round(x)}" y2="${round(
      y
    )}" stroke="#e8ebf0" stroke-width="1"/>`;
  }).join('');
  const labels = Array.from({ length: categoryCount }, (_, index) => {
    const [x, y] = point(index, 1.14);
    const [tickX, tickY] = point(index, 1);
    const horizontal = x - centerX;
    const anchor = Math.abs(horizontal) < 8 ? 'middle' : horizontal > 0 ? 'start' : 'end';
    const visible =
      (categoryAxis?.labelPosition ?? 'nextTo') !== 'none' &&
      spreadsheetChartCategoryLabelVisible(index, categoryCount, categoryAxis);
    return `${spreadsheetChartMajorTickSvg(categoryAxis, 'bottom', index, tickX, tickY)}${
      visible
        ? `<text data-axis-category-label="${index}" x="${round(x)}" y="${round(
            y + 4
          )}" fill="#536078" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${anchor}">${escapeXml(
            truncate(chart.categories[index] ?? String(index + 1), 13)
          )}</text>`
        : ''
    }`;
  }).join('');
  const style = normalizeWorkSpreadsheetRadarStyle(chart.radarStyle);
  const series = chart.series
    .map((item, seriesIndex) => {
      const customStyle = normalizeWorkSpreadsheetChartSeriesStyle(item.style);
      const fill = spreadsheetChartSeriesFillStyle(
        item,
        seriesIndex,
        customStyle?.fillColor ? 1 : style === 'filled' ? 0.24 : 0
      );
      const line = spreadsheetChartSeriesLineStyle(item, seriesIndex, 2.2);
      const points = Array.from({ length: categoryCount }, (_, index) => {
        const value = finiteNumber(item.values[index] ?? 0);
        return point(index, spreadsheetChartAxisValueRatio(value, scale, axis));
      });
      const pointList = points.map(([x, y]) => `${round(x)},${round(y)}`).join(' ');
      const markers = points
        .map(([x, y]) =>
          spreadsheetChartSeriesMarkerSvg(item, seriesIndex, x, y, {
            visible: style === 'marker',
            defaultSize: 5,
          })
        )
        .join('');
      const dataLabels = points
        .map(([x, y], pointIndex) => spreadsheetDataLabelSvg(chart, seriesIndex, pointIndex, { kind: 'point', x, y }))
        .join('');
      return `<g data-radar-series="${seriesIndex}"><polygon points="${pointList}" ${fill.attributes} ${line.attributes} stroke-linejoin="round"/>${markers}${dataLabels}</g>`;
    })
    .join('');
  return `<g data-radar-style="${style}"><g ${spreadsheetChartAxisScaleAttributes(
    axis,
    scale,
    chart.type,
    'left'
  )}>${grid}</g>${spokes}${series}<g ${spreadsheetChartAxisDisplayAttributes(
    categoryAxis,
    chart.type,
    'bottom'
  )}>${labels}</g></g>`;
}

function pieChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number }
): string {
  const values = chart.series[0]?.values ?? [];
  const usableValues = values.map((value) => Math.max(0, finiteNumber(value)));
  const total = usableValues.reduce((sum, value) => sum + value, 0);
  if (!total) return emptyChartSvg(plot, '没有可绘制的数据');
  const centerX = plot.x + plot.width / 2;
  const centerY = plot.y + plot.height / 2;
  const radius = Math.max(24, Math.min(plot.width, plot.height) * 0.38);
  let angle = -Math.PI / 2;
  return usableValues
    .map((value, index) => {
      const nextAngle = angle + (value / total) * Math.PI * 2;
      const middleAngle = angle + (nextAngle - angle) / 2;
      const largeArc = nextAngle - angle > Math.PI ? 1 : 0;
      const startX = centerX + Math.cos(angle) * radius;
      const startY = centerY + Math.sin(angle) * radius;
      const endX = centerX + Math.cos(nextAngle) * radius;
      const endY = centerY + Math.sin(nextAngle) * radius;
      const series = chart.series[0];
      const fill = spreadsheetChartSeriesFillStyle(series, 0, 1, CHART_COLORS[index % CHART_COLORS.length]);
      const line = spreadsheetChartSeriesLineStyle(series, 0, 0, '#FFFFFF');
      const path =
        usableValues.length === 1
          ? `<circle data-chart-slice="0:${index}" cx="${round(centerX)}" cy="${round(centerY)}" r="${round(
              radius
            )}" ${fill.attributes} ${line.attributes}/>`
          : `<path data-chart-slice="0:${index}" d="M ${round(centerX)} ${round(centerY)} L ${round(startX)} ${round(
              startY
            )} A ${round(radius)} ${round(radius)} 0 ${largeArc} 1 ${round(endX)} ${round(
              endY
            )} Z" ${fill.attributes} ${line.attributes}/>`;
      angle = nextAngle;
      return `${path}${spreadsheetDataLabelSvg(chart, 0, index, {
        kind: 'circular',
        centerX,
        centerY,
        angle: middleAngle,
        innerRadius: 0,
        outerRadius: radius,
      })}`;
    })
    .join('');
}

interface SpreadsheetXyPoint {
  x: number;
  y: number;
  sourceIndex: number;
  bubbleSize?: number;
}

function xyChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number }
): string {
  const showNegativeBubbles = chart.showNegativeBubbles === true;
  const seriesPoints = chart.series.map((series) =>
    spreadsheetXySeriesPoints(series, chart.type === 'bubble', showNegativeBubbles)
  );
  const points = seriesPoints.flat();
  if (!points.length) {
    return emptyChartSvg(
      plot,
      chart.type === 'bubble' ? '气泡图需要 X、Y 和非零气泡大小数据' : '散点图需要 X 和 Y 数值'
    );
  }
  const trendlinePoints = chart.series.flatMap((series) =>
    spreadsheetSeriesTrendlineFits(series, spreadsheetTrendlineXValues(series)).flatMap((item) => item.fit.points)
  );
  const scaledPoints = [...points, ...trendlinePoints];
  const paddingRatio = chart.type === 'bubble' ? 0.18 : 0.06;
  const xBounds = spreadsheetChartAxisScale(
    [
      ...scaledPoints.map((point) => point.x),
      ...chart.series.flatMap((series) => spreadsheetSeriesErrorBarBounds(series, chart.type, 'x')),
    ],
    chart.axes?.bottom,
    { paddingRatio }
  );
  const yBounds = spreadsheetChartAxisScale(
    [
      ...scaledPoints.map((point) => point.y),
      ...chart.series.flatMap((series) => spreadsheetSeriesErrorBarBounds(series, chart.type, 'y')),
    ],
    chart.axes?.left,
    { paddingRatio }
  );
  const pointX = (value: number) =>
    plot.x + spreadsheetChartAxisValueRatio(value, xBounds, chart.axes?.bottom) * plot.width;
  const pointY = (value: number) =>
    plot.y + (1 - spreadsheetChartAxisValueRatio(value, yBounds, chart.axes?.left)) * plot.height;
  const axes = spreadsheetXyChartAxesSvg(chart, plot, xBounds, yBounds);
  const errorBars = chart.series
    .map((series, seriesIndex) =>
      spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (direction, pointIndex, value) => {
        const x = series.xValues?.[pointIndex] ?? pointIndex + 1;
        const y = series.values[pointIndex] ?? 0;
        return direction === 'x' ? [pointX(value), pointY(y)] : [pointX(x), pointY(value)];
      })
    )
    .join('');
  const trendlines = chart.series
    .map((series, seriesIndex) =>
      spreadsheetSeriesTrendlinesSvg(series, seriesIndex, spreadsheetTrendlineXValues(series), (x, y) => [
        pointX(x),
        pointY(y),
      ])
    )
    .join('');
  if (chart.type === 'bubble') {
    return `${axes}${errorBars}${bubbleSeriesSvg(chart, seriesPoints, pointX, pointY)}${trendlines}`;
  }
  return `${axes}${errorBars}${scatterSeriesSvg(chart, seriesPoints, pointX, pointY)}${trendlines}`;
}

function spreadsheetTrendlineXValues(series: WorkSpreadsheetChartSeries): number[] {
  return series.values.map((_, index) => series.xValues?.[index] ?? index + 1);
}

function spreadsheetXySeriesPoints(
  series: WorkSpreadsheetChartSeries,
  requireBubbleSize: boolean,
  showNegativeBubbles: boolean
): SpreadsheetXyPoint[] {
  return series.values.flatMap((rawY, sourceIndex) => {
    const rawX = series.xValues?.[sourceIndex] ?? sourceIndex + 1;
    if (!Number.isFinite(rawX) || !Number.isFinite(rawY)) return [];
    if (!requireBubbleSize) return [{ x: rawX, y: rawY, sourceIndex }];
    const bubbleSize = series.bubbleSizes?.[sourceIndex];
    if (
      typeof bubbleSize !== 'number' ||
      !Number.isFinite(bubbleSize) ||
      bubbleSize === 0 ||
      (!showNegativeBubbles && bubbleSize < 0)
    ) {
      return [];
    }
    return [{ x: rawX, y: rawY, sourceIndex, bubbleSize }];
  });
}

function scatterSeriesSvg(
  chart: WorkSpreadsheetChart,
  seriesPoints: SpreadsheetXyPoint[][],
  pointX: (value: number) => number,
  pointY: (value: number) => number
): string {
  const style = normalizeWorkSpreadsheetScatterStyle(chart.scatterStyle);
  const showLine = style !== 'marker';
  const showMarkers = style === 'marker' || style === 'lineMarker' || style === 'smoothMarker';
  const smooth = style === 'smooth' || style === 'smoothMarker';
  const series = seriesPoints
    .map((points, seriesIndex) => {
      const sourceSeries = chart.series[seriesIndex];
      const lineStyle = spreadsheetChartSeriesLineStyle(sourceSeries, seriesIndex, 2.3);
      const positioned = points.map((point) => ({
        ...point,
        plotX: pointX(point.x),
        plotY: pointY(point.y),
      }));
      const line =
        showLine && positioned.length > 1
          ? `<path data-scatter-line="${seriesIndex}" d="${scatterPath(
              positioned,
              smooth
            )}" fill="none" ${lineStyle.attributes} stroke-linejoin="round" stroke-linecap="round"/>`
          : '';
      const markers = positioned
        .map((point) =>
          spreadsheetChartSeriesMarkerSvg(sourceSeries, seriesIndex, point.plotX, point.plotY, {
            visible: showMarkers,
            defaultSize: 5.67,
            attributes: `data-scatter-marker="${seriesIndex}:${point.sourceIndex}" data-point-x="${round(
              point.x
            )}" data-point-y="${round(point.y)}"`,
          })
        )
        .join('');
      const dataLabels = positioned
        .map((point) =>
          spreadsheetDataLabelSvg(chart, seriesIndex, point.sourceIndex, {
            kind: 'point',
            x: point.plotX,
            y: point.plotY,
          })
        )
        .join('');
      return `<g data-scatter-series="${seriesIndex}">${line}${markers}${dataLabels}</g>`;
    })
    .join('');
  return `<g data-scatter-style="${style}">${series}</g>`;
}

function scatterPath(points: Array<SpreadsheetXyPoint & { plotX: number; plotY: number }>, smooth: boolean): string {
  if (!points.length) return '';
  if (!smooth || points.length < 3) {
    return points.map((point, index) => `${index ? 'L' : 'M'} ${round(point.plotX)} ${round(point.plotY)}`).join(' ');
  }
  const commands = [`M ${round(points[0].plotX)} ${round(points[0].plotY)}`];
  for (let index = 0; index < points.length - 1; index += 1) {
    const previous = points[index - 1] ?? points[index];
    const current = points[index];
    const next = points[index + 1];
    const following = points[index + 2] ?? next;
    const control1X = current.plotX + (next.plotX - previous.plotX) / 6;
    const control1Y = current.plotY + (next.plotY - previous.plotY) / 6;
    const control2X = next.plotX - (following.plotX - current.plotX) / 6;
    const control2Y = next.plotY - (following.plotY - current.plotY) / 6;
    commands.push(
      `C ${round(control1X)} ${round(control1Y)} ${round(control2X)} ${round(control2Y)} ${round(
        next.plotX
      )} ${round(next.plotY)}`
    );
  }
  return commands.join(' ');
}

function bubbleSeriesSvg(
  chart: WorkSpreadsheetChart,
  seriesPoints: SpreadsheetXyPoint[][],
  pointX: (value: number) => number,
  pointY: (value: number) => number
): string {
  const scale = normalizeWorkSpreadsheetBubbleScale(chart.bubbleScale);
  const sizeRepresents = normalizeWorkSpreadsheetBubbleSizeRepresents(chart.bubbleSizeRepresents);
  const maximumSize = Math.max(
    1,
    ...seriesPoints.flatMap((points) => points.map((point) => Math.abs(point.bubbleSize ?? 0)))
  );
  const maximumRadius = Math.max(0, Math.min(54, 18 * (scale / 100)));
  const series = seriesPoints
    .map((points, seriesIndex) => {
      const sourceSeries = chart.series[seriesIndex];
      const sourceStyle = normalizeWorkSpreadsheetChartSeriesStyle(sourceSeries.style);
      const fill = spreadsheetChartSeriesFillStyle(sourceSeries, seriesIndex, 0.34);
      const line = spreadsheetChartSeriesLineStyle(sourceSeries, seriesIndex, 1.8);
      const bubbles = points
        .map((point) => {
          const size = point.bubbleSize ?? 0;
          const ratio = Math.abs(size) / maximumSize;
          const radius = maximumRadius * (sizeRepresents === 'area' ? Math.sqrt(ratio) : ratio);
          const negative = size < 0;
          const bubbleOpacity = sourceStyle?.fillTransparency === undefined ? (negative ? 0.2 : 0.34) : fill.opacity;
          return `<circle data-bubble-point="${seriesIndex}:${point.sourceIndex}" data-point-x="${round(
            point.x
          )}" data-point-y="${round(point.y)}" data-bubble-size="${round(
            size
          )}" data-negative-bubble="${negative}" cx="${round(pointX(point.x))}" cy="${round(
            pointY(point.y)
          )}" r="${round(radius)}" fill="${fill.color}" fill-opacity="${bubbleOpacity}" ${line.attributes}${
            negative && line.dash === 'solid' ? ' stroke-dasharray="3 2"' : ''
          }/>${spreadsheetDataLabelSvg(chart, seriesIndex, point.sourceIndex, {
            kind: 'point',
            x: pointX(point.x),
            y: pointY(point.y) - radius,
          })}`;
        })
        .join('');
      return `<g data-bubble-series="${seriesIndex}">${bubbles}</g>`;
    })
    .join('');
  return `<g data-bubble-scale="${scale}" data-show-negative-bubbles="${
    chart.showNegativeBubbles === true
  }" data-bubble-size-represents="${sizeRepresents}">${series}</g>`;
}

function cartesianChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number }
): string {
  const layout = spreadsheetChartSeriesLayout(chart);
  const trendlineValues = layout.stacked
    ? []
    : chart.series.flatMap((series) =>
        spreadsheetSeriesTrendlineFits(series, categoryTrendlineXValues(series)).flatMap((item) =>
          item.fit.points.map((point) => point.y)
        )
      );
  const values = [
    ...layout.scaleValues,
    ...(layout.stacked
      ? []
      : chart.series.flatMap((series) => spreadsheetSeriesErrorBarBounds(series, chart.type, 'y'))),
    ...trendlineValues,
  ];
  if (!values.length) return emptyChartSvg(plot, '没有可绘制的数据');
  if (chart.type === 'bar') return horizontalBarChartSvg(chart, plot, layout, values);
  const axis = chart.axes?.left;
  const scale = spreadsheetChartAxisScale(values, axis, { includeZero: true });
  const valuePosition = (value: number) =>
    plot.y + (1 - spreadsheetChartAxisValueRatio(value, scale, axis)) * plot.height;
  const baseline = valuePosition(0);
  const numberFormat = axis?.numberFormat ?? (layout.grouping === 'percentStacked' ? '0%' : undefined);
  const showGridlines = spreadsheetChartAxisGridlinesVisible(axis, chart.type, 'left');
  const grid = scale.ticks
    .map((value, index) => {
      const y = valuePosition(value);
      const label = spreadsheetChartAxisLabelLayout(axis, chart.type, 'left', plot, plot.x, y);
      return `${
        showGridlines
          ? `<line x1="${plot.x}" y1="${round(y)}" x2="${round(plot.x + plot.width)}" y2="${round(
              y
            )}" stroke="#e8ebf0" stroke-width="1"/>`
          : ''
      }${spreadsheetChartMajorTickSvg(axis, 'left', index, plot.x, y)}${
        label
          ? `<text data-axis-tick="left" x="${round(label.x)}" y="${round(
              label.y
            )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${label.textAnchor}">${escapeXml(
              formatSpreadsheetChartAxisNumber(value, numberFormat)
            )}</text>`
          : ''
      }`;
    })
    .join('');
  const axes = [
    `<g ${spreadsheetChartAxisScaleAttributes(axis, scale, chart.type, 'left')}>${grid}<line x1="${
      plot.x
    }" y1="${plot.y}" x2="${plot.x}" y2="${round(plot.y + plot.height)}" stroke="#aeb6c4" stroke-width="1.2"/></g>`,
    `<g ${spreadsheetChartAxisDisplayAttributes(
      chart.axes?.bottom,
      chart.type,
      'bottom'
    )}><line x1="${plot.x}" y1="${round(baseline)}" x2="${round(
      plot.x + plot.width
    )}" y2="${round(baseline)}" stroke="#aeb6c4" stroke-width="1.2"/></g>`,
  ].join('');
  const trendlines = layout.stacked ? '' : cartesianTrendlinesSvg(chart, plot, valuePosition);
  if (chart.type === 'line' || chart.type === 'area') {
    return `${axes}${spreadsheetLineChartSvg(chart, plot, valuePosition, layout)}${trendlines}${categoryLabelsSvg(
      chart,
      plot,
      baseline
    )}`;
  }
  return `${axes}${barChartSvg(chart, plot, valuePosition, baseline, layout)}${trendlines}${categoryLabelsSvg(
    chart,
    plot,
    baseline
  )}`;
}

function cartesianTrendlinesSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number },
  valuePosition: (value: number) => number
): string {
  const categoryCount = Math.max(1, chart.categories.length, ...chart.series.map((series) => series.values.length));
  const categoryAxis = chart.axes?.bottom;
  const categoryPosition = (value: number) =>
    categoryCount === 1
      ? plot.x + plot.width / 2
      : plot.x +
        (spreadsheetChartCategoryVisualIndex(value - 1, categoryCount, categoryAxis) / Math.max(1, categoryCount - 1)) *
          plot.width;
  return chart.series
    .map((series, seriesIndex) =>
      spreadsheetSeriesTrendlinesSvg(series, seriesIndex, categoryTrendlineXValues(series), (x, y) => [
        categoryPosition(x),
        valuePosition(y),
      ])
    )
    .join('');
}

function categoryTrendlineXValues(series: WorkSpreadsheetChartSeries): number[] {
  return series.values.map((_, index) => index + 1);
}

function barChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number },
  valuePosition: (value: number) => number,
  baseline: number,
  layout: SpreadsheetChartSeriesLayout
): string {
  const categoryCount = layout.categoryCount;
  const seriesCount = Math.max(1, chart.series.length);
  const groupWidth = plot.width / categoryCount;
  const categoryAxis = chart.axes?.bottom;
  const geometry = spreadsheetChartBarGeometry(groupWidth, seriesCount, chart, 44);
  return layout.series
    .map((points, seriesIndex) => {
      const seriesX = (pointIndex: number) =>
        plot.x +
        spreadsheetChartCategoryVisualIndex(pointIndex, categoryCount, categoryAxis) * groupWidth +
        geometry.offset(seriesIndex);
      const renderedWidth = geometry.renderedSize;
      const bars = points
        .map((point) => {
          const startY = layout.stacked ? valuePosition(point.start) : baseline;
          const endY = valuePosition(point.end);
          const x = seriesX(point.categoryIndex);
          return `<rect data-chart-bar="${seriesIndex}:${point.categoryIndex}" data-stack-start="${round(
            point.start
          )}" data-stack-end="${round(point.end)}" x="${round(x)}" y="${round(
            Math.min(endY, startY)
          )}" width="${round(renderedWidth)}" height="${round(
            Math.max(1, Math.abs(endY - startY))
          )}" rx="1.5" ${spreadsheetChartSeriesFillStyle(chart.series[seriesIndex], seriesIndex).attributes} ${
            spreadsheetChartSeriesLineStyle(chart.series[seriesIndex], seriesIndex, 0).attributes
          }/>${spreadsheetDataLabelSvg(chart, seriesIndex, point.categoryIndex, {
            kind: 'verticalBar',
            x: x + renderedWidth / 2,
            valueY: endY,
            baselineY: startY,
            value: point.rawValue,
          })}`;
        })
        .join('');
      const errorBars = layout.stacked
        ? ''
        : spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (_direction, pointIndex, value) => [
            seriesX(pointIndex) + renderedWidth / 2,
            valuePosition(value),
          ]);
      return `${bars}${errorBars}`;
    })
    .join('');
}

function horizontalBarChartSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number },
  layout: SpreadsheetChartSeriesLayout,
  values: number[]
): string {
  const axis = chart.axes?.bottom;
  const scale = spreadsheetChartAxisScale(values, axis, { includeZero: true });
  const valuePosition = (value: number) => plot.x + spreadsheetChartAxisValueRatio(value, scale, axis) * plot.width;
  const zero = valuePosition(0);
  const categoryCount = layout.categoryCount;
  const seriesCount = Math.max(1, chart.series.length);
  const groupHeight = plot.height / categoryCount;
  const geometry = spreadsheetChartBarGeometry(groupHeight, seriesCount, chart, 30);
  const categoryAxis = chart.axes?.left;
  const numberFormat = axis?.numberFormat ?? (layout.grouping === 'percentStacked' ? '0%' : undefined);
  const showGridlines = spreadsheetChartAxisGridlinesVisible(axis, chart.type, 'bottom');
  const grid = scale.ticks
    .map((value, index) => {
      const x = valuePosition(value);
      const label = spreadsheetChartAxisLabelLayout(axis, chart.type, 'bottom', plot, x, plot.y + plot.height);
      return `${
        showGridlines
          ? `<line x1="${round(x)}" y1="${plot.y}" x2="${round(x)}" y2="${round(
              plot.y + plot.height
            )}" stroke="#e8ebf0" stroke-width="1"/>`
          : ''
      }${spreadsheetChartMajorTickSvg(axis, 'bottom', index, x, plot.y + plot.height)}${
        label
          ? `<text data-axis-tick="bottom" x="${round(label.x)}" y="${round(
              label.y
            )}" fill="#717b8f" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${label.textAnchor}">${escapeXml(
              formatSpreadsheetChartAxisNumber(value, numberFormat)
            )}</text>`
          : ''
      }`;
    })
    .join('');
  const labels = Array.from({ length: categoryCount }, (_, index) => {
    const visible = spreadsheetChartCategoryLabelVisible(index, categoryCount, categoryAxis);
    const label = chart.categories[index] ?? String(index + 1);
    const visualIndex = spreadsheetChartCategoryVisualIndex(index, categoryCount, categoryAxis);
    const y = plot.y + (visualIndex + 0.5) * groupHeight;
    const labelLayout = spreadsheetChartAxisLabelLayout(categoryAxis, chart.type, 'left', plot, zero, y);
    return `${spreadsheetChartMajorTickSvg(categoryAxis, 'left', index, zero, y)}${
      visible && labelLayout
        ? `<text data-axis-category-label="${index}" x="${round(labelLayout.x)}" y="${round(
            labelLayout.y
          )}" fill="#536078" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${labelLayout.textAnchor}">${escapeXml(
            truncate(label, 11)
          )}</text>`
        : ''
    }`;
  }).join('');
  const bars = layout.series
    .map((points, seriesIndex) => {
      const seriesY = (pointIndex: number) =>
        plot.y +
        spreadsheetChartCategoryVisualIndex(pointIndex, categoryCount, categoryAxis) * groupHeight +
        geometry.offset(seriesIndex);
      const renderedHeight = geometry.renderedSize;
      const fill = spreadsheetChartSeriesFillStyle(chart.series[seriesIndex], seriesIndex);
      const line = spreadsheetChartSeriesLineStyle(chart.series[seriesIndex], seriesIndex, 0);
      const seriesBars = points
        .map((point) => {
          const startX = layout.stacked ? valuePosition(point.start) : zero;
          const endX = valuePosition(point.end);
          const y = seriesY(point.categoryIndex);
          return `<rect data-chart-bar="${seriesIndex}:${point.categoryIndex}" data-stack-start="${round(
            point.start
          )}" data-stack-end="${round(point.end)}" x="${round(Math.min(endX, startX))}" y="${round(y)}" width="${round(
            Math.max(1, Math.abs(endX - startX))
          )}" height="${round(renderedHeight)}" rx="1.5" ${fill.attributes} ${line.attributes}/>${spreadsheetDataLabelSvg(
            chart,
            seriesIndex,
            point.categoryIndex,
            {
              kind: 'horizontalBar',
              y: y + renderedHeight / 2,
              valueX: endX,
              baselineX: startX,
              value: point.rawValue,
            }
          )}`;
        })
        .join('');
      const errorBars = layout.stacked
        ? ''
        : spreadsheetSeriesErrorBarsSvg(chart, seriesIndex, (_direction, pointIndex, value) => [
            valuePosition(value),
            seriesY(pointIndex) + renderedHeight / 2,
          ]);
      return `${seriesBars}${errorBars}`;
    })
    .join('');
  const trendlines = layout.stacked
    ? ''
    : chart.series
        .map((series, seriesIndex) =>
          spreadsheetSeriesTrendlinesSvg(series, seriesIndex, categoryTrendlineXValues(series), (category, value) => [
            valuePosition(value),
            plot.y +
              ((spreadsheetChartCategoryVisualIndex(category - 1, categoryCount, categoryAxis) + 0.5) / categoryCount) *
                plot.height,
          ])
        )
        .join('');
  return `<g ${spreadsheetChartAxisScaleAttributes(axis, scale, chart.type, 'bottom')}>${grid}<line x1="${round(
    zero
  )}" y1="${plot.y}" x2="${round(zero)}" y2="${round(
    plot.y + plot.height
  )}" stroke="#aeb6c4" stroke-width="1.2"/></g><g ${spreadsheetChartAxisDisplayAttributes(
    categoryAxis,
    chart.type,
    'left'
  )}>${labels}</g>${bars}${trendlines}`;
}

function categoryLabelsSvg(
  chart: WorkSpreadsheetChart,
  plot: { x: number; y: number; width: number; height: number },
  axisY: number
): string {
  const categoryCount = Math.max(1, chart.categories.length, ...chart.series.map((series) => series.values.length));
  const axis = chart.axes?.bottom;
  const labels = Array.from({ length: categoryCount }, (_, index) => {
    const visible = spreadsheetChartCategoryLabelVisible(index, categoryCount, axis);
    const label = chart.categories[index] ?? String(index + 1);
    const visualIndex = spreadsheetChartCategoryVisualIndex(index, categoryCount, axis);
    const x = plot.x + ((visualIndex + 0.5) / categoryCount) * plot.width;
    const labelLayout = spreadsheetChartAxisLabelLayout(axis, chart.type, 'bottom', plot, x, axisY);
    return `${spreadsheetChartMajorTickSvg(axis, 'bottom', index, x, axisY)}${
      visible && labelLayout
        ? `<text data-axis-category-label="${index}" x="${round(labelLayout.x)}" y="${round(
            labelLayout.y
          )}" fill="#536078" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="10" text-anchor="${labelLayout.textAnchor}">${escapeXml(
            truncate(label, 13)
          )}</text>`
        : ''
    }`;
  }).join('');
  return `<g ${spreadsheetChartAxisDisplayAttributes(axis, chart.type, 'bottom')}>${labels}</g>`;
}

function emptyChartSvg(plot: { x: number; y: number; width: number; height: number }, message: string): string {
  return `<text x="${round(plot.x + plot.width / 2)}" y="${round(
    plot.y + plot.height / 2
  )}" fill="#8791a3" font-family="-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif" font-size="13" text-anchor="middle">${escapeXml(
    message
  )}</text>`;
}
