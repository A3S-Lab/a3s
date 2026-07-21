import {
  defaultWorkSpreadsheetChartSeriesStyle,
  normalizeWorkSpreadsheetChartSeriesStyle,
} from '../work-spreadsheet-chart-series-style';
import type {
  WorkSpreadsheetChartLineDash,
  WorkSpreadsheetChartMarkerStyle,
  WorkSpreadsheetChartMarkerSymbol,
  WorkSpreadsheetChartSeriesStyle,
} from '../work-types';

interface SpreadsheetChartSeriesStyleEditorProps {
  seriesNumber: number;
  supportsMarkers: boolean;
  value?: WorkSpreadsheetChartSeriesStyle;
  onChange: (style: WorkSpreadsheetChartSeriesStyle | undefined) => void;
}

export function SpreadsheetChartSeriesStyleEditor({
  seriesNumber,
  supportsMarkers,
  value,
  onChange,
}: SpreadsheetChartSeriesStyleEditorProps) {
  const defaults = seriesStyleDefaults(seriesNumber - 1, supportsMarkers);
  const style = normalizeWorkSpreadsheetChartSeriesStyle(value) ?? defaults;
  const marker = style.marker ?? defaultWorkSpreadsheetChartSeriesStyle(seriesNumber - 1).marker!;
  const update = (change: Partial<WorkSpreadsheetChartSeriesStyle>) =>
    onChange(normalizeWorkSpreadsheetChartSeriesStyle({ ...style, ...change }));
  const updateMarker = (change: Partial<WorkSpreadsheetChartMarkerStyle>) =>
    update({ marker: { ...marker, ...change } });
  return (
    <section className='work-spreadsheet-chart-series-style' aria-label={`系列 ${seriesNumber} 外观设置`}>
      <label className='check enable-series-style'>
        <input
          type='checkbox'
          aria-label={`系列 ${seriesNumber} 使用自定义外观`}
          checked={Boolean(value)}
          onChange={(event) => onChange(event.target.checked ? defaults : undefined)}
        />
        <span>自定义系列外观</span>
      </label>
      {value && (
        <div>
          <label>
            <span>填充颜色</span>
            <input
              type='color'
              aria-label={`系列 ${seriesNumber} 填充颜色`}
              value={style.fillColor ?? '#4F6BED'}
              onChange={(event) => update({ fillColor: event.target.value })}
            />
          </label>
          <label>
            <span>填充透明度（%）</span>
            <input
              type='number'
              aria-label={`系列 ${seriesNumber} 填充透明度`}
              min={0}
              max={100}
              step={1}
              value={style.fillTransparency ?? 0}
              onChange={(event) => update({ fillTransparency: Number(event.target.value) })}
            />
          </label>
          <label>
            <span>线条颜色</span>
            <input
              type='color'
              aria-label={`系列 ${seriesNumber} 线条颜色`}
              value={style.lineColor ?? '#4F6BED'}
              onChange={(event) => update({ lineColor: event.target.value })}
            />
          </label>
          <label>
            <span>线条宽度（磅）</span>
            <input
              type='number'
              aria-label={`系列 ${seriesNumber} 线条宽度`}
              min={0.25}
              max={20}
              step={0.25}
              value={style.lineWidth ?? 2.25}
              onChange={(event) => update({ lineWidth: Number(event.target.value) })}
            />
          </label>
          <label>
            <span>线条虚线</span>
            <select
              aria-label={`系列 ${seriesNumber} 线条虚线`}
              value={style.lineDash ?? 'solid'}
              onChange={(event) => update({ lineDash: event.target.value as WorkSpreadsheetChartLineDash })}
            >
              <option value='solid'>实线</option>
              <option value='dash'>虚线</option>
              <option value='dot'>点线</option>
              <option value='dashDot'>点划线</option>
            </select>
          </label>
          {supportsMarkers && (
            <>
              <label>
                <span>数据标记符号</span>
                <select
                  aria-label={`系列 ${seriesNumber} 数据标记符号`}
                  value={marker.symbol ?? 'circle'}
                  onChange={(event) => updateMarker({ symbol: event.target.value as WorkSpreadsheetChartMarkerSymbol })}
                >
                  <option value='none'>无</option>
                  <option value='circle'>圆形</option>
                  <option value='square'>方形</option>
                  <option value='diamond'>菱形</option>
                  <option value='triangle'>三角形</option>
                  <option value='plus'>加号</option>
                  <option value='x'>叉号</option>
                  <option value='star'>星形</option>
                </select>
              </label>
              <label>
                <span>数据标记大小（磅）</span>
                <input
                  type='number'
                  aria-label={`系列 ${seriesNumber} 数据标记大小`}
                  min={2}
                  max={72}
                  step={1}
                  value={marker.size ?? 5}
                  onChange={(event) => updateMarker({ size: Number(event.target.value) })}
                />
              </label>
              <label>
                <span>数据标记填充</span>
                <input
                  type='color'
                  aria-label={`系列 ${seriesNumber} 数据标记填充颜色`}
                  value={marker.fillColor ?? '#FFFFFF'}
                  onChange={(event) => updateMarker({ fillColor: event.target.value })}
                />
              </label>
              <label>
                <span>数据标记轮廓</span>
                <input
                  type='color'
                  aria-label={`系列 ${seriesNumber} 数据标记轮廓颜色`}
                  value={marker.lineColor ?? style.lineColor ?? '#4F6BED'}
                  onChange={(event) => updateMarker({ lineColor: event.target.value })}
                />
              </label>
            </>
          )}
        </div>
      )}
    </section>
  );
}

function seriesStyleDefaults(seriesIndex: number, supportsMarkers: boolean): WorkSpreadsheetChartSeriesStyle {
  const style = defaultWorkSpreadsheetChartSeriesStyle(seriesIndex);
  return supportsMarkers ? style : { ...style, marker: undefined };
}
