import type { Sheet } from '@fortune-sheet/core';
import { describe, expect, it, vi } from 'vitest';
import { drawSpreadsheetConditionalDataBar } from './work-spreadsheet-conditional-canvas';
import { drawSpreadsheetConditionalIcon } from './work-spreadsheet-conditional-icons';
import { spreadsheetConditionalFormatStyles } from './work-spreadsheet-conditional-format';

describe('Work spreadsheet conditional-format evaluation', () => {
  it('computes common cell, duplicate, ranking, average, and relative-formula rules', () => {
    const sheet = testSheet([
      ['Value', 'State', 'Code', 'Rank', 'Formula'],
      [1, 'Ready', 1, 40, 'a'],
      [2, 'Hold', 1, 10, 'b'],
      [3, 'ready', 2, 30, 'c'],
      [4, 'Hold', 3, 20, 'd'],
    ]);
    sheet.luckysheet_conditionformat_save = [
      defaultRule('greaterThan', [0, 0], ['2'], { textColor: '#ffffff', cellColor: '#c00000' }),
      defaultRule('textContains', [1, 1], ['READY'], { textColor: '#006100', cellColor: null }),
      defaultRule('duplicateValue', [2, 2], ['0'], { textColor: null, cellColor: '#ffc7ce' }),
      defaultRule('top10', [3, 3], ['1'], { textColor: '#0000ff', cellColor: null }),
      defaultRule('aboveAverage', [3, 3], [''], { textColor: null, cellColor: '#fff2cc' }),
      defaultRule('formula', [4, 4], ['A2>=3'], { textColor: '#7030a0', cellColor: null }),
    ];

    const styles = spreadsheetConditionalFormatStyles(sheet);

    expect(styles.get('1_1')).toMatchObject({ textColor: '#006100' });
    expect(styles.get('3_1')).toMatchObject({ textColor: '#006100' });
    expect(styles.get('1_2')).toMatchObject({ cellColor: '#ffc7ce' });
    expect(styles.get('2_2')).toMatchObject({ cellColor: '#ffc7ce' });
    expect(styles.get('1_3')).toMatchObject({ textColor: '#0000ff', cellColor: '#fff2cc' });
    expect(styles.get('3_3')).toMatchObject({ cellColor: '#fff2cc' });
    expect(styles.get('3_4')).toMatchObject({ textColor: '#7030a0' });
    expect(styles.get('4_4')).toMatchObject({ textColor: '#7030a0' });
    expect(styles.get('3_0')).toMatchObject({ textColor: '#ffffff', cellColor: '#c00000' });
    expect(styles.get('4_0')).toMatchObject({ textColor: '#ffffff', cellColor: '#c00000' });
    expect(styles.has('2_0')).toBe(false);
  });

  it('interpolates two- and three-color scales and places mixed-sign data bars', () => {
    const sheet = testSheet([
      [0, -10],
      [10, 0],
      [100, 30],
    ]);
    sheet.luckysheet_conditionformat_save = [
      {
        type: 'colorGradation',
        cellrange: [{ row: [0, 2], column: [0, 0] }],
        format: ['#63be7b', '#ffeb84', '#f8696b'],
      },
      {
        type: 'dataBar',
        cellrange: [{ row: [0, 2], column: [1, 1] }],
        format: { textColor: null, cellColor: '#5b9bd5' },
        visualOptions: {
          thresholds: [{ type: 'min' }, { type: 'max' }],
          showValue: false,
          minLength: 20,
          maxLength: 80,
        },
      },
    ];

    const styles = spreadsheetConditionalFormatStyles(sheet);

    expect(styles.get('0_0')?.cellColor).toBe('rgb(248, 105, 107)');
    expect(styles.get('1_0')?.cellColor).toBe('rgb(255, 235, 132)');
    expect(styles.get('2_0')?.cellColor).toBe('rgb(99, 190, 123)');
    expect(styles.get('0_1')?.dataBar).toEqual({
      color: '#5b9bd5',
      startPercent: 5,
      widthPercent: 20,
      axisPercent: 25,
      showValue: false,
    });
    expect(styles.get('1_1')?.dataBar).toEqual({
      color: '#5b9bd5',
      startPercent: 25,
      widthPercent: 0,
      axisPercent: 25,
      showValue: false,
    });
    expect(styles.get('2_1')?.dataBar).toEqual({
      color: '#5b9bd5',
      startPercent: 25,
      widthPercent: 60,
      axisPercent: 25,
      showValue: false,
    });
  });

  it('retains an earlier conditional fill when a later rule changes only text color', () => {
    const sheet = testSheet([[5]]);
    sheet.luckysheet_conditionformat_save = [
      defaultRule('greaterThan', [0, 0], ['0'], { textColor: null, cellColor: '#fff2cc' }, [0, 0]),
      defaultRule('lessThan', [0, 0], ['10'], { textColor: '#9c0006', cellColor: null }, [0, 0]),
    ];

    expect(spreadsheetConditionalFormatStyles(sheet).get('0_0')).toEqual({
      textColor: '#9c0006',
      cellColor: '#fff2cc',
    });
  });

  it('keeps higher-priority styles and stops lower rules only for matched cells', () => {
    const sheet = testSheet([[5], [15]]);
    sheet.luckysheet_conditionformat_save = [
      {
        ...defaultRule('greaterThan', [0, 0], ['10'], { textColor: null, cellColor: '#c00000' }, [0, 1]),
        stopIfTrue: true,
      },
      defaultRule('greaterThan', [0, 0], ['0'], { textColor: null, cellColor: '#4472c4' }, [0, 1]),
      defaultRule('greaterThan', [0, 0], ['0'], { textColor: '#006100', cellColor: '#fff2cc' }, [0, 1]),
    ];

    const styles = spreadsheetConditionalFormatStyles(sheet);

    expect(styles.get('0_0')).toEqual({ textColor: '#006100', cellColor: '#4472c4' });
    expect(styles.get('1_0')).toEqual({ cellColor: '#c00000' });
  });

  it('evaluates the complete set of core cell comparison operators', () => {
    const sheet = testSheet([
      [5, 5, 5, 5],
      [10, 10, 10, 10],
      [15, 15, 15, 15],
    ]);
    sheet.luckysheet_conditionformat_save = [
      defaultRule('notEqual', [0, 0], ['10'], { textColor: '#ffffff', cellColor: '#c00000' }, [0, 2]),
      defaultRule('greaterThanOrEqual', [1, 1], ['10'], { textColor: '#ffffff', cellColor: '#006100' }, [0, 2]),
      defaultRule('lessThanOrEqual', [2, 2], ['10'], { textColor: '#ffffff', cellColor: '#9c6500' }, [0, 2]),
      defaultRule('notBetween', [3, 3], ['7', '12'], { textColor: '#ffffff', cellColor: '#7030a0' }, [0, 2]),
    ];

    const styles = spreadsheetConditionalFormatStyles(sheet);

    expect([styles.has('0_0'), styles.has('1_0'), styles.has('2_0')]).toEqual([true, false, true]);
    expect([styles.has('0_1'), styles.has('1_1'), styles.has('2_1')]).toEqual([false, true, true]);
    expect([styles.has('0_2'), styles.has('1_2'), styles.has('2_2')]).toEqual([true, true, false]);
    expect([styles.has('0_3'), styles.has('1_3'), styles.has('2_3')]).toEqual([true, false, true]);
  });

  it('uses explicit numeric color-scale thresholds instead of recomputing the midpoint', () => {
    const sheet = testSheet([[0], [25], [100]]);
    sheet.luckysheet_conditionformat_save = [
      {
        type: 'colorGradation',
        cellrange: [{ row: [0, 2], column: [0, 0] }],
        format: ['#63be7b', '#ffeb84', '#f8696b'],
        visualOptions: {
          thresholds: [
            { type: 'num', value: 0 },
            { type: 'num', value: 50 },
            { type: 'num', value: 100 },
          ],
        },
      },
    ];

    expect(spreadsheetConditionalFormatStyles(sheet).get('1_0')?.cellColor).toBe('rgb(252, 170, 120)');
  });

  it('evaluates icon-set thresholds, exclusivity, reverse order, and hidden values deterministically', () => {
    const sheet = testSheet([[0], [33], [34], [67], [100]]);
    sheet.luckysheet_conditionformat_save = [
      {
        type: 'icons',
        cellrange: [{ row: [0, 4], column: [0, 0] }],
        format: {
          iconSet: '3TrafficLights1',
          showValue: false,
          reverse: true,
          percent: true,
          thresholds: [
            { type: 'min', gte: true },
            { type: 'percent', value: 33, gte: false },
            { type: 'percent', value: 67, gte: true },
          ],
        },
      },
    ];

    const styles = spreadsheetConditionalFormatStyles(sheet);

    expect(styles.get('0_0')?.icon).toEqual({
      iconSet: '3TrafficLights1',
      index: 2,
      count: 3,
      showValue: false,
    });
    expect(styles.get('1_0')?.icon?.index).toBe(2);
    expect(styles.get('2_0')?.icon?.index).toBe(1);
    expect(styles.get('3_0')?.icon?.index).toBe(0);
    expect(styles.get('4_0')?.icon?.index).toBe(0);
  });

  it('draws evaluated icons into FortuneSheet cells and masks values only when requested', () => {
    const context = {
      save: vi.fn(),
      beginPath: vi.fn(),
      rect: vi.fn(),
      clip: vi.fn(),
      fillRect: vi.fn(),
      fillText: vi.fn(),
      restore: vi.fn(),
      fillStyle: '',
      font: '',
      textAlign: '',
      textBaseline: '',
    } as unknown as CanvasRenderingContext2D;

    drawSpreadsheetConditionalIcon(
      context,
      { startX: 10, startY: 8, endX: 90, endY: 32 },
      { iconSet: '3Arrows', index: 2, count: 3, showValue: false },
      '#fff2cc'
    );

    expect(context.fillRect).toHaveBeenCalledWith(11, 9, 77, 21);
    expect(context.fillText).toHaveBeenCalledWith('↑', 14, 20);
    expect(context.fillStyle).toBe('#2e7d32');
  });

  it('repaints FortuneSheet data bars with deterministic geometry and hidden-value semantics', () => {
    const context = {
      save: vi.fn(),
      beginPath: vi.fn(),
      rect: vi.fn(),
      clip: vi.fn(),
      fillRect: vi.fn(),
      fillText: vi.fn(),
      moveTo: vi.fn(),
      lineTo: vi.fn(),
      stroke: vi.fn(),
      restore: vi.fn(),
      fillStyle: '',
      strokeStyle: '',
      globalAlpha: 1,
      lineWidth: 1,
      font: '',
      textAlign: '',
      textBaseline: '',
    } as unknown as CanvasRenderingContext2D;

    drawSpreadsheetConditionalDataBar(
      context,
      { startX: 10, startY: 8, endX: 90, endY: 32 },
      {
        color: '#5b9bd5',
        startPercent: 25,
        widthPercent: 60,
        axisPercent: 25,
        showValue: false,
      },
      { v: 30, m: '30' },
      '#ffffff'
    );

    expect(context.fillRect).toHaveBeenNthCalledWith(1, 11, 9, 77, 21);
    expect(context.fillRect).toHaveBeenNthCalledWith(2, 30, 10, 48, 19);
    expect(context.stroke).toHaveBeenCalledOnce();
    expect(context.fillText).not.toHaveBeenCalled();
  });
});

function testSheet(values: Array<Array<string | number>>): Sheet {
  return {
    id: 'sheet-1',
    name: 'Sheet 1',
    status: 1,
    row: values.length,
    column: Math.max(...values.map((row) => row.length)),
    data: values.map((row) => row.map((value) => ({ v: value, m: String(value) }))),
  };
}

function defaultRule(
  conditionName: string,
  columns: [number, number],
  conditionValue: string[],
  format: { textColor: string | null; cellColor: string | null },
  rows: [number, number] = [1, 4]
) {
  return {
    type: 'default',
    cellrange: [{ row: rows, column: columns }],
    format,
    conditionName,
    conditionRange: [],
    conditionValue,
  };
}
