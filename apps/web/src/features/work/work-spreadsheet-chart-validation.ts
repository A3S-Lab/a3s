import type { WorkSpreadsheetChartSeries } from './work-types';

export function validateSpreadsheetChartSeriesTrendlines(
  series: WorkSpreadsheetChartSeries,
  seriesIndex: number
): string | null {
  for (const [trendlineIndex, trendline] of (series.trendlines ?? []).entries()) {
    const prefix = `系列 ${seriesIndex + 1} 的趋势线 ${trendlineIndex + 1}`;
    if (
      trendline.type === 'polynomial' &&
      (!Number.isInteger(trendline.order) || (trendline.order ?? 0) < 2 || (trendline.order ?? 0) > 6)
    ) {
      return `${prefix}的多项式阶数必须是 2 到 6 之间的整数。`;
    }
    if (
      trendline.type === 'movingAverage' &&
      (!Number.isInteger(trendline.period) || (trendline.period ?? 0) < 2 || (trendline.period ?? 0) > 255)
    ) {
      return `${prefix}的移动平均周期必须是 2 到 255 之间的整数。`;
    }
    if (trendline.forward !== undefined && (!Number.isFinite(trendline.forward) || trendline.forward < 0)) {
      return `${prefix}的前推值必须是非负数。`;
    }
    if (trendline.backward !== undefined && (!Number.isFinite(trendline.backward) || trendline.backward < 0)) {
      return `${prefix}的后推值必须是非负数。`;
    }
    if (trendline.intercept !== undefined && !Number.isFinite(trendline.intercept)) {
      return `${prefix}的固定截距必须是有效数字。`;
    }
  }
  return null;
}
