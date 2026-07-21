import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  type WorkSpreadsheetChartGrouping,
  workSpreadsheetChartGroupingIsStacked,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSmoothLines,
} from '../work-spreadsheet-chart-layout';
import {
  normalizePresentationChartLegendPosition,
  presentationChartShowsLegend,
  withPresentationChartLayout,
} from '../work-presentation-charts';
import type { WorkSlideChart, WorkSlideChartLegendPosition } from '../work-types';

export function PresentationChartLayoutEditor({
  chart,
  onChange,
}: {
  chart: WorkSlideChart;
  onChange: (chart: WorkSlideChart) => void;
}) {
  const showLegend = presentationChartShowsLegend(chart);
  const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
  const change = (patch: Parameters<typeof withPresentationChartLayout>[1]) =>
    onChange(withPresentationChartLayout(chart, patch));
  return (
    <section className='work-presentation-chart-layout' aria-label='演示图表图例与绘图区设置'>
      <header>
        <strong>图例与绘图区</strong>
        <span>位置、堆积、间距与线条布局</span>
      </header>
      <div>
        <label className='check'>
          <span>图例</span>
          <span className='work-presentation-chart-check-control'>
            <input
              type='checkbox'
              aria-label='显示演示图表图例'
              checked={showLegend}
              onChange={(event) => change({ showLegend: event.target.checked })}
            />
            显示
          </span>
        </label>
        {showLegend && (
          <>
            <label>
              <span>图例位置</span>
              <select
                aria-label='演示图表图例位置'
                value={normalizePresentationChartLegendPosition(chart.legendPosition)}
                onChange={(event) => change({ legendPosition: event.target.value as WorkSlideChartLegendPosition })}
              >
                <option value='right'>右侧</option>
                <option value='left'>左侧</option>
                <option value='top'>顶部</option>
                <option value='bottom'>底部</option>
                <option value='topRight'>右上角</option>
              </select>
            </label>
            <label className='check'>
              <span>图例布局</span>
              <span className='work-presentation-chart-check-control'>
                <input
                  type='checkbox'
                  aria-label='演示图表图例叠加在绘图区'
                  checked={normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay)}
                  onChange={(event) => change({ legendOverlay: event.target.checked })}
                />
                叠加绘图区
              </span>
            </label>
          </>
        )}
        {workSpreadsheetChartSupportsGrouping(chart.type) && (
          <label>
            <span>分组方式</span>
            <select
              aria-label='演示图表分组方式'
              value={grouping}
              onChange={(event) => {
                const nextGrouping = event.target.value as WorkSpreadsheetChartGrouping;
                if (!workSpreadsheetChartSupportsBarSpacing(chart.type)) {
                  change({ grouping: nextGrouping });
                  return;
                }
                const currentDefault = workSpreadsheetChartGroupingIsStacked(grouping) ? 100 : 0;
                const nextDefault = workSpreadsheetChartGroupingIsStacked(nextGrouping) ? 100 : 0;
                const currentOverlap = normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping);
                change({
                  grouping: nextGrouping,
                  overlap: currentOverlap === currentDefault ? nextDefault : currentOverlap,
                });
              }}
            >
              {workSpreadsheetChartSupportsBarSpacing(chart.type) && <option value='clustered'>簇状</option>}
              <option value='standard'>标准</option>
              <option value='stacked'>堆积</option>
              <option value='percentStacked'>百分比堆积</option>
            </select>
          </label>
        )}
        {workSpreadsheetChartSupportsBarSpacing(chart.type) && (
          <>
            <label>
              <span>分类间距（%）</span>
              <input
                type='number'
                aria-label='演示图表分类间距（%）'
                min={0}
                max={500}
                step={1}
                value={normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)}
                onChange={(event) => change({ gapWidth: Number(event.target.value) })}
              />
            </label>
            <label>
              <span>系列重叠（%）</span>
              <input
                type='number'
                aria-label='演示图表系列重叠（%）'
                min={-100}
                max={100}
                step={1}
                value={normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping)}
                onChange={(event) => change({ overlap: Number(event.target.value) })}
              />
            </label>
          </>
        )}
        {workSpreadsheetChartSupportsSmoothLines(chart.type) && (
          <label className='check'>
            <span>折线</span>
            <span className='work-presentation-chart-check-control'>
              <input
                type='checkbox'
                aria-label='演示图表使用平滑线'
                checked={normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines)}
                onChange={(event) => change({ smoothLines: event.target.checked })}
              />
              使用平滑线
            </span>
          </label>
        )}
      </div>
      {workSpreadsheetChartGroupingIsStacked(grouping) && (
        <p>正值与负值会分别累计；切换到堆积布局时会移除不适用的趋势线和误差线。</p>
      )}
    </section>
  );
}
