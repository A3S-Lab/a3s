import { describe, expect, it } from 'vitest';
import {
  presentationChartErrorBarCount,
  presentationChartTrendlineCount,
  withPresentationChartSeriesAnalysis,
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
});

function scatterChart(): WorkSlideChart {
  return {
    type: 'scatter',
    categories: ['1', '2'],
    series: [{ name: 'Latency', values: [42, 58] }],
  };
}
