import { patchPptxChartAxes } from './work-pptx-chart-axes';
import { patchPptxChartDataLabels } from './work-pptx-chart-data-labels';
import { patchPptxChartSeriesAnalysis } from './work-pptx-chart-series-analysis';
import { patchPptxChartXySettings } from './work-pptx-chart-xy';
import { patchPptxComments } from './work-pptx-comments';
import { definePptxSlideLayouts } from './work-pptx-layout-export';
import { patchPptxTransitions } from './work-pptx-transition';
import { presentationChartAxes } from './work-presentation-chart-axes';
import {
  normalizeDoughnutHoleSize,
  normalizePresentationChartLegendPosition,
  normalizePresentationScatterStyle,
  normalizeRadarStyle,
  presentationChartBubbleSizes,
  presentationChartShowsLegend,
  presentationChartSupportsAxisTitles,
  presentationChartUsesNumericXAxis,
  presentationChartXValues,
} from './work-presentation-charts';
import type { WorkArtifact, WorkSlideChart, WorkSlideChartLegendPosition, WorkSlideElement } from './work-types';

type PptxConstructor = typeof import('pptxgenjs').default;
type PptxPresentation = InstanceType<PptxConstructor>;
type PptxSlide = ReturnType<PptxPresentation['addSlide']>;

export function createPptxPresentation(artifact: WorkArtifact, PptxGenJS: PptxConstructor): PptxPresentation {
  if (artifact.content.type !== 'presentation') {
    throw new Error('Only presentation artifacts can be exported as PPTX.');
  }
  const presentation = new PptxGenJS();
  const slideWidth = artifact.content.width ?? 13.333;
  const slideHeight = artifact.content.height ?? 7.5;
  presentation.defineLayout({ name: 'A3S_WORK', width: slideWidth, height: slideHeight });
  presentation.layout = 'A3S_WORK';
  presentation.author = 'A3S Work';
  presentation.subject = artifact.title;
  presentation.title = artifact.title;
  presentation.company = 'A3S Lab';
  const design = definePptxSlideLayouts(presentation, artifact.content, slideWidth, slideHeight);
  for (const source of design.content.slides) {
    const binding = design.bindings.get(source.layoutId ?? '') ?? design.bindings.values().next().value;
    const slide = binding ? presentation.addSlide({ masterName: binding.masterName }) : presentation.addSlide();
    if (!source.useLayoutBackground) {
      slide.background = { color: source.background.replace('#', '') };
    }
    for (const element of source.elements) {
      addPresentationElement(
        slide,
        element,
        presentation,
        slideWidth,
        slideHeight,
        element.placeholder ? binding?.placeholderNames.get(element.placeholder.key) : undefined
      );
    }
    if (source.notes?.trim()) slide.addNotes(source.notes);
  }
  return presentation;
}

export async function createPptxBlob(artifact: WorkArtifact, PptxGenJS: PptxConstructor): Promise<Blob> {
  const presentation = createPptxPresentation(artifact, PptxGenJS);
  const output = await presentation.write({ outputType: 'arraybuffer', compression: true });
  const buffer = await presentationArrayBuffer(output);
  const slides = artifact.content.type === 'presentation' ? artifact.content.slides : [];
  const withChartXySettings = await patchPptxChartXySettings(buffer, slides);
  const withChartDataLabels = await patchPptxChartDataLabels(withChartXySettings, slides);
  const withChartAxes = await patchPptxChartAxes(withChartDataLabels, slides);
  const withChartSeriesAnalysis = await patchPptxChartSeriesAnalysis(withChartAxes, slides);
  const withTransitions = await patchPptxTransitions(withChartSeriesAnalysis, slides);
  const patched = await patchPptxComments(
    withTransitions,
    slides,
    artifact.content.type === 'presentation' ? (artifact.content.width ?? 13.333) : 13.333,
    artifact.content.type === 'presentation' ? (artifact.content.height ?? 7.5) : 7.5
  );
  return new Blob([patched], {
    type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
  });
}

function addPresentationElement(
  slide: PptxSlide,
  element: WorkSlideElement,
  presentation: PptxPresentation,
  slideWidth: number,
  slideHeight: number,
  placeholder?: string
) {
  const x = (element.x / 100) * slideWidth;
  const y = (element.y / 100) * slideHeight;
  const width = (element.width / 100) * slideWidth;
  const height = (element.height / 100) * slideHeight;
  const color = element.color.replace('#', '');
  const fillColor = element.fill.replace('#', '');
  const borderColor = (element.borderColor ?? element.fill).replace('#', '');
  const rotation = element.rotation;

  if (element.placeholder && !element.text && !element.image && !element.table && !element.chart) {
    return;
  }
  if (element.type === 'image' && element.image) {
    slide.addImage({
      data: element.image.dataUrl,
      x,
      y,
      w: width,
      h: height,
      rotate: rotation,
      placeholder,
    });
    return;
  }
  if (element.type === 'table' && element.table) {
    const options = {
      x,
      y,
      w: width,
      h: height,
      border: { color: borderColor, pt: element.borderWidth ?? 1 },
      color,
      fill: { color: fillColor },
      fontFace: element.fontFamily ?? 'Aptos',
      fontSize: Math.max(7, element.fontSize * 0.75),
      margin: 0.06,
      placeholder,
    };
    slide.addTable(
      element.table.rows.map((row) => row.map((text) => ({ text }))),
      options
    );
    return;
  }
  if (element.type === 'chart' && element.chart) {
    const axes = presentationChartAxes(element.chart);
    const categoryAxis = element.chart.type === 'bar' ? axes?.left : axes?.bottom;
    const valueAxis = element.chart.type === 'bar' ? axes?.bottom : axes?.left;
    const chartType =
      element.chart.type === 'scatter'
        ? presentation.ChartType.scatter
        : element.chart.type === 'bubble'
          ? presentation.ChartType.bubble
          : element.chart.type === 'line'
            ? presentation.ChartType.line
            : element.chart.type === 'pie'
              ? presentation.ChartType.pie
              : element.chart.type === 'doughnut'
                ? presentation.ChartType.doughnut
                : element.chart.type === 'radar'
                  ? presentation.ChartType.radar
                  : element.chart.type === 'area'
                    ? presentation.ChartType.area
                    : presentation.ChartType.bar;
    const options = {
      x,
      y,
      w: width,
      h: height,
      catAxisLabelFontFace: element.fontFamily ?? 'Aptos',
      chartColors: ['4472C4', 'ED7D31', 'A5A5A5', 'FFC000', '5B9BD5', '70AD47'],
      showLegend: presentationChartShowsLegend(element.chart),
      legendPos: pptxLegendPosition(element.chart.legendPosition),
      showTitle: Boolean(element.chart.title),
      title: element.chart.title,
      placeholder,
      ...(presentationChartSupportsAxisTitles(element.chart)
        ? {
            catAxisTitle: categoryAxis?.title,
            showCatAxisTitle: Boolean(categoryAxis?.title),
            valAxisTitle: valueAxis?.title,
            showValAxisTitle: Boolean(valueAxis?.title),
          }
        : {}),
      ...(element.chart.type === 'column' ? { barDir: 'col' as const } : {}),
      ...(element.chart.type === 'doughnut'
        ? { holeSize: normalizeDoughnutHoleSize(element.chart.doughnutHoleSize) }
        : {}),
      ...(element.chart.type === 'radar' ? { radarStyle: normalizeRadarStyle(element.chart.radarStyle) } : {}),
      ...(element.chart.type === 'scatter' ? pptxScatterOptions(element.chart) : {}),
    };
    slide.addChart(chartType, pptxChartData(element.chart), options);
    return;
  }
  if (element.type === 'line') {
    slide.addShape(presentation.ShapeType.line, {
      x,
      y,
      w: width,
      h: height,
      rotate: rotation,
      line: { color: borderColor, width: element.borderWidth ?? 1 },
    });
    return;
  }
  if (element.type === 'shape') {
    addShape(slide, element, presentation, x, y, width, height, placeholder);
  }
  if (element.text) addText(slide, element, x, y, width, height, placeholder);
}

function pptxChartData(chart: WorkSlideChart) {
  if (presentationChartUsesNumericXAxis(chart.type)) {
    return [
      { name: 'X values', values: presentationChartXValues(chart) },
      ...chart.series.map((series) => ({
        name: series.name,
        values: series.values,
        ...(chart.type === 'bubble' ? { sizes: presentationChartBubbleSizes(series) } : {}),
      })),
    ];
  }
  return chart.series.map((series) => ({
    name: series.name,
    labels: chart.categories,
    values: series.values,
  }));
}

function pptxScatterOptions(chart: WorkSlideChart) {
  const style = normalizePresentationScatterStyle(chart.scatterStyle);
  return {
    lineDataSymbol: style === 'line' || style === 'smooth' ? ('none' as const) : ('circle' as const),
    lineSize: style === 'marker' ? 0 : 2,
    lineSmooth: style === 'smooth' || style === 'smoothMarker',
  };
}

function pptxLegendPosition(position: WorkSlideChartLegendPosition | undefined) {
  const normalized = normalizePresentationChartLegendPosition(position);
  if (normalized === 'left') return 'l' as const;
  if (normalized === 'top') return 't' as const;
  if (normalized === 'bottom') return 'b' as const;
  if (normalized === 'topRight') return 'tr' as const;
  return 'r' as const;
}

function addShape(
  slide: PptxSlide,
  element: WorkSlideElement,
  presentation: PptxPresentation,
  x: number,
  y: number,
  width: number,
  height: number,
  placeholder?: string
) {
  const shapeType =
    element.shapeType === 'ellipse'
      ? presentation.ShapeType.ellipse
      : element.shapeType === 'triangle'
        ? presentation.ShapeType.triangle
        : element.shapeType === 'diamond'
          ? presentation.ShapeType.diamond
          : element.shapeType === 'roundRect'
            ? presentation.ShapeType.roundRect
            : presentation.ShapeType.rect;
  const fillColor = element.fill.replace('#', '');
  const borderColor = (element.borderColor ?? element.fill).replace('#', '');
  const options = {
    x,
    y,
    w: width,
    h: height,
    rotate: element.rotation,
    placeholder,
    fill:
      element.fill === 'transparent'
        ? { color: 'FFFFFF', transparency: 100 }
        : {
            color: fillColor,
            transparency: Math.round((1 - (element.opacity ?? 1)) * 100),
          },
    line: {
      color: borderColor,
      width: element.borderWidth ?? 0,
      transparency: element.borderWidth ? 0 : 100,
    },
  };
  slide.addShape(shapeType, options);
}

function addText(
  slide: PptxSlide,
  element: WorkSlideElement,
  x: number,
  y: number,
  width: number,
  height: number,
  placeholder?: string
) {
  const text = element.textRuns?.length
    ? element.textRuns.map((run) => ({
        text: run.text,
        options: {
          fontFace: run.fontFamily ?? element.fontFamily ?? 'Aptos',
          fontSize: Math.max(7, (run.fontSize ?? element.fontSize) * 0.75),
          color: (run.color ?? element.color).replace('#', ''),
          bold: run.bold,
          italic: run.italic,
          underline: run.underline ? { style: 'sng' as const } : undefined,
          hyperlink: run.href ? { url: run.href } : undefined,
        },
      }))
    : element.text;
  slide.addText(text, {
    x,
    y,
    w: width,
    h: height,
    rotate: element.rotation,
    fontFace: element.fontFamily ?? 'Aptos',
    fontSize: Math.max(8, element.fontSize * 0.75),
    color: element.color.replace('#', ''),
    bold: element.bold,
    italic: element.italic,
    underline: element.underline ? { style: 'sng' } : undefined,
    align: element.align,
    valign: element.verticalAlign ?? 'middle',
    margin: element.fill === 'transparent' ? 0 : 10,
    breakLine: false,
    fill: element.fill === 'transparent' ? undefined : { color: element.fill.replace('#', '') },
    hyperlink: element.href ? { url: element.href } : undefined,
    placeholder,
  });
}

async function presentationArrayBuffer(output: string | ArrayBuffer | Blob | Uint8Array): Promise<ArrayBuffer> {
  if (output instanceof ArrayBuffer) return output;
  if (output instanceof Blob) return output.arrayBuffer();
  if (output instanceof Uint8Array) return output.slice().buffer as ArrayBuffer;
  throw new Error('PowerPoint export returned an unsupported binary representation.');
}
