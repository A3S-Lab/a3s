import {
  ChevronDown,
  CloudLightning,
  CloudRain,
  CloudSun,
  Gauge,
  Pause,
  Play,
  RotateCcw,
  ScanLine,
  Snowflake,
  Waves,
  Wind,
} from 'lucide-react';
import { useMemo } from 'react';
import { useSnapshot } from 'valtio';
import type { BenchController } from '../features/bench/use-bench-controller';
import { calculateTelemetry } from '../lib/aerodynamics';
import { labState } from '../state/lab-state';
import { taskWeather, type WeatherId } from './scene/task-weather';

interface TelemetryPanelProps {
  actions: BenchController;
}

const FLIGHT_PROFILES = [
  { id: 'stealth', label: '静默', code: 'STEALTH', mach: 0.55, angleOfAttack: 2, turbulence: 0.04 },
  { id: 'cruise', label: '巡航', code: 'CRUISE', mach: 0.82, angleOfAttack: 4, turbulence: 0.12 },
  { id: 'intercept', label: '截击', code: 'INTERCEPT', mach: 1.45, angleOfAttack: 8, turbulence: 0.22 },
  { id: 'limit', label: '极限', code: 'REDLINE', mach: 2.05, angleOfAttack: 14, turbulence: 0.45 },
] as const;

export function TelemetryPanel({ actions }: TelemetryPanelProps) {
  const state = useSnapshot(labState);
  const telemetry = useMemo(
    () => calculateTelemetry({ ...state.tunnel }),
    [state.tunnel.mach, state.tunnel.angleOfAttack, state.tunnel.airDensity, state.tunnel.turbulence]
  );
  const weather = taskWeather(state.catalog.selectedTaskId);
  const activeProfile = FLIGHT_PROFILES.find(
    (profile) =>
      Math.abs(profile.mach - state.tunnel.mach) < 0.015 &&
      Math.abs(profile.angleOfAttack - state.tunnel.angleOfAttack) < 0.1 &&
      Math.abs(profile.turbulence - state.tunnel.turbulence) < 0.015
  );
  const gaugeProgress = Math.min(100, Math.max(0, (state.tunnel.mach / 2.2) * 100));

  const applyProfile = (profile: (typeof FLIGHT_PROFILES)[number]) => {
    actions.setTunnelParameter('mach', profile.mach);
    actions.setTunnelParameter('angleOfAttack', profile.angleOfAttack);
    actions.setTunnelParameter('turbulence', profile.turbulence);
  };

  return (
    <aside className='telemetry-panel' aria-label='飞行遥测与控制台'>
      <details className='flight-console-disclosure'>
        <summary aria-label='展开飞行控制台'>
          <span className='telemetry-hud-icon' aria-hidden='true'>
            <Gauge size={15} />
          </span>
          <span className='telemetry-hud-reading'>
            <small>实时速度</small>
            <strong>M {state.tunnel.mach.toFixed(2)}</strong>
          </span>
          <span className='telemetry-hud-weather'>
            {weatherIcon(weather.id)}
            <span>{weather.labelZh}</span>
          </span>
          <span className='telemetry-hud-profile'>{activeProfile?.label ?? '手动'}</span>
          <ChevronDown size={14} aria-hidden='true' />
        </summary>

        <div className='telemetry-console-popover'>
          <div className='telemetry-console-heading'>
            <div>
              <h2>飞行控制台</h2>
              <p>调整三维战场的飞行与流场表现</p>
            </div>
            <span className={`console-state ${state.tunnel.paused ? 'is-paused' : ''}`}>
              <i /> {state.tunnel.paused ? '模拟已暂停' : '模拟中'}
            </span>
          </div>

          <div className='telemetry-scroll'>
            <section className='primary-gauge' aria-label={`当前马赫数 ${state.tunnel.mach.toFixed(2)}`}>
              <meter className='sr-only' aria-label='马赫数' min={0.15} max={2.2} value={state.tunnel.mach} />
              <div
                className='gauge-dial'
                aria-hidden='true'
                style={{ '--gauge-progress': `${gaugeProgress * 2.7}deg` } as React.CSSProperties}
              >
                <span className='gauge-arc' aria-hidden='true' />
                <span className='gauge-reticle' aria-hidden='true' />
                <div className='gauge-reading'>
                  <span>MACH</span>
                  <strong>{state.tunnel.mach.toFixed(2)}</strong>
                  <small>{telemetry.velocity.toFixed(0)} m/s</small>
                </div>
              </div>
              <div className='gauge-state'>
                <span>流场</span>
                <strong className={`flow-${telemetry.flowState}`}>{flowLabel(telemetry.flowState)}</strong>
                <small>Re {telemetry.reynolds.toExponential(2)}</small>
              </div>
            </section>

            <section className='mission-environment' aria-label={`地图天气 ${weather.labelZh}`}>
              <span className='environment-icon'>{weatherIcon(weather.id)}</span>
              <div>
                <small>地图天气</small>
                <strong>{weather.labelZh}</strong>
                <span>环境随地图同步</span>
              </div>
              <dl>
                <div>
                  <dt>侧风</dt>
                  <dd>{Math.round(weather.crosswind * 100)}%</dd>
                </div>
                <div>
                  <dt>能见度</dt>
                  <dd>{Math.max(8, Math.round((1 - weather.fogDensity * 8) * 100))}%</dd>
                </div>
              </dl>
            </section>

            <section className='flight-profile-picker' aria-label='飞行模式'>
              <div className='console-section-heading'>
                <span>
                  <Gauge size={12} aria-hidden='true' /> 飞行模式
                </span>
                <output>当前：{activeProfile?.label ?? '手动'}</output>
              </div>
              <fieldset className='flight-profile-options'>
                <legend className='sr-only'>选择飞行模式预设</legend>
                {FLIGHT_PROFILES.map((profile) => (
                  <button
                    className={profile.id === activeProfile?.id ? 'is-active' : ''}
                    onClick={() => applyProfile(profile)}
                    aria-pressed={profile.id === activeProfile?.id}
                    key={profile.id}
                  >
                    <span>{profile.label}</span>
                    <small>M {profile.mach.toFixed(2)}</small>
                  </button>
                ))}
              </fieldset>
            </section>

            <details className='telemetry-data-disclosure'>
              <summary>
                <span>完整飞行数据</span>
                <small>6 项实时遥测</small>
                <ChevronDown size={13} aria-hidden='true' />
              </summary>
              <section className='telemetry-grid' aria-label='实时飞行数据'>
                <TelemetryValue label='动压' value={compactNumber(telemetry.dynamicPressure)} unit='Pa' />
                <TelemetryValue label='攻角' value={signed(state.tunnel.angleOfAttack, 1)} unit='deg' />
                <TelemetryValue label='升力系数' value={telemetry.liftCoefficient.toFixed(3)} unit='CL' />
                <TelemetryValue label='阻力系数' value={telemetry.dragCoefficient.toFixed(3)} unit='CD' />
                <TelemetryValue label='升力' value={compactNumber(telemetry.lift)} unit='N' accent />
                <TelemetryValue label='阻力' value={compactNumber(telemetry.drag)} unit='N' />
              </section>
            </details>

            <details className='manual-flight-trim'>
              <summary>
                <span>
                  <Waves size={12} aria-hidden='true' /> 手动飞控微调
                </span>
                <small>4 项参数</small>
                <ChevronDown size={13} aria-hidden='true' />
              </summary>
              <div className='control-stack'>
                <RangeControl
                  label='来流速度'
                  value={state.tunnel.mach}
                  min={0.15}
                  max={2.2}
                  step={0.01}
                  display={`M ${state.tunnel.mach.toFixed(2)}`}
                  onChange={(value) => actions.setTunnelParameter('mach', value)}
                />
                <RangeControl
                  label='攻角'
                  value={state.tunnel.angleOfAttack}
                  min={-10}
                  max={22}
                  step={0.5}
                  display={`${signed(state.tunnel.angleOfAttack, 1)}°`}
                  onChange={(value) => actions.setTunnelParameter('angleOfAttack', value)}
                />
                <RangeControl
                  label='湍流强度'
                  value={state.tunnel.turbulence}
                  min={0}
                  max={1}
                  step={0.01}
                  display={`${Math.round(state.tunnel.turbulence * 100)}%`}
                  onChange={(value) => actions.setTunnelParameter('turbulence', value)}
                />
                <RangeControl
                  label='空气密度'
                  value={state.tunnel.airDensity}
                  min={0.6}
                  max={1.5}
                  step={0.005}
                  display={`${state.tunnel.airDensity.toFixed(3)} kg/m³`}
                  onChange={(value) => actions.setTunnelParameter('airDensity', value)}
                />
              </div>
            </details>

            <fieldset className='view-controls'>
              <legend className='sr-only'>场景控制</legend>
              <ToggleButton
                active={state.tunnel.smokeVisible}
                label='气流轨迹'
                icon={<ScanLine size={14} />}
                onClick={() => actions.setTunnelParameter('smokeVisible', !state.tunnel.smokeVisible)}
              />
              <ToggleButton
                active={state.tunnel.paused}
                label={state.tunnel.paused ? '继续模拟' : '暂停模拟'}
                icon={state.tunnel.paused ? <Play size={14} /> : <Pause size={14} />}
                onClick={() => actions.setTunnelParameter('paused', !state.tunnel.paused)}
              />
              <button
                className='icon-control'
                onClick={actions.resetTunnel}
                title='恢复默认评测参数'
                aria-label='恢复默认评测参数'
              >
                <RotateCcw size={14} />
              </button>
            </fieldset>
          </div>
        </div>
      </details>
    </aside>
  );
}

interface TelemetryValueProps {
  label: string;
  value: string;
  unit: string;
  accent?: boolean;
}

function TelemetryValue({ label, value, unit, accent }: TelemetryValueProps) {
  return (
    <div className={`telemetry-value ${accent ? 'is-accent' : ''}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{unit}</small>
    </div>
  );
}

interface RangeControlProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step: number;
  display: string;
  onChange: (value: number) => void;
}

function RangeControl({ label, value, min, max, step, display, onChange }: RangeControlProps) {
  return (
    <label className='range-control'>
      <span>{label}</span>
      <output>{display}</output>
      <input
        type='range'
        min={min}
        max={max}
        step={step}
        value={value}
        style={{ '--range-progress': `${((value - min) / (max - min)) * 100}%` } as React.CSSProperties}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

interface ToggleButtonProps {
  active: boolean;
  label: string;
  icon: React.ReactNode;
  onClick: () => void;
}

function ToggleButton({ active, label, icon, onClick }: ToggleButtonProps) {
  return (
    <button className={`view-toggle ${active ? 'is-active' : ''}`} onClick={onClick} aria-pressed={active}>
      {icon}
      <span>{label}</span>
    </button>
  );
}

function weatherIcon(weatherId: WeatherId): React.ReactNode {
  if (weatherId === 'clear') return <CloudSun size={20} aria-hidden='true' />;
  if (weatherId === 'hail') return <Snowflake size={20} aria-hidden='true' />;
  if (weatherId === 'typhoon') return <Wind size={20} aria-hidden='true' />;
  if (weatherId === 'thunderstorm' || weatherId === 'mixed') {
    return <CloudLightning size={20} aria-hidden='true' />;
  }
  return <CloudRain size={20} aria-hidden='true' />;
}

function compactNumber(value: number): string {
  return Intl.NumberFormat('zh-CN', { notation: 'compact', maximumFractionDigits: 1 }).format(value);
}

function signed(value: number, digits: number): string {
  return `${value >= 0 ? '+' : ''}${value.toFixed(digits)}`;
}

function flowLabel(state: 'laminar' | 'transitional' | 'turbulent'): string {
  if (state === 'laminar') return '层流稳定';
  if (state === 'transitional') return '临界过渡';
  return '强湍流';
}
