import { describe, expect, it } from 'vitest';
import {
  formatSpreadsheetChartAxisNumber,
  normalizeWorkSpreadsheetChartAxes,
  spreadsheetChartAxisGridlinesVisible,
  spreadsheetChartAxisScale,
} from './work-spreadsheet-chart-axis';

describe('Work spreadsheet chart axes', () => {
  it('builds bounded ticks from an explicit range and major unit', () => {
    expect(
      spreadsheetChartAxisScale([42, 55, 61], { minimum: 0, maximum: 100, majorUnit: 25 }, { includeZero: true })
    ).toEqual({
      minimum: 0,
      maximum: 100,
      span: 100,
      majorUnit: 25,
      ticks: [0, 25, 50, 75, 100],
    });
  });

  it('formats common decimal, grouped, currency, percentage, and scientific tick labels', () => {
    expect(formatSpreadsheetChartAxisNumber(1_250, '#,##0')).toBe('1,250');
    expect(formatSpreadsheetChartAxisNumber(12.5, '¥#,##0.00')).toBe('¥12.50');
    expect(formatSpreadsheetChartAxisNumber(0.125, '0.0%')).toBe('12.5%');
    expect(formatSpreadsheetChartAxisNumber(1_250, '0.0E+00')).toBe('1.3E+03');
  });

  it('uses native-like major-gridline defaults while honoring explicit overrides', () => {
    expect(spreadsheetChartAxisGridlinesVisible(undefined, 'column', 'left')).toBe(true);
    expect(spreadsheetChartAxisGridlinesVisible(undefined, 'scatter', 'bottom')).toBe(false);
    expect(spreadsheetChartAxisGridlinesVisible({ showMajorGridlines: true }, 'scatter', 'bottom')).toBe(true);
    expect(spreadsheetChartAxisGridlinesVisible({ showMajorGridlines: false }, 'column', 'left')).toBe(false);
  });

  it('normalizes common value- and category-axis display settings by axis kind', () => {
    expect(
      normalizeWorkSpreadsheetChartAxes(
        {
          bottom: {
            reverseOrder: true,
            labelPosition: 'high',
            majorTickMark: 'outside',
            labelInterval: 3,
          },
          left: {
            reverseOrder: true,
            labelPosition: 'none',
            majorTickMark: 'cross',
            labelInterval: 4,
          },
        },
        'column'
      )
    ).toEqual({
      bottom: {
        reverseOrder: true,
        labelPosition: 'high',
        majorTickMark: 'outside',
        labelInterval: 3,
      },
      left: {
        reverseOrder: true,
        labelPosition: 'none',
        majorTickMark: 'cross',
      },
    });
    expect(normalizeWorkSpreadsheetChartAxes({ bottom: { labelInterval: 1 } }, 'column')).toEqual({
      bottom: { labelInterval: 1 },
    });
  });
});
