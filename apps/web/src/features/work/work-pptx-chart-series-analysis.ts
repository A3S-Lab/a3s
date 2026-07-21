import JSZip from 'jszip';
import { attribute, directChild, directChildren, firstDescendant, parseXml } from './work-ooxml-package';
import {
  normalizeWorkSpreadsheetErrorBars,
  normalizeWorkSpreadsheetTrendline,
  type WorkSlide,
  type WorkSlideChart,
  type WorkSpreadsheetErrorBars,
  type WorkSpreadsheetTrendline,
  workSpreadsheetChartSupportsErrorBars,
  workSpreadsheetChartSupportsTrendlines,
  workSpreadsheetChartUsesNumericXAxis,
} from './work-types';
import { parseXlsxErrorBars, parseXlsxTrendline } from './work-xlsx-charts';

export interface PptxChartSeriesAnalysisDiagnostic {
  code: string;
  message: string;
}

export interface PptxChartSeriesAnalysisReadResult {
  trendlines?: WorkSpreadsheetTrendline[];
  errorBars?: WorkSpreadsheetErrorBars[];
  diagnostics: PptxChartSeriesAnalysisDiagnostic[];
}

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

export function readPptxChartSeriesAnalysis(
  seriesNode: Element,
  chartType: WorkSlideChart['type']
): PptxChartSeriesAnalysisReadResult {
  const diagnostics: PptxChartSeriesAnalysisDiagnostic[] = [];
  const trendlineNodes = directChildren(seriesNode, 'trendline');
  const errorBarNodes = directChildren(seriesNode, 'errBars');
  if (
    (!workSpreadsheetChartSupportsTrendlines(chartType) && trendlineNodes.length) ||
    (!workSpreadsheetChartSupportsErrorBars(chartType) && errorBarNodes.length)
  ) {
    diagnostics.push({
      code: 'pptx.chart.series-analysis-type',
      message:
        'Trendlines or error bars on this chart type were omitted because the editable chart type does not support them.',
    });
  }

  const trendlines = workSpreadsheetChartSupportsTrendlines(chartType)
    ? trendlineNodes.flatMap((node) => {
        const trendline = parseXlsxTrendline(node);
        if (!trendline) {
          diagnostics.push({
            code: 'pptx.chart.trendline-type',
            message: 'An unsupported or incomplete trendline was omitted from the editable chart.',
          });
          return [];
        }
        readTrendlineDiagnostics(node, diagnostics);
        return [trendline];
      })
    : [];

  const directions = new Set<string>();
  const errorBars = workSpreadsheetChartSupportsErrorBars(chartType)
    ? errorBarNodes.flatMap((node) => {
        const errorBar = parseXlsxErrorBars(node, chartType);
        if (!errorBar) {
          diagnostics.push({
            code: 'pptx.chart.error-bars-type',
            message: 'Unsupported or incomplete error-bar settings were omitted from the editable chart.',
          });
          return [];
        }
        if (directions.has(errorBar.direction)) {
          diagnostics.push({
            code: 'pptx.chart.error-bars-duplicate',
            message: `Multiple ${errorBar.direction.toUpperCase()} error bars on one series were normalized to the first editable setting.`,
          });
          return [];
        }
        directions.add(errorBar.direction);
        readErrorBarDiagnostics(node, errorBar, diagnostics);
        return [errorBar];
      })
    : [];

  return {
    ...(trendlines.length ? { trendlines } : {}),
    ...(errorBars.length ? { errorBars } : {}),
    diagnostics,
  };
}

export async function patchPptxChartSeriesAnalysis(
  buffer: ArrayBuffer,
  slides: readonly WorkSlide[]
): Promise<ArrayBuffer> {
  const charts = slides.flatMap((slide) =>
    slide.elements.flatMap((element) => (element.type === 'chart' && element.chart ? [element.chart] : []))
  );
  if (!charts.some(chartHasSeriesAnalysis)) return buffer;
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
    writePptxChartSeriesAnalysis(document, chartNode, chart);
    archive.file(path, new XMLSerializer().serializeToString(document));
  }
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export function writePptxChartSeriesAnalysis(document: Document, chartNode: Element, chart: WorkSlideChart): void {
  const seriesNodes = directChildren(chartNode, 'ser');
  if (seriesNodes.length !== chart.series.length) {
    throw new Error(
      `PPTX chart export produced ${seriesNodes.length} native series for ${chart.series.length} editable series.`
    );
  }
  for (const [seriesIndex, seriesNode] of seriesNodes.entries()) {
    for (const name of ['trendline', 'errBars']) {
      for (const child of directChildren(seriesNode, name)) child.remove();
    }
    const series = chart.series[seriesIndex];
    const anchor =
      directChildren(seriesNode).find((child) =>
        ['cat', 'val', 'xVal', 'yVal', 'bubbleSize', 'smooth', 'extLst'].includes(child.localName)
      ) ?? null;
    if (workSpreadsheetChartSupportsTrendlines(chart.type)) {
      for (const source of series.trendlines ?? []) {
        seriesNode.insertBefore(createTrendlineElement(document, seriesNode, source), anchor);
      }
    }
    if (workSpreadsheetChartSupportsErrorBars(chart.type)) {
      const directions = new Set<string>();
      for (const source of series.errorBars ?? []) {
        const errorBars = normalizeWorkSpreadsheetErrorBars(source, chart.type);
        if (errorBars.direction === 'x' && !workSpreadsheetChartUsesNumericXAxis(chart.type)) continue;
        if (directions.has(errorBars.direction)) continue;
        directions.add(errorBars.direction);
        seriesNode.insertBefore(createErrorBarsElement(document, seriesNode, errorBars), anchor);
      }
    }
  }
}

function readTrendlineDiagnostics(node: Element, diagnostics: PptxChartSeriesAnalysisDiagnostic[]): void {
  if (directChild(node, 'trendlineLbl')) {
    diagnostics.push({
      code: 'pptx.chart.trendline-label',
      message:
        'Manual trendline-label positioning and formatting were normalized to the editable equation and R-squared display.',
    });
  }
  if (['spPr', 'extLst'].some((name) => directChildren(node, name).length > 0)) {
    diagnostics.push({
      code: 'pptx.chart.trendline-format',
      message: 'Advanced trendline styling and extensions are normalized by the editable chart model.',
    });
  }
  const type = attribute(directChild(node, 'trendlineType') ?? node, 'val');
  const parameter =
    type === 'poly' ? numericSetting(node, 'order') : type === 'movingAvg' ? numericSetting(node, 'period') : undefined;
  const parameterInvalid =
    (type === 'poly' && (parameter === undefined || parameter < 2 || parameter > 6 || !Number.isInteger(parameter))) ||
    (type === 'movingAvg' &&
      (parameter === undefined || parameter < 2 || parameter > 255 || !Number.isInteger(parameter)));
  const forecastInvalid = ['forward', 'backward'].some((name) => {
    const value = numericSetting(node, name);
    return directChild(node, name) !== undefined && (value === undefined || value < 0);
  });
  if (parameterInvalid || forecastInvalid) {
    diagnostics.push({
      code: 'pptx.chart.trendline-parameter',
      message: 'Trendline order, period, or forecast values were normalized to the editable range.',
    });
  }
}

function readErrorBarDiagnostics(
  node: Element,
  errorBars: WorkSpreadsheetErrorBars,
  diagnostics: PptxChartSeriesAnalysisDiagnostic[]
): void {
  if (['spPr', 'extLst'].some((name) => directChildren(node, name).length > 0)) {
    diagnostics.push({
      code: 'pptx.chart.error-bars-format',
      message: 'Advanced error-bar styling and extensions are normalized by the editable chart model.',
    });
  }
  const incompatibleCustomSource =
    errorBars.valueType === 'custom' &&
    ((errorBars.barType === 'plus' && directChild(node, 'minus')) ||
      (errorBars.barType === 'minus' && directChild(node, 'plus')));
  const value = numericSetting(node, 'val');
  const invalidValue =
    directChild(node, 'val') !== undefined &&
    (value === undefined || value < 0) &&
    errorBars.valueType !== 'standardError' &&
    errorBars.valueType !== 'custom';
  if (incompatibleCustomSource || invalidValue) {
    diagnostics.push({
      code: 'pptx.chart.error-bars-value',
      message: 'Error-bar values or custom sources were normalized to the editable direction and non-negative range.',
    });
  }
}

function createTrendlineElement(document: Document, parent: Element, source: WorkSpreadsheetTrendline): Element {
  const trendline = normalizeWorkSpreadsheetTrendline(source);
  const container = createChartElement(document, parent, 'trendline');
  if (trendline.name) appendTextElement(document, container, 'name', trendline.name);
  appendSetting(document, container, 'trendlineType', trendlineTypeValue(trendline.type));
  if (trendline.type === 'polynomial') appendSetting(document, container, 'order', trendline.order ?? 2);
  if (trendline.type === 'movingAverage') appendSetting(document, container, 'period', trendline.period ?? 2);
  if (trendline.forward) appendSetting(document, container, 'forward', trendline.forward);
  if (trendline.backward) appendSetting(document, container, 'backward', trendline.backward);
  if (trendline.intercept !== undefined) appendSetting(document, container, 'intercept', trendline.intercept);
  if (trendline.displayRSquared) appendSetting(document, container, 'dispRSq', 1);
  if (trendline.displayEquation) appendSetting(document, container, 'dispEq', 1);
  return container;
}

function createErrorBarsElement(document: Document, parent: Element, errorBars: WorkSpreadsheetErrorBars): Element {
  const container = createChartElement(document, parent, 'errBars');
  appendSetting(document, container, 'errDir', errorBars.direction);
  appendSetting(document, container, 'errBarType', errorBars.barType);
  appendSetting(document, container, 'errValType', errorBarValueType(errorBars.valueType));
  if (errorBars.showEndCaps === false) appendSetting(document, container, 'noEndCap', 1);
  if (errorBars.valueType === 'custom') {
    if (errorBars.barType !== 'minus') {
      appendCustomErrorSource(document, container, 'plus', errorBars.plusReference, errorBars.plusValues);
    }
    if (errorBars.barType !== 'plus') {
      appendCustomErrorSource(document, container, 'minus', errorBars.minusReference, errorBars.minusValues);
    }
  } else if (
    errorBars.valueType === 'fixedValue' ||
    errorBars.valueType === 'percentage' ||
    errorBars.valueType === 'standardDeviation'
  ) {
    appendSetting(document, container, 'val', errorBars.value ?? (errorBars.valueType === 'percentage' ? 5 : 1));
  }
  return container;
}

function appendCustomErrorSource(
  document: Document,
  parent: Element,
  localName: 'plus' | 'minus',
  reference: string | undefined,
  values: readonly number[] | undefined
): void {
  if (!reference?.trim() && !values?.length) return;
  const container = appendChartElement(document, parent, localName);
  if (reference?.trim()) {
    const numberReference = appendChartElement(document, container, 'numRef');
    appendTextElement(document, numberReference, 'f', reference.trim().replace(/^=/, ''));
    if (values?.length) appendNumberCollection(document, numberReference, 'numCache', values);
    return;
  }
  appendNumberCollection(document, container, 'numLit', values ?? []);
}

function appendNumberCollection(
  document: Document,
  parent: Element,
  localName: 'numCache' | 'numLit',
  values: readonly number[]
): void {
  const collection = appendChartElement(document, parent, localName);
  appendTextElement(document, collection, 'formatCode', 'General');
  appendSetting(document, collection, 'ptCount', values.length);
  for (const [index, value] of values.entries()) {
    const point = appendSetting(document, collection, 'pt', index, 'idx');
    appendTextElement(document, point, 'v', String(finiteNumber(value)));
  }
}

function appendSetting(
  document: Document,
  parent: Element,
  localName: string,
  value: string | number,
  attributeName = 'val'
): Element {
  const element = appendChartElement(document, parent, localName);
  element.setAttribute(attributeName, String(value));
  return element;
}

function appendTextElement(document: Document, parent: Element, localName: string, value: string): Element {
  const element = appendChartElement(document, parent, localName);
  element.textContent = value;
  return element;
}

function appendChartElement(document: Document, parent: Element, localName: string): Element {
  const prefix = parent.lookupPrefix(parent.namespaceURI) ?? parent.prefix ?? 'c';
  const element = document.createElementNS(parent.namespaceURI, `${prefix}:${localName}`);
  parent.append(element);
  return element;
}

function createChartElement(document: Document, parent: Element, localName: string): Element {
  const prefix = parent.lookupPrefix(parent.namespaceURI) ?? parent.prefix ?? 'c';
  return document.createElementNS(parent.namespaceURI, `${prefix}:${localName}`);
}

function trendlineTypeValue(type: WorkSpreadsheetTrendline['type']): string {
  if (type === 'exponential') return 'exp';
  if (type === 'logarithmic') return 'log';
  if (type === 'polynomial') return 'poly';
  if (type === 'movingAverage') return 'movingAvg';
  return type;
}

function errorBarValueType(type: WorkSpreadsheetErrorBars['valueType']): string {
  if (type === 'fixedValue') return 'fixedVal';
  if (type === 'standardDeviation') return 'stdDev';
  if (type === 'standardError') return 'stdErr';
  if (type === 'custom') return 'cust';
  return 'percentage';
}

function chartHasSeriesAnalysis(chart: WorkSlideChart): boolean {
  return chart.series.some((series) => series.trendlines?.length || series.errorBars?.length);
}

function numericSetting(parent: Element, localName: string): number | undefined {
  const element = directChild(parent, localName);
  if (!element) return undefined;
  const value = Number(attribute(element, 'val'));
  return Number.isFinite(value) ? value : undefined;
}

function finiteNumber(value: unknown): number {
  const number = Number(value);
  return Number.isFinite(number) ? number : 0;
}

function chartPartNumber(path: string): number {
  return Number(/chart(\d+)\.xml$/.exec(path)?.[1] ?? Number.MAX_SAFE_INTEGER);
}
