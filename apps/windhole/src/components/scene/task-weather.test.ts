import { describe, expect, it } from 'vitest';
import { taskWeather, WEATHER_PRESETS } from './task-weather';

describe('task weather', () => {
  it.each([
    ['quick_file_edit', 'clear'],
    ['ann_vector_search_qps', 'light-rain'],
    ['bipedalwalker_locomotion_rl', 'moderate-rain'],
    ['portfolio_risk_calibration', 'heavy-rain'],
    ['rust_multicrate_reconstruction', 'hail'],
    ['warehouse_forklift_routing', 'typhoon'],
    ['college_english_exam_bank', 'thunderstorm'],
    ['wireless_electricity_layout', 'mixed'],
  ] as const)('maps %s to %s', (taskId, weatherId) => {
    expect(taskWeather(taskId)).toBe(WEATHER_PRESETS[weatherId]);
  });

  it('uses a stable fallback for newly added Tasks', () => {
    expect(taskWeather('future_agent_task')).toBe(taskWeather('future_agent_task'));
    expect(Object.values(WEATHER_PRESETS)).toContain(taskWeather('future_agent_task'));
  });

  it('keeps extreme weather readable enough to inspect the aircraft', () => {
    expect(Math.max(...Object.values(WEATHER_PRESETS).map((preset) => preset.fogDensity))).toBeLessThanOrEqual(0.055);
  });
});
