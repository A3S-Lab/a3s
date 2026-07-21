import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartOverlap,
  type WorkSpreadsheetChartGrouping,
  workSpreadsheetChartGroupingIsStacked,
} from './work-spreadsheet-chart-layout';
import { finiteChartNumber } from './work-spreadsheet-chart-svg-utils';
import type { WorkSpreadsheetChart } from './work-types';

type SpreadsheetChartSeriesLayoutSource = Pick<WorkSpreadsheetChart, 'categories' | 'grouping' | 'series' | 'type'>;

export interface SpreadsheetChartSeriesLayoutPoint {
  categoryIndex: number;
  rawValue: number;
  start: number;
  end: number;
}

export interface SpreadsheetChartSeriesLayout {
  grouping: WorkSpreadsheetChartGrouping;
  stacked: boolean;
  categoryCount: number;
  series: SpreadsheetChartSeriesLayoutPoint[][];
  scaleValues: number[];
}

export interface SpreadsheetChartBarGeometry {
  barSize: number;
  renderedSize: number;
  offset: (seriesIndex: number) => number;
}

export function spreadsheetChartSeriesLayout(chart: SpreadsheetChartSeriesLayoutSource): SpreadsheetChartSeriesLayout {
  const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
  const stacked = workSpreadsheetChartGroupingIsStacked(grouping);
  const categoryCount = Math.max(1, chart.categories.length, ...chart.series.map((series) => series.values.length));
  const result = chart.series.map(() => [] as SpreadsheetChartSeriesLayoutPoint[]);

  for (let categoryIndex = 0; categoryIndex < categoryCount; categoryIndex += 1) {
    const rawValues = chart.series.map((series) => finiteChartNumber(series.values[categoryIndex] ?? 0));
    const positiveTotal = rawValues.reduce((sum, value) => sum + Math.max(0, value), 0);
    const negativeTotal = rawValues.reduce((sum, value) => sum + Math.abs(Math.min(0, value)), 0);
    let positive = 0;
    let negative = 0;

    rawValues.forEach((rawValue, seriesIndex) => {
      const value =
        grouping === 'percentStacked'
          ? rawValue >= 0
            ? positiveTotal
              ? rawValue / positiveTotal
              : 0
            : negativeTotal
              ? rawValue / negativeTotal
              : 0
          : rawValue;
      const start = stacked ? (value >= 0 ? positive : negative) : 0;
      const end = start + value;
      if (stacked) {
        if (value >= 0) positive = end;
        else negative = end;
      }
      if (categoryIndex < (chart.series[seriesIndex]?.values.length ?? 0)) {
        result[seriesIndex].push({ categoryIndex, rawValue, start, end });
      }
    });
  }

  const scaleValues = result.flatMap((series) => series.flatMap((point) => [point.start, point.end]));
  return { grouping, stacked, categoryCount, series: result, scaleValues };
}

export function spreadsheetChartBarGeometry(
  groupSize: number,
  seriesCount: number,
  chart: Pick<WorkSpreadsheetChart, 'gapWidth' | 'overlap' | 'grouping' | 'type'>,
  maximumBarSize: number
): SpreadsheetChartBarGeometry {
  const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
  const gapWidth = normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth);
  const overlap = normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping);
  const count = Math.max(1, seriesCount);
  const clusterSize = Math.max(1, (groupSize * 100) / (100 + gapWidth));
  const stepRatio = 1 - overlap / 100;
  const idealBarSize = clusterSize / Math.max(1, 1 + (count - 1) * stepRatio);
  const barSize = Math.max(1, Math.min(maximumBarSize, idealBarSize));
  const step = barSize * stepRatio;
  const totalSize = barSize + step * (count - 1);
  const start = (groupSize - totalSize) / 2;
  return {
    barSize,
    renderedSize: Math.max(0.75, barSize - Math.min(1.5, barSize * 0.08)),
    offset: (seriesIndex) => start + seriesIndex * step,
  };
}
