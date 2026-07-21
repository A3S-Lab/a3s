import { describe, expect, it } from 'vitest';
import { parseXml } from './work-ooxml-package';
import { readPptxChart } from './work-pptx-chart-import';

const CHART_NAMESPACE = 'http://schemas.openxmlformats.org/drawingml/2006/chart';

describe('Work PPTX chart import', () => {
  it('combines multi-level category caches and diagnoses an invalid doughnut hole size', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:doughnutChart>
          <c:ser>
            <c:tx><c:strLit><c:pt idx="0"><c:v>Revenue</c:v></c:pt></c:strLit></c:tx>
            <c:cat><c:multiLvlStrRef><c:multiLvlStrCache>
              <c:lvl><c:pt idx="0"><c:v>North</c:v></c:pt><c:pt idx="1"><c:v>South</c:v></c:pt></c:lvl>
              <c:lvl><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:lvl>
            </c:multiLvlStrCache></c:multiLvlStrRef></c:cat>
            <c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numLit></c:val>
          </c:ser>
          <c:holeSize val="5"/>
        </c:doughnutChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart).toMatchObject({
      type: 'doughnut',
      categories: ['North / Q1', 'South / Q2'],
      series: [{ name: 'Revenue', values: [42, 58] }],
      doughnutHoleSize: 10,
    });
    expect(result.diagnostics).toContainEqual(
      expect.objectContaining({ code: 'pptx.chart.doughnut-hole', message: expect.stringContaining('normalized') })
    );
  });

  it('normalizes unknown radar styles with a precise diagnostic', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:radarChart>
          <c:radarStyle val="wireframe"/>
          <c:ser><c:tx><c:v>Current</c:v></c:tx><c:cat><c:strLit><c:pt idx="0"><c:v>Speed</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:pt idx="0"><c:v>8</c:v></c:pt></c:numLit></c:val></c:ser>
        </c:radarChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart).toMatchObject({ type: 'radar', radarStyle: 'standard' });
    expect(result.diagnostics).toContainEqual(
      expect.objectContaining({ code: 'pptx.chart.radar-style', message: expect.stringContaining('wireframe') })
    );
  });

  it('keeps chart titles distinct from axis titles and imports the legend position', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <c:chart><c:plotArea>
            <c:barChart><c:barDir val="col"/><c:ser><c:tx><c:v>Revenue</c:v></c:tx><c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt></c:numLit></c:val></c:ser></c:barChart>
            <c:catAx><c:scaling><c:orientation val="maxMin"/></c:scaling><c:axPos val="b"/><c:title><c:tx><c:rich><a:p><a:r><a:t>Quarter</a:t></a:r></a:p></c:rich></c:tx></c:title><c:majorTickMark val="out"/><c:tickLblPos val="high"/><c:tickLblSkip val="2"/></c:catAx>
            <c:valAx><c:scaling><c:orientation val="minMax"/><c:max val="100"/><c:min val="0"/></c:scaling><c:axPos val="l"/><c:title><c:tx><c:rich><a:p><a:r><a:t>Revenue (CNY)</a:t></a:r></a:p></c:rich></c:tx></c:title><c:numFmt formatCode="#,##0" sourceLinked="0"/><c:majorTickMark val="cross"/><c:tickLblPos val="low"/><c:majorUnit val="20"/></c:valAx>
          </c:plotArea><c:legend><c:legendPos val="b"/></c:legend></c:chart>
        </c:chartSpace>
      `)
    );

    expect(result.chart).toMatchObject({
      type: 'column',
      title: undefined,
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
          title: 'Revenue (CNY)',
          labelPosition: 'low',
          majorTickMark: 'cross',
          minimum: 0,
          maximum: 100,
          majorUnit: 20,
          showMajorGridlines: false,
          numberFormat: '#,##0',
          numberFormatSourceLinked: false,
        },
      },
    });
  });

  it('imports editable legend overlay, plot layout, smoothing, and portable series appearance', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <c:chart><c:plotArea><c:lineChart><c:grouping val="percentStacked"/><c:ser>
            <c:tx><c:v>Revenue</c:v></c:tx>
            <c:spPr><a:solidFill><a:srgbClr val="112233"><a:alpha val="65000"/></a:srgbClr></a:solidFill><a:ln w="41275"><a:solidFill><a:srgbClr val="445566"/></a:solidFill><a:prstDash val="dashDot"/></a:ln></c:spPr>
            <c:marker><c:symbol val="diamond"/><c:size val="9"/><c:spPr><a:solidFill><a:srgbClr val="778899"/></a:solidFill><a:ln><a:solidFill><a:srgbClr val="AABBCC"/></a:solidFill></a:ln></c:spPr></c:marker>
            <c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt><c:pt idx="1"><c:v>Q2</c:v></c:pt></c:strLit></c:cat>
            <c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numLit></c:val>
          </c:ser><c:smooth val="1"/></c:lineChart></c:plotArea>
          <c:legend><c:legendPos val="b"/><c:layout/><c:overlay val="1"/></c:legend></c:chart>
        </c:chartSpace>
      `)
    );

    expect(result.chart).toMatchObject({
      type: 'line',
      legendPosition: 'bottom',
      legendOverlay: true,
      grouping: 'percentStacked',
      smoothLines: true,
      series: [
        {
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
        },
      ],
    });
    expect(result.diagnostics.some((diagnostic) => diagnostic.code.startsWith('pptx.chart.format'))).toBe(false);
  });

  it('diagnoses unsupported manual legend, plot, theme, effect, and per-point series formatting precisely', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="stacked3d"/><c:ser>
            <c:tx><c:v>Revenue</c:v></c:tx>
            <c:spPr><a:solidFill><a:schemeClr val="accent1"/></a:solidFill><a:effectLst><a:outerShdw/></a:effectLst></c:spPr>
            <c:dPt><c:idx val="0"/><c:spPr><a:gradFill/></c:spPr></c:dPt>
            <c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strLit></c:cat>
            <c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt></c:numLit></c:val>
          </c:ser><c:gapWidth val="900"/></c:barChart></c:plotArea>
          <c:legend><c:legendPos val="b"/><c:layout><c:manualLayout><c:x val="0.2"/></c:manualLayout></c:layout></c:legend>
          </c:chart>
        </c:chartSpace>
      `)
    );

    expect(result.diagnostics).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ code: 'pptx.chart.format.legend', feature: 'Chart formatting' }),
        expect.objectContaining({ code: 'pptx.chart.format.layout', feature: 'Chart formatting' }),
        expect.objectContaining({ code: 'pptx.chart.format.series', feature: 'Chart formatting' }),
      ])
    );
  });

  it('imports common presentation data-label content and placement', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:barChart>
          <c:barDir val="col"/><c:ser><c:tx><c:v>Revenue</c:v></c:tx><c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt></c:numLit></c:val></c:ser>
          <c:dLbls><c:dLblPos val="outEnd"/><c:showVal val="1"/><c:showCatName val="1"/><c:showSerName val="1"/><c:separator> / </c:separator></c:dLbls>
        </c:barChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart.dataLabels).toEqual({
      showValue: true,
      showCategoryName: true,
      showSeriesName: true,
      separator: ' / ',
      position: 'outsideEnd',
    });
  });

  it('diagnoses per-point, formatting, and unsupported presentation data-label settings', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:lineChart>
          <c:ser><c:tx><c:v>Revenue</c:v></c:tx><c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt></c:numLit></c:val></c:ser>
          <c:dLbls>
            <c:dLbl><c:idx val="0"/><c:showCatName val="1"/></c:dLbl>
            <c:dLblPos val="outEnd"/><c:numFmt formatCode="0.0" sourceLinked="0"/>
            <c:showVal val="1"/><c:showLeaderLines val="1"/>
          </c:dLbls>
        </c:lineChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart.dataLabels).toEqual({ showValue: true, position: 'above' });
    expect(result.diagnostics).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ code: 'pptx.chart.data-label-overrides', feature: 'Chart data labels' }),
        expect.objectContaining({ code: 'pptx.chart.data-label-position', feature: 'Chart data labels' }),
        expect.objectContaining({ code: 'pptx.chart.data-label-content', feature: 'Chart data labels' }),
        expect.objectContaining({ code: 'pptx.chart.data-label-format', feature: 'Chart data labels' }),
      ])
    );
  });

  it('imports native scatter data, style, and numeric axis titles', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
          <c:chart><c:plotArea><c:scatterChart><c:scatterStyle val="smoothMarker"/><c:ser>
            <c:tx><c:v>Latency</c:v></c:tx>
            <c:xVal><c:numLit><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2.5</c:v></c:pt></c:numLit></c:xVal>
            <c:yVal><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numLit></c:yVal>
          </c:ser></c:scatterChart>
          <c:valAx><c:axPos val="b"/><c:title><c:tx><c:rich><a:p><a:r><a:t>Concurrency</a:t></a:r></a:p></c:rich></c:tx></c:title></c:valAx>
          <c:valAx><c:axPos val="l"/><c:title><c:tx><c:rich><a:p><a:r><a:t>Latency</a:t></a:r></a:p></c:rich></c:tx></c:title></c:valAx>
          </c:plotArea></c:chart>
        </c:chartSpace>
      `)
    );

    expect(result.chart).toMatchObject({
      type: 'scatter',
      categories: ['1', '2.5'],
      series: [{ name: 'Latency', values: [42, 58] }],
      scatterStyle: 'smoothMarker',
      axes: { bottom: { title: 'Concurrency' }, left: { title: 'Latency' } },
    });
    expect(result.diagnostics.some((diagnostic) => diagnostic.code === 'pptx.chart.type')).toBe(false);
  });

  it('imports native bubble sizes, display settings, and bubble-size labels', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:bubbleChart><c:ser>
          <c:tx><c:v>Capacity</c:v></c:tx>
          <c:xVal><c:numLit><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:xVal>
          <c:yVal><c:numLit><c:pt idx="0"><c:v>5</c:v></c:pt><c:pt idx="1"><c:v>8</c:v></c:pt></c:numLit></c:yVal>
          <c:bubbleSize><c:numLit><c:pt idx="0"><c:v>9</c:v></c:pt><c:pt idx="1"><c:v>-16</c:v></c:pt></c:numLit></c:bubbleSize><c:bubble3D val="0"/>
        </c:ser><c:dLbls><c:dLblPos val="t"/><c:showBubbleSize val="1"/></c:dLbls>
        <c:bubbleScale val="140"/><c:showNegBubbles val="1"/><c:sizeRepresents val="w"/>
        </c:bubbleChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart).toMatchObject({
      type: 'bubble',
      categories: ['1', '2'],
      series: [{ name: 'Capacity', values: [5, 8], bubbleSizes: [9, -16] }],
      bubbleScale: 140,
      showNegativeBubbles: true,
      bubbleSizeRepresents: 'width',
      dataLabels: { showBubbleSize: true, position: 'above' },
    });
    expect(result.diagnostics.some((diagnostic) => diagnostic.code === 'pptx.chart.type')).toBe(false);
  });

  it('imports native per-series trendlines and statistical or custom error bars', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:scatterChart><c:scatterStyle val="lineMarker"/><c:ser>
          <c:tx><c:v>Latency</c:v></c:tx>
          <c:trendline><c:name>Forecast</c:name><c:trendlineType val="poly"/><c:order val="3"/><c:forward val="2"/><c:dispRSq val="1"/><c:dispEq val="1"/></c:trendline>
          <c:errBars><c:errDir val="y"/><c:errBarType val="both"/><c:errValType val="percentage"/><c:noEndCap val="1"/><c:val val="10"/></c:errBars>
          <c:errBars><c:errDir val="x"/><c:errBarType val="both"/><c:errValType val="cust"/><c:plus><c:numLit><c:pt idx="0"><c:v>0.5</c:v></c:pt><c:pt idx="1"><c:v>1</c:v></c:pt></c:numLit></c:plus><c:minus><c:numLit><c:pt idx="0"><c:v>0.25</c:v></c:pt><c:pt idx="1"><c:v>0.5</c:v></c:pt></c:numLit></c:minus></c:errBars>
          <c:xVal><c:numLit><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:xVal>
          <c:yVal><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt><c:pt idx="1"><c:v>58</c:v></c:pt></c:numLit></c:yVal>
        </c:ser></c:scatterChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart.series[0]).toMatchObject({
      trendlines: [
        {
          type: 'polynomial',
          name: 'Forecast',
          order: 3,
          forward: 2,
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
        {
          direction: 'x',
          barType: 'both',
          valueType: 'custom',
          plusValues: [0.5, 1],
          minusValues: [0.25, 0.5],
        },
      ],
    });
  });

  it('diagnoses normalized trendline labels, parameters, styling, and duplicate or malformed error bars', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:lineChart><c:ser>
          <c:tx><c:v>Revenue</c:v></c:tx>
          <c:trendline><c:spPr/><c:trendlineType val="poly"/><c:order val="9"/><c:forward val="-1"/><c:trendlineLbl/></c:trendline>
          <c:errBars><c:errDir val="y"/><c:errBarType val="both"/><c:errValType val="fixedVal"/><c:val val="-2"/><c:spPr/></c:errBars>
          <c:errBars><c:errDir val="y"/><c:errBarType val="plus"/><c:errValType val="percentage"/><c:val val="10"/></c:errBars>
          <c:errBars><c:errDir val="z"/><c:errBarType val="both"/><c:errValType val="stdErr"/></c:errBars>
          <c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strLit></c:cat>
          <c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt></c:numLit></c:val>
        </c:ser></c:lineChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart.series[0]).toMatchObject({
      trendlines: [{ type: 'polynomial', order: 6 }],
      errorBars: [{ direction: 'y', barType: 'both', valueType: 'fixedValue', value: 0 }],
    });
    expect(result.diagnostics).toEqual(
      expect.arrayContaining(
        [
          'pptx.chart.trendline-label',
          'pptx.chart.trendline-format',
          'pptx.chart.trendline-parameter',
          'pptx.chart.error-bars-format',
          'pptx.chart.error-bars-value',
          'pptx.chart.error-bars-duplicate',
          'pptx.chart.error-bars-type',
        ].map((code) => expect.objectContaining({ code, feature: 'Chart series analysis' }))
      )
    );
  });

  it('reports differing per-series scatter X values before normalizing them to the shared editor axis', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea><c:scatterChart>
          <c:scatterStyle val="lineMarker"/>
          <c:ser><c:tx><c:v>Current</c:v></c:tx><c:xVal><c:numLit><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:xVal><c:yVal><c:numLit><c:pt idx="0"><c:v>5</c:v></c:pt><c:pt idx="1"><c:v>8</c:v></c:pt></c:numLit></c:yVal></c:ser>
          <c:ser><c:tx><c:v>Target</c:v></c:tx><c:xVal><c:numLit><c:pt idx="0"><c:v>1</c:v></c:pt><c:pt idx="1"><c:v>3</c:v></c:pt></c:numLit></c:xVal><c:yVal><c:numLit><c:pt idx="0"><c:v>6</c:v></c:pt><c:pt idx="1"><c:v>9</c:v></c:pt></c:numLit></c:yVal></c:ser>
        </c:scatterChart></c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.chart.categories).toEqual(['1', '2']);
    expect(result.diagnostics).toContainEqual(
      expect.objectContaining({ code: 'pptx.chart.xy-values', feature: 'XY chart data' })
    );
  });

  it('reports advanced and secondary axis settings that the primary-axis editor normalizes', () => {
    const result = readPptxChart(
      parseXml(`
        <c:chartSpace xmlns:c="${CHART_NAMESPACE}"><c:chart><c:plotArea>
          <c:barChart><c:barDir val="col"/><c:ser><c:tx><c:v>Revenue</c:v></c:tx><c:cat><c:strLit><c:pt idx="0"><c:v>Q1</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:pt idx="0"><c:v>42</c:v></c:pt></c:numLit></c:val></c:ser></c:barChart>
          <c:catAx><c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="b"/></c:catAx>
          <c:valAx><c:scaling><c:logBase val="10"/><c:orientation val="minMax"/></c:scaling><c:axPos val="l"/><c:minorUnit val="2"/><c:dispUnits><c:builtInUnit val="thousands"/></c:dispUnits></c:valAx>
          <c:valAx><c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="r"/></c:valAx>
        </c:plotArea></c:chart></c:chartSpace>
      `)
    );

    expect(result.diagnostics).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ code: 'pptx.chart.axis-secondary', feature: 'Chart axes' }),
        expect.objectContaining({ code: 'pptx.chart.axis-logarithmic', feature: 'Chart axes' }),
        expect.objectContaining({ code: 'pptx.chart.axis-minor-unit', feature: 'Chart axes' }),
        expect.objectContaining({ code: 'pptx.chart.axis-display-units', feature: 'Chart axes' }),
      ])
    );
  });
});
