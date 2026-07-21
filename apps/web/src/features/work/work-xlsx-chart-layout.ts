import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartLegendPosition,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  type WorkSpreadsheetChartGrouping,
  type WorkSpreadsheetChartLayout,
  type WorkSpreadsheetChartLegendPosition,
  workSpreadsheetChartGroupingIsStacked,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSmoothLines,
} from './work-spreadsheet-chart-layout';
import { attribute, descendants, directChild, directChildren, firstDescendant } from './work-ooxml-package';
import type { WorkSpreadsheetChart, WorkSpreadsheetChartType } from './work-types';

const LEGEND_POSITIONS = new Set(['r', 'l', 't', 'b', 'tr']);
const LEGEND_CHILDREN = new Set(['legendPos', 'layout', 'overlay']);
const BAR_GROUPINGS = new Set(['clustered', 'standard', 'stacked', 'percentStacked']);
const LINE_AREA_GROUPINGS = new Set(['standard', 'stacked', 'percentStacked']);

export interface ParsedXlsxChartLegend extends Pick<WorkSpreadsheetChart, 'showLegend'>, WorkSpreadsheetChartLayout {}

export function parseXlsxChartLegend(document: Document): ParsedXlsxChartLegend {
  const legend = firstDescendant(document, 'legend');
  if (!legend) return { showLegend: false };
  const position = xlsxLegendPosition(attribute(directChild(legend, 'legendPos') ?? legend, 'val'));
  const overlayElement = directChild(legend, 'overlay');
  const overlay = overlayElement ? xlsxBooleanValue(attribute(overlayElement, 'val')) : false;
  return {
    showLegend: true,
    legendPosition: normalizeWorkSpreadsheetChartLegendPosition(position),
    legendOverlay: normalizeWorkSpreadsheetChartLegendOverlay(overlay),
  };
}

export function xlsxChartLegendXml(chart: WorkSpreadsheetChart): string {
  if (!chart.showLegend) return '';
  const position = xlsxLegendPositionValue(normalizeWorkSpreadsheetChartLegendPosition(chart.legendPosition));
  const overlay = normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay) ? 1 : 0;
  return `<c:legend><c:legendPos val="${position}"/><c:layout/><c:overlay val="${overlay}"/></c:legend>`;
}

export function parseXlsxChartPlotLayout(
  chartNode: Element,
  chartType: WorkSpreadsheetChartType
): WorkSpreadsheetChartLayout {
  if (!workSpreadsheetChartSupportsGrouping(chartType)) return {};
  const grouping = normalizeWorkSpreadsheetChartGrouping(
    attribute(directChild(chartNode, 'grouping') ?? chartNode, 'val'),
    chartType
  );
  const gapWidth = directChild(chartNode, 'gapWidth');
  const overlap = directChild(chartNode, 'overlap');
  return {
    grouping,
    ...(workSpreadsheetChartSupportsBarSpacing(chartType)
      ? {
          gapWidth: normalizeWorkSpreadsheetChartGapWidth(gapWidth ? attribute(gapWidth, 'val') : undefined),
          overlap: normalizeWorkSpreadsheetChartOverlap(overlap ? attribute(overlap, 'val') : undefined, grouping),
        }
      : {}),
    ...(workSpreadsheetChartSupportsSmoothLines(chartType)
      ? { smoothLines: normalizeWorkSpreadsheetChartSmoothLines(consistentLineSmooth(chartNode)) }
      : {}),
  };
}

export function xlsxChartGroupingValue(chart: WorkSpreadsheetChart): WorkSpreadsheetChartGrouping {
  return normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
}

export function xlsxChartGapWidthValue(chart: WorkSpreadsheetChart): number {
  return normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth);
}

export function xlsxChartOverlapValue(chart: WorkSpreadsheetChart): number {
  const grouping = xlsxChartGroupingValue(chart);
  return normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping);
}

export function xlsxChartSmoothLinesValue(chart: WorkSpreadsheetChart): boolean {
  return normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines);
}

export function xlsxChartNodeUsesStackedGrouping(chartNode: Element): boolean {
  const grouping = attribute(directChild(chartNode, 'grouping') ?? chartNode, 'val');
  return grouping === 'stacked' || grouping === 'percentStacked';
}

export function isSupportedXlsxChartLegend(document: Document): boolean {
  const chart = firstDescendant(document, 'chart');
  if (!chart) return true;
  const legends = directChildren(chart, 'legend');
  if (legends.length > 1) return false;
  const legend = legends[0];
  if (!legend) return true;
  const children = directChildren(legend);
  if (children.some((child) => !LEGEND_CHILDREN.has(child.localName))) return false;
  const positions = directChildren(legend, 'legendPos');
  if (positions.length > 1 || (positions[0] && !LEGEND_POSITIONS.has(attribute(positions[0], 'val') ?? ''))) {
    return false;
  }
  const layouts = directChildren(legend, 'layout');
  if (layouts.length > 1 || layouts.some((layout) => directChildren(layout).length > 0)) return false;
  return validOptionalBooleanElement(legend, 'overlay');
}

export function isSupportedXlsxChartPlotLayout(chart: Element): boolean {
  const grouping = directChildren(chart, 'grouping');
  if (grouping.length > 1) return false;
  if (chart.localName === 'barChart') {
    if (!grouping.length || !BAR_GROUPINGS.has(attribute(grouping[0], 'val') ?? '')) return false;
    if (!validOptionalIntegerElement(chart, 'gapWidth', 0, 500)) return false;
    if (!validOptionalIntegerElement(chart, 'overlap', -100, 100)) return false;
  } else if (chart.localName === 'lineChart' || chart.localName === 'areaChart') {
    if (!grouping.length || !LINE_AREA_GROUPINGS.has(attribute(grouping[0], 'val') ?? '')) return false;
    if (directChildren(chart, 'gapWidth').length || directChildren(chart, 'overlap').length) return false;
  } else if (grouping.length || directChildren(chart, 'gapWidth').length || directChildren(chart, 'overlap').length) {
    return false;
  }

  if (chart.localName === 'lineChart') {
    if (!hasConsistentLineSmooth(chart)) return false;
  } else if (
    chart.localName !== 'scatterChart' &&
    descendants(chart, 'smooth').some((element) => xlsxBooleanValue(attribute(element, 'val')))
  ) {
    return false;
  }
  if (
    chart.localName === 'scatterChart' &&
    !['marker', 'line', 'lineMarker', 'smooth', 'smoothMarker'].includes(
      attribute(firstDescendant(chart, 'scatterStyle') ?? chart, 'val') ?? ''
    )
  ) {
    return false;
  }
  if (descendants(chart, 'bubble3D').some((element) => xlsxBooleanValue(attribute(element, 'val')))) return false;
  if (descendants(chart, 'explosion').some((element) => Number(attribute(element, 'val')) !== 0)) return false;
  return !descendants(chart, 'firstSliceAng').some((element) => Number(attribute(element, 'val')) !== 0);
}

export function isSupportedXlsxCombinationPlotNode(chart: Element): boolean {
  if (!isSupportedXlsxChartPlotLayout(chart)) return false;
  const grouping = attribute(directChild(chart, 'grouping') ?? chart, 'val');
  if (chart.localName === 'lineChart' || chart.localName === 'areaChart') {
    return grouping === 'standard' && (chart.localName !== 'lineChart' || consistentLineSmooth(chart) === false);
  }
  if (chart.localName !== 'barChart' || grouping !== 'clustered') return false;
  const gapWidth = directChild(chart, 'gapWidth');
  const overlap = directChild(chart, 'overlap');
  return (
    Number(attribute(gapWidth ?? chart, 'val') ?? 150) === 150 && Number(attribute(overlap ?? chart, 'val') ?? 0) === 0
  );
}

export function xlsxChartGroupingIsStacked(chart: WorkSpreadsheetChart): boolean {
  return workSpreadsheetChartGroupingIsStacked(xlsxChartGroupingValue(chart));
}

function xlsxLegendPosition(value: string | null): WorkSpreadsheetChartLegendPosition {
  if (value === 'l') return 'left';
  if (value === 't') return 'top';
  if (value === 'b') return 'bottom';
  if (value === 'tr') return 'topRight';
  return 'right';
}

function xlsxLegendPositionValue(position: WorkSpreadsheetChartLegendPosition): string {
  if (position === 'left') return 'l';
  if (position === 'top') return 't';
  if (position === 'bottom') return 'b';
  if (position === 'topRight') return 'tr';
  return 'r';
}

function hasConsistentLineSmooth(chart: Element): boolean {
  const chartSmooth = directChildren(chart, 'smooth');
  if (chartSmooth.length > 1 || !chartSmooth.every(validBooleanElement)) return false;
  const series = directChildren(chart, 'ser');
  if (series.some((item) => directChildren(item, 'smooth').length > 1)) return false;
  const explicit = [...chartSmooth, ...series.flatMap((item) => directChildren(item, 'smooth'))];
  if (!explicit.every(validBooleanElement)) return false;
  if (!explicit.length) return true;
  const values = explicit.map((element) => xlsxBooleanValue(attribute(element, 'val')));
  return values.every((value) => value === values[0]);
}

function consistentLineSmooth(chart: Element): boolean {
  const explicit = [
    ...directChildren(chart, 'smooth'),
    ...directChildren(chart, 'ser').flatMap((series) => directChildren(series, 'smooth')),
  ];
  if (!explicit.length || !hasConsistentLineSmooth(chart)) return false;
  return xlsxBooleanValue(attribute(explicit[0], 'val'));
}

function validOptionalIntegerElement(parent: Element, localName: string, minimum: number, maximum: number): boolean {
  const elements = directChildren(parent, localName);
  if (elements.length > 1) return false;
  if (!elements.length) return true;
  const value = Number(attribute(elements[0], 'val'));
  return Number.isInteger(value) && value >= minimum && value <= maximum;
}

function validOptionalBooleanElement(parent: Element, localName: string): boolean {
  const elements = directChildren(parent, localName);
  return elements.length <= 1 && elements.every(validBooleanElement);
}

function validBooleanElement(element: Element): boolean {
  const value = attribute(element, 'val');
  return value === null || value === '0' || value === '1' || value === 'false' || value === 'true';
}

function xlsxBooleanValue(value: string | null): boolean {
  return value === null || value === '1' || value === 'true';
}
