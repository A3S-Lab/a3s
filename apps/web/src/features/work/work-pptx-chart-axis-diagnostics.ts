import { attribute, directChild, directChildren, firstDescendant } from './work-ooxml-package';

export interface PptxChartAxisDiagnostic {
  code: string;
  message: string;
}

export function readPptxChartAxisDiagnostics(document: Document): PptxChartAxisDiagnostic[] {
  const plotArea = firstDescendant(document, 'plotArea');
  if (!plotArea) return [];
  const axes = directChildren(plotArea).filter(
    (child) => child.localName === 'catAx' || child.localName === 'valAx' || child.localName === 'dateAx'
  );
  const diagnostics: PptxChartAxisDiagnostic[] = [];
  if (
    axes.length > 2 ||
    axes.some((axis) => ['t', 'r'].includes(attribute(directChild(axis, 'axPos') ?? axis, 'val') ?? ''))
  ) {
    diagnostics.push({
      code: 'pptx.chart.axis-secondary',
      message:
        'Secondary or additional chart axes were normalized to the editable primary horizontal and vertical axes.',
    });
  }
  if (axes.some((axis) => axis.localName === 'dateAx')) {
    diagnostics.push({
      code: 'pptx.chart.axis-date',
      message: 'Date-axis units and calendar behavior were normalized to an editable category axis.',
    });
  }
  if (axes.some((axis) => directChild(directChild(axis, 'scaling') ?? axis, 'logBase'))) {
    diagnostics.push({
      code: 'pptx.chart.axis-logarithmic',
      message: 'Logarithmic chart-axis scaling was normalized to the editable linear scale.',
    });
  }
  if (axes.some((axis) => directChild(axis, 'minorUnit'))) {
    diagnostics.push({
      code: 'pptx.chart.axis-minor-unit',
      message: 'Minor chart-axis units were normalized because only the primary unit is editable.',
    });
  }
  if (axes.some((axis) => directChild(axis, 'dispUnits'))) {
    diagnostics.push({
      code: 'pptx.chart.axis-display-units',
      message: 'Chart-axis display units were flattened into the editable number format.',
    });
  }
  if (
    axes.some(
      (axis) =>
        directChild(axis, 'minorGridlines') || directChild(axis, 'minorTickMark') || directChild(axis, 'tickMarkSkip')
    )
  ) {
    diagnostics.push({
      code: 'pptx.chart.axis-minor-display',
      message: 'Minor gridlines, minor tick marks, or tick-mark intervals were normalized to primary-axis settings.',
    });
  }
  if (axes.some((axis) => directChild(axis, 'crossesAt'))) {
    diagnostics.push({
      code: 'pptx.chart.axis-crossing',
      message: 'Custom chart-axis crossing points were normalized to automatic crossing.',
    });
  }
  return diagnostics;
}
