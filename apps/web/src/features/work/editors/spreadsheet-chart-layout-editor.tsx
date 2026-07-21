import {
  normalizeWorkSpreadsheetChartGapWidth,
  normalizeWorkSpreadsheetChartGrouping,
  normalizeWorkSpreadsheetChartLegendOverlay,
  normalizeWorkSpreadsheetChartLegendPosition,
  normalizeWorkSpreadsheetChartOverlap,
  normalizeWorkSpreadsheetChartSmoothLines,
  type WorkSpreadsheetChartGrouping,
  type WorkSpreadsheetChartLayout,
  type WorkSpreadsheetChartLegendPosition,
  workSpreadsheetChartGroupingIsStacked,
  workSpreadsheetChartSupportsBarSpacing,
  workSpreadsheetChartSupportsGrouping,
  workSpreadsheetChartSupportsSmoothLines,
} from '../work-spreadsheet-chart-layout';
import type { WorkSpreadsheetChartType } from '../work-types';

interface SpreadsheetChartLayoutEditorProps {
  chart: WorkSpreadsheetChartLayout & { type: WorkSpreadsheetChartType; showLegend: boolean };
  onChange: (change: Partial<WorkSpreadsheetChartLayout & { showLegend: boolean }>) => void;
}

export function SpreadsheetChartLayoutEditor({ chart, onChange }: SpreadsheetChartLayoutEditorProps) {
  const grouping = normalizeWorkSpreadsheetChartGrouping(chart.grouping, chart.type);
  return (
    <section className='work-spreadsheet-chart-layout' aria-label='图表图例与绘图区设置'>
      <header>
        <strong>图例与绘图区</strong>
        <span>位置、堆积与系列间距</span>
      </header>
      <div>
        <label className='check'>
          <input
            type='checkbox'
            aria-label='显示图表图例'
            checked={chart.showLegend}
            onChange={(event) => onChange({ showLegend: event.target.checked })}
          />
          <span>显示图例</span>
        </label>
        {chart.showLegend && (
          <>
            <label>
              <span>图例位置</span>
              <select
                aria-label='图例位置'
                value={normalizeWorkSpreadsheetChartLegendPosition(chart.legendPosition)}
                onChange={(event) =>
                  onChange({ legendPosition: event.target.value as WorkSpreadsheetChartLegendPosition })
                }
              >
                <option value='right'>右侧</option>
                <option value='left'>左侧</option>
                <option value='top'>顶部</option>
                <option value='bottom'>底部</option>
                <option value='topRight'>右上角</option>
              </select>
            </label>
            <label className='check'>
              <input
                type='checkbox'
                aria-label='图例叠加在绘图区'
                checked={normalizeWorkSpreadsheetChartLegendOverlay(chart.legendOverlay)}
                onChange={(event) => onChange({ legendOverlay: event.target.checked })}
              />
              <span>叠加在绘图区</span>
            </label>
          </>
        )}
        {workSpreadsheetChartSupportsGrouping(chart.type) && (
          <label>
            <span>分组方式</span>
            <select
              aria-label='图表分组方式'
              value={grouping}
              onChange={(event) => {
                const nextGrouping = event.target.value as WorkSpreadsheetChartGrouping;
                if (!workSpreadsheetChartSupportsBarSpacing(chart.type)) {
                  onChange({ grouping: nextGrouping });
                  return;
                }
                const currentDefault = workSpreadsheetChartGroupingIsStacked(grouping) ? 100 : 0;
                const nextDefault = workSpreadsheetChartGroupingIsStacked(nextGrouping) ? 100 : 0;
                const currentOverlap = normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping);
                onChange({
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
                aria-label='分类间距（%）'
                min={0}
                max={500}
                step={1}
                value={normalizeWorkSpreadsheetChartGapWidth(chart.gapWidth)}
                onChange={(event) => onChange({ gapWidth: Number(event.target.value) })}
              />
            </label>
            <label>
              <span>系列重叠（%）</span>
              <input
                type='number'
                aria-label='系列重叠（%）'
                min={-100}
                max={100}
                step={1}
                value={normalizeWorkSpreadsheetChartOverlap(chart.overlap, grouping)}
                onChange={(event) => onChange({ overlap: Number(event.target.value) })}
              />
            </label>
          </>
        )}
        {workSpreadsheetChartSupportsSmoothLines(chart.type) && (
          <label className='check'>
            <input
              type='checkbox'
              aria-label='使用平滑线'
              checked={normalizeWorkSpreadsheetChartSmoothLines(chart.smoothLines)}
              onChange={(event) => onChange({ smoothLines: event.target.checked })}
            />
            <span>使用平滑线</span>
          </label>
        )}
      </div>
      {workSpreadsheetChartGroupingIsStacked(grouping) && (
        <p>堆积布局按正值和负值分别累计；保存时会移除不适用于堆积语义的趋势线与误差线。</p>
      )}
    </section>
  );
}
