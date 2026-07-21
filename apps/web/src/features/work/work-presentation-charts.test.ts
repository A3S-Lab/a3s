import { describe, expect, it } from 'vitest';
import {
  normalizePresentationChartSeriesStyle,
  presentationChartErrorBarCount,
  presentationChartHasCustomSeriesStyles,
  presentationChartTrendlineCount,
  withPresentationChartLayout,
  withPresentationChartSeriesAnalysis,
  withPresentationChartSeriesStyle,
  withPresentationChartType,
} from './work-presentation-charts';
import type { WorkSlideChart } from './work-types';

describe('Work presentation chart series analysis', () => {
  it('normalizes trendlines and keeps one editable error bar per axis direction', () => {
    const chart = withPresentationChartSeriesAnalysis(scatterChart(), 0, {
      trendlines: [{ type: 'polynomial', order: 12, forward: -2 }],
      errorBars: [
        { direction: 'x', barType: 'both', valueType: 'fixedValue', value: -1 },
        { direction: 'x', barType: 'plus', valueType: 'percentage', value: 20 },
        { direction: 'y', barType: 'minus', valueType: 'standardError' },
      ],
    });

    expect(chart.series[0]).toMatchObject({
      trendlines: [{ type: 'polynomial', order: 6 }],
      errorBars: [
        { direction: 'x', barType: 'both', valueType: 'fixedValue', value: 0 },
        { direction: 'y', barType: 'minus', valueType: 'standardError' },
      ],
    });
    expect(presentationChartTrendlineCount(chart)).toBe(1);
    expect(presentationChartErrorBarCount(chart)).toBe(2);
  });

  it('preserves analysis across supported families and removes it from unsupported families', () => {
    const source = withPresentationChartSeriesAnalysis(scatterChart(), 0, {
      trendlines: [{ type: 'linear' }],
      errorBars: [{ direction: 'x', barType: 'both', valueType: 'percentage', value: 10 }],
    });
    const column = withPresentationChartType(source, 'column');
    expect(column.series[0]).toMatchObject({
      trendlines: [{ type: 'linear' }],
      errorBars: [{ direction: 'y', barType: 'both', valueType: 'percentage', value: 10 }],
    });

    const pie = withPresentationChartType(column, 'pie');
    expect(pie.series[0].trendlines).toBeUndefined();
    expect(pie.series[0].errorBars).toBeUndefined();
    expect(presentationChartTrendlineCount(pie)).toBe(0);
    expect(presentationChartErrorBarCount(pie)).toBe(0);
  });

  it('normalizes editable plot layout and removes analysis when a chart becomes stacked', () => {
    const source = withPresentationChartSeriesAnalysis(scatterChart(), 0, {
      trendlines: [{ type: 'linear' }],
      errorBars: [{ direction: 'y', barType: 'both', valueType: 'fixedValue', value: 2 }],
    });
    const column = withPresentationChartType(source, 'column');
    const stacked = withPresentationChartLayout(column, {
      legendOverlay: true,
      grouping: 'stacked',
      gapWidth: 900,
      overlap: -140,
    });

    expect(stacked).toMatchObject({
      legendOverlay: true,
      grouping: 'stacked',
      gapWidth: 500,
      overlap: -100,
    });
    expect(stacked.series[0].trendlines).toBeUndefined();
    expect(stacked.series[0].errorBars).toBeUndefined();
    expect(presentationChartTrendlineCount(stacked)).toBe(0);
    expect(presentationChartErrorBarCount(stacked)).toBe(0);
  });

  it('normalizes portable series appearance and removes markers from unsupported chart families', () => {
    const styled = withPresentationChartSeriesStyle(scatterChart(), 0, {
      fillColor: '#abc',
      fillTransparency: 150,
      lineColor: '112233',
      lineWidth: 80,
      lineDash: 'dashDot',
      marker: { symbol: 'star', size: 90, fillColor: '#fed', lineColor: '#456' },
    });

    expect(styled.series[0].style).toEqual({
      fillColor: '#AABBCC',
      fillTransparency: 100,
      lineColor: '#112233',
      lineWidth: 20,
      lineDash: 'dashDot',
      marker: { symbol: 'star', size: 72, fillColor: '#FFEEDD', lineColor: '#445566' },
    });
    expect(presentationChartHasCustomSeriesStyles(styled)).toBe(true);
    expect(normalizePresentationChartSeriesStyle(styled.series[0].style, 'scatter')).toEqual(styled.series[0].style);

    const column = withPresentationChartType(styled, 'column');
    expect(column.series[0].style).toEqual({
      fillColor: '#AABBCC',
      fillTransparency: 100,
      lineColor: '#112233',
      lineWidth: 20,
      lineDash: 'dashDot',
    });
  });
});

function scatterChart(): WorkSlideChart {
  return {
    type: 'scatter',
    categories: ['1', '2'],
    series: [{ name: 'Latency', values: [42, 58] }],
  };
}
