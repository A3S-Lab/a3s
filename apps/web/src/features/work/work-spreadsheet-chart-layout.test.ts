import { describe, expect, it } from 'vitest';
import { spreadsheetChartSvg } from './work-spreadsheet-chart-svg';
import type { WorkSpreadsheetChart } from './work-types';

describe('Work spreadsheet chart legends and plot layout', () => {
  it('positions legends on every supported edge and only reserves plot space when overlay is disabled', () => {
    const chart = chartFixture();
    const positions = ['right', 'left', 'top', 'bottom', 'topRight'] as const;

    for (const legendPosition of positions) {
      const svg = spreadsheetChartSvg({ ...chart, legendPosition, legendOverlay: false });
      expect(svg).toContain(`data-chart-legend-position="${legendPosition}"`);
      expect(svg).toContain('data-chart-legend-overlay="false"');
    }

    const reservedLeft = spreadsheetChartSvg({ ...chart, legendPosition: 'left', legendOverlay: false });
    const overlayLeft = spreadsheetChartSvg({ ...chart, legendPosition: 'left', legendOverlay: true });
    const reservedBottom = spreadsheetChartSvg({ ...chart, legendPosition: 'bottom', legendOverlay: false });
    const overlayBottom = spreadsheetChartSvg({ ...chart, legendPosition: 'bottom', legendOverlay: true });

    expect(plotMetric(reservedLeft, 'plot-x')).toBeGreaterThan(plotMetric(overlayLeft, 'plot-x'));
    expect(plotMetric(reservedBottom, 'plot-height')).toBeLessThan(plotMetric(overlayBottom, 'plot-height'));
  });

  it('stacks positive and negative column values independently', () => {
    const svg = spreadsheetChartSvg({
      ...chartFixture(),
      grouping: 'stacked',
      series: [
        { name: 'Actual', values: [4, -3] },
        { name: 'Forecast', values: [6, -2] },
      ],
    });

    expect(svg).toContain('data-chart-grouping="stacked"');
    expect(svg).toMatch(/data-chart-bar="0:0"[^>]*data-stack-start="0"[^>]*data-stack-end="4"/);
    expect(svg).toMatch(/data-chart-bar="1:0"[^>]*data-stack-start="4"[^>]*data-stack-end="10"/);
    expect(svg).toMatch(/data-chart-bar="0:1"[^>]*data-stack-start="0"[^>]*data-stack-end="-3"/);
    expect(svg).toMatch(/data-chart-bar="1:1"[^>]*data-stack-start="-3"[^>]*data-stack-end="-5"/);
  });

  it('normalizes positive and negative percent stacks per category and uses stacked area boundaries', () => {
    const chart: WorkSpreadsheetChart = {
      ...chartFixture(),
      type: 'area',
      grouping: 'percentStacked',
      series: [
        { name: 'Actual', values: [20, -30] },
        { name: 'Forecast', values: [80, -70] },
      ],
    };

    const svg = spreadsheetChartSvg(chart);

    expect(svg).toContain('data-chart-grouping="percentStacked"');
    expect(svg).toMatch(/data-chart-point="0:0"[^>]*data-stack-start="0"[^>]*data-stack-end="0.2"/);
    expect(svg).toMatch(/data-chart-point="1:0"[^>]*data-stack-start="0.2"[^>]*data-stack-end="1"/);
    expect(svg).toMatch(/data-chart-point="0:1"[^>]*data-stack-start="0"[^>]*data-stack-end="-0.3"/);
    expect(svg).toMatch(/data-chart-point="1:1"[^>]*data-stack-start="-0.3"[^>]*data-stack-end="-1"/);
    expect(svg).toContain('data-area-baseline="stacked"');
    expect(svg).toContain('>100%</text>');
  });

  it('applies gap width, series overlap, and smooth line geometry', () => {
    const defaultColumns = spreadsheetChartSvg(chartFixture());
    const spacedColumns = spreadsheetChartSvg({
      ...chartFixture(),
      gapWidth: 300,
      overlap: -40,
    });
    const smoothLine = spreadsheetChartSvg({
      ...chartFixture(),
      type: 'line',
      grouping: 'standard',
      smoothLines: true,
      series: [{ name: 'Actual', values: [4, 9, 6] }],
    });

    expect(spacedColumns).toContain('data-chart-gap-width="300"');
    expect(spacedColumns).toContain('data-chart-overlap="-40"');
    expect(firstBarWidth(spacedColumns)).toBeLessThan(firstBarWidth(defaultColumns));
    expect(smoothLine).toContain('data-chart-smooth-lines="true"');
    expect(smoothLine).toMatch(/data-line-series="0"[^>]*data-smooth="true"[^>]*d="[^"]*C /);
  });
});

function chartFixture(): WorkSpreadsheetChart {
  return {
    id: 'chart-layout',
    name: 'Layout chart',
    title: 'Revenue layout',
    type: 'column',
    categories: ['Q1', 'Q2', 'Q3'],
    series: [
      { name: 'Actual', values: [4, 9, 6] },
      { name: 'Forecast', values: [6, 8, 10] },
    ],
    showLegend: true,
    left: 0,
    top: 0,
    width: 480,
    height: 288,
  };
}

function plotMetric(svg: string, name: string): number {
  const match = new RegExp(`data-${name}="([^"]+)"`).exec(svg);
  if (!match) throw new Error(`Missing ${name} in chart SVG.`);
  return Number(match[1]);
}

function firstBarWidth(svg: string): number {
  const match = /data-chart-bar="0:0"[^>]*width="([^"]+)"/.exec(svg);
  if (!match) throw new Error('Missing first chart bar in SVG.');
  return Number(match[1]);
}
