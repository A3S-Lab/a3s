import { describe, expect, it } from 'vitest';
import { spreadsheetChartSvg } from './work-spreadsheet-chart-svg';
import {
  normalizeWorkSpreadsheetChartSeriesStyle,
  spreadsheetChartSeriesStyleContext,
} from './work-spreadsheet-chart-series-style';
import type { WorkSpreadsheetChart } from './work-types';

describe('Work spreadsheet chart series style', () => {
  it('normalizes portable sRGB colors, transparency, line settings, and markers', () => {
    expect(
      normalizeWorkSpreadsheetChartSeriesStyle({
        fillColor: '#abc',
        fillTransparency: 140,
        lineColor: '445566',
        lineWidth: 50,
        lineDash: 'dashDot',
        marker: {
          symbol: 'diamond',
          size: 1,
          fillColor: '#778899',
          lineColor: '#aabbcc',
        },
      })
    ).toEqual({
      fillColor: '#AABBCC',
      fillTransparency: 100,
      lineColor: '#445566',
      lineWidth: 20,
      lineDash: 'dashDot',
      marker: {
        symbol: 'diamond',
        size: 2,
        fillColor: '#778899',
        lineColor: '#AABBCC',
      },
    });
    expect(normalizeWorkSpreadsheetChartSeriesStyle({ fillColor: 'theme:accent1' })).toBeUndefined();
  });

  it('uses one series style in line, marker, column, area, and legend SVG paths', () => {
    const chart = styledChart('line');
    const line = spreadsheetChartSvg(chart);
    const column = spreadsheetChartSvg({ ...chart, type: 'column' });
    const area = spreadsheetChartSvg({ ...chart, type: 'area' });

    expect(line).toMatch(
      /data-line-series="0"[^>]*fill="none" stroke="#445566" stroke-width="3.25" stroke-dasharray="8 4 2 4"/
    );
    expect(line).toContain('data-marker-symbol="diamond"');
    expect(line).toContain('data-marker-size="9"');
    expect(line).toContain('fill="#778899" stroke="#AABBCC"');
    expect(line).toContain('data-chart-legend-entry="0"');
    expect(line).toContain('fill="#112233"');

    expect(column).toMatch(/data-chart-bar="0:0"[^>]*fill="#112233" fill-opacity="0.65"/);
    expect(column).toMatch(/data-chart-bar="0:0"[^>]*stroke="#445566" stroke-width="3.25"/);
    expect(column).toContain('stroke-dasharray="8 4 2 4"');

    expect(area).toMatch(/data-area-series="0"[^>]*fill="#112233" fill-opacity="0.65"/);
    expect(area).toMatch(/data-line-series="0"[^>]*stroke="#445566" stroke-width="3.25"/);
  });

  it('uses the same style for scatter, bubble, radar, circular, and combination previews', () => {
    const chart = styledChart('scatter');
    const scatter = spreadsheetChartSvg({ ...chart, scatterStyle: 'lineMarker', categories: [] });
    const bubble = spreadsheetChartSvg({
      ...chart,
      type: 'bubble',
      categories: [],
      series: [{ ...chart.series[0], bubbleSizes: [9, 16, 25] }],
    });
    const radar = spreadsheetChartSvg({ ...chart, type: 'radar', radarStyle: 'filled' });
    const pie = spreadsheetChartSvg({ ...chart, type: 'pie' });
    const combination = spreadsheetChartSvg({
      ...chart,
      type: 'combination',
      series: [
        { ...chart.series[0], chartType: 'column', axisGroup: 'primary' },
        {
          ...chart.series[0],
          name: 'Trend',
          chartType: 'line',
          axisGroup: 'secondary',
          style: { ...chart.series[0].style, marker: { symbol: 'square', size: 8 } },
        },
      ],
    });

    expect(scatter).toMatch(/data-scatter-line="0"[^>]*stroke="#445566" stroke-width="3.25"/);
    expect(scatter).toContain('data-marker-symbol="diamond"');
    expect(bubble).toMatch(/data-bubble-point="0:0"[^>]*fill="#112233" fill-opacity="0.65"/);
    expect(bubble).toMatch(/data-bubble-point="0:0"[^>]*stroke="#445566" stroke-width="3.25"/);
    expect(radar).toMatch(/data-radar-series="0"><polygon[^>]*fill="#112233" fill-opacity="0.65"/);
    expect(pie).toMatch(/data-chart-slice="0:0"[^>]*fill="#112233" fill-opacity="0.65"/);
    expect(combination).toMatch(/data-combination-series="0"[^>]*><rect[^>]*fill="#112233"/);
    expect(combination).toContain('data-marker-symbol="square"');
  });

  it('describes explicit series appearance for Copilot without exposing implementation syntax', () => {
    expect(spreadsheetChartSeriesStyleContext(styledChart('line').series[0].style)).toBe(
      '；系列外观：填充 #112233（透明度 35%），线条 #445566、3.25 磅、点划线，数据标记 菱形、9 磅、填充 #778899、轮廓 #AABBCC'
    );
    expect(spreadsheetChartSeriesStyleContext(undefined)).toBe('');
  });
});

function styledChart(type: WorkSpreadsheetChart['type']): WorkSpreadsheetChart {
  return {
    id: 'styled-series',
    name: 'Styled series',
    type,
    categories: ['Q1', 'Q2', 'Q3'],
    series: [
      {
        name: 'Revenue',
        xValues: [1, 2, 3],
        values: [20, 40, 60],
        style: {
          fillColor: '#112233',
          fillTransparency: 35,
          lineColor: '#445566',
          lineWidth: 3.25,
          lineDash: 'dashDot',
          marker: {
            symbol: 'diamond',
            size: 9,
            fillColor: '#778899',
            lineColor: '#AABBCC',
          },
        },
      },
    ],
    showLegend: true,
    left: 0,
    top: 0,
    width: 480,
    height: 288,
  };
}
