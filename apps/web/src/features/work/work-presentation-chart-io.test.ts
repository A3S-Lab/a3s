import JSZip from 'jszip';
import PptxGenJS from 'pptxgenjs';
import { describe, expect, it } from 'vitest';
import { importWorkFile } from './work-file-io';
import { createPptxBlob } from './work-pptx-export';
import { createWorkArtifact } from './work-templates';
import type { WorkSlideChart, WorkSlideElement } from './work-types';

describe('Work presentation chart interoperability', () => {
  it('writes and reopens editable doughnut and radar charts with native settings', async () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') return;
    artifact.content.slides[0].elements = [
      chartElement('chart-doughnut', 8, {
        type: 'doughnut',
        title: 'Revenue mix',
        categories: ['Hardware', 'Software', 'Services'],
        series: [{ name: 'Revenue', values: [38, 44, 18] }],
        doughnutHoleSize: 72,
        dataLabels: {
          showCategoryName: true,
          showPercentage: true,
          separator: ' · ',
          position: 'outsideEnd',
        },
      }),
      chartElement('chart-radar', 54, {
        type: 'radar',
        title: 'Capability profile',
        categories: ['Speed', 'Quality', 'Reach'],
        series: [{ name: 'Current', values: [8, 9, 7] }],
        radarStyle: 'filled',
      }),
      chartElement('chart-column', 28, {
        type: 'column',
        title: 'Quarterly revenue',
        categories: ['Q1', 'Q2'],
        series: [
          {
            name: 'Actual',
            values: [42, 58],
            trendlines: [
              {
                type: 'polynomial',
                name: 'Forecast',
                order: 3,
                forward: 1.5,
                displayEquation: true,
                displayRSquared: true,
              },
            ],
            errorBars: [
              {
                direction: 'y',
                barType: 'both',
                valueType: 'percentage',
                value: 10,
                showEndCaps: false,
              },
            ],
          },
          { name: 'Target', values: [50, 60] },
        ],
        showLegend: true,
        legendPosition: 'bottom',
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
            majorUnit: 20,
            showMajorGridlines: false,
            numberFormat: '#,##0',
            numberFormatSourceLinked: false,
          },
        },
        dataLabels: {
          showValue: true,
          showCategoryName: true,
          showSeriesName: true,
          separator: ' / ',
          position: 'outsideEnd',
        },
      }),
    ];

    const blob = await createPptxBlob(artifact, PptxGenJS);
    const archive = await JSZip.loadAsync(await blob.arrayBuffer());
    const chartParts = Object.keys(archive.files).filter((path) => /^ppt\/charts\/chart\d+\.xml$/.test(path));
    const chartXml = await Promise.all(chartParts.map((path) => archive.file(path)?.async('text')));
    expect(chartXml.some((xml) => xml?.includes('<c:doughnutChart>') && xml.includes('<c:holeSize val="72"/>'))).toBe(
      true
    );
    expect(
      chartXml.some((xml) => xml?.includes('<c:radarChart>') && xml.includes('<c:radarStyle val="filled"/>'))
    ).toBe(true);
    expect(
      chartXml.some(
        (xml) =>
          xml?.includes('<c:doughnutChart>') &&
          xml.includes('<c:dLblPos val="outEnd"/>') &&
          xml.includes('<c:showCatName val="1"/>') &&
          xml.includes('<c:showPercent val="1"/>') &&
          xml.includes('<c:separator> · </c:separator>')
      )
    ).toBe(true);
    expect(
      chartXml.some(
        (xml) =>
          xml?.includes('<c:barChart>') &&
          xml.includes('<c:legendPos val="b"/>') &&
          xml.includes('<a:t>Quarter</a:t>') &&
          xml.includes('<a:t>Revenue</a:t>') &&
          xml.includes('<c:dLblPos val="outEnd"/>') &&
          xml.includes('<c:showVal val="1"/>') &&
          xml.includes('<c:showCatName val="1"/>') &&
          xml.includes('<c:showSerName val="1"/>') &&
          xml.includes('<c:separator> / </c:separator>') &&
          xml.includes('<c:orientation val="maxMin"/>') &&
          xml.includes('<c:min val="0"/>') &&
          xml.includes('<c:max val="100"/>') &&
          xml.includes('<c:majorUnit val="20"/>') &&
          xml.includes('<c:tickLblPos val="high"/>') &&
          xml.includes('<c:majorTickMark val="out"/>') &&
          xml.includes('<c:tickLblSkip val="2"/>') &&
          xml.includes('<c:numFmt formatCode="#,##0" sourceLinked="0"/>') &&
          xml.includes('<c:trendlineType val="poly"/>') &&
          xml.includes('<c:order val="3"/>') &&
          xml.includes('<c:forward val="1.5"/>') &&
          xml.includes('<c:dispRSq val="1"/>') &&
          xml.includes('<c:dispEq val="1"/>') &&
          xml.includes('<c:errDir val="y"/>') &&
          xml.includes('<c:errBarType val="both"/>') &&
          xml.includes('<c:errValType val="percentage"/>') &&
          xml.includes('<c:noEndCap val="1"/>') &&
          xml.includes('<c:val val="10"/>')
      )
    ).toBe(true);

    const reopened = await importWorkFile(
      new File([blob], 'Editable presentation charts.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );
    expect(reopened.content.type).toBe('presentation');
    if (reopened.content.type !== 'presentation') return;
    const charts = reopened.content.slides[0].elements.flatMap((element) => (element.chart ? [element.chart] : []));
    expect(charts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'doughnut',
          title: 'Revenue mix',
          categories: ['Hardware', 'Software', 'Services'],
          series: [{ name: 'Revenue', values: [38, 44, 18] }],
          doughnutHoleSize: 72,
          dataLabels: {
            showCategoryName: true,
            showPercentage: true,
            separator: ' · ',
            position: 'outsideEnd',
          },
        }),
        expect.objectContaining({
          type: 'radar',
          title: 'Capability profile',
          categories: ['Speed', 'Quality', 'Reach'],
          series: [{ name: 'Current', values: [8, 9, 7] }],
          radarStyle: 'filled',
        }),
        expect.objectContaining({
          type: 'column',
          title: 'Quarterly revenue',
          series: [
            expect.objectContaining({
              name: 'Actual',
              values: [42, 58],
              trendlines: [
                {
                  type: 'polynomial',
                  name: 'Forecast',
                  order: 3,
                  forward: 1.5,
                  displayEquation: true,
                  displayRSquared: true,
                },
              ],
              errorBars: [
                {
                  direction: 'y',
                  barType: 'both',
                  valueType: 'percentage',
                  value: 10,
                  showEndCaps: false,
                },
              ],
            }),
            expect.objectContaining({ name: 'Target', values: [50, 60] }),
          ],
          showLegend: true,
          legendPosition: 'bottom',
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
              majorUnit: 20,
              showMajorGridlines: false,
              numberFormat: '#,##0',
              numberFormatSourceLinked: false,
            },
          },
          dataLabels: {
            showValue: true,
            showCategoryName: true,
            showSeriesName: true,
            separator: ' / ',
            position: 'outsideEnd',
          },
        }),
      ])
    );
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'pptx.chart.type')).toBe(false);
  });

  it('writes and reopens editable scatter and bubble charts with native XY settings', async () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') return;
    artifact.content.slides[0].elements = [
      chartElement('chart-scatter', 8, {
        type: 'scatter',
        title: 'Latency response',
        categories: ['1', '2', '4'],
        series: [
          {
            name: 'Latency',
            values: [42, 58, 73],
            trendlines: [{ type: 'movingAverage', name: 'Rolling', period: 2 }],
            errorBars: [
              {
                direction: 'x',
                barType: 'both',
                valueType: 'custom',
                plusValues: [0.5, 1, 1.5],
                minusValues: [0.25, 0.5, 0.75],
              },
            ],
          },
        ],
        scatterStyle: 'smoothMarker',
        axes: {
          bottom: {
            title: 'Concurrency',
            minimum: 0,
            maximum: 5,
            majorUnit: 1,
            numberFormat: '0.0',
            numberFormatSourceLinked: false,
          },
          left: { title: 'Latency', minimum: 0, maximum: 100, majorUnit: 20, showMajorGridlines: true },
        },
      }),
      chartElement('chart-bubble', 54, {
        type: 'bubble',
        title: 'Capacity map',
        categories: ['1', '2', '4'],
        series: [{ name: 'Capacity', values: [5, 8, 13], bubbleSizes: [9, -16, 25] }],
        bubbleScale: 140,
        showNegativeBubbles: true,
        bubbleSizeRepresents: 'width',
        dataLabels: {
          showValue: true,
          showBubbleSize: true,
          separator: ' / ',
          position: 'above',
        },
      }),
    ];

    const blob = await createPptxBlob(artifact, PptxGenJS);
    const archive = await JSZip.loadAsync(await blob.arrayBuffer());
    const chartParts = Object.keys(archive.files).filter((path) => /^ppt\/charts\/chart\d+\.xml$/.test(path));
    const chartXml = await Promise.all(chartParts.map((path) => archive.file(path)?.async('text')));
    expect(
      chartXml.some(
        (xml) =>
          xml?.includes('<c:scatterChart>') &&
          xml.includes('<c:scatterStyle val="smoothMarker"/>') &&
          xml.includes('<c:trendlineType val="movingAvg"/>') &&
          xml.includes('<c:period val="2"/>') &&
          xml.includes('<c:errDir val="x"/>') &&
          xml.includes('<c:errValType val="cust"/>') &&
          xml.includes('<c:plus><c:numLit>') &&
          xml.includes('<c:minus><c:numLit>') &&
          xml.includes('<c:v>1</c:v>') &&
          xml.includes('<c:v>73</c:v>') &&
          xml.includes('<a:t>Concurrency</a:t>') &&
          xml.includes('<a:t>Latency</a:t>') &&
          xml.includes('<c:min val="0"/>') &&
          xml.includes('<c:max val="5"/>') &&
          xml.includes('<c:majorUnit val="1"/>') &&
          xml.includes('<c:numFmt formatCode="0.0" sourceLinked="0"/>')
      )
    ).toBe(true);
    expect(
      chartXml.some(
        (xml) =>
          xml?.includes('<c:bubbleChart>') &&
          xml.includes('<c:bubbleScale val="140"/>') &&
          xml.includes('<c:showNegBubbles val="1"/>') &&
          xml.includes('<c:sizeRepresents val="w"/>') &&
          xml.includes('<c:v>-16</c:v>') &&
          xml.includes('<c:showBubbleSize val="1"/>')
      )
    ).toBe(true);

    const reopened = await importWorkFile(
      new File([blob], 'Editable presentation XY charts.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );
    expect(reopened.content.type).toBe('presentation');
    if (reopened.content.type !== 'presentation') return;
    const charts = reopened.content.slides[0].elements.flatMap((element) => (element.chart ? [element.chart] : []));
    expect(charts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'scatter',
          title: 'Latency response',
          categories: ['1', '2', '4'],
          series: [
            {
              name: 'Latency',
              values: [42, 58, 73],
              trendlines: [{ type: 'movingAverage', name: 'Rolling', period: 2 }],
              errorBars: [
                {
                  direction: 'x',
                  barType: 'both',
                  valueType: 'custom',
                  plusValues: [0.5, 1, 1.5],
                  minusValues: [0.25, 0.5, 0.75],
                },
              ],
            },
          ],
          scatterStyle: 'smoothMarker',
          axes: {
            bottom: {
              title: 'Concurrency',
              minimum: 0,
              maximum: 5,
              majorUnit: 1,
              numberFormat: '0.0',
              numberFormatSourceLinked: false,
            },
            left: { title: 'Latency', minimum: 0, maximum: 100, majorUnit: 20, showMajorGridlines: true },
          },
        }),
        expect.objectContaining({
          type: 'bubble',
          title: 'Capacity map',
          categories: ['1', '2', '4'],
          series: [{ name: 'Capacity', values: [5, 8, 13], bubbleSizes: [9, -16, 25] }],
          bubbleScale: 140,
          showNegativeBubbles: true,
          bubbleSizeRepresents: 'width',
          dataLabels: {
            showValue: true,
            showBubbleSize: true,
            separator: ' / ',
            position: 'above',
          },
        }),
      ])
    );
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'pptx.chart.type')).toBe(false);
  });

  it('round-trips presentation legend overlay, plot layout, smoothing, and series appearance through ChartML', async () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') return;
    artifact.content.slides[0].elements = [
      chartElement('chart-layout-column', 8, {
        type: 'column',
        title: 'Revenue layout',
        categories: ['Q1', 'Q2'],
        series: [
          {
            name: 'Revenue',
            values: [42, 58],
            style: {
              fillColor: '#112233',
              fillTransparency: 35,
              lineColor: '#445566',
              lineWidth: 3.25,
              lineDash: 'dashDot',
            },
          },
          { name: 'Target', values: [50, 60] },
        ],
        showLegend: true,
        legendPosition: 'bottom',
        legendOverlay: true,
        grouping: 'clustered',
        gapWidth: 240,
        overlap: -25,
      }),
      chartElement('chart-layout-line', 54, {
        type: 'line',
        title: 'Share trend',
        categories: ['Q1', 'Q2'],
        series: [
          {
            name: 'Revenue',
            values: [40, 60],
            style: {
              lineColor: '#2255AA',
              lineWidth: 2.75,
              lineDash: 'dot',
              marker: { symbol: 'star', size: 11, fillColor: '#FFCC00', lineColor: '#2255AA' },
            },
          },
          { name: 'Cost', values: [60, 40] },
        ],
        grouping: 'percentStacked',
        smoothLines: true,
      }),
    ];

    const blob = await createPptxBlob(artifact, PptxGenJS);
    const archive = await JSZip.loadAsync(await blob.arrayBuffer());
    const chartParts = Object.keys(archive.files)
      .filter((path) => /^ppt\/charts\/chart\d+\.xml$/.test(path))
      .sort();
    const chartXml = await Promise.all(chartParts.map((path) => archive.file(path)?.async('text')));

    expect(
      chartXml.some(
        (xml) =>
          xml?.includes('<c:barChart>') &&
          xml.includes('<c:grouping val="clustered"/>') &&
          xml.includes('<c:gapWidth val="240"/>') &&
          xml.includes('<c:overlap val="-25"/>') &&
          xml.includes('<c:overlay val="1"/>') &&
          xml.includes('<a:srgbClr val="112233"><a:alpha val="65000"/>') &&
          xml.includes('<a:ln w="41275">') &&
          xml.includes('<a:prstDash val="dashDot"/>')
      )
    ).toBe(true);
    expect(
      chartXml.some(
        (xml) =>
          xml?.includes('<c:lineChart>') &&
          xml.includes('<c:grouping val="percentStacked"/>') &&
          xml.includes('<c:smooth val="1"/>') &&
          xml.includes('<c:marker><c:symbol val="star"/><c:size val="11"/>') &&
          xml.includes('<a:prstDash val="dot"/>')
      )
    ).toBe(true);

    const reopened = await importWorkFile(
      new File([blob], 'Presentation chart layout and style.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );
    expect(reopened.content.type).toBe('presentation');
    if (reopened.content.type !== 'presentation') return;
    const charts = reopened.content.slides[0].elements.flatMap((element) => (element.chart ? [element.chart] : []));
    expect(charts).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          type: 'column',
          legendPosition: 'bottom',
          legendOverlay: true,
          grouping: 'clustered',
          gapWidth: 240,
          overlap: -25,
          series: [
            expect.objectContaining({
              style: {
                fillColor: '#112233',
                fillTransparency: 35,
                lineColor: '#445566',
                lineWidth: 3.25,
                lineDash: 'dashDot',
              },
            }),
            expect.any(Object),
          ],
        }),
        expect.objectContaining({
          type: 'line',
          grouping: 'percentStacked',
          smoothLines: true,
          series: [
            expect.objectContaining({
              style: {
                lineColor: '#2255AA',
                lineWidth: 2.75,
                lineDash: 'dot',
                marker: { symbol: 'star', size: 11, fillColor: '#FFCC00', lineColor: '#2255AA' },
              },
            }),
            expect.any(Object),
          ],
        }),
      ])
    );
    expect(
      reopened.compatibility?.issues.some((issue) =>
        ['pptx.chart.format.legend', 'pptx.chart.format.layout', 'pptx.chart.format.series'].includes(issue.code)
      )
    ).toBe(false);
  });
});

function chartElement(id: string, x: number, chart: WorkSlideChart): WorkSlideElement {
  return {
    id,
    type: 'chart',
    x,
    y: 18,
    width: 38,
    height: 55,
    text: '',
    fontSize: 14,
    color: '#172033',
    fill: '#ffffff',
    bold: false,
    align: 'center',
    chart,
    altText: chart.title,
  };
}
