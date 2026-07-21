import JSZip from 'jszip';
import {
  normalizePresentationBubbleScale,
  normalizePresentationBubbleSizeRepresents,
  normalizePresentationScatterStyle,
} from './work-presentation-charts';
import { directChild, directChildren, firstDescendant, parseXml } from './work-ooxml-package';
import type { WorkSlide, WorkSlideChart } from './work-types';

export async function patchPptxChartXySettings(
  buffer: ArrayBuffer,
  slides: readonly WorkSlide[]
): Promise<ArrayBuffer> {
  const charts = slides.flatMap((slide) =>
    slide.elements.flatMap((element) => (element.type === 'chart' && element.chart ? [element.chart] : []))
  );
  if (!charts.some((chart) => chart.type === 'scatter' || chart.type === 'bubble')) return buffer;
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
    if (chart.type !== 'scatter' && chart.type !== 'bubble') continue;
    const path = chartPaths[index];
    const entry = archive.file(path);
    if (!entry) throw new Error(`PPTX chart export is missing chart part: ${path}`);
    const document = parseXml(await entry.async('text'), path);
    const chartNode = firstDescendant(document, chart.type === 'scatter' ? 'scatterChart' : 'bubbleChart');
    if (!chartNode) throw new Error(`PPTX chart part does not contain the expected ${chart.type} chart: ${path}`);
    writePptxChartXySettings(document, chartNode, chart);
    archive.file(path, new XMLSerializer().serializeToString(document));
  }
  return archive.generateAsync({ type: 'arraybuffer', compression: 'DEFLATE' });
}

export function writePptxChartXySettings(document: Document, chartNode: Element, chart: WorkSlideChart): void {
  if (chart.type === 'scatter') {
    replaceChartSetting(
      document,
      chartNode,
      'scatterStyle',
      normalizePresentationScatterStyle(chart.scatterStyle),
      directChildren(chartNode).find((child) => child.localName !== 'scatterStyle') ?? null
    );
    return;
  }
  if (chart.type !== 'bubble') return;
  const anchor = directChildren(chartNode).find((child) => child.localName === 'axId') ?? null;
  for (const name of ['bubble3D', 'bubbleScale', 'showNegBubbles', 'sizeRepresents']) {
    for (const child of directChildren(chartNode, name)) child.remove();
  }
  insertChartSetting(document, chartNode, 'bubble3D', '0', anchor);
  insertChartSetting(
    document,
    chartNode,
    'bubbleScale',
    String(normalizePresentationBubbleScale(chart.bubbleScale)),
    anchor
  );
  insertChartSetting(document, chartNode, 'showNegBubbles', chart.showNegativeBubbles ? '1' : '0', anchor);
  insertChartSetting(
    document,
    chartNode,
    'sizeRepresents',
    normalizePresentationBubbleSizeRepresents(chart.bubbleSizeRepresents) === 'width' ? 'w' : 'area',
    anchor
  );
  for (const series of directChildren(chartNode, 'ser')) {
    const bubble3D = directChild(series, 'bubble3D');
    if (bubble3D) bubble3D.setAttribute('val', '0');
  }
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

function chartPartNumber(path: string): number {
  return Number(/chart(\d+)\.xml$/.exec(path)?.[1] ?? Number.MAX_SAFE_INTEGER);
}
