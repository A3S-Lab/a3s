import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';
import { createWorkArtifact } from './work-templates';
import type { WorkArtifact, WorkSpreadsheetChart } from './work-types';

describe('Work XLSX chart legend and plot-layout interoperability', () => {
  it('round-trips native legend, grouping, spacing, overlap, and smoothing settings', async () => {
    const artifact = layoutArtifact();
    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const columnXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    const lineXml = await archive.file('xl/charts/chart2.xml')?.async('text');

    expect(columnXml).toContain('<c:legendPos val="b"/>');
    expect(columnXml).toContain('<c:overlay val="1"/>');
    expect(columnXml).toContain('<c:grouping val="percentStacked"/>');
    expect(columnXml).toContain('<c:gapWidth val="240"/>');
    expect(columnXml).toContain('<c:overlap val="85"/>');
    expect(lineXml).toContain('<c:legendPos val="l"/>');
    expect(lineXml).toContain('<c:overlay val="0"/>');
    expect(lineXml).toContain('<c:grouping val="stacked"/>');
    expect(lineXml).toContain('<c:smooth val="1"/>');

    const reopened = await importWorkFile(workbookFile(exported, 'Chart layouts.xlsx'));
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts).toEqual([
      expect.objectContaining({
        type: 'column',
        legendPosition: 'bottom',
        legendOverlay: true,
        grouping: 'percentStacked',
        gapWidth: 240,
        overlap: 85,
      }),
      expect.objectContaining({
        type: 'line',
        legendPosition: 'left',
        legendOverlay: false,
        grouping: 'stacked',
        smoothLines: true,
      }),
    ]);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.legend')).toBe(false);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.layout')).toBe(false);

    if (!columnXml || !lineXml) throw new Error('ChartML layout fixtures were not generated.');
    archive.file(
      'xl/charts/chart1.xml',
      columnXml
        .replace('<c:gapWidth val="240"/>', '<c:gapWidth val="900"/>')
        .replace('</c:ser>', '<c:trendline><c:trendlineType val="linear"/></c:trendline></c:ser>')
        .replace(
          '<c:legend><c:legendPos val="b"/><c:layout/>',
          '<c:legend><c:legendPos val="b"/><c:layout><c:manualLayout><c:x val="0.2"/></c:manualLayout></c:layout><c:legendEntry><c:idx val="0"/><c:delete val="1"/></c:legendEntry>'
        )
    );
    archive.file(
      'xl/charts/chart2.xml',
      lineXml.replace('<c:ser><c:idx val="0"', '<c:ser><c:smooth val="0"/><c:idx val="0"')
    );
    const unsupported = await importWorkFile(
      workbookFile(await archive.generateAsync({ type: 'arraybuffer' }), 'Unsupported chart layouts.xlsx')
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.legend')).toBe(true);
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.layout')).toBe(true);
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.trendline')).toBe(true);
    expect(unsupported.content.type).toBe('spreadsheet');
    if (unsupported.content.type === 'spreadsheet') {
      expect(unsupported.content.sheets[0].charts?.[0].series[0].trendlines).toBeUndefined();
    }
  });

  it('does not import a stacked combination as a misleading standard combination', async () => {
    const artifact = layoutArtifact();
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid.');
    artifact.content.sheets[0].charts = [combinationChart()];
    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    if (!chartXml) throw new Error('Combination ChartML was not generated.');
    archive.file(
      'xl/charts/chart1.xml',
      chartXml.replace('<c:grouping val="clustered"/>', '<c:grouping val="stacked"/>')
    );

    const reopened = await importWorkFile(
      workbookFile(await archive.generateAsync({ type: 'arraybuffer' }), 'Stacked combination.xlsx')
    );

    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts).toBeUndefined();
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.combination')).toBe(true);
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.unsupported-type')).toBe(true);
  });
});

function layoutArtifact(): WorkArtifact {
  const artifact = createWorkArtifact('blank-spreadsheet');
  if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid.');
  const sheet = artifact.content.sheets[0];
  sheet.name = 'Layout Data';
  sheet.data = [
    [{ v: 'Quarter' }, { v: 'Actual' }, { v: 'Forecast' }],
    [{ v: 'Q1' }, { v: 20 }, { v: 80 }],
    [{ v: 'Q2' }, { v: -30 }, { v: -70 }],
  ];
  const base: Omit<WorkSpreadsheetChart, 'id' | 'name' | 'type' | 'left' | 'top'> = {
    title: 'Layout comparison',
    categories: ['Q1', 'Q2'],
    series: [
      { name: 'Actual', values: [20, -30] },
      { name: 'Forecast', values: [80, -70] },
    ],
    showLegend: true,
    width: 480,
    height: 288,
  };
  sheet.charts = [
    {
      ...base,
      id: 'chart-column-layout',
      name: 'Column layout',
      type: 'column',
      legendPosition: 'bottom',
      legendOverlay: true,
      grouping: 'percentStacked',
      gapWidth: 240,
      overlap: 85,
      left: 48,
      top: 48,
    },
    {
      ...base,
      id: 'chart-line-layout',
      name: 'Line layout',
      type: 'line',
      legendPosition: 'left',
      legendOverlay: false,
      grouping: 'stacked',
      smoothLines: true,
      left: 576,
      top: 48,
    },
  ];
  return artifact;
}

function combinationChart(): WorkSpreadsheetChart {
  return {
    id: 'chart-combination-layout',
    name: 'Combination layout',
    type: 'combination',
    categories: ['Q1', 'Q2'],
    series: [
      { name: 'Actual', values: [20, 30], chartType: 'column', axisGroup: 'primary' },
      { name: 'Forecast', values: [22, 34], chartType: 'line', axisGroup: 'secondary' },
    ],
    showLegend: true,
    left: 48,
    top: 48,
    width: 480,
    height: 288,
  };
}

function workbookFile(data: BlobPart, name: string): File {
  return new File([data], name, {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}
