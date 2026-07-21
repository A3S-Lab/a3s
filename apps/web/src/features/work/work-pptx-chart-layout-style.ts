import JSZip from 'jszip';
import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  type WorkSpreadsheetChartLayout,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSeriesAnalysis,
  workSpreadsheetChartSupportsSmoothLines,
} from './work-spreadsheet-chart-layout';
import { normalizeWorkSpreadsheetChartSeriesStyle } from './work-spreadsheet-chart-series-style';
import { directChild, directChildren, firstDescendant, parseXml } from './work-ooxml-package';
import { normalizePresentationChartSeriesStyle } from './work-presentation-charts';
import type { WorkSlide, WorkSlideChart, WorkSlideChartSeriesStyle } from './work-types';
import {
  isSupportedXlsxChartLegend,
  isSupportedXlsxChartPlotLayout,
  parseXlsxChartLegend,
  parseXlsxChartPlotLayout,
} from './work-xlsx-chart-layout';
import {
  isSupportedXlsxChartSeriesFormatting,
  parseXlsxChartSeriesStyle,
  xlsxChartSeriesMarkerXml,
  xlsxChartSeriesShapePropertiesXml,
} from './work-xlsx-chart-series-style';

const CHART_TYPES = [
  'barChart',
  'lineChart',
  'pieChart',
  'doughnutChart',
  'areaChart',
  'radarChart',
  'scatterChart',
  'bubbleChart',
];
const DRAWING_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/main';

export interface PptxChartFormattingDiagnostic {
  code: string;
  message: string;
}

export interface PptxChartLayoutStyleReadResult {
  legendOverlay?: boolean;
  layout: WorkSpreadsheetChartLayout;
  seriesStyles: Array<WorkSlideChartSeriesStyle | undefined>;
  diagnostics: PptxChartFormattingDiagnostic[];
}

export function readPptxChartLayoutAndSeriesStyles(
  document: Document,
  chartNode: Element | undefined,
  chartType: WorkSlideChart['type'],
  seriesNodes: readonly Element[]
): PptxChartLayoutStyleReadResult {
  const diagnostics: PptxChartFormattingDiagnostic[] = [];
  const legend = firstDescendant(document, 'legend');
  const parsedLegend = parseXlsxChartLegend(document);
  if (legend && !isSupportedXlsxChartLegend(document)) {
    diagnostics.push({
      code: 'pptx.chart.format.legend',
      message: 'Manual legend layout, entry overrides, or advanced legend formatting was normalized.',
    });
  }

  const hasLayout = Boolean(chartNode && chartHasEditableLayoutElements(chartNode));
  const layout = chartNode && hasLayout ? parseXlsxChartPlotLayout(chartNode, chartType) : {};
  if (chartNode && hasLayout && !isSupportedXlsxChartPlotLayout(chartNode)) {
    diagnostics.push({
      code: 'pptx.chart.format.layout',
      message: 'Invalid grouping, spacing, overlap, or mixed smoothing settings were normalized.',
    });
  }

  const seriesStyles = seriesNodes.map((series) => parseXlsxChartSeriesStyle(series));
  if (
    chartNode &&
    seriesNodes.some(
      (series) =>
        !isSupportedXlsxChartSeriesFormatting(series, chartNode.localName) || directChildren(series, 'dPt').length > 0
    )
  ) {
    diagnostics.push({
      code: 'pptx.chart.format.series',
      message:
        'Theme, gradient, pattern, effect, custom dash, or per-point series formatting was normalized to portable series appearance.',
    });
  }
  if (
    !workSpreadsheetChartSupportsSeriesAnalysis({ type: chartType, ...layout }) &&
    seriesNodes.some((series) => directChild(series, 'trendline') || directChild(series, 'errBars'))
  ) {
    diagnostics.push({
      code: 'pptx.chart.format.series-analysis',
      message: 'Trendlines and error bars were removed because stacked chart layouts do not support series analysis.',
    });
  }

  return {
    ...(legend ? { legendOverlay: normalizeWorkSpreadsheetChartLegendOverlay(parsedLegend.legendOverlay) } : {}),
    layout,
    seriesStyles,
    diagnostics,
  };
}

export async function patchPptxChartLayoutAndSeriesStyles(
  buffer: ArrayBuffer,
  slides: readonly WorkSlide[]
): Promise<ArrayBuffer> {
  const charts = slides.flatMap((slide) =>
    slide.elements.flatMap((element) => (element.type === 'chart' && element.chart ? [element.chart] : []))
  );
  if (!charts.length) return buffer;
  const archive = await JSZip.loadAsync(buffer);
  const chartPaths = Object.keys(archive.files)
    .filter((path) => /^ppt\/charts\/chart\d+\.xml$/.test(path))
    .sort((left, right) => chartPartNumber(left) - chartPartNumber(right));
  if (chartPaths.length !== charts.length) {
    throw new Error(
      `PPTX chart export produced ${chartPaths.length} chart part(s) for ${charts.length} chart element(s).`
    );
  }
  for (const [index, chart] of charts.entries()) {
    const path = chartPaths[index];
    const entry = archive.file(path);
    if (!entry) throw new Error(`PPTX chart export is missing chart part: ${path}`);
    const document = parseXml(await entry.async('text'), path);
    const chartNode = CHART_TYPES.map((name) => firstDescendant(document, name)).find(Boolean);
    if (!chartNode) throw new Error(`PPTX chart part is missing a supported chart node: ${path}`);
    writePptxChartLayoutAndSeriesStyles(document, chartNode, chart);
    archive.file(path, new XMLSerializer().serializeToString(document));
  }
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export function writePptxChartLayoutAndSeriesStyles(
  document: Document,
  chartNode: Element,
  chart: WorkSlideChart
): void {
  writeLegendOverlay(document, chart);
  writePlotLayout(document, chartNode, chart);
  const seriesNodes = directChildren(chartNode, 'ser');
  if (seriesNodes.length !== chart.series.length) {
    throw new Error(
      `PPTX chart export produced ${seriesNodes.length} native series for ${chart.series.length} editable series.`
    );
  }
  for (const [seriesIndex, seriesNode] of seriesNodes.entries()) {
    writeSeriesStyle(document, seriesNode, chart.series[seriesIndex].style, chart.type);
  }
}

function writeLegendOverlay(document: Document, chart: WorkSlideChart): void {
  if (!chart.showLegend) return;
  const chartRoot = firstDescendant(document, 'chart');
  const legend = chartRoot ? directChild(chartRoot, 'legend') : undefined;
  if (!legend) return;
  for (const overlay of directChildren(legend, 'overlay')) overlay.remove();
  const anchor = directChildren(legend).find((child) => ['spPr', 'txPr', 'extLst'].includes(child.localName)) ?? null;
  insertChartSetting(
    document,
    legend,
    'overlay',
    normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay) ? '1' : '0',
    anchor
  );
}

function writePlotLayout(document: Document, chartNode: Element, chart: WorkSlideChart): void {
  if (workSpreadsheetChartSupportsGrouping(chart.type)) {
    replaceChartSetting(
      document,
      chartNode,
      'grouping',
      normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type),
      directChildren(chartNode).find((child) => ['varyColors', 'ser'].includes(child.localName)) ?? null
    );
  }
  if (workSpreadsheetChartSupportsBarSpacing(chart.type)) {
    const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
    for (const name of ['gapWidth', 'overlap']) {
      for (const child of directChildren(chartNode, name)) child.remove();
    }
    const anchor = directChildren(chartNode).find((child) => ['axId', 'extLst'].includes(child.localName)) ?? null;
    insertChartSetting(
      document,
      chartNode,
      'gapWidth',
      String(normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)),
      anchor
    );
    insertChartSetting(
      document,
      chartNode,
      'overlap',
      String(normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping)),
      anchor
    );
  }
  if (workSpreadsheetChartSupportsSmoothLines(chart.type)) {
    for (const child of directChildren(chartNode, 'smooth')) child.remove();
    for (const series of directChildren(chartNode, 'ser')) {
      for (const child of directChildren(series, 'smooth')) child.remove();
    }
    const anchor = directChildren(chartNode).find((child) => ['axId', 'extLst'].includes(child.localName)) ?? null;
    insertChartSetting(
      document,
      chartNode,
      'smooth',
      normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines) ? '1' : '0',
      anchor
    );
  }
}

function writeSeriesStyle(
  document: Document,
  seriesNode: Element,
  source: WorkSlideChartSeriesStyle | undefined,
  chartType: WorkSlideChart['type']
): void {
  const style = normalizePresentationChartSeriesStyle(source, chartType);
  for (const shape of directChildren(seriesNode, 'spPr')) shape.remove();
  for (const marker of directChildren(seriesNode, 'marker')) marker.remove();
  if (!style) return;
  const shapeXml = xlsxChartSeriesShapePropertiesXml(style);
  if (shapeXml) {
    const anchor = directChildren(seriesNode).find((child) =>
      [
        'marker',
        'dPt',
        'dLbls',
        'trendline',
        'errBars',
        'cat',
        'xVal',
        'val',
        'yVal',
        'bubbleSize',
        'smooth',
        'extLst',
      ].includes(child.localName)
    );
    seriesNode.insertBefore(chartFragmentElement(document, seriesNode, shapeXml), anchor ?? null);
  }
  const normalized = normalizeWorkSpreadsheetChartSeriesStyle(style);
  if (!normalized?.marker) return;
  const markerXml = xlsxChartSeriesMarkerXml(style, true);
  const anchor = directChildren(seriesNode).find((child) =>
    ['dPt', 'dLbls', 'trendline', 'errBars', 'cat', 'xVal', 'val', 'yVal', 'bubbleSize', 'smooth', 'extLst'].includes(
      child.localName
    )
  );
  seriesNode.insertBefore(chartFragmentElement(document, seriesNode, markerXml), anchor ?? null);
}

function chartFragmentElement(document: Document, parent: Element, xml: string): Element {
  const namespace = parent.namespaceURI ?? 'http://schemas.openxmlformats.org/drawingml/2006/chart';
  const parsed = parseXml(
    `<c:root xmlns:c="${namespace}" xmlns:a="${DRAWING_NAMESPACE}">${xml}</c:root>`,
    'PPTX chart formatting fragment'
  );
  const element = parsed.documentElement.firstElementChild;
  if (!element) throw new Error('PPTX chart formatting fragment did not contain an element.');
  return document.importNode(element, true) as Element;
}

function replaceChartSetting(
  document: Document,
  parent: Element,
  localName: string,
  value: string,
  anchor: Element | null
): void {
  for (const child of directChildren(parent, localName)) child.remove();
  insertChartSetting(document, parent, localName, value, anchor);
}

function insertChartSetting(
  document: Document,
  parent: Element,
  localName: string,
  value: string,
  anchor: Element | null
): Element {
  const prefix = parent.lookupPrefix(parent.namespaceURI) ?? parent.prefix ?? 'c';
  const element = document.createElementNS(parent.namespaceURI, `${prefix}:${localName}`);
  element.setAttribute('val', value);
  parent.insertBefore(element, anchor);
  return element;
}

function chartHasEditableLayoutElements(chartNode: Element): boolean {
  return (
    ['grouping', 'gapWidth', 'overlap'].some((name) => directChildren(chartNode, name).length > 0) ||
    (chartNode.localName === 'lineChart' &&
      (directChildren(chartNode, 'smooth').length > 0 ||
        directChildren(chartNode, 'ser').some((series) => directChildren(series, 'smooth').length > 0)))
  );
}

function chartPartNumber(path: string): number {
  return Number(/chart(\d+)\.xml$/.exec(path)?.[1] ?? Number.MAX_SAFE_INTEGER);
}
