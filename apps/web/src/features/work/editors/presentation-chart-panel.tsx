import { Plus, Trash2, X } from 'lucide-react';
import {
  createPresentationChartSeries,
  normalizeDoughnutHoleSize,
  normalizePresentationBubbleScale,
  normalizePresentationBubbleSizeRepresents,
  normalizePresentationScatterStyle,
  parsePresentationChartCategories,
  parsePresentationChartValues,
  parsePresentationChartXValues,
  presentationChartSupportsSeriesMarkers,
  presentationChartSupportsAxisTitles,
  presentationChartTypeLabel,
  presentationChartUsesNumericXAxis,
  withPresentationChartDataLabels,
  withPresentationChartSeriesStyle,
  withPresentationChartType,
} from '../work-presentation-charts';
import type {
  WorkSlideBubbleSizeRepresents,
  WorkSlideChart,
  WorkSlideChartType,
  WorkSlideRadarStyle,
  WorkSlideScatterStyle,
} from '../work-types';
import { PresentationChartAxisEditor } from './presentation-chart-axis-editor';
import { PresentationChartDataLabelEditor } from './presentation-chart-data-label-editor';
import { PresentationChartLayoutEditor } from './presentation-chart-layout-editor';
import { PresentationChartSeriesAnalysisEditor } from './presentation-chart-series-analysis-editor';
import { SpreadsheetChartSeriesStyleEditor } from './spreadsheet-chart-series-style-editor';

const CHART_TYPES: WorkSlideChartType[] = [
  'column',
  'bar',
  'line',
  'area',
  'pie',
  'doughnut',
  'radar',
  'scatter',
  'bubble',
];

export function PresentationChartPanel({
  chart,
  onChange,
  onDelete,
  onClose,
}: {
  chart: WorkSlideChart;
  onChange: (chart: WorkSlideChart) => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const numericXAxis = presentationChartUsesNumericXAxis(chart.type);
  const updateSeries = (index: number, patch: Partial<WorkSlideChart['series'][number]>) => {
    onChange({
      ...chart,
      series: chart.series.map((series, current) => (current === index ? { ...series, ...patch } : series)),
    });
  };
  return (
    <section className='work-presentation-chart-panel' aria-label='演示图表数据'>
      <header>
        <div>
          <strong>图表数据</strong>
          <span>编辑后的数据会同步到预览和原生 PPTX。</span>
        </div>
        <div>
          <button type='button' className='danger' aria-label='删除演示图表' onClick={onDelete}>
            <Trash2 size={13} />
            删除图表
          </button>
          <button type='button' className='close' aria-label='关闭演示图表数据' onClick={onClose}>
            <X size={14} />
          </button>
        </div>
      </header>
      <div className='work-presentation-chart-controls'>
        <label>
          <span>类型</span>
          <select
            aria-label='演示图表类型'
            value={chart.type}
            onChange={(event) => onChange(withPresentationChartType(chart, event.target.value as WorkSlideChartType))}
          >
            {CHART_TYPES.map((type) => (
              <option value={type} key={type}>
                {presentationChartTypeLabel(type)}
              </option>
            ))}
          </select>
        </label>
        <label>
          <span>标题</span>
          <input
            aria-label='演示图表标题'
            value={chart.title ?? ''}
            onChange={(event) => onChange({ ...chart, title: event.target.value || undefined })}
          />
        </label>
        <PresentationChartLayoutEditor chart={chart} onChange={onChange} />
        {presentationChartSupportsAxisTitles(chart) && (
          <PresentationChartAxisEditor chart={chart} onChange={onChange} />
        )}
        <PresentationChartDataLabelEditor
          chartType={chart.type}
          value={chart.dataLabels}
          onChange={(dataLabels) => onChange(withPresentationChartDataLabels(chart, dataLabels))}
        />
        {chart.type === 'doughnut' && (
          <label>
            <span>孔径</span>
            <input
              type='number'
              aria-label='圆环孔径'
              min={10}
              max={90}
              value={normalizeDoughnutHoleSize(chart.doughnutHoleSize)}
              onChange={(event) =>
                onChange({ ...chart, doughnutHoleSize: normalizeDoughnutHoleSize(Number(event.target.value)) })
              }
            />
          </label>
        )}
        {chart.type === 'radar' && (
          <label>
            <span>样式</span>
            <select
              aria-label='雷达图样式'
              value={chart.radarStyle ?? 'standard'}
              onChange={(event) => onChange({ ...chart, radarStyle: event.target.value as WorkSlideRadarStyle })}
            >
              <option value='standard'>标准</option>
              <option value='marker'>带数据标记</option>
              <option value='filled'>填充</option>
            </select>
          </label>
        )}
        {chart.type === 'scatter' && (
          <label>
            <span>散点样式</span>
            <select
              aria-label='演示散点图样式'
              value={normalizePresentationScatterStyle(chart.scatterStyle)}
              onChange={(event) => onChange({ ...chart, scatterStyle: event.target.value as WorkSlideScatterStyle })}
            >
              <option value='marker'>仅数据标记</option>
              <option value='line'>直线</option>
              <option value='lineMarker'>直线和数据标记</option>
              <option value='smooth'>平滑线</option>
              <option value='smoothMarker'>平滑线和数据标记</option>
            </select>
          </label>
        )}
        {chart.type === 'bubble' && (
          <>
            <label>
              <span>气泡缩放</span>
              <input
                type='number'
                aria-label='演示气泡图缩放'
                min={5}
                max={300}
                value={normalizePresentationBubbleScale(chart.bubbleScale)}
                onChange={(event) =>
                  onChange({ ...chart, bubbleScale: normalizePresentationBubbleScale(event.target.value) })
                }
              />
            </label>
            <label>
              <span>大小表示</span>
              <select
                aria-label='演示气泡大小表示'
                value={normalizePresentationBubbleSizeRepresents(chart.bubbleSizeRepresents)}
                onChange={(event) =>
                  onChange({ ...chart, bubbleSizeRepresents: event.target.value as WorkSlideBubbleSizeRepresents })
                }
              >
                <option value='area'>面积</option>
                <option value='width'>宽度</option>
              </select>
            </label>
            <label className='check'>
              <span>负气泡</span>
              <span className='work-presentation-chart-check-control'>
                <input
                  type='checkbox'
                  aria-label='显示负气泡'
                  checked={chart.showNegativeBubbles === true}
                  onChange={(event) => onChange({ ...chart, showNegativeBubbles: event.target.checked })}
                />
                显示
              </span>
            </label>
          </>
        )}
        <label className='categories'>
          <span>{numericXAxis ? 'X 值' : '分类'}（每行一项）</span>
          <textarea
            aria-label={numericXAxis ? '演示图表 X 值' : '演示图表分类'}
            value={chart.categories.join('\n')}
            onChange={(event) =>
              onChange({
                ...chart,
                categories: numericXAxis
                  ? parsePresentationChartXValues(event.target.value)
                  : parsePresentationChartCategories(event.target.value),
              })
            }
          />
        </label>
        <div className='work-presentation-chart-series-list'>
          {chart.series.map((series, index) => (
            <div className='work-presentation-chart-series-card' key={index}>
              <fieldset>
                <legend>系列 {index + 1}</legend>
                <input
                  aria-label={`演示图表系列 ${index + 1} 名称`}
                  value={series.name}
                  onChange={(event) => updateSeries(index, { name: event.target.value.slice(0, 255) })}
                />
                <textarea
                  aria-label={`演示图表系列 ${index + 1} ${numericXAxis ? 'Y 值' : '数据'}`}
                  value={series.values.join(', ')}
                  onChange={(event) =>
                    updateSeries(index, { values: parsePresentationChartValues(event.target.value) })
                  }
                />
                {chart.type === 'bubble' && (
                  <textarea
                    aria-label={`演示气泡图系列 ${index + 1} 大小`}
                    value={series.bubbleSizes?.join(', ') ?? ''}
                    onChange={(event) =>
                      updateSeries(index, { bubbleSizes: parsePresentationChartValues(event.target.value) })
                    }
                  />
                )}
                <button
                  type='button'
                  aria-label={`删除演示图表系列 ${index + 1}`}
                  disabled={chart.series.length === 1}
                  onClick={() => onChange({ ...chart, series: chart.series.filter((_, current) => current !== index) })}
                >
                  <Trash2 size={12} />
                </button>
              </fieldset>
              <SpreadsheetChartSeriesStyleEditor
                seriesNumber={index + 1}
                supportsMarkers={presentationChartSupportsSeriesMarkers(chart.type)}
                value={series.style}
                onChange={(style) => onChange(withPresentationChartSeriesStyle(chart, index, style))}
              />
              <PresentationChartSeriesAnalysisEditor chart={chart} seriesIndex={index} onChange={onChange} />
            </div>
          ))}
          <button
            type='button'
            className='add-series'
            aria-label='添加图表系列'
            onClick={() => onChange({ ...chart, series: [...chart.series, createPresentationChartSeries(chart)] })}
          >
            <Plus size={13} />
            添加系列
          </button>
        </div>
      </div>
    </section>
  );
}
