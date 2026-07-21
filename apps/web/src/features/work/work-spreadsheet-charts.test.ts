import type { Selection } from '@fortune-sheet/core';
import { describe, expect, it } from 'vitest';
import {
  createSpreadsheetChartFromSelection,
  reconcileSpreadsheetChartPreviews,
  resolveSpreadsheetChart,
  spreadsheetSheetsWithChartPreviews,
} from './work-spreadsheet-charts';
import { spreadsheetChartSvg } from './work-spreadsheet-chart-svg';
import type { WorkSpreadsheetChart, WorkSpreadsheetContent } from './work-types';

describe('Work spreadsheet chart model', () => {
  it('creates a live-reference chart from the selected table', () => {
    const content = spreadsheetContent();
    const selection: Selection = { row: [0, 2], column: [0, 2] };
    const chart = createSpreadsheetChartFromSelection(content, 'sheet-1', selection);

    expect(chart).toMatchObject({
      type: 'column',
      categories: ['Q1', 'Q2'],
      categoryReference: 'Report!$A$2:$A$3',
      series: [
        {
          name: 'Revenue',
          nameReference: 'Report!$B$1',
          values: [42, 55],
          valuesReference: 'Report!$B$2:$B$3',
        },
        {
          name: 'Margin',
          nameReference: 'Report!$C$1',
          values: [10, 12],
          valuesReference: 'Report!$C$2:$C$3',
        },
      ],
      showLegend: true,
      left: 304,
      top: 0,
      width: 480,
      height: 288,
    });
  });

  it('refreshes referenced chart values and reconciles move, resize, and deletion from FortuneSheet previews', () => {
    const content = spreadsheetContent();
    const chart = createSpreadsheetChartFromSelection(content, 'sheet-1', {
      row: [0, 2],
      column: [0, 1],
    });
    expect(chart).not.toBeNull();
    if (!chart) return;
    chart.axes = {
      bottom: { title: 'Stale quarter', titleReference: 'Report!$A$1' },
      left: { title: 'Stale revenue', titleReference: 'Report!$B$1' },
    };
    content.sheets[0].charts = [chart];
    const cell = content.sheets[0].data?.[1]?.[1];
    if (cell) {
      cell.v = 99;
      cell.m = '99';
    }

    const resolved = resolveSpreadsheetChart(content, content.sheets[0], chart);
    expect(resolved.series[0].values).toEqual([99, 55]);
    expect(resolved.axes).toEqual({
      bottom: { title: 'Quarter', titleReference: 'Report!$A$1' },
      left: { title: 'Revenue', titleReference: 'Report!$B$1' },
    });
    const previewSheets = spreadsheetSheetsWithChartPreviews(content);
    const preview = previewSheets[0].images?.find((image) => image.id.startsWith('work-chart-preview-'));
    expect(preview?.src).toMatch(/^data:image\/svg\+xml/);
    expect(preview?.altText).toContain('图表');
    if (!preview) return;
    preview.left = 192;
    preview.top = 48;
    preview.width = 384;
    preview.height = 240;

    const moved = reconcileSpreadsheetChartPreviews(content, previewSheets);
    expect(moved.sheets[0].charts?.[0]).toMatchObject({
      left: 192,
      top: 48,
      width: 384,
      height: 240,
    });
    expect(moved.sheets[0].images).toBeUndefined();

    const renamed = reconcileSpreadsheetChartPreviews(content, [
      {
        ...previewSheets[0],
        name: 'Forecast',
      },
    ]);
    expect(renamed.sheets[0].charts?.[0]).toMatchObject({
      categoryReference: 'Forecast!$A$2:$A$3',
      axes: {
        bottom: { titleReference: 'Forecast!$A$1' },
        left: { titleReference: 'Forecast!$B$1' },
      },
      series: [
        expect.objectContaining({
          nameReference: 'Forecast!$B$1',
          valuesReference: 'Forecast!$B$2:$B$3',
        }),
      ],
    });

    const deleted = reconcileSpreadsheetChartPreviews(content, [
      {
        ...previewSheets[0],
        images: [],
      },
    ]);
    expect(deleted.sheets[0].charts).toBeUndefined();
  });

  it('renders editable doughnut holes and filled radar series in shared chart previews', () => {
    const base: WorkSpreadsheetChart = {
      id: 'chart-advanced',
      name: 'Advanced chart',
      title: 'Regional mix',
      type: 'column',
      categories: ['North', 'South', 'West'],
      series: [
        { name: 'Revenue', values: [40, 35, 25] },
        { name: 'Margin', values: [22, 28, 31] },
      ],
      showLegend: true,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };
    const doughnut = spreadsheetChartSvg({
      ...base,
      type: 'doughnut',
      doughnutHoleSize: 64,
    });
    const radar = spreadsheetChartSvg({
      ...base,
      type: 'radar',
      radarStyle: 'filled',
      axes: {
        left: { minimum: 0, maximum: 50, majorUnit: 10, showMajorGridlines: false, numberFormat: '0' },
      },
    });

    expect(doughnut).toContain('data-chart-type="doughnut"');
    expect(doughnut).toContain('data-hole-size="64"');
    expect(doughnut).not.toContain('没有可绘制的数据');
    expect(radar).toContain('data-chart-type="radar"');
    expect(radar).toContain('data-radar-style="filled"');
    expect(radar).toContain('data-radar-series="1"');
    expect(radar).toContain('data-axis-scale="left"');
    expect(radar).toContain('data-axis-maximum="50"');
    expect(radar).toContain('data-axis-major-unit="10"');
    expect(radar).toContain('data-axis-gridlines="false"');
  });

  it('renders scatter subtypes and scaled positive or negative bubbles from independent X values', () => {
    const base: WorkSpreadsheetChart = {
      id: 'chart-xy',
      name: 'XY chart',
      title: 'Response curve',
      type: 'scatter',
      scatterStyle: 'smoothMarker',
      axes: {
        bottom: { minimum: 0, maximum: 4, majorUnit: 1, showMajorGridlines: true, numberFormat: '0.0' },
        left: { minimum: 0, maximum: 10, majorUnit: 2, showMajorGridlines: false, numberFormat: '0.0' },
      },
      categories: [],
      series: [
        {
          name: 'Observed',
          xValues: [1, 2, 4],
          values: [3, 8, 5],
        },
      ],
      showLegend: true,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };
    const scatter = spreadsheetChartSvg(base);
    const positiveBubbles = spreadsheetChartSvg({
      ...base,
      type: 'bubble',
      bubbleScale: 140,
      showNegativeBubbles: false,
      bubbleSizeRepresents: 'area',
      series: [{ ...base.series[0], bubbleSizes: [9, -16, 25] }],
    });
    const allBubbles = spreadsheetChartSvg({
      ...base,
      type: 'bubble',
      bubbleScale: 140,
      showNegativeBubbles: true,
      bubbleSizeRepresents: 'width',
      series: [{ ...base.series[0], bubbleSizes: [9, -16, 25] }],
    });

    expect(scatter).toContain('data-chart-type="scatter"');
    expect(scatter).toContain('data-scatter-style="smoothMarker"');
    expect(scatter).toContain('data-scatter-series="0"');
    expect(scatter).toContain('data-scatter-marker="0:2"');
    expect(scatter).toContain('data-axis-scale="bottom"');
    expect(scatter).toContain('data-axis-major-unit="1"');
    expect(scatter).toContain('data-axis-gridlines="true"');
    expect(scatter).toContain('data-axis-scale="left"');
    expect(scatter).toContain('data-axis-major-unit="2"');
    expect(scatter).toContain('data-axis-gridlines="false"');
    expect(scatter).toContain('>10.0</text>');
    expect(positiveBubbles).toContain('data-chart-type="bubble"');
    expect(positiveBubbles).toContain('data-bubble-scale="140"');
    expect(positiveBubbles).toContain('data-bubble-size-represents="area"');
    expect(positiveBubbles).toContain('data-bubble-size="25"');
    expect(positiveBubbles).not.toContain('data-bubble-size="-16"');
    expect(allBubbles).toContain('data-bubble-size-represents="width"');
    expect(allBubbles).toContain('data-bubble-size="-16"');
    expect(allBubbles).toContain('data-negative-bubble="true"');
  });

  it('renders column, line, and area combination series against independent primary and secondary axes', () => {
    const chart: WorkSpreadsheetChart = {
      id: 'chart-combination',
      name: 'Revenue and margin',
      type: 'combination',
      title: 'Revenue and margin',
      axes: {
        bottom: { title: 'Quarter <period>' },
        left: { title: 'Revenue' },
        top: { title: 'Secondary period' },
        right: { title: 'Margin %' },
      },
      categories: ['Q1', 'Q2', 'Q3'],
      series: [
        { name: 'Revenue', values: [42, 55, 61], chartType: 'column', axisGroup: 'primary' },
        { name: 'Margin', values: [0.12, 0.18, 0.2], chartType: 'line', axisGroup: 'secondary' },
        { name: 'Forecast', values: [40, 53, 64], chartType: 'area', axisGroup: 'primary' },
      ],
      showLegend: true,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };

    const svg = spreadsheetChartSvg(chart);

    expect(svg).toContain('data-chart-type="combination"');
    expect(svg).toContain('data-combination-chart="true"');
    expect(svg).toContain('data-combination-series="0"');
    expect(svg).toContain('data-series-chart-type="column"');
    expect(svg).toContain('data-combination-series="1"');
    expect(svg).toContain('data-series-chart-type="line"');
    expect(svg).toContain('data-axis-group="secondary"');
    expect(svg).toContain('data-series-chart-type="area"');
    expect(svg).toContain('data-secondary-axis="true"');
    expect(svg).toContain('data-axis-title="bottom"');
    expect(svg).toContain('Quarter &lt;period&gt;');
    expect(svg).toContain('data-axis-title="left"');
    expect(svg).toContain('data-axis-title="top"');
    expect(svg).toContain('data-axis-title="right"');
  });

  it('renders editable value-axis ranges, units, gridlines, and number formats', () => {
    const chart: WorkSpreadsheetChart = {
      id: 'chart-axis-settings',
      name: 'Axis settings',
      type: 'combination',
      axes: {
        left: {
          minimum: 0,
          maximum: 100,
          majorUnit: 25,
          showMajorGridlines: false,
          numberFormat: '#,##0',
          numberFormatSourceLinked: false,
        },
        right: {
          minimum: 0,
          maximum: 0.3,
          majorUnit: 0.1,
          showMajorGridlines: true,
          numberFormat: '0%',
          numberFormatSourceLinked: false,
        },
      },
      categories: ['Q1', 'Q2', 'Q3'],
      series: [
        { name: 'Revenue', values: [42, 55, 61], chartType: 'column', axisGroup: 'primary' },
        { name: 'Margin', values: [0.12, 0.18, 0.2], chartType: 'line', axisGroup: 'secondary' },
      ],
      showLegend: false,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };

    const svg = spreadsheetChartSvg(chart);

    expect(svg).toContain('data-axis-scale="left"');
    expect(svg).toContain('data-axis-minimum="0"');
    expect(svg).toContain('data-axis-maximum="100"');
    expect(svg).toContain('data-axis-major-unit="25"');
    expect(svg).toContain('data-axis-gridlines="false"');
    expect(svg).toContain('data-axis-number-format="#,##0"');
    expect(svg).toContain('data-axis-scale="right"');
    expect(svg).toContain('data-axis-maximum="0.3"');
    expect(svg).toContain('data-axis-gridlines="true"');
    expect(svg).toContain('>30%</text>');
  });

  it('renders reversed axes, label placement, major ticks, and category-label intervals', () => {
    const chart: WorkSpreadsheetChart = {
      id: 'chart-axis-display',
      name: 'Axis display',
      type: 'column',
      axes: {
        bottom: {
          reverseOrder: true,
          labelPosition: 'high',
          majorTickMark: 'outside',
          labelInterval: 2,
        },
        left: {
          minimum: 0,
          maximum: 100,
          majorUnit: 50,
          reverseOrder: true,
          labelPosition: 'none',
          majorTickMark: 'cross',
        },
      },
      categories: ['Q1', 'Q2', 'Q3'],
      series: [{ name: 'Revenue', values: [20, 50, 80] }],
      showLegend: false,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };

    const svg = spreadsheetChartSvg(chart);

    expect(svg).toContain('data-axis-display="bottom"');
    expect(svg).toContain('data-axis-reverse-order="true"');
    expect(svg).toContain('data-axis-label-position="high"');
    expect(svg).toContain('data-axis-major-tick-mark="outside"');
    expect(svg).toContain('data-axis-label-interval="2"');
    expect(svg).toContain('data-axis-category-label="0"');
    expect(svg).not.toContain('data-axis-category-label="1"');
    expect(svg).toContain('data-axis-category-label="2"');
    expect(svg).toContain('data-axis-major-tick="bottom:0"');
    expect(svg).toContain('data-axis-major-tick="left:0"');
    expect(svg).not.toContain('data-axis-tick="left"');
    const q1X = Number(/data-axis-category-label="0" x="([^"]+)"/.exec(svg)?.[1]);
    const q3X = Number(/data-axis-category-label="2" x="([^"]+)"/.exec(svg)?.[1]);
    const firstBarY = Number(/data-chart-bar="0:0"[^>]* y="([^"]+)"/.exec(svg)?.[1]);
    expect(q1X).toBeGreaterThan(q3X);
    expect(firstBarY).toBeLessThan(100);
  });

  it('renders multiple fitted trendlines with forecast, equations, and moving averages', () => {
    const chart: WorkSpreadsheetChart = {
      id: 'chart-trendlines',
      name: 'Trend analysis',
      type: 'scatter',
      scatterStyle: 'marker',
      categories: [],
      series: [
        {
          name: 'Observed',
          xValues: [1, 2, 3, 4],
          values: [3, 5, 7, 9],
          trendlines: [
            {
              type: 'linear',
              name: 'Growth <linear>',
              forward: 1,
              backward: 0.5,
              displayEquation: true,
              displayRSquared: true,
            },
            { type: 'movingAverage', period: 2 },
          ],
        },
      ],
      showLegend: false,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };

    const svg = spreadsheetChartSvg(chart);

    expect(svg).toContain('data-trendline-series="0:0"');
    expect(svg).toContain('data-trendline-type="linear"');
    expect(svg).toContain('data-trendline-name="Growth &lt;linear&gt;"');
    expect(svg).toContain('data-trendline-equation=');
    expect(svg).toContain('data-trendline-r-squared=');
    expect(svg).toContain('data-trendline-series="0:1"');
    expect(svg).toContain('data-trendline-type="movingAverage"');
    expect(svg).not.toContain('NaN');
  });

  it('renders editable cartesian and circular data-label content and positions', () => {
    const column: WorkSpreadsheetChart = {
      id: 'chart-column-labels',
      name: 'Revenue labels',
      type: 'column',
      categories: ['Q1', 'Q2'],
      series: [
        {
          name: 'Revenue',
          values: [42, 55],
          dataLabels: {
            showValue: true,
            showCategoryName: true,
            showSeriesName: true,
            separator: ' | ',
            position: 'outsideEnd',
          },
        },
      ],
      showLegend: false,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };
    const pie: WorkSpreadsheetChart = {
      ...column,
      id: 'chart-pie-labels',
      type: 'pie',
      series: [
        {
          name: 'Revenue',
          values: [42, 55],
          dataLabels: {
            showValue: true,
            showCategoryName: true,
            showPercentage: true,
            separator: ' / ',
            position: 'outsideEnd',
          },
        },
      ],
    };

    const columnSvg = spreadsheetChartSvg(column);
    const pieSvg = spreadsheetChartSvg(pie);

    expect(columnSvg).toContain('data-data-label-series="0:0"');
    expect(columnSvg).toContain('data-data-label-position="outsideEnd"');
    expect(columnSvg).toContain('data-data-label-text="Revenue | Q1 | 42"');
    expect(pieSvg).toContain('data-data-label-series="0:0"');
    expect(pieSvg).toContain('data-data-label-text="Q1 / 42 / 43.3%"');
    expect(pieSvg).not.toContain('NaN');
  });

  it('renders horizontal and vertical error bars with independent amounts and end caps', () => {
    const chart: WorkSpreadsheetChart = {
      id: 'chart-error-bars',
      name: 'Observed error',
      type: 'scatter',
      scatterStyle: 'marker',
      categories: [],
      series: [
        {
          name: 'Observed',
          xValues: [1, 2, 3],
          values: [10, 20, 30],
          errorBars: [
            {
              direction: 'x',
              barType: 'plus',
              valueType: 'percentage',
              value: 10,
              showEndCaps: false,
            },
            {
              direction: 'y',
              barType: 'both',
              valueType: 'fixedValue',
              value: 2,
            },
          ],
        },
      ],
      showLegend: false,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };

    const svg = spreadsheetChartSvg(chart);

    expect(svg).toContain('data-error-bars-series="0:0"');
    expect(svg).toContain('data-error-bars-direction="x"');
    expect(svg).toContain('data-error-bars-value-type="percentage"');
    expect(svg).toContain('data-error-bar-point="0:0:0"');
    expect(svg).toContain('data-error-minus="0"');
    expect(svg).toContain('data-error-plus="0.1"');
    expect(svg).toContain('data-error-bars-series="0:1"');
    expect(svg).toContain('data-error-bars-direction="y"');
    expect(svg).toContain('data-error-minus="2"');
    expect(svg).toContain('data-error-plus="2"');
    expect(svg).not.toContain('NaN');
  });

  it('refreshes and renames live X, Y, and bubble-size references together', () => {
    const content = spreadsheetContent();
    const chart: WorkSpreadsheetChart = {
      id: 'chart-bubble-live',
      name: 'Live bubble',
      type: 'bubble',
      title: 'Live bubble',
      categories: [],
      series: [
        {
          name: 'Margin by revenue',
          xValues: [],
          xValuesReference: 'Report!$B$2:$B$3',
          values: [],
          valuesReference: 'Report!$C$2:$C$3',
          bubbleSizes: [],
          bubbleSizesReference: 'Report!$B$2:$B$3',
        },
      ],
      showLegend: false,
      left: 0,
      top: 0,
      width: 480,
      height: 288,
    };
    content.sheets[0].charts = [chart];

    expect(resolveSpreadsheetChart(content, content.sheets[0], chart).series[0]).toMatchObject({
      xValues: [42, 55],
      values: [10, 12],
      bubbleSizes: [42, 55],
    });

    const renamed = reconcileSpreadsheetChartPreviews(content, [
      {
        ...spreadsheetSheetsWithChartPreviews(content)[0],
        name: 'Forecast',
      },
    ]);
    expect(renamed.sheets[0].charts?.[0].series[0]).toMatchObject({
      xValuesReference: 'Forecast!$B$2:$B$3',
      valuesReference: 'Forecast!$C$2:$C$3',
      bubbleSizesReference: 'Forecast!$B$2:$B$3',
    });
  });
});

function spreadsheetContent(): WorkSpreadsheetContent {
  return {
    type: 'spreadsheet',
    sheets: [
      {
        id: 'sheet-1',
        name: 'Report',
        order: 0,
        status: 1,
        row: 3,
        column: 3,
        data: [
          [
            { v: 'Quarter', m: 'Quarter' },
            { v: 'Revenue', m: 'Revenue' },
            { v: 'Margin', m: 'Margin' },
          ],
          [
            { v: 'Q1', m: 'Q1' },
            { v: 42, m: '42' },
            { v: 10, m: '10' },
          ],
          [
            { v: 'Q2', m: 'Q2' },
            { v: 55, m: '55' },
            { v: 12, m: '12' },
          ],
        ],
      },
    ],
  };
}
