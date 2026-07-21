import {
  normalizeWorkSpreadsheetErrorBars,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetChartType,
  type WorkSpreadsheetErrorBars,
} from './work-types';

export interface SpreadsheetErrorBarAmount {
  minus: number;
  plus: number;
}

export function spreadsheetErrorBarAmounts(
  series: WorkSpreadsheetChartSeries,
  source: WorkSpreadsheetErrorBars,
  chartType: WorkSpreadsheetChartType
): SpreadsheetErrorBarAmount[] {
  const errorBars = normalizeWorkSpreadsheetErrorBars(source, chartType);
  const values = errorBarSourceValues(series, errorBars);
  const sharedAmount = sharedErrorAmount(values, errorBars);
  return values.map((value, index) => {
    const amount =
      errorBars.valueType === 'percentage'
        ? Math.abs(value) * ((errorBars.value ?? 5) / 100)
        : errorBars.valueType === 'custom'
          ? 0
          : sharedAmount;
    const minus = errorBars.valueType === 'custom' ? finiteNonNegative(errorBars.minusValues?.[index]) : amount;
    const plus = errorBars.valueType === 'custom' ? finiteNonNegative(errorBars.plusValues?.[index]) : amount;
    return {
      minus: errorBars.barType === 'plus' ? 0 : minus,
      plus: errorBars.barType === 'minus' ? 0 : plus,
    };
  });
}

export function spreadsheetSeriesErrorBarBounds(
  series: WorkSpreadsheetChartSeries,
  chartType: WorkSpreadsheetChartType,
  direction: 'x' | 'y'
): number[] {
  const values = direction === 'x' ? numericXValues(series) : series.values;
  const matching = (series.errorBars ?? []).filter(
    (source) => normalizeWorkSpreadsheetErrorBars(source, chartType).direction === direction
  );
  if (!matching.length) return values.filter(Number.isFinite);
  return matching.flatMap((source) => {
    const amounts = spreadsheetErrorBarAmounts(series, source, chartType);
    return values.flatMap((value, index) => {
      if (!Number.isFinite(value)) return [];
      const amount = amounts[index] ?? { minus: 0, plus: 0 };
      return [value - amount.minus, value, value + amount.plus];
    });
  });
}

export function errorBarSourceValues(
  series: WorkSpreadsheetChartSeries,
  errorBars: Pick<WorkSpreadsheetErrorBars, 'direction'>
): number[] {
  return errorBars.direction === 'x' ? numericXValues(series) : series.values;
}

function numericXValues(series: WorkSpreadsheetChartSeries): number[] {
  return series.values.map((_, index) => series.xValues?.[index] ?? index + 1);
}

function sharedErrorAmount(values: number[], errorBars: WorkSpreadsheetErrorBars): number {
  if (errorBars.valueType === 'fixedValue') return errorBars.value ?? 1;
  if (errorBars.valueType === 'standardDeviation') {
    return populationStandardDeviation(values) * (errorBars.value ?? 1);
  }
  if (errorBars.valueType === 'standardError') {
    const usableCount = values.filter(Number.isFinite).length;
    return usableCount ? populationStandardDeviation(values) / Math.sqrt(usableCount) : 0;
  }
  return 0;
}

function populationStandardDeviation(values: number[]): number {
  const usable = values.filter(Number.isFinite);
  if (!usable.length) return 0;
  const mean = usable.reduce((sum, value) => sum + value, 0) / usable.length;
  return Math.sqrt(usable.reduce((sum, value) => sum + (value - mean) ** 2, 0) / usable.length);
}

function finiteNonNegative(value: number | undefined): number {
  return typeof value === 'number' && Number.isFinite(value) ? Math.max(0, value) : 0;
}
