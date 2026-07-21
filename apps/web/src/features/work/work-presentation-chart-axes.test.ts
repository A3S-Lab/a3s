import { describe, expect, it } from 'vitest';
import {
  presentationChartAxes,
  presentationChartAxesForType,
  withPresentationChartAxes,
} from './work-presentation-chart-axes';
import type { WorkSlideChart } from './work-types';

const chart: WorkSlideChart = {
  type: 'column',
  categories: ['Q1'],
  series: [{ name: 'Revenue', values: [42] }],
  categoryAxisTitle: 'Quarter',
  valueAxisTitle: 'Revenue',
};

describe('Work presentation chart axes', () => {
  it('migrates legacy axis titles into the normalized editable model', () => {
    expect(presentationChartAxes(chart)).toEqual({
      bottom: { title: 'Quarter' },
      left: { title: 'Revenue' },
    });
    expect(withPresentationChartAxes(chart, presentationChartAxes(chart))).toEqual({
      type: 'column',
      categories: ['Q1'],
      series: [{ name: 'Revenue', values: [42] }],
      axes: {
        bottom: { title: 'Quarter' },
        left: { title: 'Revenue' },
      },
    });
  });

  it('keeps category and value settings attached to their meaning when switching to a bar chart', () => {
    expect(
      presentationChartAxesForType(
        {
          ...chart,
          categoryAxisTitle: undefined,
          valueAxisTitle: undefined,
          axes: {
            bottom: { title: 'Quarter', reverseOrder: true, labelInterval: 2 },
            left: { title: 'Revenue', minimum: 0, maximum: 100 },
          },
        },
        'bar'
      )
    ).toEqual({
      bottom: { title: 'Revenue', minimum: 0, maximum: 100 },
      left: { title: 'Quarter', reverseOrder: true, labelInterval: 2 },
    });
  });
});
