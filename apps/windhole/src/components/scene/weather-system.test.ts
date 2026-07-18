import * as THREE from 'three';
import { describe, expect, it } from 'vitest';
import { WEATHER_PRESETS } from './task-weather';
import { createWeatherSystem, setWeatherPreset, updateWeatherSystem } from './weather-system';

describe('weather system', () => {
  it('keeps clear weather free of precipitation', () => {
    const system = createWeatherSystem(new THREE.Scene());
    setWeatherPreset(system, WEATHER_PRESETS.clear, true);
    updateWeatherSystem(system, 1, 0.016);

    expect(system.rain.lines.visible).toBe(false);
    expect(system.hail.points.visible).toBe(false);
    expect(system.fog.density).toBeCloseTo(WEATHER_PRESETS.clear.fogDensity);
  });

  it('distinguishes rain, hail, typhoon, and lightning targets', () => {
    const system = createWeatherSystem(new THREE.Scene());

    setWeatherPreset(system, WEATHER_PRESETS.hail, true);
    updateWeatherSystem(system, 1, 0.016);
    expect(system.hail.points.visible).toBe(true);
    expect(system.hail.points.geometry.drawRange.count).toBeGreaterThan(0);

    setWeatherPreset(system, WEATHER_PRESETS.typhoon, true);
    expect(system.current.crosswind).toBe(1);
    expect(system.rain.lines.visible).toBe(true);

    setWeatherPreset(system, WEATHER_PRESETS.thunderstorm, true);
    expect(system.current.lightning).toBe(1);
    expect(system.preset?.id).toBe('thunderstorm');
    updateWeatherSystem(system, 0.25, 0.016);
    expect(system.lightningBolt.visible).toBe(true);
    expect(system.lightningLight.intensity).toBeGreaterThan(0);
  });

  it('smoothly approaches a new task preset', () => {
    const system = createWeatherSystem(new THREE.Scene());
    setWeatherPreset(system, WEATHER_PRESETS.clear, true);
    setWeatherPreset(system, WEATHER_PRESETS['heavy-rain']);
    updateWeatherSystem(system, 2, 0.1);

    expect(system.current.rain).toBeGreaterThan(0);
    expect(system.current.rain).toBeLessThan(WEATHER_PRESETS['heavy-rain'].rain);
  });
});
