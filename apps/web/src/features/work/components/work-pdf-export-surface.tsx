import { millimetersToPixels } from '../work-document-layout';
import { spreadsheetChartSvgDataUrl } from '../work-spreadsheet-chart-svg';
import { resolveSpreadsheetChart } from '../work-spreadsheet-charts';
import { spreadsheetConditionalIconAppearance } from '../work-spreadsheet-conditional-icons';
import { spreadsheetConditionalFormatStyles } from '../work-spreadsheet-conditional-format';
import {
  hasSpreadsheetHeaderFooterSections,
  resolveSpreadsheetHeaderFooterTemplate,
  type SpreadsheetHeaderFooterTokenContext,
} from '../work-spreadsheet-header-footer';
import {
  effectiveSpreadsheetPageSetup,
  fitSpreadsheetAxisCapacity,
  spreadsheetPageCapacity,
  type EffectiveSpreadsheetPageSetup,
} from '../work-spreadsheet-page-setup';
import { refreshSpreadsheetPivotTables } from '../work-spreadsheet-pivots';
import { spreadsheetPrintBounds, spreadsheetPrintTitleBounds } from '../work-spreadsheet-ranges';
import type {
  WorkArtifact,
  WorkPresentationPrintLayout,
  WorkSpreadsheetChart,
  WorkSpreadsheetContent,
  WorkSpreadsheetImage,
  WorkSpreadsheetSheet,
} from '../work-types';
import { WorkDocumentPdfPages } from './work-document-pages';
import { WorkPresentationPdfPages } from './work-presentation-pdf-pages';

export function WorkPdfExportSurface({
  artifact,
  presentationLayout = 'slides',
  mode = 'export',
}: {
  artifact: WorkArtifact;
  presentationLayout?: WorkPresentationPrintLayout;
  mode?: 'export' | 'preview';
}) {
  if (artifact.content.type === 'pdf') return null;
  return (
    <div
      className={`work-pdf-export-surface ${mode}`}
      data-work-pdf-artifact={artifact.id}
      data-work-pdf-surface={mode}
      data-presentation-layout={artifact.content.type === 'presentation' ? presentationLayout : undefined}
      aria-hidden={mode === 'export' ? 'true' : undefined}
    >
      {artifact.content.type === 'document' && (
        <WorkDocumentPdfPages content={artifact.content} title={artifact.title} />
      )}
      {artifact.content.type === 'spreadsheet' && (
        <SpreadsheetPages content={artifact.content} title={artifact.title} fileName={`${artifact.title}.xlsx`} />
      )}
      {artifact.content.type === 'presentation' && (
        <WorkPresentationPdfPages content={artifact.content} layout={presentationLayout} />
      )}
    </div>
  );
}

function SpreadsheetPages({
  content,
  title,
  fileName,
}: {
  content: WorkSpreadsheetContent;
  title: string;
  fileName: string;
}) {
  const materializedContent = refreshSpreadsheetPivotTables(content);
  const visibleSheets = materializedContent.sheets.filter((sheet) => !sheet.hide);
  const sheets = visibleSheets.length ? visibleSheets : materializedContent.sheets;
  return sheets.flatMap((sheet) => {
    const data = sheet.data ?? [];
    const conditionalStyles = spreadsheetConditionalFormatStyles(sheet);
    const maximumRow = Math.max(lastUsedRow(data), lastWorksheetDrawingRow(sheet));
    const maximumColumn = Math.max(lastUsedColumn(data), lastWorksheetDrawingColumn(sheet));
    const printArea = materializedContent.printAreas?.find((area) => area.sheetId === sheet.id);
    const printTitles = materializedContent.printTitles?.find((titles) => titles.sheetId === sheet.id);
    const pageBreaks = materializedContent.pageBreaks?.find((breaks) => breaks.sheetId === sheet.id);
    const savedPageSetup = materializedContent.pageSetups?.find((pageSetup) => pageSetup.sheetId === sheet.id);
    const pageSetup = effectiveSpreadsheetPageSetup(savedPageSetup);
    const bounds = printArea ? spreadsheetPrintBounds(printArea.reference, maximumRow, maximumColumn) : null;
    const titleBounds = printTitles
      ? spreadsheetPrintTitleBounds(printTitles.rows, printTitles.columns, maximumRow, maximumColumn)
      : null;
    const startRow = bounds?.startRow ?? 0;
    const endRow = bounds?.endRow ?? maximumRow;
    const startColumn = bounds?.startColumn ?? 0;
    const endColumn = bounds?.endColumn ?? maximumColumn;
    const defaultCapacity = spreadsheetPageCapacity(pageSetup);
    const rowCapacity =
      pageSetup.fitToPage && pageSetup.fitToHeight
        ? fitSpreadsheetAxisCapacity(startRow, endRow, titleBounds?.rows, pageSetup.fitToHeight, defaultCapacity.rows)
        : defaultCapacity.rows;
    const columnCapacity =
      pageSetup.fitToPage && pageSetup.fitToWidth
        ? fitSpreadsheetAxisCapacity(
            startColumn,
            endColumn,
            titleBounds?.columns,
            pageSetup.fitToWidth,
            defaultCapacity.columns
          )
        : defaultCapacity.columns;
    const rowPages = paginateSpreadsheetAxis(startRow, endRow, titleBounds?.rows, pageBreaks?.rows, rowCapacity);
    const columnPages = paginateSpreadsheetAxis(
      startColumn,
      endColumn,
      titleBounds?.columns,
      pageBreaks?.columns,
      columnCapacity
    );
    const pageDescriptors = spreadsheetPageDescriptors(rowPages, columnPages, pageSetup.pageOrder);
    const hasCustomHeader = hasSpreadsheetHeaderFooterSections(pageSetup.header);
    const hasCustomFooter = hasSpreadsheetHeaderFooterSections(pageSetup.footer);
    const now = new Date();
    return pageDescriptors.map(({ rowPage, columnPage }, index) => {
      const pageNumber = pageSetup.pageNumberStart + index;
      const pageRange = `${sheet.name} · ${cellAddress(rowPage.bodyStart, columnPage.bodyStart)}–${cellAddress(
        rowPage.bodyEnd,
        columnPage.bodyEnd
      )}`;
      const tokenContext: SpreadsheetHeaderFooterTokenContext = {
        page: pageNumber,
        pages: pageDescriptors.length,
        sheetName: sheet.name,
        fileName,
        now,
      };
      const pageImages = spreadsheetPageImages(sheet, rowPage, columnPage);
      const pageCharts = spreadsheetPageCharts(materializedContent, sheet, rowPage, columnPage);
      return (
        <section
          className={`work-pdf-export-page spreadsheet ${pageSetup.paperSize} ${pageSetup.orientation}`}
          data-work-pdf-page
          data-pdf-orientation={pageSetup.orientation}
          data-pdf-page-size={pageSetup.paperSize}
          data-pdf-print-area={printArea?.reference}
          data-pdf-print-title-rows={printTitles?.rows}
          data-pdf-print-title-columns={printTitles?.columns}
          data-pdf-scale={pageSetup.scale}
          data-pdf-fit-to-width={pageSetup.fitToPage ? pageSetup.fitToWidth : undefined}
          data-pdf-fit-to-height={pageSetup.fitToPage ? pageSetup.fitToHeight : undefined}
          data-pdf-horizontal-centered={String(pageSetup.horizontalCentered)}
          data-pdf-vertical-centered={String(pageSetup.verticalCentered)}
          data-pdf-page-number={pageNumber}
          data-pdf-page-order={pageSetup.pageOrder}
          data-pdf-page-range={pageRange}
          aria-label={`${pageRange}，打印预览第 ${index + 1} 页`}
          key={`${sheet.id}-${rowPage.bodyStart}-${columnPage.bodyStart}`}
          style={
            savedPageSetup?.margins || hasCustomHeader || hasCustomFooter
              ? {
                  padding: `${millimetersToPixels(pageSetup.margins.top)}px ${millimetersToPixels(
                    pageSetup.margins.right
                  )}px ${millimetersToPixels(pageSetup.margins.bottom)}px ${millimetersToPixels(
                    pageSetup.margins.left
                  )}px`,
                }
              : undefined
          }
        >
          {hasCustomHeader ? (
            <SpreadsheetPrintChrome
              kind='header'
              sections={pageSetup.header}
              context={tokenContext}
              pageSetup={pageSetup}
            />
          ) : (
            <header className='work-pdf-spreadsheet-fallback-header'>
              <strong>{title}</strong>
              <span>{pageRange}</span>
            </header>
          )}
          <div
            className='work-pdf-spreadsheet-grid'
            style={{
              marginBlock: pageSetup.verticalCentered ? 'auto' : undefined,
              marginInline: pageSetup.horizontalCentered ? 'auto' : undefined,
              width: pageSetup.horizontalCentered
                ? `${Math.min(100, ((columnPage.indices.length + 1) / (columnCapacity + 1)) * 100)}%`
                : undefined,
            }}
          >
            <table>
              <thead>
                <tr>
                  <th />
                  {columnPage.indices.map((cellColumn) => (
                    <th
                      data-print-title-column={String(isWithinRange(cellColumn, titleBounds?.columns))}
                      key={cellColumn}
                    >
                      {columnName(cellColumn)}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {rowPage.indices.map((cellRow) => (
                  <tr data-print-title-row={String(isWithinRange(cellRow, titleBounds?.rows))} key={cellRow}>
                    <th>{cellRow + 1}</th>
                    {columnPage.indices.map((cellColumn) => {
                      const cell = data[cellRow]?.[cellColumn];
                      const conditionalStyle = conditionalStyles.get(`${cellRow}_${cellColumn}`);
                      const dataBar = conditionalStyle?.dataBar;
                      const icon = conditionalStyle?.icon;
                      const iconAppearance = icon ? spreadsheetConditionalIconAppearance(icon) : null;
                      return (
                        <td
                          key={cellColumn}
                          data-print-title-column={String(isWithinRange(cellColumn, titleBounds?.columns))}
                          data-conditional-fill={conditionalStyle?.cellColor}
                          data-conditional-data-bar={
                            dataBar ? `${dataBar.startPercent}:${dataBar.widthPercent}` : undefined
                          }
                          data-conditional-icon={icon ? `${icon.iconSet}:${icon.index}` : undefined}
                          data-conditional-show-value={
                            icon ? String(icon.showValue) : dataBar ? String(dataBar.showValue) : undefined
                          }
                          style={{
                            background: conditionalStyle?.cellColor ?? cell?.bg,
                            color: conditionalStyle?.textColor ?? cell?.fc,
                            fontFamily: typeof cell?.ff === 'string' ? cell.ff : undefined,
                            fontSize: cell?.fs ? `${cell.fs}px` : undefined,
                            fontStyle: cell?.it ? 'italic' : undefined,
                            fontWeight: cell?.bl ? 700 : undefined,
                            textAlign: cell?.ht === 0 ? 'center' : cell?.ht === 2 ? 'right' : 'left',
                          }}
                        >
                          {dataBar && (
                            <>
                              <span
                                className='work-pdf-spreadsheet-data-bar'
                                style={{
                                  left: `${dataBar.startPercent}%`,
                                  width: `${dataBar.widthPercent}%`,
                                  background: dataBar.color,
                                }}
                              />
                              {dataBar.axisPercent !== undefined &&
                                dataBar.axisPercent > 0 &&
                                dataBar.axisPercent < 100 && (
                                  <span
                                    className='work-pdf-spreadsheet-data-bar-axis'
                                    style={{ left: `${dataBar.axisPercent}%` }}
                                  />
                                )}
                            </>
                          )}
                          {iconAppearance && (
                            <span
                              className='work-pdf-spreadsheet-conditional-icon'
                              style={{ color: iconAppearance.color }}
                              title={iconAppearance.label}
                            >
                              {iconAppearance.glyph}
                            </span>
                          )}
                          {(!icon || icon.showValue) && (!dataBar || dataBar.showValue) && (
                            <span className='work-pdf-spreadsheet-cell-value'>
                              {cell?.m ?? cell?.v ?? (cell?.f ? cell.f : '')}
                            </span>
                          )}
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
            {pageImages.map(({ image, style }) => (
              <img
                className='work-pdf-spreadsheet-image'
                data-spreadsheet-image={image.id}
                src={image.src}
                alt={image.altText?.trim() || image.name?.trim() || '工作表图片'}
                key={image.id}
                style={style}
              />
            ))}
            {pageCharts.map(({ chart, style }) => (
              <img
                className='work-pdf-spreadsheet-chart'
                data-spreadsheet-chart={chart.id}
                src={spreadsheetChartSvgDataUrl(chart)}
                alt={chart.altText?.trim() || chart.title?.trim() || chart.name}
                key={chart.id}
                style={style}
              />
            ))}
          </div>
          {hasCustomFooter && (
            <SpreadsheetPrintChrome
              kind='footer'
              sections={pageSetup.footer}
              context={tokenContext}
              pageSetup={pageSetup}
            />
          )}
        </section>
      );
    });
  });
}

interface SpreadsheetAxisPage {
  indices: number[];
  bodyStart: number;
  bodyEnd: number;
}

interface SpreadsheetPageDescriptor {
  rowPage: SpreadsheetAxisPage;
  columnPage: SpreadsheetAxisPage;
}

function spreadsheetPageDescriptors(
  rowPages: SpreadsheetAxisPage[],
  columnPages: SpreadsheetAxisPage[],
  pageOrder: EffectiveSpreadsheetPageSetup['pageOrder']
): SpreadsheetPageDescriptor[] {
  if (pageOrder === 'downThenOver') {
    return columnPages.flatMap((columnPage) => rowPages.map((rowPage) => ({ rowPage, columnPage })));
  }
  return rowPages.flatMap((rowPage) => columnPages.map((columnPage) => ({ rowPage, columnPage })));
}

function SpreadsheetPrintChrome({
  kind,
  sections,
  context,
  pageSetup,
}: {
  kind: 'header' | 'footer';
  sections: EffectiveSpreadsheetPageSetup['header'];
  context: SpreadsheetHeaderFooterTokenContext;
  pageSetup: EffectiveSpreadsheetPageSetup;
}) {
  const scale = pageSetup.scaleWithDocument ? pageSetup.scale / 100 : 1;
  const horizontalInset = pageSetup.alignWithMargins
    ? {
        left: millimetersToPixels(pageSetup.margins.left),
        right: millimetersToPixels(pageSetup.margins.right),
      }
    : { left: 0, right: 0 };
  const Container = kind === 'header' ? 'header' : 'footer';
  return (
    <Container
      className={`work-pdf-spreadsheet-print-chrome ${kind}`}
      data-pdf-spreadsheet-chrome={kind}
      style={{
        ...horizontalInset,
        [kind === 'header' ? 'top' : 'bottom']: millimetersToPixels(
          kind === 'header' ? pageSetup.margins.header : pageSetup.margins.footer
        ),
        fontSize: `${10 * scale}px`,
      }}
    >
      {(['left', 'center', 'right'] as const).map((section) => (
        <span
          data-pdf-header-section={kind === 'header' ? section : undefined}
          data-pdf-footer-section={kind === 'footer' ? section : undefined}
          key={section}
        >
          {resolveSpreadsheetHeaderFooterTemplate(sections[section], context)}
        </span>
      ))}
    </Container>
  );
}

function paginateSpreadsheetAxis(
  start: number,
  end: number,
  titleRange: [number, number] | undefined,
  manualBreaks: number[] | undefined,
  capacity: number
): SpreadsheetAxisPage[] {
  const titleIndices = titleRange ? inclusiveIndexes(titleRange[0], titleRange[1]) : [];
  const titleSet = new Set(titleIndices);
  const boundedManualBreaks = Array.from(
    new Set(
      (manualBreaks ?? []).filter(
        (pageBreak) => Number.isSafeInteger(pageBreak) && pageBreak > start && pageBreak <= end
      )
    )
  ).sort((left, right) => left - right);
  const pages: SpreadsheetAxisPage[] = [];
  let bodyStart = start;
  while (bodyStart <= end) {
    const nextManualBreak = boundedManualBreaks.find((pageBreak) => pageBreak > bodyStart);
    let bodyEnd = Math.min(end, bodyStart + capacity - 1, nextManualBreak ? nextManualBreak - 1 : end);
    while (bodyEnd > bodyStart) {
      const missingTitleCount = titleIndices.filter((index) => index < bodyStart || index > bodyEnd).length;
      if (bodyEnd - bodyStart + 1 + missingTitleCount <= capacity) break;
      bodyEnd -= 1;
    }
    const bodyIndices = inclusiveIndexes(bodyStart, bodyEnd);
    const missingTitles = titleIndices.filter((index) => index < bodyStart || index > bodyEnd);
    pages.push({
      indices: missingTitles.length
        ? [...titleIndices, ...bodyIndices.filter((index) => !titleSet.has(index))]
        : bodyIndices,
      bodyStart,
      bodyEnd,
    });
    bodyStart = bodyEnd + 1;
  }
  return pages;
}

function inclusiveIndexes(start: number, end: number): number[] {
  return Array.from({ length: Math.max(0, end - start + 1) }, (_, index) => start + index);
}

function isWithinRange(value: number, range: [number, number] | undefined): boolean {
  return Boolean(range && value >= range[0] && value <= range[1]);
}

function spreadsheetPageImages(
  sheet: WorkSpreadsheetSheet,
  rowPage: SpreadsheetAxisPage,
  columnPage: SpreadsheetAxisPage
): Array<{
  image: WorkSpreadsheetImage;
  style: { left: string; top: string; width: string; height: string };
}> {
  return (sheet.images ?? []).flatMap((image) => {
    const style = spreadsheetPageDrawingStyle(sheet, image, rowPage, columnPage);
    return style ? [{ image, style }] : [];
  });
}

function spreadsheetPageCharts(
  content: WorkSpreadsheetContent,
  sheet: WorkSpreadsheetSheet,
  rowPage: SpreadsheetAxisPage,
  columnPage: SpreadsheetAxisPage
): Array<{
  chart: WorkSpreadsheetChart;
  style: { left: string; top: string; width: string; height: string };
}> {
  return (sheet.charts ?? []).flatMap((source) => {
    const chart = resolveSpreadsheetChart(content, sheet, source);
    const style = spreadsheetPageDrawingStyle(sheet, chart, rowPage, columnPage);
    return style ? [{ chart, style }] : [];
  });
}

function spreadsheetPageDrawingStyle(
  sheet: WorkSpreadsheetSheet,
  drawing: { left: number; top: number; width: number; height: number },
  rowPage: SpreadsheetAxisPage,
  columnPage: SpreadsheetAxisPage
): { left: string; top: string; width: string; height: string } | null {
  const pageColumnWidth = columnPage.indices.reduce((total, index) => total + spreadsheetColumnWidth(sheet, index), 0);
  const column = spreadsheetAxisCell(drawing.left, (index) => spreadsheetColumnWidth(sheet, index));
  const row = spreadsheetAxisCell(drawing.top, (index) => spreadsheetRowHeight(sheet, index));
  const columnPosition = columnPage.indices.indexOf(column.index);
  const rowPosition = rowPage.indices.indexOf(row.index);
  if (columnPosition < 0 || rowPosition < 0) return null;
  const left = ((columnPosition + column.offsetRatio + 0.3) / Math.max(1, columnPage.indices.length)) * 100;
  const width = (drawing.width / Math.max(1, pageColumnWidth)) * 100;
  const rowScale = 27 / Math.max(1, spreadsheetRowHeight(sheet, row.index));
  return {
    left: `${roundSpreadsheetPrintValue(Math.max(0, left))}%`,
    top: `${roundSpreadsheetPrintValue(27 + (rowPosition + row.offsetRatio) * 27)}px`,
    width: `${roundSpreadsheetPrintValue(Math.max(1, Math.min(100 - left, width)))}%`,
    height: `${roundSpreadsheetPrintValue(Math.max(1, drawing.height * rowScale))}px`,
  };
}

function lastWorksheetDrawingRow(sheet: WorkSpreadsheetSheet): number {
  return [...(sheet.images ?? []), ...(sheet.charts ?? [])].reduce(
    (maximum, image) =>
      Math.max(
        maximum,
        spreadsheetAxisCell(Math.max(0, image.top + image.height - 0.001), (index) =>
          spreadsheetRowHeight(sheet, index)
        ).index
      ),
    0
  );
}

function lastWorksheetDrawingColumn(sheet: WorkSpreadsheetSheet): number {
  return [...(sheet.images ?? []), ...(sheet.charts ?? [])].reduce(
    (maximum, image) =>
      Math.max(
        maximum,
        spreadsheetAxisCell(Math.max(0, image.left + image.width - 0.001), (index) =>
          spreadsheetColumnWidth(sheet, index)
        ).index
      ),
    0
  );
}

function spreadsheetAxisCell(
  position: number,
  size: (index: number) => number
): { index: number; offsetRatio: number } {
  let remaining = Math.max(0, Number.isFinite(position) ? position : 0);
  let index = 0;
  while (index < 1_048_576) {
    const currentSize = Math.max(0, size(index));
    if (currentSize > 0 && remaining < currentSize) {
      return { index, offsetRatio: remaining / currentSize };
    }
    if (currentSize > 0) remaining -= currentSize;
    index += 1;
  }
  return { index: 0, offsetRatio: 0 };
}

function spreadsheetColumnWidth(sheet: WorkSpreadsheetSheet, index: number): number {
  const key = String(index);
  if (Object.hasOwn(sheet.config?.colhidden ?? {}, key)) return 0;
  const width = sheet.config?.columnlen?.[key];
  return typeof width === 'number' && width > 0 ? width : 96;
}

function spreadsheetRowHeight(sheet: WorkSpreadsheetSheet, index: number): number {
  const key = String(index);
  if (Object.hasOwn(sheet.config?.rowhidden ?? {}, key)) return 0;
  const height = sheet.config?.rowlen?.[key];
  return typeof height === 'number' && height > 0 ? height : 24;
}

function roundSpreadsheetPrintValue(value: number): number {
  return Math.round(value * 1_000) / 1_000;
}

function lastUsedRow(data: WorkSpreadsheetContent['sheets'][number]['data']): number {
  if (!data) return 0;
  for (let row = data.length - 1; row >= 0; row -= 1) {
    if (data[row]?.some(Boolean)) return row;
  }
  return 0;
}

function lastUsedColumn(data: WorkSpreadsheetContent['sheets'][number]['data']): number {
  if (!data) return 0;
  let maximum = 0;
  for (const row of data) {
    for (let column = row.length - 1; column >= 0; column -= 1) {
      if (row[column]) {
        maximum = Math.max(maximum, column);
        break;
      }
    }
  }
  return maximum;
}

function columnName(index: number): string {
  let value = index + 1;
  let result = '';
  while (value > 0) {
    value -= 1;
    result = String.fromCharCode(65 + (value % 26)) + result;
    value = Math.floor(value / 26);
  }
  return result;
}

function cellAddress(row: number, column: number): string {
  return `${columnName(column)}${row + 1}`;
}
