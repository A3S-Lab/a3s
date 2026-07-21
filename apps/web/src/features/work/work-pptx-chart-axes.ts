import JSZip from 'jszip';
import { presentationChartAxes } from './work-presentation-chart-axes';
import { attribute, directChild, directChildren, firstDescendant, parseXml } from './work-ooxml-package';
import {
  workSpreadsheetChartAxisDefaultLabelPosition,
  workSpreadsheetChartAxisIsCategoryAxis,
  workSpreadsheetChartAxisIsValueAxis,
  workSpreadsheetChartAxisShowsMajorGridlinesByDefault,
} from './work-spreadsheet-chart-axis';
import type { WorkSlide, WorkSlideChart, WorkSlideChartAxis, WorkSpreadsheetChartAxisPosition } from './work-types';

const DRAWING_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/main';

export async function patchPptxChartAxes(buffer: ArrayBuffer, slides: readonly WorkSlide[]): Promise<ArrayBuffer> {
  const charts = slides.flatMap((slide) =>
    slide.elements.flatMap((element) => (element.type === 'chart' && element.chart ? [element.chart] : []))
  );
  if (!charts.some((chart) => presentationChartAxes(chart))) return buffer;
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
    const axes = presentationChartAxes(chart);
    if (!axes) continue;
    const path = chartPaths[index];
    const entry = archive.file(path);
    if (!entry) throw new Error(`PPTX chart export is missing chart part: ${path}`);
    const document = parseXml(await entry.async('text'), path);
    const plotArea = firstDescendant(document, 'plotArea');
    if (!plotArea) throw new Error(`PPTX chart part is missing its plot area: ${path}`);
    const bottom = findChartAxis(plotArea, 'bottom');
    const left = findChartAxis(plotArea, 'left');
    if (!bottom || !left) throw new Error(`PPTX chart part is missing an editable primary axis: ${path}`);
    writePptxChartAxis(document, bottom, chart, 'bottom', axes.bottom);
    writePptxChartAxis(document, left, chart, 'left', axes.left);
    archive.file(path, new XMLSerializer().serializeToString(document));
  }
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export function writePptxChartAxis(
  document: Document,
  axisNode: Element,
  chart: WorkSlideChart,
  position: WorkSpreadsheetChartAxisPosition,
  axis: WorkSlideChartAxis | undefined
): void {
  writeScaling(document, axisNode, axis, workSpreadsheetChartAxisIsValueAxis(chart.type, position));
  replaceAxisTitle(document, axisNode, axis);
  replaceAxisValueElement(document, axisNode, 'majorTickMark', nativeTickMark(axis?.majorTickMark), [
    'minorTickMark',
    'tickLblPos',
    'spPr',
    'txPr',
    'crossAx',
    'extLst',
  ]);
  replaceAxisValueElement(
    document,
    axisNode,
    'tickLblPos',
    axis?.labelPosition ?? workSpreadsheetChartAxisDefaultLabelPosition(chart.type, position),
    ['spPr', 'txPr', 'crossAx', 'extLst']
  );

  const valueAxis = workSpreadsheetChartAxisIsValueAxis(chart.type, position);
  if (valueAxis) {
    replaceAxisGridlines(
      document,
      axisNode,
      axis?.showMajorGridlines ?? workSpreadsheetChartAxisShowsMajorGridlinesByDefault(chart.type, position)
    );
    replaceAxisNumberFormat(document, axisNode, axis);
    replaceAxisValueElement(
      document,
      axisNode,
      'majorUnit',
      Number.isFinite(axis?.majorUnit) && Number(axis?.majorUnit) > 0 ? String(Number(axis?.majorUnit)) : undefined,
      ['minorUnit', 'dispUnits', 'extLst']
    );
  }
  if (workSpreadsheetChartAxisIsCategoryAxis(chart.type, position)) {
    replaceAxisValueElement(
      document,
      axisNode,
      'tickLblSkip',
      Number.isInteger(axis?.labelInterval) && Number(axis?.labelInterval) >= 1
        ? String(Number(axis?.labelInterval))
        : undefined,
      ['tickMarkSkip', 'noMultiLvlLbl', 'extLst']
    );
  }
}

function findChartAxis(plotArea: Element, position: 'bottom' | 'left'): Element | undefined {
  const nativePosition = position === 'bottom' ? 'b' : 'l';
  return directChildren(plotArea).find(
    (child) =>
      (child.localName === 'catAx' || child.localName === 'valAx' || child.localName === 'dateAx') &&
      attribute(directChild(child, 'axPos') ?? child, 'val') === nativePosition
  );
}

function writeScaling(
  document: Document,
  axisNode: Element,
  axis: WorkSlideChartAxis | undefined,
  includeBounds: boolean
): void {
  const scaling = directChild(axisNode, 'scaling') ?? createChartElement(document, axisNode, 'scaling');
  if (!scaling.parentElement) {
    const anchor = directChildren(axisNode).find((child) => child.localName !== 'axId') ?? null;
    axisNode.insertBefore(scaling, anchor);
  }
  for (const name of ['orientation', 'max', 'min']) {
    for (const child of directChildren(scaling, name)) child.remove();
  }
  appendChartValue(document, scaling, 'orientation', axis?.reverseOrder ? 'maxMin' : 'minMax');
  if (includeBounds && Number.isFinite(axis?.maximum)) {
    appendChartValue(document, scaling, 'max', String(Number(axis?.maximum)));
  }
  if (includeBounds && Number.isFinite(axis?.minimum)) {
    appendChartValue(document, scaling, 'min', String(Number(axis?.minimum)));
  }
}

function replaceAxisGridlines(document: Document, axisNode: Element, visible: boolean): void {
  removeDirectChildren(axisNode, 'majorGridlines');
  if (!visible) return;
  const element = createChartElement(document, axisNode, 'majorGridlines');
  insertBeforeNames(axisNode, element, [
    'minorGridlines',
    'title',
    'numFmt',
    'majorTickMark',
    'minorTickMark',
    'tickLblPos',
    'spPr',
    'txPr',
    'crossAx',
    'extLst',
  ]);
}

function replaceAxisNumberFormat(document: Document, axisNode: Element, axis: WorkSlideChartAxis | undefined): void {
  removeDirectChildren(axisNode, 'numFmt');
  const element = createChartElement(document, axisNode, 'numFmt');
  const numberFormat = axis?.numberFormat?.trim();
  element.setAttribute('formatCode', numberFormat || 'General');
  element.setAttribute(
    'sourceLinked',
    numberFormat
      ? axis?.numberFormatSourceLinked === true
        ? '1'
        : '0'
      : axis?.numberFormatSourceLinked === false
        ? '0'
        : '1'
  );
  insertBeforeNames(axisNode, element, [
    'majorTickMark',
    'minorTickMark',
    'tickLblPos',
    'spPr',
    'txPr',
    'crossAx',
    'extLst',
  ]);
}

function replaceAxisTitle(document: Document, axisNode: Element, axis: WorkSlideChartAxis | undefined): void {
  removeDirectChildren(axisNode, 'title');
  const title = axis?.title?.trim();
  const reference = axis?.titleReference?.trim().replace(/^=/, '');
  if (!title && !reference) return;
  const titleNode = createChartElement(document, axisNode, 'title');
  const text = appendChartElement(document, titleNode, 'tx');
  if (reference) {
    const stringReference = appendChartElement(document, text, 'strRef');
    appendChartText(document, stringReference, 'f', reference);
    const cache = appendChartElement(document, stringReference, 'strCache');
    appendChartValue(document, cache, 'ptCount', '1');
    const point = appendChartValue(document, cache, 'pt', '0', 'idx');
    appendChartText(document, point, 'v', title ?? '');
  } else {
    const rich = appendChartElement(document, text, 'rich');
    appendDrawingElement(document, rich, 'bodyPr');
    appendDrawingElement(document, rich, 'lstStyle');
    const paragraph = appendDrawingElement(document, rich, 'p');
    const run = appendDrawingElement(document, paragraph, 'r');
    const runProperties = appendDrawingElement(document, run, 'rPr');
    runProperties.setAttribute('lang', 'zh-CN');
    runProperties.setAttribute('sz', '1400');
    const textNode = appendDrawingElement(document, run, 't');
    textNode.textContent = title ?? '';
    const endProperties = appendDrawingElement(document, paragraph, 'endParaRPr');
    endProperties.setAttribute('lang', 'zh-CN');
  }
  appendChartElement(document, titleNode, 'layout');
  appendChartValue(document, titleNode, 'overlay', '0');
  insertBeforeNames(axisNode, titleNode, [
    'numFmt',
    'majorTickMark',
    'minorTickMark',
    'tickLblPos',
    'spPr',
    'txPr',
    'crossAx',
    'extLst',
  ]);
}

function replaceAxisValueElement(
  document: Document,
  axisNode: Element,
  localName: string,
  value: string | undefined,
  anchors: readonly string[]
): void {
  removeDirectChildren(axisNode, localName);
  if (value === undefined) return;
  const element = createChartElement(document, axisNode, localName);
  element.setAttribute('val', value);
  insertBeforeNames(axisNode, element, anchors);
}

function nativeTickMark(value: WorkSlideChartAxis['majorTickMark']): string {
  if (value === 'inside') return 'in';
  if (value === 'outside') return 'out';
  if (value === 'cross') return 'cross';
  return 'none';
}

function removeDirectChildren(parent: Element, localName: string): void {
  for (const child of directChildren(parent, localName)) child.remove();
}

function insertBeforeNames(parent: Element, element: Element, names: readonly string[]): void {
  const anchor = directChildren(parent).find((child) => names.includes(child.localName)) ?? null;
  parent.insertBefore(element, anchor);
}

function appendChartValue(
  document: Document,
  parent: Element,
  localName: string,
  value: string,
  attributeName = 'val'
): Element {
  const element = appendChartElement(document, parent, localName);
  element.setAttribute(attributeName, value);
  return element;
}

function appendChartText(document: Document, parent: Element, localName: string, value: string): Element {
  const element = appendChartElement(document, parent, localName);
  element.textContent = value;
  return element;
}

function appendChartElement(document: Document, parent: Element, localName: string): Element {
  const element = createChartElement(document, parent, localName);
  parent.append(element);
  return element;
}

function createChartElement(document: Document, parent: Element, localName: string): Element {
  const prefix = parent.lookupPrefix(parent.namespaceURI) ?? parent.prefix ?? 'c';
  return document.createElementNS(parent.namespaceURI, `${prefix}:${localName}`);
}

function appendDrawingElement(document: Document, parent: Element, localName: string): Element {
  const prefix = parent.lookupPrefix(DRAWING_NAMESPACE) ?? 'a';
  const element = document.createElementNS(DRAWING_NAMESPACE, `${prefix}:${localName}`);
  parent.append(element);
  return element;
}

function chartPartNumber(path: string): number {
  return Number(/chart(\d+)\.xml$/.exec(path)?.[1] ?? Number.MAX_SAFE_INTEGER);
}
