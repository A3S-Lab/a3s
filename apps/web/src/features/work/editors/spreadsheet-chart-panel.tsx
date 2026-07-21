import type { Selection } from '@fortune-sheet/core';
import { Plus, Trash2 } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { createSpreadsheetChartFromSelection, parseSpreadsheetChartReference } from '../work-spreadsheet-charts';
import { validateSpreadsheetChartSeriesTrendlines } from '../work-spreadsheet-chart-validation';
import { normalizeWorkSpreadsheetChartAxes } from '../work-spreadsheet-chart-axis';
import {
  normalizeWorkSpreadsheetChartLayout,
  workSpreadsheetChartSupportsSeriesAnalysis,
} from '../work-spreadsheet-chart-layout';
import {
  normalizeWorkSpreadsheetChartAxisGroup,
  normalizeWorkSpreadsheetCombinationSeriesType,
  normalizeWorkSpreadsheetBubbleScale,
  normalizeWorkSpreadsheetBubbleSizeRepresents,
  normalizeWorkSpreadsheetDataLabels,
  normalizeWorkSpreadsheetDoughnutHoleSize,
  normalizeWorkSpreadsheetErrorBars,
  normalizeWorkSpreadsheetRadarStyle,
  normalizeWorkSpreadsheetScatterStyle,
  normalizeWorkSpreadsheetTrendline,
  type WorkSpreadsheetChart,
  type WorkSpreadsheetChartAxisGroup,
  type WorkSpreadsheetBubbleSizeRepresents,
  type WorkSpreadsheetChartSeries,
  type WorkSpreadsheetChartType,
  type WorkSpreadsheetCombinationSeriesType,
  type WorkSpreadsheetContent,
  type WorkSpreadsheetRadarStyle,
  type WorkSpreadsheetScatterStyle,
  workSpreadsheetChartSupportsErrorBars,
  workSpreadsheetChartSupportsAxes,
  workSpreadsheetChartSupportsTrendlines,
  workSpreadsheetChartUsesNumericXAxis,
  workSpreadsheetChartTypeLabel,
} from '../work-types';
import { SpreadsheetDataLabelEditor } from './spreadsheet-data-label-editor';
import { SpreadsheetChartAxisEditor } from './spreadsheet-chart-axis-editor';
import { SpreadsheetChartLayoutEditor } from './spreadsheet-chart-layout-editor';
import { SpreadsheetErrorBarEditor } from './spreadsheet-error-bar-editor';
import { SpreadsheetTrendlineEditor } from './spreadsheet-trendline-editor';
import { SpreadsheetChartSeriesStyleEditor } from './spreadsheet-chart-series-style-editor';
import { normalizeWorkSpreadsheetChartSeriesStyle } from '../work-spreadsheet-chart-series-style';

interface SpreadsheetChartPanelProps {
  content: WorkSpreadsheetContent;
  activeSheetId: string;
  selection?: Selection;
  onChange: (content: WorkSpreadsheetContent) => void;
}

interface ChartListItem {
  sheetId: string;
  sheetName: string;
  chart: WorkSpreadsheetChart;
}

interface ChartDraft extends Omit<WorkSpreadsheetChart, 'series'> {
  sheetId: string;
  series: WorkSpreadsheetChartSeries[];
}

export function SpreadsheetChartPanel({ content, activeSheetId, selection, onChange }: SpreadsheetChartPanelProps) {
  const items = useMemo(
    () =>
      content.sheets.flatMap((sheet) =>
        (sheet.charts ?? []).flatMap((chart) => (sheet.id ? [{ sheetId: sheet.id, sheetName: sheet.name, chart }] : []))
      ),
    [content.sheets]
  );
  const [selectedKey, setSelectedKey] = useState<string | null>(() => chartKey(items[0]));
  const [draft, setDraft] = useState<ChartDraft | null>(() => (items[0] ? chartDraft(items[0]) : null));
  const [error, setError] = useState('');

  useEffect(() => {
    const current = items.find((item) => chartKey(item) === selectedKey);
    if (current) {
      setDraft(chartDraft(current));
      return;
    }
    if (selectedKey) {
      const first = items[0];
      setSelectedKey(chartKey(first));
      setDraft(first ? chartDraft(first) : null);
    }
  }, [items, selectedKey]);

  const selectChart = (item: ChartListItem) => {
    setSelectedKey(chartKey(item));
    setDraft(chartDraft(item));
    setError('');
  };
  const addChart = () => {
    const sheet =
      content.sheets.find((candidate) => candidate.id === activeSheetId) ??
      content.sheets.find((candidate) => !candidate.hide) ??
      content.sheets[0];
    if (!sheet?.id) {
      setError('当前工作簿没有可用于创建图表的工作表。');
      return;
    }
    const fallbackSelection: Selection = {
      row: [0, Math.max(0, Math.min(4, (sheet.row ?? 5) - 1))],
      column: [0, Math.max(0, Math.min(1, (sheet.column ?? 2) - 1))],
    };
    const chart = createSpreadsheetChartFromSelection(content, sheet.id, selection ?? fallbackSelection);
    if (!chart) {
      setError('请先选择一个包含图表数据的连续单元格区域。');
      return;
    }
    const next = content.sheets.map((candidate) =>
      candidate.id === sheet.id ? { ...candidate, charts: [...(candidate.charts ?? []), chart] } : candidate
    );
    onChange({ ...content, sheets: next });
    setSelectedKey(`${sheet.id}:${chart.id}`);
    setDraft({ ...chart, sheetId: sheet.id, series: chart.series.map((item) => ({ ...item })) });
    setError('');
  };
  const saveChart = () => {
    if (!draft) return;
    const ownerSheet = content.sheets.find((sheet) => sheet.id === draft.sheetId);
    if (!ownerSheet) {
      setError('找不到图表所在的工作表。');
      return;
    }
    if (!draft.name.trim()) {
      setError('请输入图表对象名称。');
      return;
    }
    if (
      !workSpreadsheetChartUsesNumericXAxis(draft.type) &&
      draft.categoryReference?.trim() &&
      !parseSpreadsheetChartReference(content, ownerSheet, draft.categoryReference)
    ) {
      setError('分类引用必须是当前工作簿中的连续 A1 单元格范围。');
      return;
    }
    if (!draft.series.length) {
      setError('图表至少需要一个数据系列。');
      return;
    }
    if (draft.type === 'pie' && draft.series.length > 1) {
      setError('基础饼图只能使用一个数据系列，请先删除其他系列。');
      return;
    }
    if (draft.type === 'combination' && draft.series.length < 2) {
      setError('组合图至少需要两个数据系列。');
      return;
    }
    if (
      draft.type === 'doughnut' &&
      (typeof draft.doughnutHoleSize !== 'number' ||
        !Number.isFinite(draft.doughnutHoleSize) ||
        (draft.doughnutHoleSize ?? 0) < 10 ||
        (draft.doughnutHoleSize ?? 0) > 90)
    ) {
      setError('圆环孔径必须在 10% 到 90% 之间。');
      return;
    }
    if (
      draft.type === 'bubble' &&
      (typeof draft.bubbleScale !== 'number' ||
        !Number.isFinite(draft.bubbleScale) ||
        draft.bubbleScale < 0 ||
        draft.bubbleScale > 300)
    ) {
      setError('气泡缩放必须在 0% 到 300% 之间。');
      return;
    }
    const supportsSeriesAnalysis = workSpreadsheetChartSupportsSeriesAnalysis(draft);
    const hasSecondaryAxes =
      draft.type === 'combination' &&
      draft.series.some((series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary');
    if (workSpreadsheetChartSupportsAxes(draft.type)) {
      const axisError = validateChartAxes(content, ownerSheet, draft.axes, hasSecondaryAxes);
      if (axisError) {
        setError(axisError);
        return;
      }
    }
    for (const [index, series] of draft.series.entries()) {
      if (series.nameReference?.trim() && !parseSpreadsheetChartReference(content, ownerSheet, series.nameReference)) {
        setError(`系列 ${index + 1} 的名称引用无效。`);
        return;
      }
      if (
        series.valuesReference?.trim() &&
        !parseSpreadsheetChartReference(content, ownerSheet, series.valuesReference)
      ) {
        setError(`系列 ${index + 1} 的数值引用无效。`);
        return;
      }
      if (!series.valuesReference?.trim() && !series.values.length) {
        setError(`系列 ${index + 1} 需要数值引用。`);
        return;
      }
      if (workSpreadsheetChartUsesNumericXAxis(draft.type)) {
        if (
          series.xValuesReference?.trim() &&
          !parseSpreadsheetChartReference(content, ownerSheet, series.xValuesReference)
        ) {
          setError(`系列 ${index + 1} 的 X 值引用无效。`);
          return;
        }
        if (!series.xValuesReference?.trim() && !series.xValues?.length) {
          setError(`系列 ${index + 1} 需要 X 值引用。`);
          return;
        }
      }
      if (draft.type === 'bubble') {
        if (
          series.bubbleSizesReference?.trim() &&
          !parseSpreadsheetChartReference(content, ownerSheet, series.bubbleSizesReference)
        ) {
          setError(`系列 ${index + 1} 的气泡大小引用无效。`);
          return;
        }
        if (!series.bubbleSizesReference?.trim() && !series.bubbleSizes?.length) {
          setError(`系列 ${index + 1} 需要气泡大小引用。`);
          return;
        }
      }
      if (supportsSeriesAnalysis && workSpreadsheetChartSupportsErrorBars(draft.type)) {
        const errorBarError = validateSeriesErrorBars(content, ownerSheet, series, index, draft.type);
        if (errorBarError) {
          setError(errorBarError);
          return;
        }
      }
      if (supportsSeriesAnalysis && workSpreadsheetChartSupportsTrendlines(draft.type)) {
        const trendlineError = validateSpreadsheetChartSeriesTrendlines(series, index);
        if (trendlineError) {
          setError(trendlineError);
          return;
        }
      }
    }
    const numericXAxis = workSpreadsheetChartUsesNumericXAxis(draft.type);
    const supportsErrorBars = supportsSeriesAnalysis && workSpreadsheetChartSupportsErrorBars(draft.type);
    const supportsTrendlines = supportsSeriesAnalysis && workSpreadsheetChartSupportsTrendlines(draft.type);
    const saved: WorkSpreadsheetChart = {
      ...draft,
      name: draft.name.trim(),
      altText: draft.altText?.trim() || undefined,
      title: draft.title?.trim() || undefined,
      titleReference: draft.titleReference?.trim().replace(/^=/, '') || undefined,
      axes: normalizeWorkSpreadsheetChartAxes(draft.axes, draft.type, hasSecondaryAxes),
      ...normalizeWorkSpreadsheetChartLayout(draft),
      categoryReference: numericXAxis ? undefined : draft.categoryReference?.trim().replace(/^=/, '') || undefined,
      doughnutHoleSize:
        draft.type === 'doughnut' ? normalizeWorkSpreadsheetDoughnutHoleSize(draft.doughnutHoleSize) : undefined,
      radarStyle: draft.type === 'radar' ? normalizeWorkSpreadsheetRadarStyle(draft.radarStyle) : undefined,
      scatterStyle: draft.type === 'scatter' ? normalizeWorkSpreadsheetScatterStyle(draft.scatterStyle) : undefined,
      bubbleScale: draft.type === 'bubble' ? normalizeWorkSpreadsheetBubbleScale(draft.bubbleScale) : undefined,
      showNegativeBubbles: draft.type === 'bubble' ? draft.showNegativeBubbles === true : undefined,
      bubbleSizeRepresents:
        draft.type === 'bubble' ? normalizeWorkSpreadsheetBubbleSizeRepresents(draft.bubbleSizeRepresents) : undefined,
      series: draft.series.map((series, index) => {
        const {
          xValues,
          xValuesReference,
          bubbleSizes,
          bubbleSizesReference,
          chartType,
          axisGroup,
          dataLabels,
          errorBars,
          trendlines,
          style,
          ...categorySeries
        } = series;
        return {
          ...categorySeries,
          name: series.name.trim() || `系列 ${index + 1}`,
          nameReference: series.nameReference?.trim().replace(/^=/, '') || undefined,
          valuesReference: series.valuesReference?.trim().replace(/^=/, '') || undefined,
          ...(numericXAxis
            ? {
                xValues,
                xValuesReference: xValuesReference?.trim().replace(/^=/, '') || undefined,
              }
            : {}),
          ...(draft.type === 'bubble'
            ? {
                bubbleSizes,
                bubbleSizesReference: bubbleSizesReference?.trim().replace(/^=/, '') || undefined,
              }
            : {}),
          ...(draft.type === 'combination'
            ? {
                chartType: normalizeWorkSpreadsheetCombinationSeriesType(chartType),
                axisGroup: normalizeWorkSpreadsheetChartAxisGroup(axisGroup),
              }
            : {}),
          ...(dataLabels ? { dataLabels: normalizeWorkSpreadsheetDataLabels(dataLabels, draft.type) } : {}),
          ...(style ? { style: normalizeWorkSpreadsheetChartSeriesStyle(style) } : {}),
          ...(supportsErrorBars && errorBars?.length
            ? { errorBars: errorBars.map((item) => normalizeWorkSpreadsheetErrorBars(item, draft.type)) }
            : {}),
          ...(supportsTrendlines && trendlines?.length
            ? { trendlines: trendlines.map(normalizeWorkSpreadsheetTrendline) }
            : {}),
        };
      }),
    };
    const sheets = content.sheets.map((sheet) =>
      sheet.id === draft.sheetId
        ? {
            ...sheet,
            charts: (sheet.charts ?? []).map((chart) => (chart.id === saved.id ? saved : chart)),
          }
        : sheet
    );
    onChange({ ...content, sheets });
    setDraft(chartDraft({ sheetId: draft.sheetId, sheetName: ownerSheet.name, chart: saved }));
    setError('');
  };
  const deleteChart = () => {
    if (!draft) return;
    const sheets = content.sheets.map((sheet) => {
      if (sheet.id !== draft.sheetId) return sheet;
      const charts = (sheet.charts ?? []).filter((chart) => chart.id !== draft.id);
      return { ...sheet, charts: charts.length ? charts : undefined };
    });
    onChange({ ...content, sheets });
    const next = items.find((item) => chartKey(item) !== `${draft.sheetId}:${draft.id}`);
    setSelectedKey(chartKey(next));
    setDraft(next ? chartDraft(next) : null);
    setError('');
  };

  return (
    <div className='work-spreadsheet-chart-manager'>
      <aside aria-label='工作簿图表'>
        <button type='button' className='create' onClick={addChart}>
          <Plus size={13} />
          根据当前选区新建
        </button>
        <div className='work-spreadsheet-chart-list'>
          {items.map((item) => (
            <button
              type='button'
              className={chartKey(item) === selectedKey ? 'active' : ''}
              key={chartKey(item)}
              onClick={() => selectChart(item)}
            >
              <strong>{item.chart.title || item.chart.name}</strong>
              <span>
                {item.sheetName} · {workSpreadsheetChartTypeLabel(item.chart.type)}
              </span>
            </button>
          ))}
          {!items.length && <p>还没有图表。先选择带标题的数据区域，再创建图表。</p>}
        </div>
      </aside>
      {draft ? (
        <form
          onSubmit={(event) => {
            event.preventDefault();
            saveChart();
          }}
        >
          <div className='work-spreadsheet-chart-fields'>
            <label>
              <span>对象名称</span>
              <input
                aria-label='图表对象名称'
                value={draft.name}
                maxLength={255}
                onChange={(event) => setDraft({ ...draft, name: event.target.value })}
              />
            </label>
            <label>
              <span>图表类型</span>
              <select
                aria-label='图表类型'
                value={draft.type}
                onChange={(event) => {
                  const type = event.target.value as WorkSpreadsheetChartType;
                  setDraft(chartDraftWithType(draft, type));
                }}
              >
                <option value='column'>簇状柱形图</option>
                <option value='bar'>簇状条形图</option>
                <option value='line'>折线图</option>
                <option value='pie'>饼图</option>
                <option value='doughnut'>圆环图</option>
                <option value='area'>面积图</option>
                <option value='radar'>雷达图</option>
                <option value='scatter'>散点图</option>
                <option value='bubble'>气泡图</option>
                <option value='combination'>组合图</option>
              </select>
            </label>
            {draft.type === 'doughnut' && (
              <label>
                <span>圆环孔径（%）</span>
                <input
                  type='number'
                  aria-label='圆环孔径（%）'
                  min={10}
                  max={90}
                  step={1}
                  value={draft.doughnutHoleSize ?? 50}
                  onChange={(event) => setDraft({ ...draft, doughnutHoleSize: Number(event.target.value) })}
                />
              </label>
            )}
            {draft.type === 'radar' && (
              <label>
                <span>雷达图样式</span>
                <select
                  aria-label='雷达图样式'
                  value={normalizeWorkSpreadsheetRadarStyle(draft.radarStyle)}
                  onChange={(event) =>
                    setDraft({ ...draft, radarStyle: event.target.value as WorkSpreadsheetRadarStyle })
                  }
                >
                  <option value='standard'>标准雷达图</option>
                  <option value='marker'>带数据标记的雷达图</option>
                  <option value='filled'>填充雷达图</option>
                </select>
              </label>
            )}
            {draft.type === 'scatter' && (
              <label>
                <span>散点图样式</span>
                <select
                  aria-label='散点图样式'
                  value={normalizeWorkSpreadsheetScatterStyle(draft.scatterStyle)}
                  onChange={(event) =>
                    setDraft({ ...draft, scatterStyle: event.target.value as WorkSpreadsheetScatterStyle })
                  }
                >
                  <option value='marker'>仅数据标记</option>
                  <option value='line'>直线</option>
                  <option value='lineMarker'>带数据标记的直线</option>
                  <option value='smooth'>平滑线</option>
                  <option value='smoothMarker'>带数据标记的平滑线</option>
                </select>
              </label>
            )}
            {draft.type === 'bubble' && (
              <>
                <label>
                  <span>气泡缩放（%）</span>
                  <input
                    type='number'
                    aria-label='气泡缩放（%）'
                    min={0}
                    max={300}
                    step={1}
                    value={draft.bubbleScale ?? 100}
                    onChange={(event) => setDraft({ ...draft, bubbleScale: Number(event.target.value) })}
                  />
                </label>
                <label>
                  <span>气泡大小表示</span>
                  <select
                    aria-label='气泡大小表示'
                    value={normalizeWorkSpreadsheetBubbleSizeRepresents(draft.bubbleSizeRepresents)}
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        bubbleSizeRepresents: event.target.value as WorkSpreadsheetBubbleSizeRepresents,
                      })
                    }
                  >
                    <option value='area'>面积</option>
                    <option value='width'>宽度</option>
                  </select>
                </label>
                <label className='check'>
                  <input
                    type='checkbox'
                    aria-label='显示负值气泡'
                    checked={draft.showNegativeBubbles === true}
                    onChange={(event) => setDraft({ ...draft, showNegativeBubbles: event.target.checked })}
                  />
                  <span>显示负值气泡</span>
                </label>
              </>
            )}
            <label>
              <span>图表标题</span>
              <input
                aria-label='图表标题'
                value={draft.title ?? ''}
                maxLength={255}
                onChange={(event) => setDraft({ ...draft, title: event.target.value })}
              />
            </label>
            <label>
              <span>标题引用（可选）</span>
              <input
                aria-label='图表标题引用'
                value={draft.titleReference ?? ''}
                placeholder="'报告'!$B$1"
                onChange={(event) => setDraft({ ...draft, titleReference: event.target.value })}
              />
            </label>
            {workSpreadsheetChartSupportsAxes(draft.type) && (
              <SpreadsheetChartAxisEditor
                axes={draft.axes}
                chartType={draft.type}
                showSecondaryAxes={
                  draft.type === 'combination' &&
                  draft.series.some(
                    (series) => normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup) === 'secondary'
                  )
                }
                onChange={(axes) => setDraft({ ...draft, axes })}
              />
            )}
            {!workSpreadsheetChartUsesNumericXAxis(draft.type) && (
              <label className='reference'>
                <span>分类引用</span>
                <input
                  aria-label='图表分类引用'
                  value={draft.categoryReference ?? ''}
                  placeholder="'报告'!$A$2:$A$8"
                  onChange={(event) => setDraft({ ...draft, categoryReference: event.target.value })}
                />
              </label>
            )}
            <label className='alternative-text'>
              <span>替代文本</span>
              <input
                aria-label='图表替代文本'
                value={draft.altText ?? ''}
                maxLength={1_024}
                placeholder='说明图表表达的关键数据或趋势'
                onChange={(event) => setDraft({ ...draft, altText: event.target.value })}
              />
            </label>
            <SpreadsheetChartLayoutEditor chart={draft} onChange={(change) => setDraft({ ...draft, ...change })} />
          </div>
          <section className='work-spreadsheet-chart-series' aria-label='图表数据系列'>
            <header>
              <strong>数据系列</strong>
              <button
                type='button'
                onClick={() =>
                  setDraft({
                    ...draft,
                    series: [...draft.series, newChartSeries(draft.type, draft.series.length)],
                  })
                }
              >
                <Plus size={12} />
                添加系列
              </button>
            </header>
            {workSpreadsheetChartUsesNumericXAxis(draft.type) && (
              <p className='xy-note'>每个系列独立使用 X 与 Y；未设置 X 引用时使用当前缓存值或稳定序号。</p>
            )}
            {draft.series.map((series, index) => (
              <div
                className={`work-spreadsheet-chart-series-row${
                  workSpreadsheetChartUsesNumericXAxis(draft.type) ? ' xy' : ''
                }${draft.type === 'bubble' ? ' bubble' : ''}${draft.type === 'combination' ? ' combination' : ''}`}
                key={`${draft.id}-series-${index}`}
              >
                <label className='series-name'>
                  <span>系列 {index + 1} 名称</span>
                  <input
                    aria-label={`系列 ${index + 1} 名称`}
                    value={series.name}
                    onChange={(event) =>
                      setDraft({ ...draft, series: replaceSeries(draft.series, index, { name: event.target.value }) })
                    }
                  />
                </label>
                <label className='name-reference'>
                  <span>名称引用</span>
                  <input
                    aria-label={`系列 ${index + 1} 名称引用`}
                    value={series.nameReference ?? ''}
                    placeholder="'报告'!$B$1"
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        series: replaceSeries(draft.series, index, { nameReference: event.target.value }),
                      })
                    }
                  />
                </label>
                {workSpreadsheetChartUsesNumericXAxis(draft.type) && (
                  <label className='x-reference'>
                    <span>X 值引用</span>
                    <input
                      aria-label={`系列 ${index + 1} X 值引用`}
                      value={series.xValuesReference ?? ''}
                      placeholder="'报告'!$A$2:$A$8"
                      onChange={(event) =>
                        setDraft({
                          ...draft,
                          series: replaceSeries(draft.series, index, { xValuesReference: event.target.value }),
                        })
                      }
                    />
                  </label>
                )}
                <label className={workSpreadsheetChartUsesNumericXAxis(draft.type) ? 'y-reference' : 'reference'}>
                  <span>{workSpreadsheetChartUsesNumericXAxis(draft.type) ? 'Y 值引用' : '数值引用'}</span>
                  <input
                    aria-label={`系列 ${index + 1} ${
                      workSpreadsheetChartUsesNumericXAxis(draft.type) ? 'Y 值引用' : '数值引用'
                    }`}
                    value={series.valuesReference ?? ''}
                    placeholder="'报告'!$B$2:$B$8"
                    onChange={(event) =>
                      setDraft({
                        ...draft,
                        series: replaceSeries(draft.series, index, { valuesReference: event.target.value }),
                      })
                    }
                  />
                </label>
                {draft.type === 'combination' && (
                  <>
                    <label className='combination-chart-type'>
                      <span>系列图表类型</span>
                      <select
                        aria-label={`系列 ${index + 1} 图表类型`}
                        value={normalizeWorkSpreadsheetCombinationSeriesType(series.chartType)}
                        onChange={(event) =>
                          setDraft({
                            ...draft,
                            series: replaceSeries(draft.series, index, {
                              chartType: event.target.value as WorkSpreadsheetCombinationSeriesType,
                            }),
                          })
                        }
                      >
                        <option value='column'>柱形图</option>
                        <option value='line'>折线图</option>
                        <option value='area'>面积图</option>
                      </select>
                    </label>
                    <label className='combination-axis-group'>
                      <span>坐标轴</span>
                      <select
                        aria-label={`系列 ${index + 1} 坐标轴`}
                        value={normalizeWorkSpreadsheetChartAxisGroup(series.axisGroup)}
                        onChange={(event) =>
                          setDraft({
                            ...draft,
                            series: replaceSeries(draft.series, index, {
                              axisGroup: event.target.value as WorkSpreadsheetChartAxisGroup,
                            }),
                          })
                        }
                      >
                        <option value='primary'>主坐标轴</option>
                        <option value='secondary'>次坐标轴</option>
                      </select>
                    </label>
                  </>
                )}
                {draft.type === 'bubble' && (
                  <label className='bubble-reference'>
                    <span>气泡大小引用</span>
                    <input
                      aria-label={`系列 ${index + 1} 气泡大小引用`}
                      value={series.bubbleSizesReference ?? ''}
                      placeholder="'报告'!$C$2:$C$8"
                      onChange={(event) =>
                        setDraft({
                          ...draft,
                          series: replaceSeries(draft.series, index, {
                            bubbleSizesReference: event.target.value,
                          }),
                        })
                      }
                    />
                  </label>
                )}
                <button
                  type='button'
                  className='remove-series'
                  aria-label={`删除系列 ${index + 1}`}
                  disabled={draft.series.length <= 1}
                  onClick={() =>
                    setDraft({ ...draft, series: draft.series.filter((_, candidate) => candidate !== index) })
                  }
                >
                  <Trash2 size={13} />
                </button>
                <SpreadsheetDataLabelEditor
                  chartType={draft.type}
                  seriesNumber={index + 1}
                  value={series.dataLabels}
                  onChange={(dataLabels) =>
                    setDraft({
                      ...draft,
                      series: replaceSeries(draft.series, index, { dataLabels }),
                    })
                  }
                />
                <SpreadsheetChartSeriesStyleEditor
                  seriesNumber={index + 1}
                  supportsMarkers={
                    draft.type === 'line' ||
                    draft.type === 'radar' ||
                    draft.type === 'scatter' ||
                    (draft.type === 'combination' &&
                      normalizeWorkSpreadsheetCombinationSeriesType(series.chartType) === 'line')
                  }
                  value={series.style}
                  onChange={(style) => setDraft({ ...draft, series: replaceSeries(draft.series, index, { style }) })}
                />
                {workSpreadsheetChartSupportsSeriesAnalysis(draft) &&
                  workSpreadsheetChartSupportsErrorBars(draft.type) && (
                    <SpreadsheetErrorBarEditor
                      chartType={draft.type}
                      seriesNumber={index + 1}
                      errorBars={series.errorBars ?? []}
                      onChange={(errorBars) =>
                        setDraft({
                          ...draft,
                          series: replaceSeries(draft.series, index, { errorBars }),
                        })
                      }
                    />
                  )}
                {workSpreadsheetChartSupportsSeriesAnalysis(draft) &&
                  workSpreadsheetChartSupportsTrendlines(draft.type) && (
                    <SpreadsheetTrendlineEditor
                      seriesNumber={index + 1}
                      trendlines={series.trendlines ?? []}
                      onChange={(trendlines) =>
                        setDraft({
                          ...draft,
                          series: replaceSeries(draft.series, index, { trendlines }),
                        })
                      }
                    />
                  )}
              </div>
            ))}
          </section>
          <div className='actions'>
            {error && <output className='error'>{error}</output>}
            <button type='button' className='danger' onClick={deleteChart}>
              <Trash2 size={13} />
              删除图表
            </button>
            <button type='submit' className='primary'>
              保存图表
            </button>
          </div>
        </form>
      ) : (
        <div className='work-spreadsheet-chart-empty'>
          <strong>从单元格选区创建原生图表</strong>
          <p>第一行会作为系列名称，第一列会作为分类标签；创建后仍可修改引用和图表类型。</p>
          {error && <output className='error'>{error}</output>}
        </div>
      )}
    </div>
  );
}

function chartDraft(item: ChartListItem): ChartDraft {
  return {
    ...item.chart,
    sheetId: item.sheetId,
    axes: item.chart.axes
      ? {
          bottom: item.chart.axes.bottom ? { ...item.chart.axes.bottom } : undefined,
          left: item.chart.axes.left ? { ...item.chart.axes.left } : undefined,
          top: item.chart.axes.top ? { ...item.chart.axes.top } : undefined,
          right: item.chart.axes.right ? { ...item.chart.axes.right } : undefined,
        }
      : undefined,
    series: item.chart.series.map((series) => ({
      ...series,
      values: [...series.values],
      xValues: series.xValues ? [...series.xValues] : undefined,
      bubbleSizes: series.bubbleSizes ? [...series.bubbleSizes] : undefined,
      dataLabels: series.dataLabels ? { ...series.dataLabels } : undefined,
      errorBars: series.errorBars?.map((errorBars) => ({
        ...errorBars,
        plusValues: errorBars.plusValues ? [...errorBars.plusValues] : undefined,
        minusValues: errorBars.minusValues ? [...errorBars.minusValues] : undefined,
      })),
      trendlines: series.trendlines?.map((trendline) => ({ ...trendline })),
      style: series.style
        ? { ...series.style, marker: series.style.marker ? { ...series.style.marker } : undefined }
        : undefined,
    })),
  };
}

function chartDraftWithType(draft: ChartDraft, type: WorkSpreadsheetChartType): ChartDraft {
  const numericXAxis = workSpreadsheetChartUsesNumericXAxis(type);
  const numericCategories = draft.categories.map(strictNumericCategory);
  const categoriesAreNumeric = numericCategories.length > 0 && numericCategories.every((value) => value !== null);
  const typedSeries = numericXAxis
    ? draft.series.map((item) => {
        if (item.xValuesReference?.trim() || item.xValues?.length) return item;
        return {
          ...item,
          xValues: item.values.map((_, index) => numericCategories[index] ?? index + 1),
          xValuesReference: categoriesAreNumeric ? draft.categoryReference : undefined,
        };
      })
    : type === 'combination'
      ? draft.series.map((item, index) => ({
          ...item,
          chartType: item.chartType
            ? normalizeWorkSpreadsheetCombinationSeriesType(item.chartType)
            : combinationType(index),
          axisGroup: item.axisGroup
            ? normalizeWorkSpreadsheetChartAxisGroup(item.axisGroup)
            : index === 0
              ? 'primary'
              : 'secondary',
        }))
      : draft.series;
  const supportsErrorBars = workSpreadsheetChartSupportsErrorBars(type);
  const series = typedSeries.map((item) => {
    if (!supportsErrorBars) return { ...item, errorBars: undefined };
    const errorBars = item.errorBars
      ?.filter((source) => numericXAxis || source.direction !== 'x')
      .map((source) => normalizeWorkSpreadsheetErrorBars(source, type));
    return { ...item, errorBars: errorBars?.length ? errorBars : undefined };
  });
  const hasSecondaryAxes =
    type === 'combination' &&
    series.some((item) => normalizeWorkSpreadsheetChartAxisGroup(item.axisGroup) === 'secondary');
  return {
    ...draft,
    type,
    series,
    axes: normalizeWorkSpreadsheetChartAxes(draft.axes, type, hasSecondaryAxes),
    ...normalizeWorkSpreadsheetChartLayout({ ...draft, type }),
    doughnutHoleSize:
      type === 'doughnut' ? normalizeWorkSpreadsheetDoughnutHoleSize(draft.doughnutHoleSize) : draft.doughnutHoleSize,
    radarStyle: type === 'radar' ? normalizeWorkSpreadsheetRadarStyle(draft.radarStyle) : draft.radarStyle,
    scatterStyle: type === 'scatter' ? normalizeWorkSpreadsheetScatterStyle(draft.scatterStyle) : draft.scatterStyle,
    bubbleScale: type === 'bubble' ? normalizeWorkSpreadsheetBubbleScale(draft.bubbleScale) : draft.bubbleScale,
    showNegativeBubbles: type === 'bubble' ? draft.showNegativeBubbles === true : draft.showNegativeBubbles,
    bubbleSizeRepresents:
      type === 'bubble'
        ? normalizeWorkSpreadsheetBubbleSizeRepresents(draft.bubbleSizeRepresents)
        : draft.bubbleSizeRepresents,
  };
}

function strictNumericCategory(value: string): number | null {
  const text = value.trim();
  if (!text) return null;
  const number = Number(text);
  return Number.isFinite(number) ? number : null;
}

function newChartSeries(type: WorkSpreadsheetChartType, index: number): WorkSpreadsheetChartSeries {
  return {
    name: `系列 ${index + 1}`,
    values: [],
    valuesReference: '',
    ...(workSpreadsheetChartUsesNumericXAxis(type) ? { xValues: [], xValuesReference: '' } : {}),
    ...(type === 'bubble' ? { bubbleSizes: [], bubbleSizesReference: '' } : {}),
    ...(type === 'combination'
      ? {
          chartType: combinationType(index),
          axisGroup: index === 0 ? 'primary' : 'secondary',
        }
      : {}),
  };
}

function combinationType(index: number): WorkSpreadsheetCombinationSeriesType {
  return index === 0 ? 'column' : 'line';
}

function chartKey(item: ChartListItem | undefined): string | null {
  return item ? `${item.sheetId}:${item.chart.id}` : null;
}

function replaceSeries(
  series: WorkSpreadsheetChartSeries[],
  index: number,
  change: Partial<WorkSpreadsheetChartSeries>
): WorkSpreadsheetChartSeries[] {
  return series.map((item, candidate) => (candidate === index ? { ...item, ...change } : item));
}

function validateSeriesErrorBars(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetContent['sheets'][number],
  series: WorkSpreadsheetChartSeries,
  seriesIndex: number,
  chartType: WorkSpreadsheetChartType
): string | null {
  const directions = new Set<string>();
  for (const [errorBarIndex, errorBars] of (series.errorBars ?? []).entries()) {
    const prefix = `系列 ${seriesIndex + 1} 的误差线 ${errorBarIndex + 1}`;
    if (!workSpreadsheetChartUsesNumericXAxis(chartType) && errorBars.direction === 'x') {
      return `${prefix}不能在当前图表类型中使用 X 方向。`;
    }
    if (directions.has(errorBars.direction)) {
      return `系列 ${seriesIndex + 1} 的误差线方向不能重复。`;
    }
    directions.add(errorBars.direction);

    if (
      (errorBars.valueType === 'fixedValue' ||
        errorBars.valueType === 'percentage' ||
        errorBars.valueType === 'standardDeviation') &&
      (typeof errorBars.value !== 'number' || !Number.isFinite(errorBars.value) || errorBars.value < 0)
    ) {
      return `${prefix}的数值必须是非负有效数字。`;
    }
    if (errorBars.valueType !== 'custom') continue;

    if (errorBars.barType !== 'minus') {
      const issue = validateCustomErrorBarSource(
        content,
        ownerSheet,
        errorBars.plusReference,
        errorBars.plusValues,
        `${prefix}的正误差`
      );
      if (issue) return issue;
    }
    if (errorBars.barType !== 'plus') {
      const issue = validateCustomErrorBarSource(
        content,
        ownerSheet,
        errorBars.minusReference,
        errorBars.minusValues,
        `${prefix}的负误差`
      );
      if (issue) return issue;
    }
  }
  return null;
}

function validateChartAxes(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetContent['sheets'][number],
  axes: WorkSpreadsheetChart['axes'],
  hasSecondaryAxes: boolean
): string | null {
  const entries = [
    ['横坐标轴', axes?.bottom],
    ['纵坐标轴', axes?.left],
    ...(hasSecondaryAxes
      ? ([
          ['次横坐标轴', axes?.top],
          ['次纵坐标轴', axes?.right],
        ] as const)
      : []),
  ] as const;
  for (const [label, axis] of entries) {
    if ((axis?.title?.length ?? 0) > 255) return `${label}标题不能超过 255 个字符。`;
    if (axis?.titleReference?.trim() && !parseSpreadsheetChartReference(content, ownerSheet, axis.titleReference)) {
      return `${label}标题引用无效。`;
    }
    if (axis?.minimum !== undefined && !Number.isFinite(axis.minimum)) return `${label}最小值无效。`;
    if (axis?.maximum !== undefined && !Number.isFinite(axis.maximum)) return `${label}最大值无效。`;
    if (axis?.minimum !== undefined && axis.maximum !== undefined && axis.minimum >= axis.maximum) {
      return `${label}最小值必须小于最大值。`;
    }
    if (axis?.majorUnit !== undefined && (!Number.isFinite(axis.majorUnit) || axis.majorUnit <= 0)) {
      return `${label}主单位必须大于 0。`;
    }
    if (
      axis?.labelInterval !== undefined &&
      (!Number.isInteger(axis.labelInterval) || axis.labelInterval < 1 || axis.labelInterval > 31_999)
    ) {
      return `${label}标签间隔必须是 1 到 31999 之间的整数。`;
    }
    if ((axis?.numberFormat?.length ?? 0) > 255) return `${label}数字格式不能超过 255 个字符。`;
  }
  return null;
}

function validateCustomErrorBarSource(
  content: WorkSpreadsheetContent,
  ownerSheet: WorkSpreadsheetContent['sheets'][number],
  reference: string | undefined,
  values: number[] | undefined,
  label: string
): string | null {
  if (reference?.trim()) {
    return parseSpreadsheetChartReference(content, ownerSheet, reference) ? null : `${label}引用无效。`;
  }
  if (!values?.length) return `${label}需要有效的单元格引用。`;
  return values.every((value) => Number.isFinite(value) && value >= 0) ? null : `${label}缓存包含无效数值。`;
}
