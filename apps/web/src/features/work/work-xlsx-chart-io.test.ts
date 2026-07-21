import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';
import { createWorkArtifact } from './work-templates';
import type { WorkSpreadsheetChart } from './work-types';

const ONE_PIXEL_PNG = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=';

describe('Work XLSX chart interoperability', () => {
  it('imports anchored basic charts with cached data, live references, names, and alternative text', async () => {
    const artifact = await importWorkFile(await createChartFixture());

    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    expect(artifact.content.sheets[0].charts).toEqual([
      expect.objectContaining({
        id: expect.stringContaining('xlsx-chart'),
        name: 'Revenue chart',
        altText: 'Quarterly revenue trend',
        type: 'column',
        title: 'Quarterly revenue',
        categories: ['Q1', 'Q2'],
        categoryReference: 'Report!$A$2:$A$3',
        series: [
          {
            name: 'Revenue',
            nameReference: 'Report!$B$1',
            values: [42, 55],
            valuesReference: 'Report!$B$2:$B$3',
            dataLabels: { showValue: true },
            trendlines: [{ type: 'linear' }],
          },
        ],
        showLegend: true,
        left: 96,
        top: 96,
        width: 480,
        height: 288,
      }),
    ]);
    expect(artifact.compatibility?.issues.find((issue) => issue.code === 'xlsx.charts')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('preserved'),
    });
    expect(artifact.compatibility?.issues.map((issue) => issue.code)).toEqual(
      expect.arrayContaining(['xlsx.charts.format'])
    );
    expect(artifact.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.trendline')).toBe(false);
    expect(artifact.compatibility?.issues.some((issue) => issue.code === 'xlsx.drawings.unsupported')).toBe(false);
  });

  it('exports edited charts as native drawing and ChartML parts, refreshes caches, and reopens them', async () => {
    const artifact = await importWorkFile(await createChartFixture());
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    const chart = sheet.charts?.[0];
    expect(chart).toBeDefined();
    if (!chart) return;
    chart.type = 'line';
    chart.title = 'Updated revenue';
    chart.titleReference = undefined;
    chart.left = 192;
    chart.top = 48;
    chart.width = 384;
    chart.height = 240;
    sheet.images = [
      {
        id: 'proof-image',
        name: 'Proof image',
        contentType: 'image/png',
        src: `data:image/png;base64,${ONE_PIXEL_PNG}`,
        left: 0,
        top: 0,
        width: 48,
        height: 24,
      },
    ];
    const value = sheet.data?.[1]?.[1];
    if (value) {
      value.v = 99;
      value.m = '99';
    }

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const worksheet = await archive.file('xl/worksheets/sheet1.xml')?.async('text');
    const drawing = await archive.file('xl/drawings/drawing1.xml')?.async('text');
    const drawingRelationships = await archive.file('xl/drawings/_rels/drawing1.xml.rels')?.async('text');
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    const contentTypes = await archive.file('[Content_Types].xml')?.async('text');

    expect(worksheet).toContain('<drawing r:id=');
    expect(drawing).toContain('name="Revenue chart"');
    expect(drawing).toContain('name="Proof image"');
    expect(drawing).toContain('descr="Quarterly revenue trend"');
    expect(drawing).toContain('<xdr:from><xdr:col>2</xdr:col>');
    expect(drawing).toContain('<xdr:row>2</xdr:row>');
    expect(drawing).toContain('<xdr:to><xdr:col>6</xdr:col>');
    expect(drawing).toContain('<xdr:row>12</xdr:row>');
    expect(drawingRelationships).toContain('../charts/chart1.xml');
    expect(drawingRelationships).toContain('../media/image1.png');
    expect(archive.file('xl/media/image1.png')).not.toBeNull();
    expect(chartXml).toContain('<c:lineChart>');
    expect(chartXml).toContain('<a:t>Updated revenue</a:t>');
    expect(chartXml).toContain('<c:f>Report!$A$2:$A$3</c:f>');
    expect(chartXml).toContain('<c:f>Report!$B$2:$B$3</c:f>');
    expect(chartXml).toContain('<c:pt idx="0"><c:v>99</c:v></c:pt>');
    expect(contentTypes).toContain('application/vnd.openxmlformats-officedocument.drawingml.chart+xml');

    const reopened = await importWorkFile(
      new File([exported], 'Chart round trip.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0]).toMatchObject({
      name: 'Revenue chart',
      altText: 'Quarterly revenue trend',
      type: 'line',
      title: 'Updated revenue',
      categories: ['Q1', 'Q2'],
      categoryReference: 'Report!$A$2:$A$3',
      series: [
        expect.objectContaining({
          name: 'Revenue',
          values: [99, 55],
          valuesReference: 'Report!$B$2:$B$3',
        }),
      ],
      left: 192,
      top: 48,
      width: 384,
      height: 240,
    });
  });

  it('round-trips native doughnut holes and radar styles without unsupported-type diagnostics', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    const chartBase: Omit<WorkSpreadsheetChart, 'id' | 'name' | 'type' | 'left' | 'top'> = {
      title: 'Regional mix',
      categories: ['North', 'South', 'West'],
      series: [
        { name: 'Revenue', values: [40, 35, 25] },
        { name: 'Margin', values: [22, 28, 31] },
      ],
      showLegend: true,
      width: 480,
      height: 288,
    };
    sheet.charts = [
      {
        ...chartBase,
        id: 'chart-doughnut',
        name: 'Regional doughnut',
        type: 'doughnut',
        doughnutHoleSize: 64,
        left: 96,
        top: 48,
      },
      {
        ...chartBase,
        id: 'chart-radar',
        name: 'Regional radar',
        type: 'radar',
        radarStyle: 'filled',
        left: 624,
        top: 48,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const doughnutXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    const radarXml = await archive.file('xl/charts/chart2.xml')?.async('text');

    expect(doughnutXml).toContain('<c:doughnutChart>');
    expect(doughnutXml).toContain('<c:holeSize val="64"/>');
    expect(radarXml).toContain('<c:radarChart>');
    expect(radarXml).toContain('<c:radarStyle val="filled"/>');

    const reopened = await importWorkFile(
      new File([exported], 'Advanced charts.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts).toEqual([
      expect.objectContaining({ type: 'doughnut', doughnutHoleSize: 64 }),
      expect.objectContaining({ type: 'radar', radarStyle: 'filled' }),
    ]);
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'xlsx.charts')?.message).toContain('doughnut');
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.unsupported-type')).toBe(false);
  });

  it('round-trips native scatter and bubble charts with live X, Y, and size references', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'XY Data';
    sheet.data = [
      [{ v: 'X' }, { v: 'Y' }, { v: 'Size' }],
      [{ v: 1 }, { v: 5 }, { v: 9 }],
      [{ v: 2 }, { v: 8 }, { v: -16 }],
      [{ v: 4 }, { v: 6 }, { v: 25 }],
    ];
    sheet.charts = [
      {
        id: 'chart-scatter',
        name: 'Response scatter',
        type: 'scatter',
        scatterStyle: 'smoothMarker',
        title: 'Response curve',
        axes: {
          bottom: {
            minimum: 0,
            maximum: 4,
            majorUnit: 1,
            showMajorGridlines: true,
            numberFormat: '0.0',
            numberFormatSourceLinked: false,
          },
          left: {
            minimum: 0,
            maximum: 10,
            majorUnit: 2,
            showMajorGridlines: false,
            numberFormat: '0.0',
            numberFormatSourceLinked: false,
          },
        },
        categories: [],
        series: [
          {
            name: 'Observed',
            xValues: [0],
            xValuesReference: "'XY Data'!$A$2:$A$4",
            values: [0],
            valuesReference: "'XY Data'!$B$2:$B$4",
          },
        ],
        showLegend: true,
        left: 96,
        top: 48,
        width: 480,
        height: 288,
      },
      {
        id: 'chart-bubble',
        name: 'Response bubbles',
        type: 'bubble',
        bubbleScale: 135,
        showNegativeBubbles: true,
        bubbleSizeRepresents: 'width',
        title: 'Response volume',
        categories: [],
        series: [
          {
            name: 'Observed',
            xValues: [0],
            xValuesReference: "'XY Data'!$A$2:$A$4",
            values: [0],
            valuesReference: "'XY Data'!$B$2:$B$4",
            bubbleSizes: [0],
            bubbleSizesReference: "'XY Data'!$C$2:$C$4",
          },
        ],
        showLegend: false,
        left: 624,
        top: 48,
        width: 480,
        height: 288,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const scatterXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    const bubbleXml = await archive.file('xl/charts/chart2.xml')?.async('text');

    expect(scatterXml).toContain('<c:scatterChart>');
    expect(scatterXml).toContain('<c:scatterStyle val="smoothMarker"/>');
    expect(scatterXml).toContain('<c:xVal><c:numRef><c:f>&apos;XY Data&apos;!$A$2:$A$4</c:f>');
    expect(scatterXml).toContain('<c:yVal><c:numRef><c:f>&apos;XY Data&apos;!$B$2:$B$4</c:f>');
    expect(scatterXml?.match(/<c:valAx>/g)).toHaveLength(2);
    expect(scatterXml).not.toContain('<c:catAx>');
    expect(scatterXml).toContain('<c:max val="4"/><c:min val="0"/>');
    expect(scatterXml).toContain('<c:majorUnit val="1"/>');
    expect(scatterXml).toContain('<c:max val="10"/><c:min val="0"/>');
    expect(scatterXml).toContain('<c:majorUnit val="2"/>');
    expect(scatterXml?.match(/<c:majorGridlines\/>/g)).toHaveLength(1);
    expect(bubbleXml).toContain('<c:bubbleChart>');
    expect(bubbleXml).toContain('<c:bubbleSize><c:numRef><c:f>&apos;XY Data&apos;!$C$2:$C$4</c:f>');
    expect(bubbleXml).toContain('<c:bubbleScale val="135"/>');
    expect(bubbleXml).toContain('<c:showNegBubbles val="1"/>');
    expect(bubbleXml).toContain('<c:sizeRepresents val="w"/>');
    expect(bubbleXml?.match(/<c:valAx>/g)).toHaveLength(2);
    expect(bubbleXml).not.toContain('<c:catAx>');

    const reopened = await importWorkFile(
      new File([exported], 'XY charts.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts).toEqual([
      expect.objectContaining({
        type: 'scatter',
        scatterStyle: 'smoothMarker',
        categories: [],
        categoryReference: undefined,
        axes: {
          bottom: {
            minimum: 0,
            maximum: 4,
            majorUnit: 1,
            showMajorGridlines: true,
            numberFormat: '0.0',
            numberFormatSourceLinked: false,
          },
          left: {
            minimum: 0,
            maximum: 10,
            majorUnit: 2,
            showMajorGridlines: false,
            numberFormat: '0.0',
            numberFormatSourceLinked: false,
          },
        },
        series: [
          expect.objectContaining({
            xValues: [1, 2, 4],
            xValuesReference: "'XY Data'!$A$2:$A$4",
            values: [5, 8, 6],
            valuesReference: "'XY Data'!$B$2:$B$4",
          }),
        ],
      }),
      expect.objectContaining({
        type: 'bubble',
        bubbleScale: 135,
        showNegativeBubbles: true,
        bubbleSizeRepresents: 'width',
        series: [
          expect.objectContaining({
            xValues: [1, 2, 4],
            values: [5, 8, 6],
            bubbleSizes: [9, -16, 25],
            bubbleSizesReference: "'XY Data'!$C$2:$C$4",
          }),
        ],
      }),
    ]);
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'xlsx.charts')?.message).toContain('scatter');
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.unsupported-type')).toBe(false);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axes')).toBe(false);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.layout')).toBe(false);

    if (!bubbleXml) throw new Error('Bubble ChartML was not generated.');
    archive.file('xl/charts/chart2.xml', bubbleXml.replace('<c:bubble3D val="0"/>', '<c:bubble3D val="1"/>'));
    const advanced = await importWorkFile(
      new File([await archive.generateAsync({ type: 'arraybuffer' })], '3D bubbles.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(advanced.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.layout')).toBe(true);
  });

  it('round-trips editable column-line-area combinations with a native secondary axis', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Combo Data';
    sheet.data = [
      [{ v: 'Quarter' }, { v: 'Revenue' }, { v: 'Margin' }, { v: 'Forecast' }],
      [{ v: 'Q1' }, { v: 42 }, { v: 0.12 }, { v: 40 }],
      [{ v: 'Q2' }, { v: 55 }, { v: 0.18 }, { v: 53 }],
      [{ v: 'Q3' }, { v: 61 }, { v: 0.2 }, { v: 64 }],
    ];
    sheet.charts = [
      {
        id: 'chart-combination',
        name: 'Revenue combination',
        type: 'combination',
        title: 'Revenue and margin',
        categories: [],
        categoryReference: "'Combo Data'!$A$2:$A$4",
        series: [
          {
            name: 'Revenue',
            nameReference: "'Combo Data'!$B$1",
            values: [],
            valuesReference: "'Combo Data'!$B$2:$B$4",
            chartType: 'column',
            axisGroup: 'primary',
          },
          {
            name: 'Margin',
            nameReference: "'Combo Data'!$C$1",
            values: [],
            valuesReference: "'Combo Data'!$C$2:$C$4",
            chartType: 'line',
            axisGroup: 'secondary',
          },
          {
            name: 'Forecast',
            nameReference: "'Combo Data'!$D$1",
            values: [],
            valuesReference: "'Combo Data'!$D$2:$D$4",
            chartType: 'area',
            axisGroup: 'primary',
          },
        ],
        showLegend: true,
        left: 96,
        top: 48,
        width: 576,
        height: 336,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');

    expect(chartXml).toContain('<c:barChart><c:barDir val="col"/>');
    expect(chartXml).toContain('<c:lineChart>');
    expect(chartXml).toContain('<c:areaChart>');
    expect(chartXml?.match(/<c:catAx>/g)).toHaveLength(2);
    expect(chartXml?.match(/<c:valAx>/g)).toHaveLength(2);
    expect(chartXml).toContain('<c:axPos val="r"/>');
    expect(chartXml).toContain('<c:axPos val="t"/>');

    const reopened = await importWorkFile(
      new File([exported], 'Combination chart.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0]).toMatchObject({
      type: 'combination',
      categories: ['Q1', 'Q2', 'Q3'],
      categoryReference: "'Combo Data'!$A$2:$A$4",
      series: [
        expect.objectContaining({
          name: 'Revenue',
          values: [42, 55, 61],
          chartType: 'column',
          axisGroup: 'primary',
        }),
        expect.objectContaining({
          name: 'Margin',
          values: [0.12, 0.18, 0.2],
          chartType: 'line',
          axisGroup: 'secondary',
        }),
        expect.objectContaining({
          name: 'Forecast',
          values: [40, 53, 64],
          chartType: 'area',
          axisGroup: 'primary',
        }),
      ],
    });
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'xlsx.charts')?.message).toContain(
      'combination'
    );
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.unsupported-type')).toBe(false);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.combination')).toBe(false);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axes')).toBe(false);
  });

  it('round-trips editable native trendline types, forecasts, labels, and constraints', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Trend Data';
    sheet.data = [
      [{ v: 'X' }, { v: 'Y' }],
      [{ v: 1 }, { v: 3 }],
      [{ v: 2 }, { v: 5 }],
      [{ v: 3 }, { v: 7 }],
      [{ v: 4 }, { v: 9 }],
    ];
    sheet.charts = [
      {
        id: 'chart-trendlines',
        name: 'Trend analysis',
        type: 'scatter',
        scatterStyle: 'marker',
        title: 'Trend analysis',
        categories: [],
        series: [
          {
            name: 'Observed',
            xValues: [],
            xValuesReference: "'Trend Data'!$A$2:$A$5",
            values: [],
            valuesReference: "'Trend Data'!$B$2:$B$5",
            trendlines: [
              {
                type: 'linear',
                name: 'Baseline',
                forward: 1.5,
                backward: 0.5,
                intercept: 1,
                displayEquation: true,
                displayRSquared: true,
              },
              { type: 'polynomial', order: 3 },
              { type: 'movingAverage', period: 2 },
            ],
          },
        ],
        showLegend: true,
        left: 96,
        top: 48,
        width: 576,
        height: 336,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');

    expect(chartXml?.match(/<c:trendline>/g)).toHaveLength(3);
    expect(chartXml).toContain('<c:name>Baseline</c:name>');
    expect(chartXml).toContain('<c:trendlineType val="linear"/>');
    expect(chartXml).toContain('<c:forward val="1.5"/>');
    expect(chartXml).toContain('<c:backward val="0.5"/>');
    expect(chartXml).toContain('<c:intercept val="1"/>');
    expect(chartXml).toContain('<c:dispRSq val="1"/>');
    expect(chartXml).toContain('<c:dispEq val="1"/>');
    expect(chartXml).toContain('<c:trendlineType val="poly"/><c:order val="3"/>');
    expect(chartXml).toContain('<c:trendlineType val="movingAvg"/><c:period val="2"/>');

    const reopened = await importWorkFile(
      new File([exported], 'Trendlines.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0].series[0].trendlines).toEqual([
      {
        type: 'linear',
        name: 'Baseline',
        forward: 1.5,
        backward: 0.5,
        intercept: 1,
        displayEquation: true,
        displayRSquared: true,
      },
      { type: 'polynomial', order: 3 },
      { type: 'movingAverage', period: 2 },
    ]);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.trendline')).toBe(false);

    if (!chartXml) throw new Error('Trendline ChartML was not generated.');
    archive.file(
      'xl/charts/chart1.xml',
      chartXml.replace('<c:trendlineType val="linear"/>', '<c:trendlineType val="unsupported"/>')
    );
    const unsupported = await importWorkFile(
      new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Unsupported trendline.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.trendline')).toBe(true);
  });

  it('round-trips editable native data-label content, separators, and positions', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Label Data';
    sheet.data = [
      [{ v: 'Quarter' }, { v: 'Revenue' }],
      [{ v: 'Q1' }, { v: 42 }],
      [{ v: 'Q2' }, { v: 55 }],
    ];
    sheet.charts = [
      {
        id: 'chart-data-labels',
        name: 'Revenue share',
        type: 'pie',
        title: 'Revenue share',
        categories: [],
        categoryReference: "'Label Data'!$A$2:$A$3",
        series: [
          {
            name: 'Revenue',
            values: [],
            valuesReference: "'Label Data'!$B$2:$B$3",
            dataLabels: {
              showValue: true,
              showCategoryName: true,
              showSeriesName: true,
              showPercentage: true,
              separator: ' / ',
              position: 'outsideEnd',
            },
          },
        ],
        showLegend: true,
        left: 96,
        top: 48,
        width: 576,
        height: 336,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');

    expect(chartXml).toContain('<c:dLbls>');
    expect(chartXml).toContain('<c:dLblPos val="outEnd"/>');
    expect(chartXml).toContain('<c:showVal val="1"/>');
    expect(chartXml).toContain('<c:showCatName val="1"/>');
    expect(chartXml).toContain('<c:showSerName val="1"/>');
    expect(chartXml).toContain('<c:showPercent val="1"/>');
    expect(chartXml).toContain('<c:separator> / </c:separator>');

    const reopened = await importWorkFile(
      new File([exported], 'Data labels.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0].series[0].dataLabels).toEqual({
      showValue: true,
      showCategoryName: true,
      showSeriesName: true,
      showPercentage: true,
      separator: ' / ',
      position: 'outsideEnd',
    });
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.data-labels')).toBe(false);

    if (!chartXml) throw new Error('Data-label ChartML was not generated.');
    archive.file('xl/charts/chart1.xml', chartXml.replace('<c:showLegendKey val="0"/>', '<c:showLegendKey val="1"/>'));
    const unsupported = await importWorkFile(
      new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Unsupported data labels.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.data-labels')).toBe(true);
  });

  it('round-trips fixed and custom native error bars with live references', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Error Data';
    sheet.data = [
      [{ v: 'X' }, { v: 'Y' }, { v: 'X plus' }, { v: 'X minus' }],
      [{ v: 1 }, { v: 10 }, { v: 0.2 }, { v: 0.1 }],
      [{ v: 2 }, { v: 20 }, { v: 0.3 }, { v: 0.15 }],
    ];
    sheet.charts = [
      {
        id: 'chart-error-bars',
        name: 'Measurement uncertainty',
        type: 'scatter',
        scatterStyle: 'marker',
        categories: [],
        series: [
          {
            name: 'Observed',
            xValues: [],
            xValuesReference: "'Error Data'!$A$2:$A$3",
            values: [],
            valuesReference: "'Error Data'!$B$2:$B$3",
            errorBars: [
              {
                direction: 'x',
                barType: 'both',
                valueType: 'custom',
                plusValues: [],
                plusReference: "'Error Data'!$C$2:$C$3",
                minusValues: [],
                minusReference: "'Error Data'!$D$2:$D$3",
                showEndCaps: false,
              },
              {
                direction: 'y',
                barType: 'plus',
                valueType: 'fixedValue',
                value: 2,
              },
            ],
          },
        ],
        showLegend: false,
        left: 96,
        top: 48,
        width: 576,
        height: 336,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');

    expect(chartXml?.match(/<c:errBars>/g)).toHaveLength(2);
    expect(chartXml).toContain('<c:errDir val="x"/><c:errBarType val="both"/><c:errValType val="cust"/>');
    expect(chartXml).toContain('<c:noEndCap val="1"/>');
    expect(chartXml).toContain('<c:f>&apos;Error Data&apos;!$C$2:$C$3</c:f>');
    expect(chartXml).toContain('<c:f>&apos;Error Data&apos;!$D$2:$D$3</c:f>');
    expect(chartXml).toContain('<c:errDir val="y"/><c:errBarType val="plus"/><c:errValType val="fixedVal"/>');
    expect(chartXml).toContain('<c:val val="2"/>');

    const reopened = await importWorkFile(
      new File([exported], 'Error bars.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0].series[0].errorBars).toEqual([
      {
        direction: 'x',
        barType: 'both',
        valueType: 'custom',
        plusValues: [0.2, 0.3],
        plusReference: "'Error Data'!$C$2:$C$3",
        minusValues: [0.1, 0.15],
        minusReference: "'Error Data'!$D$2:$D$3",
        showEndCaps: false,
      },
      {
        direction: 'y',
        barType: 'plus',
        valueType: 'fixedValue',
        value: 2,
      },
    ]);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.error-bars')).toBe(false);

    if (!chartXml) throw new Error('Error-bar ChartML was not generated.');
    archive.file(
      'xl/charts/chart1.xml',
      chartXml.replace('<c:errValType val="cust"/>', '<c:errValType val="unsupported"/>')
    );
    const unsupported = await importWorkFile(
      new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Unsupported error bars.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.error-bars')).toBe(true);
  });

  it('round-trips editable primary and secondary axis titles with live references', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Axis Data';
    sheet.data = [
      [{ v: 'Quarter' }, { v: 'Revenue' }, { v: 'Margin' }, { v: 'Revenue (USD)' }, { v: 'Margin (%)' }],
      [{ v: 'Q1' }, { v: 42 }, { v: 0.12 }],
      [{ v: 'Q2' }, { v: 55 }, { v: 0.18 }],
    ];
    sheet.charts = [
      {
        id: 'chart-axis-titles',
        name: 'Performance axes',
        type: 'combination',
        title: 'Performance',
        axes: {
          bottom: { title: 'Quarter' },
          left: { title: 'Stale revenue title', titleReference: "'Axis Data'!$D$1" },
          top: { title: 'Secondary period' },
          right: { title: 'Stale margin title', titleReference: "'Axis Data'!$E$1" },
        },
        categories: [],
        categoryReference: "'Axis Data'!$A$2:$A$3",
        series: [
          {
            name: 'Revenue',
            values: [],
            valuesReference: "'Axis Data'!$B$2:$B$3",
            chartType: 'column',
            axisGroup: 'primary',
          },
          {
            name: 'Margin',
            values: [],
            valuesReference: "'Axis Data'!$C$2:$C$3",
            chartType: 'line',
            axisGroup: 'secondary',
          },
        ],
        showLegend: true,
        left: 96,
        top: 48,
        width: 576,
        height: 336,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');

    expect(chartXml?.match(/<c:title>/g)).toHaveLength(5);
    expect(chartXml).toContain('<c:axPos val="b"/><c:title>');
    expect(chartXml).toContain('<a:t>Quarter</a:t>');
    expect(chartXml).toContain('<c:f>&apos;Axis Data&apos;!$D$1</c:f>');
    expect(chartXml).toContain('<c:f>&apos;Axis Data&apos;!$E$1</c:f>');
    expect(chartXml).toContain('<c:axPos val="r"/><c:title>');

    const reopened = await importWorkFile(
      new File([exported], 'Axis titles.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0]).toMatchObject({
      title: 'Performance',
      axes: {
        bottom: { title: 'Quarter' },
        left: { title: 'Revenue (USD)', titleReference: "'Axis Data'!$D$1" },
        top: { title: 'Secondary period' },
        right: { title: 'Margin (%)', titleReference: "'Axis Data'!$E$1" },
      },
    });
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axis-titles')).toBe(false);

    if (!chartXml) throw new Error('Axis-title ChartML was not generated.');
    archive.file('xl/charts/chart1.xml', chartXml.replace('<a:t>Quarter</a:t>', '<a:t>Quarter</a:t><a:br/>'));
    const unsupported = await importWorkFile(
      new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Unsupported axis title.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axis-titles')).toBe(true);
  });

  it('round-trips value-axis ranges, major units, gridlines, and number formats', async () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    expect(artifact.content.type).toBe('spreadsheet');
    if (artifact.content.type !== 'spreadsheet') return;
    const sheet = artifact.content.sheets[0];
    sheet.name = 'Scale Data';
    sheet.data = [
      [{ v: 'Quarter' }, { v: 'Revenue' }, { v: 'Margin' }],
      [{ v: 'Q1' }, { v: 42 }, { v: 0.12 }],
      [{ v: 'Q2' }, { v: 55 }, { v: 0.18 }],
    ];
    sheet.charts = [
      {
        id: 'chart-axis-settings',
        name: 'Axis settings',
        type: 'combination',
        axes: {
          left: {
            minimum: 0,
            maximum: 100,
            majorUnit: 25,
            showMajorGridlines: false,
            numberFormat: '¥#,##0',
            numberFormatSourceLinked: false,
          },
          right: {
            minimum: 0,
            maximum: 0.3,
            majorUnit: 0.1,
            showMajorGridlines: true,
            numberFormat: '0.0%',
            numberFormatSourceLinked: false,
          },
        },
        categories: [],
        categoryReference: "'Scale Data'!$A$2:$A$3",
        series: [
          {
            name: 'Revenue',
            values: [],
            valuesReference: "'Scale Data'!$B$2:$B$3",
            chartType: 'column',
            axisGroup: 'primary',
          },
          {
            name: 'Margin',
            values: [],
            valuesReference: "'Scale Data'!$C$2:$C$3",
            chartType: 'line',
            axisGroup: 'secondary',
          },
        ],
        showLegend: true,
        left: 96,
        top: 48,
        width: 576,
        height: 336,
      },
    ];

    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');

    expect(chartXml).toContain('<c:max val="100"/><c:min val="0"/>');
    expect(chartXml).toContain('<c:majorUnit val="25"/>');
    expect(chartXml).toContain('<c:max val="0.3"/><c:min val="0"/>');
    expect(chartXml).toContain('<c:majorUnit val="0.1"/>');
    expect(chartXml?.match(/<c:majorGridlines\/>/g)).toHaveLength(1);
    expect(chartXml).toContain('<c:numFmt formatCode="¥#,##0" sourceLinked="0"/>');
    expect(chartXml).toContain('<c:numFmt formatCode="0.0%" sourceLinked="0"/>');

    const reopened = await importWorkFile(
      new File([exported], 'Axis settings.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0].axes).toEqual({
      left: {
        minimum: 0,
        maximum: 100,
        majorUnit: 25,
        showMajorGridlines: false,
        numberFormat: '¥#,##0',
        numberFormatSourceLinked: false,
      },
      right: {
        minimum: 0,
        maximum: 0.3,
        majorUnit: 0.1,
        showMajorGridlines: true,
        numberFormat: '0.0%',
        numberFormatSourceLinked: false,
      },
    });
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axis-settings')).toBe(false);

    if (!chartXml) throw new Error('Axis-setting ChartML was not generated.');
    archive.file(
      'xl/charts/chart1.xml',
      chartXml.replace('<c:majorUnit val="25"/>', '<c:majorUnit val="25"/><c:minorUnit val="5"/>')
    );
    const unsupported = await importWorkFile(
      new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Unsupported axis settings.xlsx', {
        type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
      })
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axis-settings')).toBe(true);
  });
});

async function createChartFixture(): Promise<File> {
  const XLSX = await import('xlsx');
  const workbook = XLSX.utils.book_new();
  XLSX.utils.book_append_sheet(
    workbook,
    XLSX.utils.aoa_to_sheet([
      ['Quarter', 'Revenue'],
      ['Q1', 42],
      ['Q2', 55],
    ]),
    'Report'
  );
  const buffer = XLSX.write(workbook, { type: 'array', bookType: 'xlsx' }) as ArrayBuffer;
  const archive = await JSZip.loadAsync(buffer);
  const worksheetPath = 'xl/worksheets/sheet1.xml';
  const worksheet = await archive.file(worksheetPath)?.async('text');
  if (!worksheet) throw new Error('Fixture worksheet was not generated.');
  const withRelationshipNamespace = worksheet.includes('xmlns:r=')
    ? worksheet
    : worksheet.replace(
        '<worksheet ',
        '<worksheet xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" '
      );
  archive.file(
    worksheetPath,
    withRelationshipNamespace.replace('</worksheet>', '<drawing r:id="rIdChartDrawing"/></worksheet>')
  );
  archive.file(
    'xl/worksheets/_rels/sheet1.xml.rels',
    relationships([
      [
        'rIdChartDrawing',
        '../drawings/drawing1.xml',
        'http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing',
      ],
    ])
  );
  archive.file(
    'xl/drawings/drawing1.xml',
    [
      '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
      '<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing"',
      ' xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"',
      ' xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"',
      ' xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">',
      '<xdr:twoCellAnchor editAs="oneCell">',
      marker('from', 1, 4),
      marker('to', 6, 16),
      '<xdr:graphicFrame macro=""><xdr:nvGraphicFramePr>',
      '<xdr:cNvPr id="2" name="Revenue chart" descr="Quarterly revenue trend"/>',
      '<xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>',
      '<xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm>',
      '<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">',
      '<c:chart r:id="rId1"/></a:graphicData></a:graphic>',
      '</xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>',
      '</xdr:wsDr>',
    ].join('')
  );
  archive.file(
    'xl/drawings/_rels/drawing1.xml.rels',
    relationships([
      ['rId1', '../charts/chart1.xml', 'http://schemas.openxmlformats.org/officeDocument/2006/relationships/chart'],
    ])
  );
  archive.file('xl/charts/chart1.xml', chartXml());
  const contentTypes = await archive.file('[Content_Types].xml')?.async('text');
  if (!contentTypes) throw new Error('Fixture content types were not generated.');
  archive.file(
    '[Content_Types].xml',
    contentTypes.replace(
      '</Types>',
      [
        '<Override PartName="/xl/drawings/drawing1.xml"',
        ' ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/>',
        '<Override PartName="/xl/charts/chart1.xml"',
        ' ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/>',
        '</Types>',
      ].join('')
    )
  );
  return new File([await archive.generateAsync({ type: 'arraybuffer' })], 'Charts.xlsx', {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}

function chartXml(): string {
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    '<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart"',
    ' xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">',
    '<c:style val="10"/><c:chart>',
    '<c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r>',
    '<a:t>Quarterly revenue</a:t></a:r></a:p></c:rich></c:tx></c:title>',
    '<c:plotArea><c:layout/><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>',
    '<c:ser><c:idx val="0"/><c:order val="0"/>',
    '<c:tx><c:strRef><c:f>Report!$B$1</c:f><c:strCache><c:ptCount val="1"/>',
    '<c:pt idx="0"><c:v>Revenue</c:v></c:pt></c:strCache></c:strRef></c:tx>',
    '<c:cat><c:strRef><c:f>Report!$A$2:$A$3</c:f><c:strCache><c:ptCount val="2"/>',
    '<c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt>',
    '</c:strCache></c:strRef></c:cat>',
    '<c:val><c:numRef><c:f>Report!$B$2:$B$3</c:f><c:numCache><c:formatCode>General</c:formatCode>',
    '<c:ptCount val="2"/><c:pt idx="0"><c:v>42</c:v></c:pt>',
    '<c:pt idx="1"><c:v>55</c:v></c:pt></c:numCache></c:numRef></c:val>',
    '<c:trendline><c:trendlineType val="linear"/></c:trendline>',
    '</c:ser><c:dLbls><c:showVal val="1"/></c:dLbls>',
    '<c:axId val="10000001"/><c:axId val="10000002"/></c:barChart>',
    '<c:catAx><c:axId val="10000001"/><c:crossAx val="10000002"/></c:catAx>',
    '<c:valAx><c:axId val="10000002"/><c:crossAx val="10000001"/></c:valAx>',
    '</c:plotArea><c:legend><c:legendPos val="r"/></c:legend></c:chart>',
    '</c:chartSpace>',
  ].join('');
}

function marker(kind: 'from' | 'to', column: number, row: number): string {
  return [
    `<xdr:${kind}>`,
    `<xdr:col>${column}</xdr:col><xdr:colOff>0</xdr:colOff>`,
    `<xdr:row>${row}</xdr:row><xdr:rowOff>0</xdr:rowOff>`,
    `</xdr:${kind}>`,
  ].join('');
}

function relationships(items: Array<[string, string, string]>): string {
  return [
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>',
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">',
    ...items.map(([id, target, type]) => `<Relationship Id="${id}" Target="${target}" Type="${type}"/>`),
    '</Relationships>',
  ].join('');
}
