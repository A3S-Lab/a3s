import type { WorkSpreadsheetChartType } from './work-types';

export type WorkSpreadsheetChartLegendPosition = 'right' | 'left' | 'top' | 'bottom' | 'topRight';
export type WorkSpreadsheetChartGrouping = 'clustered' | 'standard' | 'stacked' | 'percentStacked';

export interface WorkSpreadsheetChartLayout {
  legendPosition?: WorkSpreadsheetChartLegendPosition;
  legendOverlay?: boolean;
  grouping?: WorkSpreadsheetChartGrouping;
  gapWidth?: number;
  overlap?: number;
  smoothLines?: boolean;
}

export type WorkSpreadsheetChartLayoutSource = WorkSpreadsheetChartLayout & { type: WorkSpreadsheetChartType };

export function normalizeWorkSpreadsheetChartLegendPosition(value: unknown): WorkSpreadsheetChartLegendPosition {
  return value === 'left' || value === 'top' || value === 'bottom' || value === 'topRight' || value === 'right'
    ? value
    : 'right';
}

export function normalizeWorkSpreadsheetChartLegendOverlay(value: unknown): boolean {
  return value === true;
}

export function workSpreadsheetChartSupportsGrouping(type: WorkSpreadsheetChartType): boolean {
  return type === 'column' || type === 'bar' || type === 'line' || type === 'area';
}

export function workSpreadsheetChartSupportsBarSpacing(type: WorkSpreadsheetChartType): boolean {
  return type === 'column' || type === 'bar';
}

export function workSpreadsheetChartSupportsSmoothLines(type: WorkSpreadsheetChartType): boolean {
  return type === 'line';
}

export function workSpreadsheetChartDefaultGrouping(type: WorkSpreadsheetChartType): WorkSpreadsheetChartGrouping {
  return workSpreadsheetChartSupportsBarSpacing(type) ? 'clustered' : 'standard';
}

export function normalizeWorkSpreadsheetChartGrouping(
  value: unknown,
  type: WorkSpreadsheetChartType
): WorkSpreadsheetChartGrouping {
  const fallback = workSpreadsheetChartDefaultGrouping(type);
  if (!workSpreadsheetChartSupportsGrouping(type)) return fallback;
  if (value === 'stacked' || value === 'percentStacked') return value;
  if (workSpreadsheetChartSupportsBarSpacing(type)) {
    return value === 'standard' || value === 'clustered' ? value : fallback;
  }
  return value === 'standard' ? value : fallback;
}

export function workSpreadsheetChartGroupingIsStacked(grouping: WorkSpreadsheetChartGrouping): boolean {
  return grouping === 'stacked' || grouping === 'percentStacked';
}

export function normalizeWorkSpreadsheetChartGapWidth(value: unknown): number {
  return normalizedInteger(value, 0, 500, 150);
}

export function normalizeWorkSpreadsheetChartOverlap(value: unknown, grouping: WorkSpreadsheetChartGrouping): number {
  return normalizedInteger(value, -100, 100, workSpreadsheetChartGroupingIsStacked(grouping) ? 100 : 0);
}

export function normalizeWorkSpreadsheetChartSmoothLines(value: unknown): boolean {
  return value === true;
}

export function normalizeWorkSpreadsheetChartLayout(
  source: WorkSpreadsheetChartLayoutSource
): WorkSpreadsheetChartLayout {
  const grouping = normalizeWorkSpreadsheetChartGrouping(source.grouping, source.type);
  return {
    legendPosition: normalizeWorkSpreadsheetChartLegendPosition(source.legendPosition),
    legendOverlay: normalizeWorkSpreadsheetChartLegendOverlay(source.legendOverlay),
    grouping: workSpreadsheetChartSupportsGrouping(source.type) ? grouping : undefined,
    gapWidth: workSpreadsheetChartSupportsBarSpacing(source.type)
      ? normalizeWorkSpreadsheetChartGapWidth(source.gapWidth)
      : undefined,
    overlap: workSpreadsheetChartSupportsBarSpacing(source.type)
      ? normalizeWorkSpreadsheetChartOverlap(source.overlap, grouping)
      : undefined,
    smoothLines: workSpreadsheetChartSupportsSmoothLines(source.type)
      ? normalizeWorkSpreadsheetChartSmoothLines(source.smoothLines)
      : undefined,
  };
}

export function workSpreadsheetChartSupportsSeriesAnalysis(source: WorkSpreadsheetChartLayoutSource): boolean {
  if (!workSpreadsheetChartSupportsGrouping(source.type)) return true;
  return !workSpreadsheetChartGroupingIsStacked(normalizeWorkSpreadsheetChartGrouping(source.grouping, source.type));
}

export function workSpreadsheetChartLegendPositionLabel(position: WorkSpreadsheetChartLegendPosition): string {
  if (position === 'left') return '左侧';
  if (position === 'top') return '顶部';
  if (position === 'bottom') return '底部';
  if (position === 'topRight') return '右上角';
  return '右侧';
}

export function workSpreadsheetChartGroupingLabel(grouping: WorkSpreadsheetChartGrouping): string {
  if (grouping === 'stacked') return '堆积';
  if (grouping === 'percentStacked') return '百分比堆积';
  if (grouping === 'standard') return '标准';
  return '簇状';
}

function normalizedInteger(value: unknown, minimum: number, maximum: number, fallback: number): number {
  if (value === null || value === undefined || value === '') return fallback;
  const number = Number(value);
  if (!Number.isFinite(number)) return fallback;
  return Math.min(maximum, Math.max(minimum, Math.round(number)));
}
