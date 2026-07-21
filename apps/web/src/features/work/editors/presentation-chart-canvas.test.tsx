import { render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { WorkSlideChart } from '../work-types';
import { SlideChart } from './presentation-chart-canvas';

describe('Work presentation chart canvas', () => {
  afterEach(() => vi.restoreAllMocks());

  it('renders series legends and semantic axis titles through the shared canvas path', () => {
    const fillText = vi.fn();
    const context = chartContext(fillText);
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context);
    const chart: WorkSlideChart = {
      type: 'column',
      title: '季度收入',
      categories: ['Q1', 'Q2'],
      series: [{ name: '收入', values: [42, 58] }],
      showLegend: true,
      legendPosition: 'bottom',
      axes: {
        bottom: { title: '季度', reverseOrder: true, labelInterval: 1 },
        left: {
          title: '收入（万元）',
          minimum: 0,
          maximum: 100,
          majorUnit: 50,
          numberFormat: '¥#,##0',
          showMajorGridlines: true,
        },
      },
      dataLabels: {
        showValue: true,
        showCategoryName: true,
        separator: ' / ',
        position: 'outsideEnd',
      },
    };

    const { container } = render(<SlideChart chart={chart} label='季度收入图表' />);

    expect(fillText.mock.calls.map(([value]) => value)).toEqual(
      expect.arrayContaining([
        '季度收入',
        '收入',
        '季度',
        '收入（万元）',
        'Q1',
        'Q2',
        '¥0',
        '¥50',
        '¥100',
        'Q1 / 42',
        'Q2 / 58',
      ])
    );
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-legend-position', 'bottom');
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-data-labels', 'true');
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-axes', 'true');
  });

  it('renders native bubble geometry, XY labels, and axis titles through the shared canvas path', () => {
    const fillText = vi.fn();
    const arc = vi.fn();
    const context = chartContext(fillText, arc);
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context);
    const chart: WorkSlideChart = {
      type: 'bubble',
      title: '响应容量',
      categories: ['1', '2', '4'],
      series: [{ name: '容量', values: [3, 5, 9], bubbleSizes: [9, 16, 25] }],
      categoryAxisTitle: '并发数',
      valueAxisTitle: '延迟',
      bubbleScale: 140,
      bubbleSizeRepresents: 'width',
      dataLabels: {
        showValue: true,
        showCategoryName: true,
        showBubbleSize: true,
        separator: ' / ',
        position: 'above',
      },
    };

    const { container } = render(<SlideChart chart={chart} label='响应容量图表' />);

    expect(fillText.mock.calls.map(([value]) => value)).toEqual(
      expect.arrayContaining(['响应容量', '并发数', '延迟', '1 / 3 / 9', '2 / 5 / 16', '4 / 9 / 25'])
    );
    expect(arc).toHaveBeenCalled();
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-type', 'bubble');
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-bubble-scale', '140');
    expect(container.querySelector('canvas')).toHaveAttribute(
      'data-presentation-chart-bubble-size-represents',
      'width'
    );
  });

  it('renders per-series trendlines, equations, R squared, and error bars through the shared canvas path', () => {
    const fillText = vi.fn();
    const context = chartContext(fillText);
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue(context);
    const chart: WorkSlideChart = {
      type: 'line',
      categories: ['Q1', 'Q2', 'Q3'],
      series: [
        {
          name: '收入',
          values: [2, 4, 8],
          trendlines: [{ type: 'linear', displayEquation: true, displayRSquared: true }],
          errorBars: [{ direction: 'y', barType: 'both', valueType: 'fixedValue', value: 1 }],
        },
      ],
    };

    const { container } = render(<SlideChart chart={chart} label='收入趋势图表' />);

    const renderedText = fillText.mock.calls.map(([value]) => String(value));
    expect(renderedText.some((value) => value.startsWith('y = '))).toBe(true);
    expect(renderedText.some((value) => value.startsWith('R² = '))).toBe(true);
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-trendlines', '1');
    expect(container.querySelector('canvas')).toHaveAttribute('data-presentation-chart-error-bars', '1');
  });
});

function chartContext(
  fillText: ReturnType<typeof vi.fn>,
  arc: ReturnType<typeof vi.fn> = vi.fn()
): CanvasRenderingContext2D {
  return {
    scale: vi.fn(),
    clearRect: vi.fn(),
    fillText,
    beginPath: vi.fn(),
    moveTo: vi.fn(),
    lineTo: vi.fn(),
    closePath: vi.fn(),
    stroke: vi.fn(),
    fill: vi.fn(),
    fillRect: vi.fn(),
    arc,
    bezierCurveTo: vi.fn(),
    save: vi.fn(),
    restore: vi.fn(),
    translate: vi.fn(),
    rotate: vi.fn(),
  } as unknown as CanvasRenderingContext2D;
}
