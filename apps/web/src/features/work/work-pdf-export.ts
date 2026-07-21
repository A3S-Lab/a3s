import type { jsPDF as JsPdf } from 'jspdf';
import type { WorkArtifact, WorkSpreadsheetPaperSize } from './work-types';

type PdfPageSize = WorkSpreadsheetPaperSize;

export interface WorkPdfExportOptions {
  pageIndexes?: number[];
}

const PDF_PAGE_DIMENSIONS: Record<PdfPageSize, { width: number; height: number }> = {
  a3: { width: 841.89, height: 1190.55 },
  a4: { width: 595.28, height: 841.89 },
  a5: { width: 419.53, height: 595.28 },
  letter: { width: 612, height: 792 },
  legal: { width: 612, height: 1008 },
  tabloid: { width: 792, height: 1224 },
};

export async function exportWorkArtifactPdf(artifact: WorkArtifact, options: WorkPdfExportOptions = {}): Promise<void> {
  const allPages = workPdfPagesForExport(artifact.id);
  if (!allPages.length) {
    throw new Error('PDF print layout is not ready. Please retry after the editor finishes loading.');
  }
  const pages = workPdfPagesForExport(artifact.id, options);
  if (!pages.length) throw new Error('Select at least one page before exporting PDF.');

  await document.fonts?.ready;
  await nextPaint();
  const [{ default: html2canvas }, { jsPDF }] = await Promise.all([import('html2canvas'), import('jspdf')]);
  let pdf: JsPdf | null = null;

  for (const page of pages) {
    const orientation = page.dataset.pdfOrientation === 'portrait' ? 'portrait' : 'landscape';
    const pageSize = pdfPageSize(page.dataset.pdfPageSize);
    const canvas = await html2canvas(page, {
      backgroundColor: '#ffffff',
      logging: false,
      scale: 2,
      useCORS: true,
      windowWidth: Math.max(page.scrollWidth, page.clientWidth),
      windowHeight: Math.max(page.scrollHeight, page.clientHeight),
    });
    pdf = appendCanvas(pdf, canvas, orientation, pageSize, jsPDF);
  }
  if (!pdf) throw new Error('PDF export did not produce any pages.');
  pdf.setProperties({
    title: artifact.title,
    author: 'A3S Work',
    creator: 'A3S Work',
  });
  pdf.save(`${safeFileName(artifact.title)}.pdf`);
}

export function workPdfPagesForExport(artifactId: string, options: WorkPdfExportOptions = {}): HTMLElement[] {
  const surface = document.querySelector<HTMLElement>(
    `[data-work-pdf-surface="export"][data-work-pdf-artifact="${cssEscape(artifactId)}"]`
  );
  const pages = surface ? Array.from(surface.querySelectorAll<HTMLElement>('[data-work-pdf-page]')) : [];
  if (options.pageIndexes === undefined) return pages;
  const selected = new Set(
    options.pageIndexes.filter((index) => Number.isSafeInteger(index) && index >= 0 && index < pages.length)
  );
  return pages.filter((_, index) => selected.has(index));
}

function appendCanvas(
  pdf: JsPdf | null,
  source: HTMLCanvasElement,
  orientation: 'portrait' | 'landscape',
  pageSize: PdfPageSize,
  Pdf: typeof import('jspdf').jsPDF
): JsPdf {
  let document = pdf;
  const ensurePage = () => {
    if (!document) {
      document = new Pdf({ orientation, unit: 'pt', format: pageSize, compress: true });
    } else {
      document.addPage(pageSize, orientation);
    }
  };

  const dimensions = PDF_PAGE_DIMENSIONS[pageSize];
  const standardWidth = orientation === 'portrait' ? dimensions.width : dimensions.height;
  const standardHeight = orientation === 'portrait' ? dimensions.height : dimensions.width;
  const sliceHeight = Math.max(1, Math.floor((source.width * standardHeight) / standardWidth));
  for (let offset = 0; offset < source.height; offset += sliceHeight) {
    ensurePage();
    const height = Math.min(sliceHeight, source.height - offset);
    const slice = documentCanvas(source.width, height);
    const context = slice.getContext('2d');
    if (!context) throw new Error('The browser could not prepare a PDF page.');
    context.fillStyle = '#ffffff';
    context.fillRect(0, 0, slice.width, slice.height);
    context.drawImage(source, 0, offset, source.width, height, 0, 0, source.width, height);
    const renderedHeight = (standardWidth * height) / source.width;
    document?.addImage(
      slice.toDataURL('image/jpeg', 0.92),
      'JPEG',
      0,
      0,
      standardWidth,
      renderedHeight,
      undefined,
      'FAST'
    );
  }
  return document as JsPdf;
}

function pdfPageSize(value: string | undefined): PdfPageSize {
  return value && Object.hasOwn(PDF_PAGE_DIMENSIONS, value) ? (value as PdfPageSize) : 'a4';
}

function documentCanvas(width: number, height: number): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  return canvas;
}

function nextPaint(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
}

function safeFileName(value: string): string {
  return value.replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').trim() || 'A3S Work file';
}

function cssEscape(value: string): string {
  return typeof CSS !== 'undefined' && typeof CSS.escape === 'function'
    ? CSS.escape(value)
    : value.replace(/["\\]/g, '\\$&');
}
