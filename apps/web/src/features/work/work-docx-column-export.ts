import { normalizeDocumentColumns } from './work-document-columns';
import type { WorkDocumentSectionLayout } from './work-types';

const MIN_COLUMN_WIDTH_TWIPS = 240;

export function docxSectionColumns(layout: WorkDocumentSectionLayout, pageWidth: number, docx: typeof import('docx')) {
  const columns = normalizeDocumentColumns(layout.columns);
  if (!columns.custom) {
    return {
      count: columns.count,
      space: millimetersToTwips(columns.spacing),
      separate: columns.separator,
      equalWidth: true,
    };
  }

  const contentWidth = Math.max(
    columns.count * MIN_COLUMN_WIDTH_TWIPS,
    pageWidth - millimetersToTwips(layout.margins.left + layout.margins.right)
  );
  const requestedSpaces = columns.custom.slice(0, -1).map((column) => millimetersToTwips(column.spacing));
  const maximumSpaceWidth = Math.max(0, contentWidth - columns.count * MIN_COLUMN_WIDTH_TWIPS);
  const requestedSpaceWidth = requestedSpaces.reduce((total, value) => total + value, 0);
  const spaceScale = requestedSpaceWidth > maximumSpaceWidth ? maximumSpaceWidth / requestedSpaceWidth : 1;
  const spaces = requestedSpaces.map((space) => Math.round(space * spaceScale));
  const availableWidth = contentWidth - spaces.reduce((total, value) => total + value, 0);
  const widths = columns.custom.map((column) => Math.round((availableWidth * column.widthPercent) / 100));
  widths[widths.length - 1] += availableWidth - widths.reduce((total, value) => total + value, 0);

  return {
    count: columns.count,
    separate: columns.separator,
    equalWidth: false,
    children: columns.custom.map(
      (_, index) =>
        new docx.Column({
          width: Math.max(MIN_COLUMN_WIDTH_TWIPS, widths[index]),
          space: index < spaces.length ? spaces[index] : undefined,
        })
    ),
  };
}

function millimetersToTwips(value: number): number {
  return Math.round((value * 1440) / 25.4);
}
