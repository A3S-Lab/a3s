import {
  attribute,
  descendants,
  directChild,
  directChildren,
  firstDescendant,
  OoxmlPackage,
} from './work-ooxml-package';
import type { WorkSpreadsheetChartLayout } from './work-spreadsheet-chart-layout';
import { resolveSpreadsheetChart } from './work-spreadsheet-charts';
import {
  normalizeWorkSpreadsheetBubbleScale,
  normalizeWorkSpreadsheetBubbleSizeRepresents,
  normalizeWorkSpreadsheetChartAxisGroup,
  normalizeWorkSpreadsheetCombinationSeriesType,
  normalizeWorkSpreadsheetDataLabels,
  normalizeWorkSpreadsheetDoughnutHoleSize,
  normalizeWorkSpreadsheetErrorBars,
  normalizeWorkSpreadsheetRadarStyle,
  normalizeWorkSpreadsheetScatterStyle,
  normalizeWorkSpreadsheetTrendline,
  type WorkSpreadsheetBubbleSizeRepresents,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetChartAxes,
  type WorkSpreadsheetChartAxisGroup,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetChartType,
  type WorkSpreadsheetCombinationSeriesType,
  type WorkSpreadsheetContent,
  type WorkSpreadsheetDataLabelPosition,
  type WorkSpreadsheetDataLabels,
  type WorkSpreadsheetErrorBars,
  type WorkSpreadsheetErrorBarValueType,
  type WorkSpreadsheetRadarStyle,
  type WorkSpreadsheetScatterStyle,
  type WorkSpreadsheetSheet,
  type WorkSpreadsheetTrendline,
  type WorkSpreadsheetTrendlineType,
  workSpreadsheetChartSupportsErrorBars,
  workSpreadsheetChartSupportsTrendlines,
  workSpreadsheetChartUsesNumericXAxis,
} from './work-types';
import { chartAxesXml, chartTextTitleXml, parseXlsxChartAxes } from './work-xlsx-chart-axes';
import {
  isSupportedXlsxCombinationPlotNode,
  parseXlsxChartLegend,
  parseXlsxChartPlotLayout,
  xlsxChartGapWidthValue,
  xlsxChartGroupingIsStacked,
  xlsxChartGroupingValue,
  xlsxChartLegendXml,
  xlsxChartNodeUsesStackedGrouping,
  xlsxChartOverlapValue,
  xlsxChartSmoothLinesValue,
} from './work-xlsx-chart-layout';
import {
  parseXlsxChartSeriesStyle,
  xlsxChartSeriesMarkerXml,
  xlsxChartSeriesShapePropertiesXml,
} from './work-xlsx-chart-series-style';
import {
  cachedText,
  cachedValues,
  escapeXml,
  finiteNumber,
  formulaReference,
  numberCacheXml,
  numberLiteralXml,
  richText,
  stringCacheXml,
  stringLiteralXml,
} from './work-xlsx-chart-values';
import {
  readXlsxDrawingAnchor,
  type XlsxDrawingAnchor,
  xlsxDrawingAnchorToBounds,
  xlsxTwoCellAnchorMarkers,
} from './work-xlsx-drawing-geometry';

export const XLSX_CHART_RELATIONSHIP = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart';
export const XLSX_CHART_CONTENT_TYPE = 'application/vnd.openxmlformats-officedocument.drawingml.chart+xml';
const RELATIONSHIP_NAMESPACE = 'http://schemas.openxmlformats.org/officeDocument/2006/relationships';
const CHART_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/chart';
const DRAWINGML_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/main';
const CHART_GRAPHIC_URI = 'http://schemas.openxmlformats.org/drawingml/2006/chart';
const SUPPORTED_CHART_ELEMENTS = new Set([
  'barChart',
  'lineChart',
  'pieChart',
  'doughnutChart',
  'areaChart',
  'radarChart',
  'scatterChart',
  'bubbleChart',
]);

export function isSupportedXlsxCombinationChartNodes(chartNodes: readonly Element[]): boolean {
  if (
    chartNodes.length < 2 ||
    !chartNodes.every((node) => xlsxCombinationSeriesType(node) !== null && isSupportedXlsxCombinationPlotNode(node))
  ) {
    return false;
  }
  const categoryNodes = chartNodes.flatMap((node) =>
    directChildren(node, 'ser').map((series) => directChild(series, 'cat'))
  );
  if (!categoryNodes.length || categoryNodes.some((category) => !category)) return false;
  const signatures = categoryNodes.map((category) => categorySourceSignature(category!));
  return signatures.every((signature) => signature === signatures[0]);
}

function categorySourceSignature(category: Element): string {
  const reference = formulaReference(category);
  return reference ? `reference:${reference}` : `literal:${JSON.stringify(cachedValues(category))}`;
}

export interface XlsxWorksheetChart extends XlsxDrawingAnchor, WorkSpreadsheetChartLayout {
  id: string;
  name: string;
  altText?: string;
  type: WorkSpreadsheetChartType;
  title?: string;
  titleReference?: string;
  categories: string[];
  categoryReference?: string;
  series: WorkSpreadsheetChartSeries[];
  showLegend: boolean;
  axes?: WorkSpreadsheetChartAxes;
  doughnutHoleSize?: number;
  radarStyle?: WorkSpreadsheetRadarStyle;
  scatterStyle?: WorkSpreadsheetScatterStyle;
  bubbleScale?: number;
  showNegativeBubbles?: boolean;
  bubbleSizeRepresents?: WorkSpreadsheetBubbleSizeRepresents;
}

export async function readXlsxWorksheetCharts(
  archive: OoxmlPackage,
  worksheetPart: string,
  worksheet: Document
): Promise<XlsxWorksheetChart[]> {
  const drawingReference = directChildren(worksheet.documentElement, 'drawing')[0];
  const relationshipId = drawingReference
    ? (attribute(drawingReference, 'r:id') ?? attribute(drawingReference, 'id'))
    : null;
  if (!relationshipId) return [];
  const worksheetRelationships = await archive.relationships(worksheetPart);
  const drawingRelationship = worksheetRelationships.get(relationshipId);
  if (
    !drawingRelationship ||
    drawingRelationship.targetMode === 'External' ||
    !drawingRelationship.type.endsWith('/drawing') ||
    !archive.has(drawingRelationship.target)
  ) {
    return [];
  }

  const drawing = await archive.xml(drawingRelationship.target);
  const relationships = await archive.relationships(drawingRelationship.target);
  const charts: XlsxWorksheetChart[] = [];
  for (const [anchorIndex, anchor] of directChildren(drawing.documentElement).entries()) {
    if (!['twoCellAnchor', 'oneCellAnchor', 'absoluteAnchor'].includes(anchor.localName)) continue;
    const frame = directChild(anchor, 'graphicFrame');
    const chartReference = firstDescendant(frame, 'chart');
    const chartRelationshipId = chartReference
      ? (attribute(chartReference, 'r:id') ?? attribute(chartReference, 'id'))
      : null;
    const relationship = chartRelationshipId ? relationships.get(chartRelationshipId) : undefined;
    if (
      !frame ||
      !relationship ||
      relationship.targetMode === 'External' ||
      !relationship.type.endsWith('/chart') ||
      !archive.has(relationship.target)
    ) {
      continue;
    }
    const parsed = parseXlsxChart(await archive.xml(relationship.target));
    if (!parsed) continue;
    const properties = firstDescendant(frame, 'cNvPr');
    const sourceId = attribute(properties ?? frame, 'id') ?? String(anchorIndex + 1);
    charts.push({
      id: `xlsx-chart-${partNumber(drawingRelationship.target)}-${sourceId}`,
      name: attribute(properties ?? frame, 'name')?.trim() || `Worksheet chart ${anchorIndex + 1}`,
      altText: attribute(properties ?? frame, 'descr')?.trim() || undefined,
      ...parsed,
      ...readXlsxDrawingAnchor(anchor),
    });
  }
  return charts;
}

export function xlsxWorksheetChartsToSheet(
  charts: readonly XlsxWorksheetChart[],
  config: WorkSpreadsheetSheet['config']
): WorkSpreadsheetChart[] {
  return charts.flatMap((chart) => {
    const bounds = xlsxDrawingAnchorToBounds(chart, config);
    if (!bounds) return [];
    return [{ ...chart, ...bounds }];
  });
}

export function xlsxChartGraphicFrameXml(
  chart: WorkSpreadsheetChart,
  relationshipId: string,
  objectId: number,
  sheet: WorkSpreadsheetSheet
): string {
  return [
    '<xdr:twoCellAnchor editAs="oneCell">',
    xlsxTwoCellAnchorMarkers(chart, sheet),
    '<xdr:graphicFrame macro="">',
    '<xdr:nvGraphicFramePr>',
    `<xdr:cNvPr id="${objectId}" name="${escapeXml(chart.name.trim() || `Chart ${objectId}`)}"${
      chart.altText?.trim() ? ` descr="${escapeXml(chart.altText.trim())}"` : ''
    }/>`,
    '<xdr:cNvGraphicFramePr/>',
    '</xdr:nvGraphicFramePr>',
    '<xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm>',
    `<a:graphic><a:graphicData uri="${CHART_GRAPHIC_URI}">`,
    `<c:chart xmlns:c="${CHART_NAMESPACE}" xmlns:r="${RELATIONSHIP_NAMESPACE}" r:id="${relationshipId}"/>`,
    '</a:graphicData></a:graphic>',
    '</xdr:graphicFrame>',
    '<xdr:clientData/>',
    '</xdr:twoCellAnchor>',
  ].join('');
}

export function xlsxChartPartXml(
  chart: WorkSpreadsheetChart,
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetSheet,
  chartIndex: number
): string {
  const resolved = resolveSpreadsheetChart(content, ownerSheet, chart);
  const categoryAxisId = 10_000_000 + chartIndex * 2;
  const valueAxisId = categoryAxisId + 1;
  const secondaryCategoryAxisId = categoryAxisId + 2;
  const secondaryValueAxisId = categoryAxisId + 3;
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    `<c:chartSpace xmlns:c="${CHART_NAMESPACE}" xmlns:a="${DRAWINGML_NAMESPACE}" xmlns:r="${RELATIONSHIP_NAMESPACE}">`,
    '<c:date1904 val="0"/><c:lang val="zh-CN"/><c:roundedCorners val="0"/>',
    '<c:chart>',
    chartTitleXml(resolved),
    `<c:autoTitleDeleted val="${resolved.title?.trim() ? 0 : 1}"/>`,
    '<c:plotArea><c:layout/>',
    chartPlotXml(resolved, categoryAxisId, valueAxisId, secondaryCategoryAxisId, secondaryValueAxisId),
    resolved.type === 'pie' || resolved.type === 'doughnut'
      ? ''
      : chartAxesXml(resolved, categoryAxisId, valueAxisId, secondaryCategoryAxisId, secondaryValueAxisId),
    '</c:plotArea>',
    xlsxChartLegendXml(resolved),
    '<c:plotVisOnly val="1"/><c:dispBlanksAs val="gap"/><c:showDLblsOverMax val="0"/>',
    '</c:chart>',
    '<c:printSettings><c:headerFooter/><c:pageMargins b="0.75" l="0.7" r="0.7" t="0.75" header="0.3" footer="0.3"/><c:pageSetup/></c:printSettings>',
    '</c:chartSpace>',
  ].join('');
}

function parseXlsxChart(document: Document): Omit<XlsxWorksheetChart, keyof XlsxDrawingAnchor | 'id' | 'name'> | null {
  const plotArea = firstDescendant(document, 'plotArea');
  if (!plotArea) return null;
  const chartNodes = directChildren(plotArea).filter((element) => /Chart$/i.test(element.localName));
  if (isSupportedXlsxCombinationChartNodes(chartNodes)) {
    return parseXlsxCombinationChart(document, chartNodes);
  }
  if (chartNodes.length !== 1 || !SUPPORTED_CHART_ELEMENTS.has(chartNodes[0].localName)) return null;
  const chartNode = chartNodes[0];
  const type = xlsxChartType(chartNode);
  const seriesNodes = directChildren(chartNode, 'ser');
  if (!seriesNodes.length) return null;
  const chartDataLabels = parseXlsxDataLabels(directChild(chartNode, 'dLbls'), type);
  const parsedSeries = seriesNodes.map((node, index) => parseXlsxChartSeries(node, index, type, chartDataLabels));
  const series = xlsxChartNodeUsesStackedGrouping(chartNode)
    ? parsedSeries.map(withoutStackedChartAnalysis)
    : parsedSeries;
  const categoryNode = workSpreadsheetChartUsesNumericXAxis(type) ? null : directChild(seriesNodes[0], 'cat');
  const title = xlsxChartTitle(document);
  const axes = parseXlsxChartAxes(document, type, false);
  const legend = parseXlsxChartLegend(document);
  const layout = parseXlsxChartPlotLayout(chartNode, type);
  return {
    type,
    title: title ? cachedText(title) || richText(title) || undefined : undefined,
    titleReference: title ? formulaReference(title) : undefined,
    categories: categoryNode ? cachedValues(categoryNode) : [],
    categoryReference: categoryNode ? formulaReference(categoryNode) : undefined,
    series,
    ...legend,
    ...layout,
    ...(axes ? { axes } : {}),
    ...(type === 'doughnut' ? { doughnutHoleSize: readDoughnutHoleSize(chartNode) } : {}),
    ...(type === 'radar' ? { radarStyle: readRadarStyle(chartNode) } : {}),
    ...(type === 'scatter' ? { scatterStyle: readScatterStyle(chartNode) } : {}),
    ...(type === 'bubble'
      ? {
          bubbleScale: readBubbleScale(chartNode),
          showNegativeBubbles: readShowNegativeBubbles(chartNode),
          bubbleSizeRepresents: readBubbleSizeRepresents(chartNode),
        }
      : {}),
  };
}

function parseXlsxCombinationChart(
  document: Document,
  chartNodes: Element[]
): Omit<XlsxWorksheetChart, keyof XlsxDrawingAnchor | 'id' | 'name'> | null {
  const entries = chartNodes
    .flatMap((chartNode) => {
      const chartType = xlsxCombinationSeriesType(chartNode);
      if (!chartType) return [];
      const axisGroup = xlsxCombinationAxisGroup(document, chartNode);
      const chartDataLabels = parseXlsxDataLabels(directChild(chartNode, 'dLbls'), chartType);
      return directChildren(chartNode, 'ser').map((seriesNode, sourceIndex) => ({
        seriesNode,
        chartType,
        axisGroup,
        chartDataLabels,
        order: finiteNumber(
          attribute(directChild(seriesNode, 'order') ?? seriesNode, 'val') ?? sourceIndex,
          sourceIndex
        ),
      }));
    })
    .sort((left, right) => left.order - right.order);
  if (!entries.length) return null;
  const categoryNode = directChild(entries[0].seriesNode, 'cat');
  const title = xlsxChartTitle(document);
  const hasSecondaryAxes = entries.some((entry) => entry.axisGroup === 'secondary');
  const axes = parseXlsxChartAxes(document, 'combination', hasSecondaryAxes);
  const legend = parseXlsxChartLegend(document);
  return {
    type: 'combination',
    title: title ? cachedText(title) || richText(title) || undefined : undefined,
    titleReference: title ? formulaReference(title) : undefined,
    categories: categoryNode ? cachedValues(categoryNode) : [],
    categoryReference: categoryNode ? formulaReference(categoryNode) : undefined,
    series: entries.map((entry, index) => ({
      ...parseXlsxChartSeries(entry.seriesNode, index, entry.chartType, entry.chartDataLabels),
      chartType: entry.chartType,
      axisGroup: entry.axisGroup,
    })),
    ...legend,
    ...(axes ? { axes } : {}),
  };
}

function withoutStackedChartAnalysis(series: WorkSpreadsheetChartSeries): WorkSpreadsheetChartSeries {
  const editable = { ...series };
  delete editable.errorBars;
  delete editable.trendlines;
  return editable;
}

function xlsxChartTitle(document: Document): Element | undefined {
  const chart = directChild(document.documentElement, 'chart') ?? firstDescendant(document, 'chart');
  return chart ? directChild(chart, 'title') : undefined;
}

function xlsxCombinationSeriesType(node: Element): WorkSpreadsheetCombinationSeriesType | null {
  if (node.localName === 'lineChart') return 'line';
  if (node.localName === 'areaChart') return 'area';
  if (node.localName === 'barChart' && attribute(firstDescendant(node, 'barDir') ?? node, 'val') !== 'bar') {
    return 'column';
  }
  return null;
}

function xlsxCombinationAxisGroup(document: Document, chartNode: Element): WorkSpreadsheetChartAxisGroup {
  const axisIds = new Set(
    directChildren(chartNode, 'axId')
      .map((axis) => attribute(axis, 'val'))
      .filter((value): value is string => Boolean(value))
  );
  const usesRightAxis = descendants(document, 'valAx').some((axis) => {
    const axisId = attribute(firstDescendant(axis, 'axId') ?? axis, 'val');
    const position = attribute(firstDescendant(axis, 'axPos') ?? axis, 'val');
    return Boolean(axisId && axisIds.has(axisId) && position === 'r');
  });
  return usesRightAxis ? 'secondary' : 'primary';
}

function parseXlsxChartSeries(
  node: Element,
  index: number,
  type: WorkSpreadsheetChartType,
  inheritedDataLabels?: WorkSpreadsheetDataLabels
): WorkSpreadsheetChartSeries {
  const title = directChild(node, 'tx');
  const values = directChild(node, workSpreadsheetChartUsesNumericXAxis(type) ? 'yVal' : 'val');
  const xValues = workSpreadsheetChartUsesNumericXAxis(type) ? directChild(node, 'xVal') : null;
  const bubbleSizes = type === 'bubble' ? directChild(node, 'bubbleSize') : null;
  const dataLabelsNode = directChild(node, 'dLbls');
  const dataLabels = dataLabelsNode ? parseXlsxDataLabels(dataLabelsNode, type) : inheritedDataLabels;
  const errorBars = workSpreadsheetChartSupportsErrorBars(type)
    ? directChildren(node, 'errBars').flatMap((errorBarNode) => {
        const parsed = parseXlsxErrorBars(errorBarNode, type);
        return parsed ? [parsed] : [];
      })
    : [];
  const trendlines = workSpreadsheetChartSupportsTrendlines(type)
    ? directChildren(node, 'trendline').flatMap((trendline) => {
        const parsed = parseXlsxTrendline(trendline);
        return parsed ? [parsed] : [];
      })
    : [];
  const style = parseXlsxChartSeriesStyle(node);
  return {
    name: (title ? cachedText(title) || richText(title) : '') || `Series ${index + 1}`,
    nameReference: title ? formulaReference(title) : undefined,
    values: values ? cachedValues(values).map((value) => finiteNumber(value)) : [],
    valuesReference: values ? formulaReference(values) : undefined,
    ...(xValues
      ? {
          xValues: cachedValues(xValues).map((value) => finiteNumber(value)),
          xValuesReference: formulaReference(xValues),
        }
      : {}),
    ...(bubbleSizes
      ? {
          bubbleSizes: cachedValues(bubbleSizes).map((value) => finiteNumber(value)),
          bubbleSizesReference: formulaReference(bubbleSizes),
        }
      : {}),
    ...(dataLabels ? { dataLabels: { ...dataLabels } } : {}),
    ...(errorBars.length ? { errorBars } : {}),
    ...(trendlines.length ? { trendlines } : {}),
    ...(style ? { style } : {}),
  };
}

function parseXlsxDataLabels(
  node: Element | undefined,
  chartType: WorkSpreadsheetChartType
): WorkSpreadsheetDataLabels | undefined {
  if (!node || xlsxBooleanElement(node, 'delete')) return undefined;
  const separatorElement = directChild(node, 'separator');
  const position = xlsxDataLabelPosition(attribute(directChild(node, 'dLblPos') ?? node, 'val'));
  return normalizeWorkSpreadsheetDataLabels(
    {
      ...(xlsxBooleanElement(node, 'showVal') ? { showValue: true } : {}),
      ...(xlsxBooleanElement(node, 'showCatName') ? { showCategoryName: true } : {}),
      ...(xlsxBooleanElement(node, 'showSerName') ? { showSeriesName: true } : {}),
      ...(xlsxBooleanElement(node, 'showPercent') ? { showPercentage: true } : {}),
      ...(xlsxBooleanElement(node, 'showBubbleSize') ? { showBubbleSize: true } : {}),
      ...(separatorElement ? { separator: separatorElement.textContent ?? '' } : {}),
      ...(position ? { position } : {}),
    },
    chartType
  );
}

function xlsxDataLabelPosition(value: string | null): WorkSpreadsheetDataLabelPosition | undefined {
  if (value === 'ctr') return 'center';
  if (value === 'inBase') return 'insideBase';
  if (value === 'inEnd') return 'insideEnd';
  if (value === 'outEnd') return 'outsideEnd';
  if (value === 'l') return 'left';
  if (value === 'r') return 'right';
  if (value === 't') return 'above';
  if (value === 'b') return 'below';
  if (value === 'bestFit') return 'bestFit';
  return undefined;
}

function xlsxBooleanElement(parent: Element, localName: string): boolean {
  const element = directChild(parent, localName);
  if (!element) return false;
  const value = attribute(element, 'val');
  return value === null || readBooleanValue(value);
}

export function parseXlsxErrorBars(
  node: Element,
  chartType: WorkSpreadsheetChartType
): WorkSpreadsheetErrorBars | null {
  if (!workSpreadsheetChartSupportsErrorBars(chartType)) return null;
  const directionValue = attribute(directChild(node, 'errDir') ?? node, 'val');
  const direction = directionValue === 'x' ? 'x' : directionValue === 'y' ? 'y' : null;
  if (!direction || (direction === 'x' && !workSpreadsheetChartUsesNumericXAxis(chartType))) return null;

  const barTypeValue = attribute(directChild(node, 'errBarType') ?? node, 'val');
  const barType = barTypeValue === 'plus' || barTypeValue === 'minus' || barTypeValue === 'both' ? barTypeValue : null;
  const valueType = workSpreadsheetErrorBarValueType(attribute(directChild(node, 'errValType') ?? node, 'val'));
  if (!barType || !valueType) return null;

  const value = numericElementValue(node, 'val');
  const plus = directChild(node, 'plus');
  const minus = directChild(node, 'minus');
  return normalizeWorkSpreadsheetErrorBars(
    {
      direction,
      barType,
      valueType,
      ...(value !== undefined ? { value } : {}),
      ...(xlsxBooleanElement(node, 'noEndCap') ? { showEndCaps: false } : {}),
      ...(plus
        ? {
            plusValues: cachedValues(plus).map((item) => finiteNumber(item)),
            plusReference: formulaReference(plus),
          }
        : {}),
      ...(minus
        ? {
            minusValues: cachedValues(minus).map((item) => finiteNumber(item)),
            minusReference: formulaReference(minus),
          }
        : {}),
    },
    chartType
  );
}

function workSpreadsheetErrorBarValueType(value: string | null): WorkSpreadsheetErrorBarValueType | null {
  if (value === 'fixedVal') return 'fixedValue';
  if (value === 'percentage') return 'percentage';
  if (value === 'stdDev') return 'standardDeviation';
  if (value === 'stdErr') return 'standardError';
  if (value === 'cust') return 'custom';
  return null;
}

export function parseXlsxTrendline(node: Element): WorkSpreadsheetTrendline | null {
  const type = xlsxTrendlineType(attribute(firstDescendant(node, 'trendlineType') ?? node, 'val'));
  if (!type) return null;
  const name = directChild(node, 'name')?.textContent?.trim();
  const order = numericElementValue(node, 'order');
  const period = numericElementValue(node, 'period');
  const forward = numericElementValue(node, 'forward');
  const backward = numericElementValue(node, 'backward');
  const intercept = numericElementValue(node, 'intercept');
  return normalizeWorkSpreadsheetTrendline({
    type,
    ...(name ? { name } : {}),
    ...(order !== undefined ? { order } : {}),
    ...(period !== undefined ? { period } : {}),
    ...(forward !== undefined ? { forward } : {}),
    ...(backward !== undefined ? { backward } : {}),
    ...(intercept !== undefined ? { intercept } : {}),
    ...(xlsxBooleanElement(node, 'dispEq') ? { displayEquation: true } : {}),
    ...(xlsxBooleanElement(node, 'dispRSq') ? { displayRSquared: true } : {}),
  });
}

function xlsxTrendlineType(value: string | null): WorkSpreadsheetTrendlineType | null {
  if (value === 'exp') return 'exponential';
  if (value === 'log') return 'logarithmic';
  if (value === 'poly') return 'polynomial';
  if (value === 'power') return 'power';
  if (value === 'movingAvg') return 'movingAverage';
  if (value === 'linear') return 'linear';
  return null;
}

function numericElementValue(parent: Element, localName: string): number | undefined {
  const element = firstDescendant(parent, localName);
  if (!element) return undefined;
  const value = Number(attribute(element, 'val'));
  return Number.isFinite(value) ? value : undefined;
}

function xlsxChartType(node: Element): WorkSpreadsheetChartType {
  if (node.localName === 'lineChart') return 'line';
  if (node.localName === 'pieChart') return 'pie';
  if (node.localName === 'doughnutChart') return 'doughnut';
  if (node.localName === 'areaChart') return 'area';
  if (node.localName === 'radarChart') return 'radar';
  if (node.localName === 'scatterChart') return 'scatter';
  if (node.localName === 'bubbleChart') return 'bubble';
  return attribute(firstDescendant(node, 'barDir') ?? node, 'val') === 'bar' ? 'bar' : 'column';
}

function readDoughnutHoleSize(node: Element): number {
  return normalizeWorkSpreadsheetDoughnutHoleSize(attribute(firstDescendant(node, 'holeSize') ?? node, 'val'));
}

function readRadarStyle(node: Element): WorkSpreadsheetRadarStyle {
  return normalizeWorkSpreadsheetRadarStyle(attribute(firstDescendant(node, 'radarStyle') ?? node, 'val'));
}

function readScatterStyle(node: Element): WorkSpreadsheetScatterStyle {
  return normalizeWorkSpreadsheetScatterStyle(attribute(firstDescendant(node, 'scatterStyle') ?? node, 'val'));
}

function readBubbleScale(node: Element): number {
  return normalizeWorkSpreadsheetBubbleScale(attribute(firstDescendant(node, 'bubbleScale') ?? node, 'val'));
}

function readShowNegativeBubbles(node: Element): boolean {
  return readBooleanValue(attribute(firstDescendant(node, 'showNegBubbles') ?? node, 'val'));
}

function readBubbleSizeRepresents(node: Element): WorkSpreadsheetBubbleSizeRepresents {
  return normalizeWorkSpreadsheetBubbleSizeRepresents(
    attribute(firstDescendant(node, 'sizeRepresents') ?? node, 'val')
  );
}

function chartTitleXml(chart: WorkSpreadsheetChart): string {
  const title = chart.title?.trim();
  if (!title && !chart.titleReference?.trim()) return '';
  return chartTextTitleXml(title ?? '', chart.titleReference);
}

function chartPlotXml(
  chart: WorkSpreadsheetChart,
  categoryAxisId: number,
  valueAxisId: number,
  secondaryCategoryAxisId: number,
  secondaryValueAxisId: number
): string {
  if (chart.type === 'combination') {
    return combinationChartPlotXml(chart, categoryAxisId, valueAxisId, secondaryCategoryAxisId, secondaryValueAxisId);
  }
  const series = chart.series
    .map((item, index) =>
      workSpreadsheetChartUsesNumericXAxis(chart.type)
        ? xyChartSeriesXml(item, chart, index)
        : chartSeriesXml(item, chart, index)
    )
    .join('');
  if (chart.type === 'pie') {
    return `<c:pieChart><c:varyColors val="1"/>${series}<c:firstSliceAng val="0"/></c:pieChart>`;
  }
  if (chart.type === 'doughnut') {
    const holeSize = normalizeWorkSpreadsheetDoughnutHoleSize(chart.doughnutHoleSize);
    return `<c:doughnutChart><c:varyColors val="1"/>${series}<c:firstSliceAng val="0"/><c:holeSize val="${holeSize}"/></c:doughnutChart>`;
  }
  const axes = `<c:axId val="${categoryAxisId}"/><c:axId val="${valueAxisId}"/>`;
  if (chart.type === 'line') {
    return `<c:lineChart><c:grouping val="${xlsxChartGroupingValue(chart)}"/><c:varyColors val="0"/>${series}<c:marker val="1"/><c:smooth val="${
      xlsxChartSmoothLinesValue(chart) ? 1 : 0
    }"/>${axes}</c:lineChart>`;
  }
  if (chart.type === 'area') {
    return `<c:areaChart><c:grouping val="${xlsxChartGroupingValue(
      chart
    )}"/><c:varyColors val="0"/>${series}${axes}</c:areaChart>`;
  }
  if (chart.type === 'radar') {
    const style = normalizeWorkSpreadsheetRadarStyle(chart.radarStyle);
    return `<c:radarChart><c:radarStyle val="${style}"/><c:varyColors val="0"/>${series}${axes}</c:radarChart>`;
  }
  if (chart.type === 'scatter') {
    const style = normalizeWorkSpreadsheetScatterStyle(chart.scatterStyle);
    return `<c:scatterChart><c:scatterStyle val="${style}"/><c:varyColors val="0"/>${series}${axes}</c:scatterChart>`;
  }
  if (chart.type === 'bubble') {
    const scale = normalizeWorkSpreadsheetBubbleScale(chart.bubbleScale);
    const showNegative = chart.showNegativeBubbles === true ? 1 : 0;
    const sizeRepresents =
      normalizeWorkSpreadsheetBubbleSizeRepresents(chart.bubbleSizeRepresents) === 'width' ? 'w' : 'area';
    return `<c:bubbleChart><c:varyColors val="0"/>${series}<c:bubble3D val="0"/><c:bubbleScale val="${scale}"/><c:showNegBubbles val="${showNegative}"/><c:sizeRepresents val="${sizeRepresents}"/>${axes}</c:bubbleChart>`;
  }
  const direction = chart.type === 'bar' ? 'bar' : 'col';
  return `<c:barChart><c:barDir val="${direction}"/><c:grouping val="${xlsxChartGroupingValue(
    chart
  )}"/><c:varyColors val="0"/>${series}<c:gapWidth val="${xlsxChartGapWidthValue(
    chart
  )}"/><c:overlap val="${xlsxChartOverlapValue(chart)}"/>${axes}</c:barChart>`;
}

interface CombinationChartGroup {
  type: WorkSpreadsheetCombinationSeriesType;
  axisGroup: WorkSpreadsheetChartAxisGroup;
  series: Array<{ item: WorkSpreadsheetChartSeries; index: number }>;
}

function combinationChartPlotXml(
  chart: WorkSpreadsheetChart,
  categoryAxisId: number,
  valueAxisId: number,
  secondaryCategoryAxisId: number,
  secondaryValueAxisId: number
): string {
  const groups = new Map<string, CombinationChartGroup>();
  chart.series.forEach((item, index) => {
    const type = normalizeWorkSpreadsheetCombinationSeriesType(item.chartType);
    const axisGroup = normalizeWorkSpreadsheetChartAxisGroup(item.axisGroup);
    const key = `${type}:${axisGroup}`;
    const group = groups.get(key) ?? { type, axisGroup, series: [] };
    group.series.push({ item, index });
    groups.set(key, group);
  });
  return Array.from(groups.values())
    .map((group) => {
      const axes =
        group.axisGroup === 'secondary'
          ? `<c:axId val="${secondaryCategoryAxisId}"/><c:axId val="${secondaryValueAxisId}"/>`
          : `<c:axId val="${categoryAxisId}"/><c:axId val="${valueAxisId}"/>`;
      const seriesChart = {
        ...chart,
        type: group.type,
        grouping: group.type === 'column' ? 'clustered' : 'standard',
        gapWidth: group.type === 'column' ? 150 : undefined,
        overlap: group.type === 'column' ? 0 : undefined,
        smoothLines: false,
      } as WorkSpreadsheetChart;
      const series = group.series.map(({ item, index }) => chartSeriesXml(item, seriesChart, index)).join('');
      if (group.type === 'line') {
        return `<c:lineChart><c:grouping val="standard"/><c:varyColors val="0"/>${series}<c:marker val="1"/><c:smooth val="0"/>${axes}</c:lineChart>`;
      }
      if (group.type === 'area') {
        return `<c:areaChart><c:grouping val="standard"/><c:varyColors val="0"/>${series}${axes}</c:areaChart>`;
      }
      return `<c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:varyColors val="0"/>${series}<c:gapWidth val="150"/><c:overlap val="0"/>${axes}</c:barChart>`;
    })
    .join('');
}

function chartSeriesXml(series: WorkSpreadsheetChartSeries, chart: WorkSpreadsheetChart, index: number): string {
  const title = chartSeriesTitleXml(series, index);
  const categories = chart.categoryReference?.trim()
    ? `<c:strRef><c:f>${escapeXml(chart.categoryReference.replace(/^=/, ''))}</c:f>${stringCacheXml(
        chart.categories
      )}</c:strRef>`
    : stringLiteralXml(chart.categories);
  const values = series.valuesReference?.trim()
    ? `<c:numRef><c:f>${escapeXml(series.valuesReference.replace(/^=/, ''))}</c:f>${numberCacheXml(
        series.values
      )}</c:numRef>`
    : numberLiteralXml(series.values);
  const showMarkers =
    chart.type === 'line' ||
    (chart.type === 'radar' && normalizeWorkSpreadsheetRadarStyle(chart.radarStyle) === 'marker');
  const shapeProperties = xlsxChartSeriesShapePropertiesXml(series.style);
  const marker =
    chart.type === 'line' || chart.type === 'radar' ? xlsxChartSeriesMarkerXml(series.style, showMarkers) : '';
  const dataLabels = chartDataLabelsXml(series, chart.type);
  const supportsAnalysis = !xlsxChartGroupingIsStacked(chart);
  const trendlines =
    supportsAnalysis && workSpreadsheetChartSupportsTrendlines(chart.type) ? chartTrendlinesXml(series) : '';
  const errorBars = supportsAnalysis ? chartErrorBarsXml(series, chart.type) : '';
  return [
    '<c:ser>',
    `<c:idx val="${index}"/><c:order val="${index}"/>`,
    `<c:tx>${title}</c:tx>`,
    shapeProperties,
    marker,
    dataLabels,
    trendlines,
    errorBars,
    `<c:cat>${categories}</c:cat><c:val>${values}</c:val>`,
    chart.type === 'line' ? `<c:smooth val="${xlsxChartSmoothLinesValue(chart) ? 1 : 0}"/>` : '',
    '</c:ser>',
  ].join('');
}

function xyChartSeriesXml(series: WorkSpreadsheetChartSeries, chart: WorkSpreadsheetChart, index: number): string {
  const title = chartSeriesTitleXml(series, index);
  const xValues = series.xValues?.length
    ? series.xValues
    : Array.from({ length: series.values.length }, (_, valueIndex) => valueIndex + 1);
  const xValuesXml = numberReferenceOrLiteralXml(series.xValuesReference, xValues);
  const yValuesXml = numberReferenceOrLiteralXml(series.valuesReference, series.values);
  const dataLabels = chartDataLabelsXml(series, chart.type);
  const trendlines = chartTrendlinesXml(series);
  const errorBars = chartErrorBarsXml(series, chart.type);
  const shapeProperties = xlsxChartSeriesShapePropertiesXml(series.style);
  if (chart.type === 'bubble') {
    const bubbleSizesXml = numberReferenceOrLiteralXml(series.bubbleSizesReference, series.bubbleSizes ?? []);
    return [
      '<c:ser>',
      `<c:idx val="${index}"/><c:order val="${index}"/>`,
      `<c:tx>${title}</c:tx>`,
      shapeProperties,
      dataLabels,
      trendlines,
      errorBars,
      `<c:xVal>${xValuesXml}</c:xVal><c:yVal>${yValuesXml}</c:yVal>`,
      `<c:bubbleSize>${bubbleSizesXml}</c:bubbleSize><c:bubble3D val="0"/>`,
      '</c:ser>',
    ].join('');
  }
  const style = normalizeWorkSpreadsheetScatterStyle(chart.scatterStyle);
  const showMarkers = style === 'marker' || style === 'lineMarker' || style === 'smoothMarker';
  const smooth = style === 'smooth' || style === 'smoothMarker';
  return [
    '<c:ser>',
    `<c:idx val="${index}"/><c:order val="${index}"/>`,
    `<c:tx>${title}</c:tx>`,
    shapeProperties,
    xlsxChartSeriesMarkerXml(series.style, showMarkers),
    dataLabels,
    trendlines,
    errorBars,
    `<c:xVal>${xValuesXml}</c:xVal><c:yVal>${yValuesXml}</c:yVal>`,
    `<c:smooth val="${smooth ? 1 : 0}"/>`,
    '</c:ser>',
  ].join('');
}

function chartDataLabelsXml(series: WorkSpreadsheetChartSeries, chartType: WorkSpreadsheetChartType): string {
  if (!series.dataLabels) return '';
  const labels = normalizeWorkSpreadsheetDataLabels(series.dataLabels, chartType);
  return [
    '<c:dLbls>',
    labels.position ? `<c:dLblPos val="${xlsxDataLabelPositionValue(labels.position)}"/>` : '',
    '<c:showLegendKey val="0"/>',
    `<c:showVal val="${labels.showValue ? 1 : 0}"/>`,
    `<c:showCatName val="${labels.showCategoryName ? 1 : 0}"/>`,
    `<c:showSerName val="${labels.showSeriesName ? 1 : 0}"/>`,
    `<c:showPercent val="${labels.showPercentage ? 1 : 0}"/>`,
    `<c:showBubbleSize val="${labels.showBubbleSize ? 1 : 0}"/>`,
    labels.separator !== undefined ? `<c:separator>${escapeXml(labels.separator)}</c:separator>` : '',
    '<c:showLeaderLines val="0"/>',
    '</c:dLbls>',
  ].join('');
}

function xlsxDataLabelPositionValue(position: WorkSpreadsheetDataLabelPosition): string {
  if (position === 'center') return 'ctr';
  if (position === 'insideBase') return 'inBase';
  if (position === 'insideEnd') return 'inEnd';
  if (position === 'outsideEnd') return 'outEnd';
  if (position === 'left') return 'l';
  if (position === 'right') return 'r';
  if (position === 'above') return 't';
  if (position === 'below') return 'b';
  return 'bestFit';
}

function chartTrendlinesXml(series: WorkSpreadsheetChartSeries): string {
  return (series.trendlines ?? []).map(chartTrendlineXml).join('');
}

function chartErrorBarsXml(series: WorkSpreadsheetChartSeries, chartType: WorkSpreadsheetChartType): string {
  if (!workSpreadsheetChartSupportsErrorBars(chartType)) return '';
  const directions = new Set<string>();
  return (series.errorBars ?? [])
    .flatMap((source) => {
      if (source.direction === 'x' && !workSpreadsheetChartUsesNumericXAxis(chartType)) return [];
      const errorBars = normalizeWorkSpreadsheetErrorBars(source, chartType);
      if (directions.has(errorBars.direction)) return [];
      directions.add(errorBars.direction);
      return [chartErrorBarXml(errorBars)];
    })
    .join('');
}

function chartErrorBarXml(errorBars: WorkSpreadsheetErrorBars): string {
  const plus =
    errorBars.valueType === 'custom' && errorBars.barType !== 'minus'
      ? chartCustomErrorBarSourceXml('plus', errorBars.plusReference, errorBars.plusValues)
      : '';
  const minus =
    errorBars.valueType === 'custom' && errorBars.barType !== 'plus'
      ? chartCustomErrorBarSourceXml('minus', errorBars.minusReference, errorBars.minusValues)
      : '';
  const value =
    errorBars.valueType === 'fixedValue' ||
    errorBars.valueType === 'percentage' ||
    errorBars.valueType === 'standardDeviation'
      ? `<c:val val="${finiteNumber(errorBars.value, errorBars.valueType === 'percentage' ? 5 : 1)}"/>`
      : '';
  return [
    '<c:errBars>',
    `<c:errDir val="${errorBars.direction}"/>`,
    `<c:errBarType val="${errorBars.barType}"/>`,
    `<c:errValType val="${xlsxErrorBarValueType(errorBars.valueType)}"/>`,
    errorBars.showEndCaps === false ? '<c:noEndCap val="1"/>' : '',
    plus,
    minus,
    value,
    '</c:errBars>',
  ].join('');
}

function chartCustomErrorBarSourceXml(
  localName: 'plus' | 'minus',
  reference: string | undefined,
  values: readonly number[] | undefined
): string {
  if (!reference?.trim() && !values?.length) return '';
  return `<c:${localName}>${numberReferenceOrLiteralXml(reference, values ?? [])}</c:${localName}>`;
}

function xlsxErrorBarValueType(type: WorkSpreadsheetErrorBarValueType): string {
  if (type === 'fixedValue') return 'fixedVal';
  if (type === 'standardDeviation') return 'stdDev';
  if (type === 'standardError') return 'stdErr';
  if (type === 'custom') return 'cust';
  return 'percentage';
}

function chartTrendlineXml(source: WorkSpreadsheetTrendline): string {
  const trendline = normalizeWorkSpreadsheetTrendline(source);
  const type = xlsxTrendlineTypeValue(trendline.type);
  return [
    '<c:trendline>',
    trendline.name ? `<c:name>${escapeXml(trendline.name)}</c:name>` : '',
    `<c:trendlineType val="${type}"/>`,
    trendline.type === 'polynomial' ? `<c:order val="${trendline.order ?? 2}"/>` : '',
    trendline.type === 'movingAverage' ? `<c:period val="${trendline.period ?? 2}"/>` : '',
    trendline.forward ? `<c:forward val="${finiteNumber(trendline.forward)}"/>` : '',
    trendline.backward ? `<c:backward val="${finiteNumber(trendline.backward)}"/>` : '',
    trendline.intercept !== undefined ? `<c:intercept val="${finiteNumber(trendline.intercept)}"/>` : '',
    trendline.displayRSquared ? '<c:dispRSq val="1"/>' : '',
    trendline.displayEquation ? '<c:dispEq val="1"/>' : '',
    '</c:trendline>',
  ].join('');
}

function xlsxTrendlineTypeValue(type: WorkSpreadsheetTrendlineType): string {
  if (type === 'exponential') return 'exp';
  if (type === 'logarithmic') return 'log';
  if (type === 'polynomial') return 'poly';
  if (type === 'movingAverage') return 'movingAvg';
  return type;
}

function chartSeriesTitleXml(series: WorkSpreadsheetChartSeries, index: number): string {
  return series.nameReference?.trim()
    ? `<c:strRef><c:f>${escapeXml(series.nameReference.replace(/^=/, ''))}</c:f>${stringCacheXml([
        series.name,
      ])}</c:strRef>`
    : `<c:v>${escapeXml(series.name || `Series ${index + 1}`)}</c:v>`;
}

function numberReferenceOrLiteralXml(reference: string | undefined, values: readonly number[]): string {
  return reference?.trim()
    ? `<c:numRef><c:f>${escapeXml(reference.replace(/^=/, ''))}</c:f>${numberCacheXml(values)}</c:numRef>`
    : numberLiteralXml(values);
}

function readBooleanValue(value: string | null): boolean {
  return value === '1' || value === 'true';
}

function partNumber(path: string): string {
  return /(\d+)(?:\.xml)?$/i.exec(path)?.[1] ?? '1';
}
