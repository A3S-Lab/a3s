import { normalizeWorkSpreadsheetChartAxes } from './work-spreadsheet-chart-axis';
import type { WorkSlideChart, WorkSlideChartAxes, WorkSlideChartType } from './work-types';

function chartTypeWithAxes(type: WorkSlideChartType): WorkSlideChartType {
  return type === 'pie' || type === 'doughnut' ? 'column' : type;
}

function semanticPresentationChartAxes(chart: WorkSlideChart): {
  categoryOrX?: WorkSlideChartAxes['bottom'];
  valueOrY?: WorkSlideChartAxes['left'];
} {
  const sourceType = chartTypeWithAxes(chart.type);
  const normalized = normalizeWorkSpreadsheetChartAxes(chart.axes, sourceType, false);
  const categoryPosition = sourceType === 'bar' ? 'left' : 'bottom';
  const valuePosition = sourceType === 'bar' ? 'bottom' : 'left';
  const legacyCategoryTitle = chart.categoryAxisTitle?.trim();
  const legacyValueTitle = chart.valueAxisTitle?.trim();
  const categoryOrX = mergeLegacyTitle(normalized?.[categoryPosition], legacyCategoryTitle);
  const valueOrY = mergeLegacyTitle(normalized?.[valuePosition], legacyValueTitle);
  return { categoryOrX, valueOrY };
}

function mergeLegacyTitle(
  axis: WorkSlideChartAxes['bottom'],
  legacyTitle: string | undefined
): WorkSlideChartAxes['bottom'] {
  if (!legacyTitle || axis?.title?.trim() || axis?.titleReference?.trim()) return axis;
  return { ...axis, title: legacyTitle };
}

/**
 * Returns the editable primary axes in their physical bottom/left positions.
 * Legacy category/value title fields are migrated on read so persisted charts
 * created before the complete axis model continue to render and export.
 */
export function presentationChartAxes(chart: WorkSlideChart): WorkSlideChartAxes | undefined {
  if (chart.type === 'pie' || chart.type === 'doughnut') return undefined;
  return presentationChartAxesForType(chart, chart.type);
}

/**
 * Maps category/X and value/Y settings by meaning when the chart type changes.
 * Axis-less chart types keep a canonical column-style copy so settings survive
 * a temporary switch to pie or doughnut.
 */
export function presentationChartAxesForType(
  chart: WorkSlideChart,
  targetType: WorkSlideChartType
): WorkSlideChartAxes | undefined {
  const { categoryOrX, valueOrY } = semanticPresentationChartAxes(chart);
  if (!categoryOrX && !valueOrY) return undefined;
  const normalizedTargetType = chartTypeWithAxes(targetType);
  const source: WorkSlideChartAxes =
    normalizedTargetType === 'bar'
      ? {
          ...(valueOrY ? { bottom: valueOrY } : {}),
          ...(categoryOrX ? { left: categoryOrX } : {}),
        }
      : {
          ...(categoryOrX ? { bottom: categoryOrX } : {}),
          ...(valueOrY ? { left: valueOrY } : {}),
        };
  return normalizeWorkSpreadsheetChartAxes(source, normalizedTargetType, false);
}

export function withPresentationChartAxes(chart: WorkSlideChart, axes: WorkSlideChartAxes | undefined): WorkSlideChart {
  const { axes: _axes, categoryAxisTitle: _categoryAxisTitle, valueAxisTitle: _valueAxisTitle, ...base } = chart;
  const normalized = normalizeWorkSpreadsheetChartAxes(axes, chartTypeWithAxes(chart.type), false);
  return normalized ? { ...base, axes: normalized } : base;
}
