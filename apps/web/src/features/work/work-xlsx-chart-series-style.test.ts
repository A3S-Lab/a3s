import JSZip from 'jszip';
import { describe, expect, it } from 'vitest';
import { createWorkArtifactBlob, importWorkFile } from './work-file-io';
import { createWorkArtifact } from './work-templates';
import type { WorkSpreadsheetChart, WorkSpreadsheetChartSeriesStyle } from './work-types';

describe('Work XLSX chart series style interoperability', () => {
  it('round-trips simple sRGB fill, transparency, line, dash, and marker formatting for normal, XY, and combination charts', async () => {
    const artifact = styledChartArtifact();
    const exported = await createWorkArtifactBlob(artifact);
    const archive = await JSZip.loadAsync(exported);
    const chartParts = await Promise.all(
      [1, 2, 3].map(async (index) => {
        const xml = await archive.file(`xl/charts/chart${index}.xml`)?.async('text');
        if (!xml) throw new Error(`ChartML style fixture ${index} was not generated.`);
        return xml;
      })
    );

    for (const chartXml of chartParts) {
      expect(chartXml).toContain('<c:spPr><a:solidFill><a:srgbClr val="112233"><a:alpha val="65000"/>');
      expect(chartXml).toContain('<a:ln w="41275"><a:solidFill><a:srgbClr val="445566"/></a:solidFill>');
      expect(chartXml).toContain('<a:prstDash val="dashDot"/>');
      expect(chartXml).toContain('<c:marker><c:symbol val="diamond"/><c:size val="9"/>');
      expect(chartXml).toContain('<a:srgbClr val="778899"/>');
      expect(chartXml).toContain('<a:srgbClr val="AABBCC"/>');
    }

    const reopened = await importWorkFile(workbookFile(exported, 'Series style.xlsx'));
    expect(reopened.content.type).toBe('spreadsheet');
    if (reopened.content.type !== 'spreadsheet') return;
    expect(reopened.content.sheets[0].charts).toHaveLength(3);
    for (const chart of reopened.content.sheets[0].charts ?? []) {
      for (const series of chart.series) expect(series.style).toEqual(seriesStyle());
    }
    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.format')).toBe(false);
  });

  it('keeps unsupported theme, gradient, pattern, effect, and custom dash formatting explicit in diagnostics', async () => {
    const exported = await createWorkArtifactBlob(styledChartArtifact());
    const archive = await JSZip.loadAsync(exported);
    const chartXml = await archive.file('xl/charts/chart1.xml')?.async('text');
    if (!chartXml) throw new Error('ChartML style fixture was not generated.');
    archive.file(
      'xl/charts/chart1.xml',
      chartXml
        .replace(
          '<a:solidFill><a:srgbClr val="112233"><a:alpha val="65000"/></a:srgbClr></a:solidFill>',
          '<a:gradFill><a:gsLst><a:gs pos="0"><a:schemeClr val="accent1"/></a:gs></a:gsLst></a:gradFill>'
        )
        .replace('<a:prstDash val="dashDot"/>', '<a:prstDash val="sysDash"/>')
        .replace('</c:spPr><c:marker>', '<a:effectLst><a:outerShdw/></a:effectLst></c:spPr><c:marker>')
    );

    const reopened = await importWorkFile(
      workbookFile(await archive.generateAsync({ type: 'arraybuffer' }), 'Unsupported series style.xlsx')
    );

    expect(reopened.compatibility?.issues.some((issue) => issue.code === 'xlsx.charts.format')).toBe(true);
  });
});

function styledChartArtifact() {
  const artifact = createWorkArtifact('blank-spreadsheet');
  if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid.');
  const sheet = artifact.content.sheets[0];
  sheet.name = 'Series Data';
  sheet.data = [
    [{ v: 'Quarter' }, { v: 'Revenue' }, { v: 'Margin' }],
    [{ v: 1 }, { v: 20 }, { v: 0.2 }],
    [{ v: 2 }, { v: 50 }, { v: 0.3 }],
    [{ v: 3 }, { v: 80 }, { v: 0.4 }],
  ];
  const base: Omit<WorkSpreadsheetChart, 'id' | 'name' | 'type' | 'series' | 'left' | 'top'> = {
    categories: ['1', '2', '3'],
    categoryReference: "'Series Data'!$A$2:$A$4",
    showLegend: true,
    width: 480,
    height: 288,
  };
  sheet.charts = [
    {
      ...base,
      id: 'styled-line',
      name: 'Styled line',
      type: 'line',
      series: [categorySeries('Revenue', "'Series Data'!$B$2:$B$4")],
      left: 0,
      top: 0,
    },
    {
      ...base,
      id: 'styled-scatter',
      name: 'Styled scatter',
      type: 'scatter',
      scatterStyle: 'lineMarker',
      categories: [],
      categoryReference: undefined,
      series: [
        {
          ...categorySeries('Revenue', "'Series Data'!$B$2:$B$4"),
          xValues: [1, 2, 3],
          xValuesReference: "'Series Data'!$A$2:$A$4",
        },
      ],
      left: 496,
      top: 0,
    },
    {
      ...base,
      id: 'styled-combination',
      name: 'Styled combination',
      type: 'combination',
      series: [
        {
          ...categorySeries('Revenue', "'Series Data'!$B$2:$B$4"),
          chartType: 'line',
          axisGroup: 'primary',
        },
        {
          ...categorySeries('Margin', "'Series Data'!$C$2:$C$4", [0.2, 0.3, 0.4]),
          chartType: 'line',
          axisGroup: 'secondary',
        },
      ],
      left: 0,
      top: 304,
    },
  ];
  return artifact;
}

function categorySeries(name: string, valuesReference: string, values = [20, 50, 80]) {
  return {
    name,
    values,
    valuesReference,
    style: seriesStyle(),
  };
}

function seriesStyle(): WorkSpreadsheetChartSeriesStyle {
  return {
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
  };
}

function workbookFile(data: BlobPart, name: string): File {
  return new File([data], name, {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  });
}
