import { describe, expect, it } from 'vitest';
import { presentationAgentSelection } from './work-presentation-agent-context';
import { spreadsheetAgentSelection } from './work-spreadsheet-agent-context';
import { createWorkArtifact } from './work-templates';

describe('spreadsheet Copilot selection context', () => {
  it('serializes the selected range with coordinates, displayed values, and formulas', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = '预算';
    sheet.data = [
      [{ v: '项目' }, { v: '金额' }],
      [{ v: '云服务' }, { v: 120, m: '¥120', f: '=SUM(B3:B4)' }],
    ];
    sheet.charts = [
      {
        id: 'budget-chart',
        name: '预算图表',
        type: 'radar',
        radarStyle: 'marker',
        title: '项目预算',
        categories: [],
        categoryReference: "'预算'!$A$2",
        series: [
          {
            name: '金额',
            values: [],
            valuesReference: "'预算'!$B$2",
          },
        ],
        showLegend: false,
        left: 220,
        top: 0,
        width: 480,
        height: 288,
      },
    ];
    artifact.content.sheets.push({
      id: 'sheet-pivot-report',
      name: '预算透视',
      order: 1,
      row: 40,
      column: 12,
      data: [],
      config: {},
      pivotTables: [
        {
          id: 'pivot-budget',
          name: 'BudgetPivot',
          sourceSheetId: sheet.id!,
          sourceReference: 'A1:B2',
          anchor: 'A1',
          outputReference: 'A1:B3',
          rowFields: [0],
          columnFields: [],
          reportFilters: [{ fieldIndex: 1, selectedItem: 120 }],
          values: [{ fieldIndex: 1, aggregation: 'sum', caption: '金额合计' }],
          rowGrandTotals: true,
          columnGrandTotals: true,
          styleName: 'PivotStyleLight16',
          refreshOnLoad: true,
        },
      ],
    });

    const selection = spreadsheetAgentSelection(artifact.content, sheet.id!, { row: [0, 1], column: [0, 1] }, 200);

    expect(selection).toMatchObject({
      reference: 'A1:B2',
      cellCount: 4,
      clipboard: '项目\t金额\n云服务\t¥120',
      truncated: false,
    });
    expect(selection?.context).toContain('工作表：预算');
    expect(selection?.context).toContain('选区：A1:B2');
    expect(selection?.context).toContain('B2：=SUM(B3:B4)');
    expect(selection?.context).toContain('公式与计算：');
    expect(selection?.context).toContain('计算模式：自动');
    expect(selection?.context).toContain('关联图表：');
    expect(selection?.context).toContain('项目预算（雷达图；分类：云服务；金额=120）');
    expect(selection?.context).toContain('关联数据透视表：');
    expect(selection?.context).toContain('BudgetPivot（来源：预算!A1:B2；行：项目；列：无；值：金额合计（求和）');
    expect(selection?.context).toContain('筛选：金额=120');
    expect(selection?.proposalTargets).toEqual([
      { id: 'A1', label: '预算!A1', before: '项目' },
      { id: 'B1', label: '预算!B1', before: '金额' },
      { id: 'A2', label: '预算!A2', before: '云服务' },
      { id: 'B2', label: '预算!B2', before: '=SUM(B3:B4)' },
    ]);
  });

  it('bounds very large selections before they reach the Copilot draft', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];

    const selection = spreadsheetAgentSelection(artifact.content, sheet.id!, { row: [0, 999], column: [0, 25] }, 20);

    expect(selection?.cellCount).toBe(26_000);
    expect(selection?.truncated).toBe(true);
    expect(selection?.context).toContain('仅包含前 20 个单元格');
  });

  it('describes chart legend placement and editable plot layout in Copilot context', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = '布局';
    sheet.data = [
      [{ v: '季度' }, { v: '实际' }, { v: '预测' }],
      [{ v: 'Q1' }, { v: 20 }, { v: 80 }],
      [{ v: 'Q2' }, { v: -30 }, { v: -70 }],
    ];
    sheet.charts = [
      {
        id: 'layout-context',
        name: '占比趋势',
        type: 'column',
        categories: [],
        categoryReference: "'布局'!$A$2:$A$3",
        series: [
          { name: '实际', values: [], valuesReference: "'布局'!$B$2:$B$3" },
          { name: '预测', values: [], valuesReference: "'布局'!$C$2:$C$3" },
        ],
        showLegend: true,
        legendPosition: 'bottom',
        legendOverlay: true,
        grouping: 'percentStacked',
        gapWidth: 240,
        overlap: 100,
        left: 0,
        top: 0,
        width: 480,
        height: 288,
      },
    ];

    const selection = spreadsheetAgentSelection(artifact.content, sheet.id!, { row: [0, 2], column: [0, 2] });

    expect(selection?.context).toContain('图例：底部（叠加绘图区）');
    expect(selection?.context).toContain('绘图区：百分比堆积，分类间距 240%，系列重叠 100%');
  });

  it('describes editable chart series appearance in Copilot context', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = '品牌';
    sheet.data = [
      [{ v: '月份' }, { v: '收入' }],
      [{ v: '一月' }, { v: 20 }],
      [{ v: '二月' }, { v: 40 }],
    ];
    sheet.charts = [
      {
        id: 'series-style-context',
        name: '品牌收入',
        type: 'line',
        categories: [],
        categoryReference: "'品牌'!$A$2:$A$3",
        series: [
          {
            name: '收入',
            values: [],
            valuesReference: "'品牌'!$B$2:$B$3",
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
        showLegend: false,
        left: 0,
        top: 0,
        width: 480,
        height: 288,
      },
    ];

    const selection = spreadsheetAgentSelection(artifact.content, sheet.id!, { row: [0, 2], column: [0, 1] });

    expect(selection?.context).toContain(
      '系列外观：填充 #112233（透明度 35%），线条 #445566、3.25 磅、点划线，数据标记 菱形、9 磅、填充 #778899、轮廓 #AABBCC'
    );
  });

  it('describes scatter and bubble X, Y, and size series in Copilot context', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.name = '实验';
    sheet.data = [
      [{ v: 'X' }, { v: 'Y' }, { v: '大小' }],
      [{ v: 1 }, { v: 5 }, { v: 9 }],
      [{ v: 2 }, { v: 8 }, { v: 16 }],
    ];
    sheet.charts = [
      {
        id: 'scatter-context',
        name: '响应散点',
        type: 'scatter',
        scatterStyle: 'lineMarker',
        categories: [],
        series: [
          {
            name: '响应',
            xValues: [],
            xValuesReference: "'实验'!$A$2:$A$3",
            values: [],
            valuesReference: "'实验'!$B$2:$B$3",
          },
        ],
        showLegend: false,
        left: 0,
        top: 0,
        width: 480,
        height: 288,
      },
      {
        id: 'bubble-context',
        name: '响应气泡',
        type: 'bubble',
        categories: [],
        series: [
          {
            name: '容量',
            xValues: [],
            xValuesReference: "'实验'!$A$2:$A$3",
            values: [],
            valuesReference: "'实验'!$B$2:$B$3",
            bubbleSizes: [],
            bubbleSizesReference: "'实验'!$C$2:$C$3",
          },
        ],
        showLegend: false,
        left: 0,
        top: 320,
        width: 480,
        height: 288,
      },
      {
        id: 'combination-context',
        name: '综合表现',
        type: 'combination',
        axes: {
          bottom: { title: '季度' },
          left: {
            title: '收入',
            minimum: 0,
            maximum: 10,
            majorUnit: 2,
            showMajorGridlines: false,
            numberFormat: '#,##0',
            numberFormatSourceLinked: false,
            reverseOrder: true,
            labelPosition: 'high',
            majorTickMark: 'outside',
          },
          right: {
            title: '利润率',
            minimum: 0,
            maximum: 0.2,
            majorUnit: 0.05,
            showMajorGridlines: true,
            numberFormat: '0%',
            numberFormatSourceLinked: false,
          },
        },
        categories: [],
        categoryReference: "'实验'!$A$2:$A$3",
        series: [
          {
            name: '收入',
            values: [],
            valuesReference: "'实验'!$B$2:$B$3",
            chartType: 'column',
            axisGroup: 'primary',
            dataLabels: {
              showValue: true,
              showCategoryName: true,
              separator: ' / ',
              position: 'outsideEnd',
            },
            errorBars: [
              {
                direction: 'y',
                barType: 'both',
                valueType: 'percentage',
                value: 10,
              },
            ],
            trendlines: [
              {
                type: 'linear',
                name: '收入趋势',
                forward: 1,
                displayEquation: true,
              },
            ],
          },
          {
            name: '比例',
            values: [],
            valuesReference: "'实验'!$C$2:$C$3",
            chartType: 'line',
            axisGroup: 'secondary',
          },
        ],
        showLegend: true,
        left: 520,
        top: 0,
        width: 480,
        height: 288,
      },
    ];

    const selection = spreadsheetAgentSelection(artifact.content, sheet.id!, { row: [0, 2], column: [0, 2] });

    expect(selection?.context).toContain('响应散点（散点图');
    expect(selection?.context).toContain('响应：X=1、2；Y=5、8');
    expect(selection?.context).toContain('响应气泡（气泡图');
    expect(selection?.context).toContain('容量：X=1、2；Y=5、8；大小=9、16');
    expect(selection?.context).toContain('综合表现（组合图');
    expect(selection?.context).toContain('坐标轴标题：横轴“季度”，纵轴“收入”，次纵轴“利润率”');
    expect(selection?.context).toContain(
      '坐标轴设置：纵轴（范围 0–10，主单位 2，数字格式 #,##0，不显示主要网格线，逆序，标签置于高位，主要刻度线向外）；次纵轴（范围 0–0.2，主单位 0.05，数字格式 0%，显示主要网格线）'
    );
    expect(selection?.context).toContain('收入（柱形图，主坐标轴）=5、8');
    expect(selection?.context).toContain('数据标签：数值、分类名称，外侧末端，分隔符“ / ”');
    expect(selection?.context).toContain('误差线：Y 双向百分比 10%');
    expect(selection?.context).toContain('趋势线：线性“收入趋势”，前推 1，显示公式');
    expect(selection?.context).toContain('比例（折线图，次坐标轴）=9、16');
  });
});

describe('presentation Copilot selection context', () => {
  it('describes a selected element without embedding image payloads', () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    const element = slide.elements[0];
    element.text = 'A3S Work 发布计划';
    element.href = 'https://a3s.dev/work';

    const selection = presentationAgentSelection(slide, 0, artifact.content.slides.length, element);

    expect(selection).toContain(`幻灯片 1 / ${artifact.content.slides.length}`);
    expect(selection).toContain('A3S Work 发布计划');
    expect(selection).toContain('链接：https://a3s.dev/work');
    expect(selection).not.toContain('data:image');
  });

  it('summarizes a whole slide with notes and table cells', () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    slide.notes = '强调时间表与责任人。';
    slide.elements.push({
      id: 'table-1',
      type: 'table',
      x: 10,
      y: 30,
      width: 80,
      height: 40,
      text: '',
      fontSize: 14,
      color: '#000000',
      fill: '#ffffff',
      bold: false,
      align: 'left',
      table: {
        rows: [
          ['负责人', '日期'],
          ['Work', '7 月'],
        ],
      },
    });

    const selection = presentationAgentSelection(slide, 0, artifact.content.slides.length);

    expect(selection).toContain('整页内容');
    expect(selection).toContain('负责人\t日期');
    expect(selection).toContain('演讲者备注：强调时间表与责任人。');
  });

  it('includes editable presentation chart types and native settings', () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    const element = {
      id: 'chart-1',
      type: 'chart' as const,
      x: 10,
      y: 20,
      width: 80,
      height: 55,
      text: '',
      fontSize: 14,
      color: '#172033',
      fill: '#ffffff',
      bold: false,
      align: 'center' as const,
      chart: {
        type: 'doughnut' as const,
        title: '区域收入',
        categories: ['华东', '华南'],
        series: [{ name: '收入', values: [42, 58] }],
        doughnutHoleSize: 68,
      },
    };
    slide.elements.push(element);

    const selection = presentationAgentSelection(slide, 0, 1, element);

    expect(selection).toContain('图表：区域收入（圆环图）');
    expect(selection).toContain('系列 收入：42，58');
    expect(selection).toContain('圆环孔径：68%');
  });

  it('includes presentation chart legend and axis settings', () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    const element = {
      id: 'chart-axis',
      type: 'chart' as const,
      x: 10,
      y: 20,
      width: 80,
      height: 55,
      text: '',
      fontSize: 14,
      color: '#172033',
      fill: '#ffffff',
      bold: false,
      align: 'center' as const,
      chart: {
        type: 'column' as const,
        title: '季度收入',
        categories: ['Q1', 'Q2'],
        series: [
          {
            name: '收入',
            values: [42, 58],
            trendlines: [
              {
                type: 'polynomial' as const,
                name: '预测',
                order: 3,
                forward: 2,
                displayEquation: true,
                displayRSquared: true,
              },
            ],
            errorBars: [
              {
                direction: 'y' as const,
                barType: 'plus' as const,
                valueType: 'percentage' as const,
                value: 10,
                showEndCaps: false,
              },
            ],
          },
        ],
        showLegend: true,
        legendPosition: 'bottom' as const,
        axes: {
          bottom: {
            title: '季度',
            reverseOrder: true,
            labelPosition: 'high' as const,
            majorTickMark: 'outside' as const,
            labelInterval: 2,
          },
          left: {
            title: '收入（万元）',
            minimum: 0,
            maximum: 100,
            majorUnit: 20,
            numberFormat: '#,##0',
            numberFormatSourceLinked: false,
            showMajorGridlines: false,
          },
        },
        dataLabels: {
          showValue: true,
          showCategoryName: true,
          separator: ' / ',
          position: 'outsideEnd' as const,
        },
      },
    };

    const selection = presentationAgentSelection(slide, 0, 1, element);

    expect(selection).toContain('图例：显示（底部）');
    expect(selection).toContain(
      '坐标轴：横轴（标题“季度”，逆序，标签高位，主要刻度向外，标签间隔 2）；纵轴（标题“收入（万元）”，范围 0–100，主单位 20，数字格式 #,##0，不显示主要网格线）'
    );
    expect(selection).toContain(
      '系列 收入：42，58；趋势线：多项式“预测”，3 阶，前推 2，显示公式，显示 R 方；误差线：Y 正向百分比 10% 无端帽'
    );
    expect(selection).toContain('数据标签：数值、分类名称；位置：外侧末端；分隔符：“ / ”');
  });

  it('includes presentation plot layout and portable series appearance', () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    const element = {
      id: 'chart-layout-style',
      type: 'chart' as const,
      x: 10,
      y: 20,
      width: 80,
      height: 55,
      text: '',
      fontSize: 14,
      color: '#172033',
      fill: '#ffffff',
      bold: false,
      align: 'center' as const,
      chart: {
        type: 'column' as const,
        title: '收入占比',
        categories: ['Q1', 'Q2'],
        series: [
          {
            name: '收入',
            values: [42, 58],
            style: {
              fillColor: '#112233',
              fillTransparency: 35,
              lineColor: '#445566',
              lineWidth: 3.25,
              lineDash: 'dashDot' as const,
            },
          },
        ],
        showLegend: true,
        legendPosition: 'bottom' as const,
        legendOverlay: true,
        grouping: 'percentStacked' as const,
        gapWidth: 240,
        overlap: 100,
      },
    };
    slide.elements.push(element);

    const selection = presentationAgentSelection(slide, 0, 1, element);

    expect(selection).toContain('图例：显示（底部）；叠加绘图区');
    expect(selection).toContain('绘图区：百分比堆积，分类间距 240%，系列重叠 100%');
    expect(selection).toContain('系列外观：填充 #112233（透明度 35%），线条 #445566、3.25 磅、点划线');
  });

  it('includes presentation scatter and bubble data in bounded AI context', () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    const element = {
      id: 'chart-bubble',
      type: 'chart' as const,
      x: 10,
      y: 20,
      width: 80,
      height: 55,
      text: '',
      fontSize: 14,
      color: '#172033',
      fill: '#ffffff',
      bold: false,
      align: 'center' as const,
      chart: {
        type: 'bubble' as const,
        title: '响应容量',
        categories: ['1', '2'],
        series: [{ name: '容量', values: [5, 8], bubbleSizes: [9, 16] }],
        bubbleScale: 140,
        showNegativeBubbles: true,
        bubbleSizeRepresents: 'width' as const,
        dataLabels: { showBubbleSize: true, position: 'above' as const },
      },
    };

    const selection = presentationAgentSelection(slide, 0, 1, element);

    expect(selection).toContain('图表：响应容量（气泡图）');
    expect(selection).toContain('系列 容量：X=1，2；Y=5，8；大小=9，16');
    expect(selection).toContain('气泡设置：缩放 140%，显示负气泡，大小表示宽度');
    expect(selection).toContain('数据标签：气泡大小；位置：上方');
  });
});
