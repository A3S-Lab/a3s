import type { CellMatrix, Sheet } from '@fortune-sheet/core';
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createWorkArtifact, createWorkId } from '../work-templates';
import { WorkPdfExportSurface } from './work-pdf-export-surface';

describe('Work PDF print layouts', () => {
  afterEach(cleanup);

  it('creates one printable surface for each presentation slide', () => {
    const artifact = createWorkArtifact('strategy-deck');
    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    expect(container.querySelectorAll('[data-work-pdf-page]')).toHaveLength(3);
    expect(container.querySelectorAll('[data-pdf-orientation="landscape"]')).toHaveLength(3);
  });

  it('paginates wide and long worksheets without dropping populated cells', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const data: CellMatrix = Array.from({ length: 35 }, (_, row) =>
      Array.from({ length: 11 }, (_, column) => ({ v: `${row}:${column}`, m: `${row}:${column}` }))
    );
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Large sheet',
      order: 0,
      status: 1,
      row: 35,
      column: 11,
      data,
    };
    artifact.content = { type: 'spreadsheet', sheets: [sheet] };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    expect(container.querySelectorAll('[data-work-pdf-page]')).toHaveLength(4);
    expect(container.textContent).toContain('34:10');
  });

  it('prints cached formula errors instead of exposing the source formula', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    artifact.content.sheets[0].data = [
      [
        {
          f: '=1/0',
          v: 7,
          m: '#DIV/0!',
          ct: { t: 'e' },
        },
      ],
    ];

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    expect(container).toHaveTextContent('#DIV/0!');
    expect(container).not.toHaveTextContent('=1/0');
  });

  it('places embedded worksheet images on the PDF page containing their anchor cell', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Image proof',
      order: 0,
      status: 1,
      row: 4,
      column: 4,
      data: Array.from({ length: 4 }, (_, row) =>
        Array.from({ length: 4 }, (_, column) => (row === 3 && column === 3 ? { v: 'end', m: 'end' } : null))
      ),
      images: [
        {
          id: 'sheet-image-1',
          left: 96,
          top: 24,
          width: 192,
          height: 48,
          src: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB',
        },
      ],
    };
    artifact.content = { type: 'spreadsheet', sheets: [sheet] };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const image = container.querySelector<HTMLImageElement>('[data-spreadsheet-image="sheet-image-1"]');
    expect(image).not.toBeNull();
    expect(image).toHaveAttribute('alt', '工作表图片');
    expect(image?.style.left).toBe('32.5%');
    expect(image?.style.top).toBe('54px');
  });

  it('places live worksheet charts on the PDF page containing their anchor cell', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const sheet: Sheet & {
      charts: NonNullable<Extract<typeof artifact.content, { type: 'spreadsheet' }>['sheets'][number]['charts']>;
    } = {
      id: createWorkId('sheet'),
      name: 'Chart proof',
      order: 0,
      status: 1,
      row: 4,
      column: 4,
      data: [
        [
          { v: 'Quarter', m: 'Quarter' },
          { v: 'Revenue', m: 'Revenue' },
        ],
        [
          { v: 'Q1', m: 'Q1' },
          { v: 99, m: '99' },
        ],
      ],
      charts: [
        {
          id: 'sheet-chart-1',
          name: 'Revenue chart',
          altText: 'Quarterly revenue',
          type: 'doughnut',
          doughnutHoleSize: 64,
          title: 'Revenue',
          categories: ['stale'],
          categoryReference: "'Chart proof'!$A$2",
          series: [
            {
              name: 'Revenue',
              values: [0],
              valuesReference: "'Chart proof'!$B$2",
            },
          ],
          showLegend: true,
          legendPosition: 'bottom',
          legendOverlay: true,
          left: 96,
          top: 24,
          width: 192,
          height: 96,
        },
        {
          id: 'sheet-chart-radar',
          name: 'Regional radar',
          altText: 'Regional comparison',
          type: 'radar',
          radarStyle: 'filled',
          title: 'Regions',
          categories: ['North', 'South', 'West'],
          series: [{ name: 'Revenue', values: [40, 35, 25] }],
          showLegend: false,
          left: 320,
          top: 24,
          width: 192,
          height: 96,
        },
      ],
    };
    artifact.content = { type: 'spreadsheet', sheets: [sheet] };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const chart = container.querySelector<HTMLImageElement>('[data-spreadsheet-chart="sheet-chart-1"]');
    expect(chart).not.toBeNull();
    expect(chart).toHaveAttribute('alt', 'Quarterly revenue');
    expect(chart?.src).toMatch(/^data:image\/svg\+xml/);
    expect(decodeURIComponent(chart?.src ?? '')).toContain('99');
    expect(decodeURIComponent(chart?.src ?? '')).toContain('data-chart-type="doughnut"');
    expect(decodeURIComponent(chart?.src ?? '')).toContain('data-hole-size="64"');
    expect(decodeURIComponent(chart?.src ?? '')).toContain('data-chart-legend-position="bottom"');
    expect(decodeURIComponent(chart?.src ?? '')).toContain('data-chart-legend-overlay="true"');
    expect(chart?.style.left).toBe('21.667%');
    expect(chart?.style.top).toBe('54px');
    const radar = container.querySelector<HTMLImageElement>('[data-spreadsheet-chart="sheet-chart-radar"]');
    expect(radar).toHaveAttribute('alt', 'Regional comparison');
    expect(decodeURIComponent(radar?.src ?? '')).toContain('data-chart-type="radar"');
    expect(decodeURIComponent(radar?.src ?? '')).toContain('data-radar-style="filled"');
  });

  it('renders live scatter and bubble references through the shared PDF SVG path', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = 'XY proof';
    sheet.data = [
      [{ v: 'X' }, { v: 'Y' }, { v: 'Size' }],
      [{ v: 1 }, { v: 5 }, { v: 9 }],
      [{ v: 2 }, { v: 8 }, { v: 16 }],
    ];
    sheet.charts = [
      {
        id: 'pdf-scatter',
        name: 'PDF scatter',
        type: 'scatter',
        scatterStyle: 'lineMarker',
        categories: [],
        series: [
          {
            name: 'Observed',
            xValues: [],
            xValuesReference: "'XY proof'!$A$2:$A$3",
            values: [],
            valuesReference: "'XY proof'!$B$2:$B$3",
            style: {
              fillColor: '#112233',
              fillTransparency: 35,
              lineColor: '#445566',
              lineWidth: 3.25,
              lineDash: 'dashDot',
              marker: {
                symbol: 'diamond',
                size: 9,
                fillColor: '#778899',
                lineColor: '#AABBCC',
              },
            },
            dataLabels: {
              showValue: true,
              showCategoryName: true,
              separator: ' / ',
              position: 'above',
            },
            errorBars: [
              {
                direction: 'y',
                barType: 'both',
                valueType: 'fixedValue',
                value: 1,
              },
            ],
            trendlines: [{ type: 'linear', displayEquation: true, displayRSquared: true }],
          },
        ],
        showLegend: false,
        left: 96,
        top: 24,
        width: 192,
        height: 96,
      },
      {
        id: 'pdf-bubble',
        name: 'PDF bubble',
        type: 'bubble',
        bubbleScale: 125,
        showNegativeBubbles: false,
        bubbleSizeRepresents: 'area',
        categories: [],
        series: [
          {
            name: 'Observed',
            xValues: [],
            xValuesReference: "'XY proof'!$A$2:$A$3",
            values: [],
            valuesReference: "'XY proof'!$B$2:$B$3",
            bubbleSizes: [],
            bubbleSizesReference: "'XY proof'!$C$2:$C$3",
          },
        ],
        showLegend: false,
        left: 320,
        top: 24,
        width: 192,
        height: 96,
      },
    ];

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const scatter = container.querySelector<HTMLImageElement>('[data-spreadsheet-chart="pdf-scatter"]');
    const bubble = container.querySelector<HTMLImageElement>('[data-spreadsheet-chart="pdf-bubble"]');
    const scatterSvg = decodeURIComponent(scatter?.src ?? '');
    const bubbleSvg = decodeURIComponent(bubble?.src ?? '');

    expect(scatterSvg).toContain('data-chart-type="scatter"');
    expect(scatterSvg).toContain('data-scatter-style="lineMarker"');
    expect(scatterSvg).toContain('data-point-x="2"');
    expect(scatterSvg).toContain('data-point-y="8"');
    expect(scatterSvg).toContain('stroke="#445566" stroke-width="3.25" stroke-dasharray="8 4 2 4"');
    expect(scatterSvg).toContain('data-marker-symbol="diamond"');
    expect(scatterSvg).toContain('data-marker-size="9"');
    expect(scatterSvg).toContain('fill="#778899" stroke="#AABBCC"');
    expect(scatterSvg).toContain('data-data-label-series="0:1"');
    expect(scatterSvg).toContain('data-data-label-text="2 / 8"');
    expect(scatterSvg).toContain('data-error-bars-direction="y"');
    expect(scatterSvg).toContain('data-error-plus="1"');
    expect(scatterSvg).toContain('data-trendline-type="linear"');
    expect(scatterSvg).toContain('data-trendline-equation=');
    expect(bubbleSvg).toContain('data-chart-type="bubble"');
    expect(bubbleSvg).toContain('data-bubble-scale="125"');
    expect(bubbleSvg).toContain('data-bubble-size="16"');
  });

  it('renders live combination series and secondary axes through the shared PDF SVG path', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Combo proof';
    sheet.data = [
      [{ v: 'Quarter' }, { v: 'Revenue' }, { v: 'Margin' }],
      [{ v: 'Q1' }, { v: 42 }, { v: 0.12 }],
      [{ v: 'Q2' }, { v: 55 }, { v: 0.18 }],
    ];
    sheet.charts = [
      {
        id: 'pdf-combination',
        name: 'PDF combination',
        type: 'combination',
        axes: {
          bottom: {
            title: 'Quarter',
            reverseOrder: true,
            labelPosition: 'high',
            majorTickMark: 'outside',
            labelInterval: 2,
          },
          left: {
            title: 'Revenue',
            minimum: 0,
            maximum: 100,
            majorUnit: 25,
            showMajorGridlines: false,
            numberFormat: '#,##0',
            reverseOrder: true,
            labelPosition: 'none',
            majorTickMark: 'cross',
          },
          right: {
            title: 'Margin',
            minimum: 0,
            maximum: 0.2,
            majorUnit: 0.05,
            showMajorGridlines: true,
            numberFormat: '0%',
          },
        },
        categories: [],
        categoryReference: "'Combo proof'!$A$2:$A$3",
        series: [
          {
            name: 'Revenue',
            values: [],
            valuesReference: "'Combo proof'!$B$2:$B$3",
            chartType: 'column',
            axisGroup: 'primary',
          },
          {
            name: 'Margin',
            values: [],
            valuesReference: "'Combo proof'!$C$2:$C$3",
            chartType: 'line',
            axisGroup: 'secondary',
          },
        ],
        showLegend: true,
        left: 96,
        top: 24,
        width: 384,
        height: 192,
      },
    ];

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const image = container.querySelector<HTMLImageElement>('[data-spreadsheet-chart="pdf-combination"]');
    const svg = decodeURIComponent(image?.src ?? '');

    expect(svg).toContain('data-chart-type="combination"');
    expect(svg).toContain('data-series-chart-type="column"');
    expect(svg).toContain('data-series-chart-type="line"');
    expect(svg).toContain('data-axis-group="secondary"');
    expect(svg).toContain('data-secondary-axis="true"');
    expect(svg).toContain('data-axis-title="bottom"');
    expect(svg).toContain('data-axis-title="left"');
    expect(svg).toContain('data-axis-title="right"');
    expect(svg).toContain('data-axis-scale="left"');
    expect(svg).toContain('data-axis-maximum="100"');
    expect(svg).toContain('data-axis-gridlines="false"');
    expect(svg).toContain('data-axis-scale="right"');
    expect(svg).toContain('data-axis-maximum="0.2"');
    expect(svg).toContain('data-axis-number-format="0%"');
    expect(svg).toContain('data-axis-display="bottom"');
    expect(svg).toContain('data-axis-label-position="high"');
    expect(svg).toContain('data-axis-label-interval="2"');
    expect(svg).toContain('data-axis-major-tick-mark="outside"');
    expect(svg).toContain('data-axis-reverse-order="true"');
    expect(svg).not.toContain('data-axis-tick="left"');
  });

  it('limits spreadsheet print layouts to the configured print area', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const data: CellMatrix = Array.from({ length: 6 }, (_, row) =>
      Array.from({ length: 6 }, (_, column) => ({ v: `${row}:${column}`, m: `${row}:${column}` }))
    );
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Print proof',
      order: 0,
      status: 1,
      row: 6,
      column: 6,
      data,
    };
    artifact.content = {
      type: 'spreadsheet',
      sheets: [sheet],
      printAreas: [{ sheetId: sheet.id!, reference: '$B$2:$C$3' }],
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');
    expect(pages).toHaveLength(1);
    expect(pages[0]).toHaveAttribute('data-pdf-print-area', '$B$2:$C$3');
    expect(pages[0]).toHaveTextContent('1:1');
    expect(pages[0]).toHaveTextContent('2:2');
    expect(pages[0]).not.toHaveTextContent('0:0');
    expect(pages[0]).not.toHaveTextContent('5:5');
  });

  it('repeats configured print-title rows and columns on every spreadsheet PDF page', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const data: CellMatrix = Array.from({ length: 40 }, (_, row) =>
      Array.from({ length: 12 }, (_, column) => ({
        v: row === 0 && column === 0 ? 'Repeated corner' : `${row}:${column}`,
        m: row === 0 && column === 0 ? 'Repeated corner' : `${row}:${column}`,
      }))
    );
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Print titles',
      order: 0,
      status: 1,
      row: 40,
      column: 12,
      data,
    };
    artifact.content = {
      type: 'spreadsheet',
      sheets: [sheet],
      printTitles: [{ sheetId: sheet.id!, rows: '$1:$2', columns: '$A:$B' }],
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = container.querySelectorAll<HTMLElement>('[data-work-pdf-page]');
    expect(pages).toHaveLength(4);
    for (const page of pages) {
      expect(page).toHaveAttribute('data-pdf-print-title-rows', '$1:$2');
      expect(page).toHaveAttribute('data-pdf-print-title-columns', '$A:$B');
      expect(page).toHaveTextContent('Repeated corner');
      expect(page.querySelectorAll('[data-print-title-row="true"]')).toHaveLength(2);
    }
  });

  it('honors manual row and column page breaks in spreadsheet PDF output', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const data: CellMatrix = Array.from({ length: 20 }, (_, row) =>
      Array.from({ length: 8 }, (_, column) => ({ v: `${row}:${column}`, m: `${row}:${column}` }))
    );
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Manual breaks',
      order: 0,
      status: 1,
      row: 20,
      column: 8,
      data,
    };
    artifact.content = {
      type: 'spreadsheet',
      sheets: [sheet],
      pageBreaks: [{ sheetId: sheet.id!, rows: [10], columns: [4] }],
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = Array.from(container.querySelectorAll<HTMLElement>('[data-work-pdf-page]'));
    expect(pages).toHaveLength(4);
    expect(pages.map((page) => page.querySelector('header span')?.textContent)).toEqual([
      'Manual breaks · A1–D10',
      'Manual breaks · E1–H10',
      'Manual breaks · A11–D20',
      'Manual breaks · E11–H20',
    ]);
  });

  it('uses down-then-over page order and resolves spreadsheet header/footer page tokens', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    artifact.title = 'Quarterly plan';
    const data: CellMatrix = Array.from({ length: 20 }, (_, row) =>
      Array.from({ length: 8 }, (_, column) => ({ v: `${row}:${column}`, m: `${row}:${column}` }))
    );
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Page order',
      order: 0,
      status: 1,
      row: 20,
      column: 8,
      data,
    };
    artifact.content = {
      type: 'spreadsheet',
      sheets: [sheet],
      pageBreaks: [{ sheetId: sheet.id!, rows: [10], columns: [4] }],
      pageSetups: [
        {
          sheetId: sheet.id!,
          header: {
            left: '{sheet}',
            center: '{file}',
            right: 'Page {page} of {pages}',
          },
          footer: { center: 'Internal' },
          pageNumberStart: 7,
          pageOrder: 'downThenOver',
          scaleWithDocument: false,
          alignWithMargins: false,
        },
      ],
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = Array.from(container.querySelectorAll<HTMLElement>('[data-work-pdf-page]'));
    expect(pages.map((page) => page.dataset.pdfPageRange)).toEqual([
      'Page order · A1–D10',
      'Page order · A11–D20',
      'Page order · E1–H10',
      'Page order · E11–H20',
    ]);
    expect(pages.map((page) => page.dataset.pdfPageNumber)).toEqual(['7', '8', '9', '10']);
    expect(pages.every((page) => page.dataset.pdfPageOrder === 'downThenOver')).toBe(true);
    expect(pages.map((page) => page.querySelector('[data-pdf-header-section="right"]')?.textContent)).toEqual([
      'Page 7 of 4',
      'Page 8 of 4',
      'Page 9 of 4',
      'Page 10 of 4',
    ]);
    expect(pages[0].querySelector('[data-pdf-header-section="left"]')).toHaveTextContent('Page order');
    expect(pages[0].querySelector('[data-pdf-header-section="center"]')).toHaveTextContent('Quarterly plan.xlsx');
    expect(pages[0].querySelector('[data-pdf-footer-section="center"]')).toHaveTextContent('Internal');
  });

  it('applies spreadsheet paper, orientation, fit-to-page, margins, and centering to PDF output', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const data: CellMatrix = Array.from({ length: 20 }, (_, row) =>
      Array.from({ length: 8 }, (_, column) => ({ v: `${row}:${column}`, m: `${row}:${column}` }))
    );
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Page setup',
      order: 0,
      status: 1,
      row: 20,
      column: 8,
      data,
    };
    artifact.content = {
      type: 'spreadsheet',
      sheets: [sheet],
      pageSetups: [
        {
          sheetId: sheet.id!,
          paperSize: 'letter',
          orientation: 'portrait',
          scale: 100,
          fitToPage: true,
          fitToWidth: 1,
          fitToHeight: 2,
          horizontalCentered: true,
          verticalCentered: true,
          margins: {
            top: 20,
            right: 23,
            bottom: 21,
            left: 22,
            header: 8,
            footer: 9,
          },
        },
      ],
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = Array.from(container.querySelectorAll<HTMLElement>('[data-work-pdf-page]'));
    expect(pages).toHaveLength(2);
    expect(pages.map((page) => page.querySelector('header span')?.textContent)).toEqual([
      'Page setup · A1–H10',
      'Page setup · A11–H20',
    ]);
    for (const page of pages) {
      expect(page).toHaveAttribute('data-pdf-page-size', 'letter');
      expect(page).toHaveAttribute('data-pdf-orientation', 'portrait');
      expect(page).toHaveAttribute('data-pdf-fit-to-width', '1');
      expect(page).toHaveAttribute('data-pdf-fit-to-height', '2');
      expect(page).toHaveAttribute('data-pdf-horizontal-centered', 'true');
      expect(page).toHaveAttribute('data-pdf-vertical-centered', 'true');
    }
  });

  it.each([
    'a3',
    'a5',
    'legal',
    'tabloid',
  ] as const)('uses the %s spreadsheet paper size in print and PDF surfaces', (paperSize) => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Expected spreadsheet fixture');
    artifact.content.pageSetups = [
      {
        sheetId: artifact.content.sheets[0].id!,
        paperSize,
        orientation: 'portrait',
      },
    ];

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const page = container.querySelector<HTMLElement>('[data-work-pdf-page]');

    expect(page).toHaveAttribute('data-pdf-page-size', paperSize);
    expect(page).toHaveClass('spreadsheet', paperSize, 'portrait');
  });

  it('renders supported conditional fills, text styles, color scales, data bars, and icon sets in spreadsheet PDF pages', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    const sheet: Sheet = {
      id: createWorkId('sheet'),
      name: 'Conditional proof',
      order: 0,
      status: 1,
      row: 3,
      column: 3,
      data: [
        [
          { v: 0, m: '0' },
          { v: -10, m: '-10' },
          { v: 0, m: '0' },
        ],
        [
          { v: 50, m: '50' },
          { v: 0, m: '0' },
          { v: 50, m: '50' },
        ],
        [
          { v: 100, m: '100' },
          { v: 30, m: '30' },
          { v: 100, m: '100' },
        ],
      ],
      luckysheet_conditionformat_save: [
        {
          type: 'colorGradation',
          cellrange: [{ row: [0, 2], column: [0, 0] }],
          format: ['#63be7b', '#ffeb84', '#f8696b'],
        },
        {
          type: 'dataBar',
          cellrange: [{ row: [0, 2], column: [1, 1] }],
          format: { textColor: null, cellColor: '#5b9bd5' },
          visualOptions: {
            thresholds: [{ type: 'min' }, { type: 'max' }],
            showValue: false,
            minLength: 20,
            maxLength: 80,
          },
        },
        {
          type: 'default',
          cellrange: [{ row: [0, 2], column: [0, 0] }],
          format: { textColor: '#ffffff', cellColor: null },
          conditionName: 'greaterThan',
          conditionRange: [],
          conditionValue: ['50'],
        },
        {
          type: 'icons',
          cellrange: [{ row: [0, 2], column: [2, 2] }],
          format: {
            iconSet: '3Arrows',
            showValue: false,
            reverse: false,
            percent: true,
            thresholds: [
              { type: 'min', gte: true },
              { type: 'percent', value: 33, gte: true },
              { type: 'percent', value: 67, gte: true },
            ],
          },
        },
      ],
    };
    artifact.content = { type: 'spreadsheet', sheets: [sheet] };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const rows = container.querySelectorAll<HTMLTableRowElement>('tbody tr');
    const minimum = rows[0].querySelectorAll<HTMLTableCellElement>('td')[0];
    const maximum = rows[2].querySelectorAll<HTMLTableCellElement>('td')[0];
    const positiveBar = rows[2].querySelectorAll<HTMLTableCellElement>('td')[1];
    const maximumIcon = rows[2].querySelectorAll<HTMLTableCellElement>('td')[2];

    expect(minimum).toHaveAttribute('data-conditional-fill', 'rgb(248, 105, 107)');
    expect(maximum).toHaveAttribute('data-conditional-fill', 'rgb(99, 190, 123)');
    expect(maximum).toHaveStyle({ color: '#ffffff' });
    expect(positiveBar).toHaveAttribute('data-conditional-data-bar', '25:60');
    expect(positiveBar.querySelector('.work-pdf-spreadsheet-data-bar')).toHaveStyle({
      left: '25%',
      width: '60%',
      background: '#5b9bd5',
    });
    expect(positiveBar.querySelector('.work-pdf-spreadsheet-data-bar-axis')).toHaveStyle({ left: '25%' });
    expect(positiveBar.querySelector('.work-pdf-spreadsheet-cell-value')).not.toBeInTheDocument();
    expect(maximumIcon).toHaveAttribute('data-conditional-icon', '3Arrows:2');
    expect(maximumIcon).toHaveAttribute('data-conditional-show-value', 'false');
    expect(maximumIcon.querySelector('.work-pdf-spreadsheet-conditional-icon')).toHaveTextContent('↑');
    expect(maximumIcon.querySelector('.work-pdf-spreadsheet-cell-value')).not.toBeInTheDocument();
  });

  it('splits documents at explicit page breaks and keeps Letter landscape page chrome', () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.content = {
      type: 'document',
      pageSize: 'letter',
      orientation: 'landscape',
      margins: { top: 20, right: 21, bottom: 22, left: 23 },
      headerText: 'A3S Work',
      footerText: 'Internal',
      showPageNumbers: true,
      pageNumberStart: 4,
      html: '<p>First page</p><div data-page-break="true"></div><p>Second page</p>',
    };

    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    const pages = Array.from(container.querySelectorAll<HTMLElement>('[data-work-pdf-page]'));
    expect(pages).toHaveLength(2);
    expect(pages.every((page) => page.dataset.pdfOrientation === 'landscape')).toBe(true);
    expect(pages.every((page) => page.dataset.pdfPageSize === 'letter')).toBe(true);
    expect(pages.every((page) => page.classList.contains('letter') && page.classList.contains('landscape'))).toBe(true);
    expect(pages[0]).toHaveTextContent('A3S Work');
    expect(pages[0]).toHaveTextContent('First page');
    expect(pages[0]).toHaveTextContent('Internal4');
    expect(pages[1]).toHaveTextContent('Second page');
    expect(pages[1]).toHaveTextContent('Internal5');
    expect(container.querySelector('[data-page-break]')).not.toBeInTheDocument();
  });

  it('does not create a duplicate print layout for a source PDF', () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.kind = 'pdf';
    artifact.content = { type: 'pdf' };
    const { container } = render(<WorkPdfExportSurface artifact={artifact} />);
    expect(container).toBeEmptyDOMElement();
  });
});
