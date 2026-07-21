import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';
import { createWorkArtifact } from './work-templates';

describe('Work XLSX chart axis display interoperability', () => {
  it('round-trips orientation, label placement, major ticks, and category-label intervals', async () => {
    const artifact = axisDisplayArtifact();
    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    if (!chartXml) throw new Error('ChartML axis fixture was not generated.');

    expect(chartXml.match(/<c:orientation val="maxMin"\/>/g)).toHaveLength(2);
    expect(chartXml).toContain('<c:tickLblPos val="high"/>');
    expect(chartXml).toContain('<c:majorTickMark val="out"/>');
    expect(chartXml).toContain('<c:tickLblSkip val="2"/>');
    expect(chartXml).toContain('<c:tickLblPos val="none"/>');
    expect(chartXml).toContain('<c:majorTickMark val="cross"/>');

    const reopened = await importWorkFile(workbookFile(exported, 'Axis display.xlsx'));
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts?.[0]).toMatchObject({
      axes: {
        bottom: {
          reverseOrder: true,
          labelPosition: 'high',
          majorTickMark: 'outside',
          labelInterval: 2,
        },
        left: {
          reverseOrder: true,
          labelPosition: 'none',
          majorTickMark: 'cross',
        },
      },
    });
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.category-axis-settings')).toBe(
      false
    );
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axis-settings')).toBe(false);

    archive.file(
      'xl/charts/chart1.xml',
      chartXml
        .replace('<c:tickLblSkip val="2"/>', '<c:tickLblSkip val="0"/>')
        .replace('<c:tickLblPos val="none"/>', '<c:tickLblPos val="diagonal"/>')
    );
    const unsupported = await importWorkFile(
      workbookFile(await archive.generateAsync({ type: 'arraybuffer' }), 'Unsupported axis display.xlsx')
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.category-axis-settings')).toBe(
      true
    );
    expect(unsupported.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.axis-settings')).toBe(true);
  });
});

function axisDisplayArtifact() {
  const artifact = createWorkArtifact('blank-spreadsheet');
  if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid.');
  const sheet = artifact.content.sheets[0];
  sheet.name = 'Axis Data';
  sheet.data = [
    [{ v: 'Quarter' }, { v: 'Revenue' }],
    [{ v: 'Q1' }, { v: 20 }],
    [{ v: 'Q2' }, { v: 50 }],
    [{ v: 'Q3' }, { v: 80 }],
  ];
  sheet.charts = [
    {
      id: 'chart-axis-display',
      name: 'Axis display',
      type: 'column',
      axes: {
        bottom: {
          reverseOrder: true,
          labelPosition: 'high',
          majorTickMark: 'outside',
          labelInterval: 2,
        },
        left: {
          minimum: 0,
          maximum: 100,
          majorUnit: 50,
          reverseOrder: true,
          labelPosition: 'none',
          majorTickMark: 'cross',
        },
      },
      categories: ['Q1', 'Q2', 'Q3'],
      categoryReference: "'Axis Data'!$A$2:$A$4",
      series: [{ name: 'Revenue', values: [20, 50, 80], valuesReference: "'Axis Data'!$B$2:$B$4" }],
      showLegend: false,
      left: 48,
      top: 48,
      width: 480,
      height: 288,
    },
  ];
  return artifact;
}

function workbookFile(data: BlobPart, name: string): File {
  return new File([data], name, {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}
