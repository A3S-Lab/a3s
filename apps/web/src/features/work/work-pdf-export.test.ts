import { afterEach, describe, expect, it, vi } from 'vitest';
import { createWorkArtifact } from './work-templates';
import { exportWorkArtifactPdf, workPdfPagesForExport } from './work-pdf-export';

const pdfMocks = vi.hoisted(() => ({
  create: vi.fn(),
  addPage: vi.fn(),
  addImage: vi.fn(),
  setProperties: vi.fn(),
  save: vi.fn(),
}));

vi.mock('html2canvas', () => ({
  default: vi.fn(async () => {
    const canvas = document.createElement('canvas');
    canvas.width = 100;
    canvas.height = 50;
    return canvas;
  }),
}));

vi.mock('jspdf', () => ({
  jsPDF: class {
    constructor(options: unknown) {
      pdfMocks.create(options);
    }

    addPage(format: string, orientation: string) {
      pdfMocks.addPage(format, orientation);
      return this;
    }

    addImage(...arguments_: unknown[]) {
      pdfMocks.addImage(...arguments_);
      return this;
    }

    setProperties(properties: unknown) {
      pdfMocks.setProperties(properties);
      return this;
    }

    save(name: string) {
      pdfMocks.save(name);
      return this;
    }
  },
}));

describe('Work PDF page selection', () => {
  afterEach(() => {
    document.body.replaceChildren();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    for (const mock of Object.values(pdfMocks)) mock.mockReset();
  });

  it('uses the canonical export surface when a visible preview also exists', () => {
    document.body.innerHTML = `
      <div data-work-pdf-artifact="artifact-1" data-work-pdf-surface="preview">
        <section data-work-pdf-page>Preview page</section>
      </div>
      <div data-work-pdf-artifact="artifact-1" data-work-pdf-surface="export">
        <section data-work-pdf-page>Export page 1</section>
        <section data-work-pdf-page>Export page 2</section>
      </div>
    `;

    expect(workPdfPagesForExport('artifact-1').map((page) => page.textContent)).toEqual([
      'Export page 1',
      'Export page 2',
    ]);
  });

  it('filters duplicate and invalid requested indexes while retaining document order', () => {
    document.body.innerHTML = `
      <div data-work-pdf-artifact="artifact-2" data-work-pdf-surface="export">
        <section data-work-pdf-page>Page 1</section>
        <section data-work-pdf-page>Page 2</section>
        <section data-work-pdf-page>Page 3</section>
      </div>
    `;

    expect(
      workPdfPagesForExport('artifact-2', { pageIndexes: [2, 0, 2, -1, 9] }).map((page) => page.textContent)
    ).toEqual(['Page 1', 'Page 3']);
    expect(workPdfPagesForExport('artifact-2', { pageIndexes: [] })).toEqual([]);
  });

  it.each([
    ['a3', 841.89, 1190.55],
    ['a4', 595.28, 841.89],
    ['a5', 419.53, 595.28],
    ['letter', 612, 792],
    ['legal', 612, 1008],
    ['tabloid', 792, 1224],
  ] as const)('creates exact portrait and landscape %s PDF pages', async (paperSize, width, height) => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    document.body.innerHTML = `
      <div data-work-pdf-artifact="${artifact.id}" data-work-pdf-surface="export">
        <section data-work-pdf-page data-pdf-page-size="${paperSize}" data-pdf-orientation="portrait"></section>
        <section data-work-pdf-page data-pdf-page-size="${paperSize}" data-pdf-orientation="landscape"></section>
      </div>
    `;
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      callback(0);
      return 1;
    });
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      () =>
        ({
          fillStyle: '',
          fillRect: vi.fn(),
          drawImage: vi.fn(),
        }) as unknown as CanvasRenderingContext2D
    );
    vi.spyOn(HTMLCanvasElement.prototype, 'toDataURL').mockReturnValue('data:image/jpeg;base64,pdf-page');

    await exportWorkArtifactPdf(artifact);

    expect(pdfMocks.create).toHaveBeenCalledWith({
      orientation: 'portrait',
      unit: 'pt',
      format: paperSize,
      compress: true,
    });
    expect(pdfMocks.addPage).toHaveBeenCalledWith(paperSize, 'landscape');
    expect(pdfMocks.addImage).toHaveBeenCalledTimes(2);
    expect(pdfMocks.addImage.mock.calls[0]?.[4]).toBe(width);
    expect(pdfMocks.addImage.mock.calls[1]?.[4]).toBe(height);
  });
});
