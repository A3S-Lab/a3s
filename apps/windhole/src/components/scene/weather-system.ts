import * as THREE from 'three';
import type { WeatherPreset } from './task-weather';

interface WeatherValues {
  rain: number;
  hail: number;
  crosswind: number;
  fogDensity: number;
  lightning: number;
}

interface RainField {
  lines: THREE.LineSegments<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  positions: Float32Array;
  attribute: THREE.BufferAttribute;
}

interface HailField {
  points: THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial>;
  positions: Float32Array;
  attribute: THREE.BufferAttribute;
}

export interface WeatherSystem {
  group: THREE.Group;
  rain: RainField;
  hail: HailField;
  lightningBolt: THREE.Line<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  lightningLight: THREE.PointLight;
  ambient: THREE.HemisphereLight;
  fog: THREE.FogExp2;
  current: WeatherValues;
  target: WeatherValues;
  targetSkyColor: THREE.Color;
  preset?: WeatherPreset;
}

const RAIN_DROP_COUNT = 420;
const HAIL_COUNT = 180;
const bounds = Object.freeze({ minX: -9.6, maxX: 9.6, minY: -3.1, maxY: 4.3, minZ: -3.8, maxZ: 3.8 });

export function createWeatherSystem(scene: THREE.Scene): WeatherSystem {
  const group = new THREE.Group();
  group.name = 'task-weather-system';
  const random = seededRandom(0xa35e2026);
  const rain = createRainField(random);
  const hail = createHailField(random);
  const { bolt, light } = createLightning();
  group.add(rain.lines, hail.points, bolt, light);
  scene.add(group);

  const fog = scene.fog instanceof THREE.FogExp2 ? scene.fog : new THREE.FogExp2(0x071014, 0.014);
  scene.fog = fog;
  const ambient = new THREE.HemisphereLight(0x9ccbd2, 0x071014, 0.5);
  ambient.name = 'weather-ambient-light';
  scene.add(ambient);

  return {
    group,
    rain,
    hail,
    lightningBolt: bolt,
    lightningLight: light,
    ambient,
    fog,
    current: values(),
    target: values(),
    targetSkyColor: new THREE.Color(0x9ccbd2),
  };
}

export function setWeatherPreset(system: WeatherSystem, preset: WeatherPreset, immediate = false): void {
  system.preset = preset;
  system.target = values(preset);
  system.targetSkyColor.set(preset.skyColor);
  if (!immediate) return;
  system.current = values(preset);
  system.fog.density = preset.fogDensity;
  system.ambient.color.set(preset.skyColor);
  applyVisibility(system);
}

export function updateWeatherSystem(system: WeatherSystem, elapsed: number, delta: number): void {
  const blend = 1 - Math.exp(-Math.max(0, delta) * 2.8);
  for (const key of ['rain', 'hail', 'crosswind', 'fogDensity', 'lightning'] as const) {
    system.current[key] = THREE.MathUtils.lerp(system.current[key], system.target[key], blend);
  }
  system.fog.density = system.current.fogDensity;
  system.ambient.color.lerp(system.targetSkyColor, blend);
  system.ambient.intensity = 0.42 + (1 - system.current.fogDensity / 0.07) * 0.28;

  updateRain(system.rain, system.current, delta);
  updateHail(system.hail, system.current, delta);
  updateLightning(system, elapsed);
  applyVisibility(system);
}

function createRainField(random: () => number): RainField {
  const positions = new Float32Array(RAIN_DROP_COUNT * 6);
  for (let index = 0; index < RAIN_DROP_COUNT; index += 1) {
    const offset = index * 6;
    positions[offset] = lerp(bounds.minX, bounds.maxX, random());
    positions[offset + 1] = lerp(bounds.minY, bounds.maxY, random());
    positions[offset + 2] = lerp(bounds.minZ, bounds.maxZ, random());
    positions[offset + 3] = positions[offset];
    positions[offset + 4] = positions[offset + 1] + 0.26;
    positions[offset + 5] = positions[offset + 2];
  }
  const attribute = new THREE.BufferAttribute(positions, 3);
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', attribute);
  geometry.setDrawRange(0, 0);
  const material = new THREE.LineBasicMaterial({ color: 0xa8d7e2, transparent: true, opacity: 0, depthWrite: false });
  const lines = new THREE.LineSegments(geometry, material);
  lines.name = 'weather-rain';
  lines.frustumCulled = false;
  return { lines, positions, attribute };
}

function createHailField(random: () => number): HailField {
  const positions = new Float32Array(HAIL_COUNT * 3);
  for (let index = 0; index < HAIL_COUNT; index += 1) {
    const offset = index * 3;
    positions[offset] = lerp(bounds.minX, bounds.maxX, random());
    positions[offset + 1] = lerp(bounds.minY, bounds.maxY, random());
    positions[offset + 2] = lerp(bounds.minZ, bounds.maxZ, random());
  }
  const attribute = new THREE.BufferAttribute(positions, 3);
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', attribute);
  geometry.setDrawRange(0, 0);
  const material = new THREE.PointsMaterial({
    color: 0xeaf7ff,
    size: 0.085,
    transparent: true,
    opacity: 0,
    depthWrite: false,
  });
  const points = new THREE.Points(geometry, material);
  points.name = 'weather-hail';
  return { points, positions, attribute };
}

function createLightning(): {
  bolt: THREE.Line<THREE.BufferGeometry, THREE.LineBasicMaterial>;
  light: THREE.PointLight;
} {
  const points = [
    new THREE.Vector3(4.5, 4.2, -2.8),
    new THREE.Vector3(4.1, 2.9, -2.6),
    new THREE.Vector3(4.55, 1.75, -2.35),
    new THREE.Vector3(3.95, 0.55, -2.05),
    new THREE.Vector3(4.3, -0.8, -1.8),
  ];
  const material = new THREE.LineBasicMaterial({ color: 0xd9f4ff, transparent: true, opacity: 0 });
  const bolt = new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), material);
  bolt.name = 'weather-lightning-bolt';
  bolt.visible = false;
  const light = new THREE.PointLight(0xd8f5ff, 0, 22, 1.35);
  light.name = 'weather-lightning-flash';
  light.position.set(4.1, 2.2, -1.8);
  return { bolt, light };
}

function updateRain(field: RainField, weather: WeatherValues, delta: number): void {
  const activeDrops = Math.floor(RAIN_DROP_COUNT * THREE.MathUtils.clamp(weather.rain, 0, 1));
  field.lines.geometry.setDrawRange(0, activeDrops * 2);
  field.lines.material.opacity = 0.12 + weather.rain * 0.58;
  const fallSpeed = 4.5 + weather.rain * 10;
  const slant = weather.crosswind * 0.34;
  for (let index = 0; index < activeDrops; index += 1) {
    const offset = index * 6;
    let x = field.positions[offset] + delta * weather.crosswind * 3.4;
    let y = field.positions[offset + 1] - delta * fallSpeed;
    if (y < bounds.minY) y = bounds.maxY;
    if (x > bounds.maxX) x = bounds.minX;
    field.positions[offset] = x;
    field.positions[offset + 1] = y;
    field.positions[offset + 3] = x - slant;
    field.positions[offset + 4] = y + 0.24 + weather.rain * 0.34;
  }
  field.attribute.needsUpdate = true;
}

function updateHail(field: HailField, weather: WeatherValues, delta: number): void {
  const activeHail = Math.floor(HAIL_COUNT * THREE.MathUtils.clamp(weather.hail, 0, 1));
  field.points.geometry.setDrawRange(0, activeHail);
  field.points.material.opacity = 0.22 + weather.hail * 0.72;
  for (let index = 0; index < activeHail; index += 1) {
    const offset = index * 3;
    let x = field.positions[offset] + delta * weather.crosswind * 2.2;
    let y = field.positions[offset + 1] - delta * (3.3 + weather.hail * 5.2);
    if (y < bounds.minY) y = bounds.maxY;
    if (x > bounds.maxX) x = bounds.minX;
    field.positions[offset] = x;
    field.positions[offset + 1] = y;
  }
  field.attribute.needsUpdate = true;
}

function updateLightning(system: WeatherSystem, elapsed: number): void {
  const cycle = (elapsed * 0.23 + 0.91) % 1;
  const flash = cycle > 0.965 ? Math.sin(((cycle - 0.965) / 0.035) * Math.PI) : 0;
  const intensity = flash * system.current.lightning;
  system.lightningBolt.visible = intensity > 0.04;
  system.lightningBolt.material.opacity = intensity;
  system.lightningLight.intensity = intensity * 110;
}

function applyVisibility(system: WeatherSystem): void {
  system.rain.lines.visible = system.current.rain > 0.01;
  system.hail.points.visible = system.current.hail > 0.01;
  if (system.current.lightning <= 0.01) {
    system.lightningBolt.visible = false;
    system.lightningLight.intensity = 0;
  }
}

function values(preset?: WeatherPreset): WeatherValues {
  return {
    rain: preset?.rain ?? 0,
    hail: preset?.hail ?? 0,
    crosswind: preset?.crosswind ?? 0,
    fogDensity: preset?.fogDensity ?? 0.014,
    lightning: preset?.lightning ?? 0,
  };
}

function seededRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
  };
}

function lerp(start: number, end: number, amount: number): number {
  return start + (end - start) * amount;
}
