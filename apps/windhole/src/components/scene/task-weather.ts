export type WeatherId =
  | 'clear'
  | 'light-rain'
  | 'moderate-rain'
  | 'heavy-rain'
  | 'hail'
  | 'typhoon'
  | 'thunderstorm'
  | 'mixed';

export interface WeatherPreset {
  id: WeatherId;
  label: string;
  labelZh: string;
  rain: number;
  hail: number;
  crosswind: number;
  fogDensity: number;
  lightning: number;
  skyColor: number;
}

export const WEATHER_PRESETS: Readonly<Record<WeatherId, WeatherPreset>> = Object.freeze({
  clear: preset('clear', 'Clear', '晴天', 0, 0, 0.04, 0.014, 0, 0x9ccbd2),
  'light-rain': preset('light-rain', 'Light rain', '小雨', 0.24, 0, 0.12, 0.024, 0, 0x6b8d99),
  'moderate-rain': preset('moderate-rain', 'Moderate rain', '中雨', 0.48, 0, 0.2, 0.032, 0, 0x526d7a),
  'heavy-rain': preset('heavy-rain', 'Heavy rain', '暴雨', 0.82, 0, 0.34, 0.047, 0, 0x3a5260),
  hail: preset('hail', 'Hail', '冰雹', 0.12, 0.78, 0.24, 0.039, 0, 0x9babb4),
  typhoon: preset('typhoon', 'Typhoon', '台风', 1, 0.08, 1, 0.054, 0, 0x263e4a),
  thunderstorm: preset('thunderstorm', 'Thunderstorm', '雷电', 0.68, 0, 0.38, 0.048, 1, 0x35495d),
  mixed: preset('mixed', 'Compound storm', '组合天气', 0.92, 0.55, 0.7, 0.055, 0.86, 0x263746),
});

const TASK_WEATHER: Readonly<Record<string, WeatherId>> = Object.freeze({
  quick_file_edit: 'clear',
  ann_vector_search_qps: 'light-rain',
  bipedalwalker_locomotion_rl: 'moderate-rain',
  portfolio_risk_calibration: 'heavy-rain',
  rust_multicrate_reconstruction: 'hail',
  warehouse_forklift_routing: 'typhoon',
  college_english_exam_bank: 'thunderstorm',
  wireless_electricity_layout: 'mixed',
});

const WEATHER_SEQUENCE = Object.keys(WEATHER_PRESETS) as WeatherId[];

export function taskWeather(taskId: string): WeatherPreset {
  const known = TASK_WEATHER[taskId];
  if (known) return WEATHER_PRESETS[known];
  return WEATHER_PRESETS[WEATHER_SEQUENCE[stableHash(taskId) % WEATHER_SEQUENCE.length]];
}

function preset(
  id: WeatherId,
  label: string,
  labelZh: string,
  rain: number,
  hail: number,
  crosswind: number,
  fogDensity: number,
  lightning: number,
  skyColor: number
): WeatherPreset {
  return Object.freeze({ id, label, labelZh, rain, hail, crosswind, fogDensity, lightning, skyColor });
}

function stableHash(value: string): number {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}
