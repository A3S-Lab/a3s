import JSZip from 'jszip';
import {
  normalizePresentationChartDataLabelPosition,
  normalizePresentationChartDataLabels,
} from './work-presentation-charts';
import { attribute, directChild, directChildren, firstDescendant, parseXml } from './work-ooxml-package';
import type {
  WorkSlide,
  WorkSlideChart,
  WorkSlideChartDataLabelPosition,
  WorkSlideChartDataLabels,
  WorkSlideChartType,
} from './work-types';

export interface PptxChartDataLabelDiagnostic {
  code: string;
  message: string;
}

export interface PptxChartDataLabelReadResult {
  dataLabels?: WorkSlideChartDataLabels;
  diagnostics: PptxChartDataLabelDiagnostic[];
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
const DATA_LABEL_ANCHORS = new Set([
  'dropLines',
  'hiLowLines',
  'upDownBars',
  'marker',
  'smooth',
  'gapWidth',
  'overlap',
  'firstSliceAng',
  'holeSize',
  'bubble3D',
  'bubbleScale',
  'showNegBubbles',
  'sizeRepresents',
  'axId',
  'extLst',
]);

export function readPptxChartDataLabels(
  chartNode: Element | undefined,
  chartType: WorkSlideChartType
): PptxChartDataLabelReadResult {
  if (!chartNode) return { diagnostics: [] };
  const diagnostics: PptxChartDataLabelDiagnostic[] = [];
  const series = directChildren(chartNode, 'ser');
  const chartLabels = directChild(chartNode, 'dLbls');
  const seriesLabels = series.flatMap((item) => directChildren(item, 'dLbls'));
  const source = chartLabels ?? seriesLabels[0];
  if (!source || pptxBooleanElement(source, 'delete')) return { diagnostics };

  if (chartLabels && seriesLabels.length) {
    diagnostics.push({
      code: 'pptx.chart.data-label-overrides',
      message: 'Per-series data-label overrides were normalized to the chart-level editable settings.',
    });
  } else if (
    !chartLabels &&
    (seriesLabels.length !== series.length || new Set(seriesLabels.map(dataLabelSignature)).size > 1)
  ) {
    diagnostics.push({
      code: 'pptx.chart.data-label-overrides',
      message: 'Different per-series data-label settings were normalized to one editable chart-level setting.',
    });
  }
  const allLabelNodes = [...(chartLabels ? [chartLabels] : []), ...seriesLabels];
  if (allLabelNodes.some((labels) => directChildren(labels, 'dLbl').length > 0)) {
    diagnostics.push({
      code: 'pptx.chart.data-label-overrides',
      message: 'Per-point data-label overrides were normalized to the chart-level editable settings.',
    });
  }

  const rawPosition = attribute(directChild(source, 'dLblPos') ?? source, 'val');
  const parsedPosition = pptxDataLabelPosition(rawPosition);
  const normalizedPosition = normalizePresentationChartDataLabelPosition(parsedPosition, chartType);
  if (rawPosition && (!parsedPosition || normalizedPosition !== parsedPosition)) {
    diagnostics.push({
      code: 'pptx.chart.data-label-position',
      message: `The data-label position “${rawPosition}” was normalized for this editable chart type.`,
    });
  }
  const separatorElement = directChild(source, 'separator');
  const separator = separatorElement?.textContent ?? undefined;
  if (separator && separator.length > 64) {
    diagnostics.push({
      code: 'pptx.chart.data-label-separator',
      message: 'The data-label separator exceeded 64 characters and was truncated.',
    });
  }
  const showPercentage = pptxBooleanElement(source, 'showPercent');
  const showBubbleSize = pptxBooleanElement(source, 'showBubbleSize');
  if (showPercentage && chartType !== 'pie' && chartType !== 'doughnut') {
    diagnostics.push({
      code: 'pptx.chart.data-label-content',
      message: 'Percentage data labels are supported only for editable pie and doughnut charts.',
    });
  }
  if (showBubbleSize && chartType !== 'bubble') {
    diagnostics.push({
      code: 'pptx.chart.data-label-content',
      message: 'Bubble-size data labels are supported only for editable bubble charts.',
    });
  }
  if (pptxBooleanElement(source, 'showLegendKey') || pptxBooleanElement(source, 'showLeaderLines')) {
    diagnostics.push({
      code: 'pptx.chart.data-label-content',
      message: 'Legend-key or leader-line data-label options were normalized.',
    });
  }
  if (['numFmt', 'spPr', 'txPr', 'extLst'].some((name) => directChildren(source, name).length > 0)) {
    diagnostics.push({
      code: 'pptx.chart.data-label-format',
      message: 'Advanced data-label number and text formatting is normalized by the editable chart model.',
    });
  }
  return {
    dataLabels: normalizePresentationChartDataLabels(
      {
        ...(pptxBooleanElement(source, 'showVal') ? { showValue: true } : {}),
        ...(pptxBooleanElement(source, 'showCatName') ? { showCategoryName: true } : {}),
        ...(pptxBooleanElement(source, 'showSerName') ? { showSeriesName: true } : {}),
        ...(showPercentage ? { showPercentage: true } : {}),
        ...(showBubbleSize ? { showBubbleSize: true } : {}),
        ...(separator !== undefined ? { separator } : {}),
        position: normalizedPosition,
      },
      chartType
    ),
    diagnostics,
  };
}

export async function patchPptxChartDataLabels(
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
  for (const [index, path] of chartPaths.entries()) {
    const entry = archive.file(path);
    if (!entry) continue;
    const document = parseXml(await entry.async('text'), path);
    const chartNode = CHART_TYPES.map((name) => firstDescendant(document, name)).find(Boolean);
    if (!chartNode) throw new Error(`PPTX chart part is missing a supported chart node: ${path}`);
    writePptxChartDataLabels(document, chartNode, charts[index]);
    archive.file(path, new XMLSerializer().serializeToString(document));
  }
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export function writePptxChartDataLabels(document: Document, chartNode: Element, chart: WorkSlideChart): void {
  for (const element of directChildren(chartNode, 'dLbls')) element.remove();
  for (const series of directChildren(chartNode, 'ser')) {
    for (const element of directChildren(series, 'dLbls')) element.remove();
  }
  if (!chart.dataLabels) return;
  const labels = normalizePresentationChartDataLabels(chart.dataLabels, chart.type);
  const namespace = chartNode.namespaceURI;
  const prefix = chartNode.lookupPrefix(namespace) ?? chartNode.prefix ?? 'c';
  const container = document.createElementNS(namespace, `${prefix}:dLbls`);
  appendChartElement(document, container, prefix, 'dLblPos', {
    val: pptxDataLabelPositionValue(normalizePresentationChartDataLabelPosition(labels.position, chart.type)),
  });
  appendChartElement(document, container, prefix, 'showLegendKey', { val: '0' });
  appendChartElement(document, container, prefix, 'showVal', { val: labels.showValue ? '1' : '0' });
  appendChartElement(document, container, prefix, 'showCatName', { val: labels.showCategoryName ? '1' : '0' });
  appendChartElement(document, container, prefix, 'showSerName', { val: labels.showSeriesName ? '1' : '0' });
  appendChartElement(document, container, prefix, 'showPercent', { val: labels.showPercentage ? '1' : '0' });
  appendChartElement(document, container, prefix, 'showBubbleSize', { val: labels.showBubbleSize ? '1' : '0' });
  if (labels.separator !== undefined) {
    const separator = appendChartElement(document, container, prefix, 'separator');
    separator.textContent = labels.separator;
  }
  appendChartElement(document, container, prefix, 'showLeaderLines', { val: '0' });
  const anchor = directChildren(chartNode).find((child) => DATA_LABEL_ANCHORS.has(child.localName)) ?? null;
  chartNode.insertBefore(container, anchor);
}

function appendChartElement(
  document: Document,
  parent: Element,
  prefix: string,
  localName: string,
  attributes: Record<string, string> = {}
): Element {
  const element = document.createElementNS(parent.namespaceURI, `${prefix}:${localName}`);
  for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
  parent.append(element);
  return element;
}

function pptxDataLabelPosition(value: string | null): WorkSlideChartDataLabelPosition | undefined {
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

function pptxDataLabelPositionValue(position: WorkSlideChartDataLabelPosition): string {
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

function pptxBooleanElement(parent: Element, localName: string): boolean {
  const element = directChild(parent, localName);
  if (!element) return false;
  const value = attribute(element, 'val');
  return value === null || value === '1' || value.toLocaleLowerCase() === 'true';
}

function dataLabelSignature(element: Element): string {
  return [
    attribute(directChild(element, 'dLblPos') ?? element, 'val') ?? '',
    String(pptxBooleanElement(element, 'showVal')),
    String(pptxBooleanElement(element, 'showCatName')),
    String(pptxBooleanElement(element, 'showSerName')),
    String(pptxBooleanElement(element, 'showPercent')),
    String(pptxBooleanElement(element, 'showBubbleSize')),
    directChild(element, 'separator')?.textContent ?? '',
  ].join('|');
}

function chartPartNumber(path: string): number {
  return Number(/chart(\d+)\.xml$/.exec(path)?.[1] ?? Number.MAX_SAFE_INTEGER);
}
