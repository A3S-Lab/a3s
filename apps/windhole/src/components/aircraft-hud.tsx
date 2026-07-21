import type { CSSProperties } from 'react';

export type AircraftHudPlacement = 'above' | 'below' | 'left' | 'right';
export type AircraftHudStatusTone = 'standby' | 'ready' | 'running' | 'success' | 'warning' | 'error';
export type AircraftHudTelemetryTone = 'default' | 'accent' | 'positive' | 'warning' | 'negative';

export interface AircraftHudAnchor {
  x: number;
  y: number;
}

export interface AircraftHudAircraft {
  model: string;
  airframe?: string;
  manufacturer?: string;
}

export interface AircraftHudAgent {
  name: string;
  pilot?: string;
  livery?: string;
}

export interface AircraftHudEffort {
  label: string;
  detail?: string;
}

export interface AircraftHudTask {
  label: string;
  id?: string;
}

export interface AircraftHudStatus {
  label: string;
  tone?: AircraftHudStatusTone;
  detail?: string;
}

export interface AircraftHudTelemetryItem {
  label: string;
  value: string | number;
  unit?: string;
  tone?: AircraftHudTelemetryTone;
}

export interface AircraftHudProps {
  open: boolean;
  aircraft: AircraftHudAircraft;
  agent: AircraftHudAgent;
  status: AircraftHudStatus;
  anchor?: AircraftHudAnchor;
  placement?: AircraftHudPlacement;
  accentColor?: string;
  effort?: AircraftHudEffort;
  loadout?: readonly string[];
  task?: AircraftHudTask;
  telemetry?: readonly AircraftHudTelemetryItem[];
  id?: string;
  ariaLabel?: string;
  className?: string;
  style?: CSSProperties;
}

type AircraftHudStyle = CSSProperties & {
  '--aircraft-hud-accent'?: string;
};

export function AircraftHud({
  open,
  aircraft,
  agent,
  status,
  anchor,
  placement = 'right',
  accentColor,
  effort,
  loadout = [],
  task,
  telemetry = [],
  id,
  ariaLabel,
  className,
  style,
}: AircraftHudProps) {
  if (!open) return null;

  const tone = status.tone ?? 'standby';
  const hudStyle: AircraftHudStyle = {
    left: anchor?.x,
    top: anchor?.y,
    '--aircraft-hud-accent': accentColor,
    ...style,
  };
  const classes = ['aircraft-hud', className].filter(Boolean).join(' ');
  const pilotLine = [agent.pilot, agent.livery].filter(Boolean).join(' · ');
  const hasMissionConfiguration = Boolean(effort || loadout.length);

  return (
    <aside
      id={id}
      className={classes}
      style={hudStyle}
      role='tooltip'
      aria-label={ariaLabel ?? `${agent.name} ${aircraft.model} 飞行信息`}
      data-placement={placement}
      data-status-tone={tone}
    >
      <div className='aircraft-hud__connector' aria-hidden='true' />
      <div className='aircraft-hud__surface'>
        <span className='aircraft-hud__corner aircraft-hud__corner--top' aria-hidden='true' />
        <span className='aircraft-hud__corner aircraft-hud__corner--bottom' aria-hidden='true' />

        <header className='aircraft-hud__header'>
          <div className='aircraft-hud__eyebrow'>
            <span>TRACKED AIRFRAME</span>
            <span className='aircraft-hud__status'>
              <i aria-hidden='true' />
              {status.label}
            </span>
          </div>
          <div className='aircraft-hud__model'>
            <div>
              <h3>{aircraft.model}</h3>
              {aircraft.manufacturer ? <p>{aircraft.manufacturer}</p> : null}
            </div>
            {aircraft.airframe ? <code>{aircraft.airframe}</code> : null}
          </div>
          {status.detail ? <p className='aircraft-hud__status-detail'>{status.detail}</p> : null}
        </header>

        <section className='aircraft-hud__agent' aria-label='智能体与涂装'>
          <span>AGENT / PILOT</span>
          <strong>{agent.name}</strong>
          {pilotLine ? <small>{pilotLine}</small> : null}
        </section>

        {hasMissionConfiguration ? (
          <section className='aircraft-hud__configuration' aria-label='推理强度与挂载'>
            {effort ? (
              <div className='aircraft-hud__effort'>
                <span>EFFORT</span>
                <strong>{effort.label}</strong>
                {effort.detail ? <small>{effort.detail}</small> : null}
              </div>
            ) : null}
            {loadout.length ? (
              <div className='aircraft-hud__loadout'>
                <span>LOADOUT</span>
                <ul>
                  {loadout.map((item) => (
                    <li key={item}>{item}</li>
                  ))}
                </ul>
              </div>
            ) : null}
          </section>
        ) : null}

        {task ? (
          <section className='aircraft-hud__task' aria-label='当前任务'>
            <span>TASK</span>
            <div>
              <strong>{task.label}</strong>
              {task.id ? <code>{task.id}</code> : null}
            </div>
          </section>
        ) : null}

        {telemetry.length ? (
          <dl className='aircraft-hud__telemetry' aria-label='实时遥测'>
            {telemetry.map((item) => (
              <div key={`${item.label}:${item.unit ?? ''}`} data-tone={item.tone ?? 'default'}>
                <dt>{item.label}</dt>
                <dd>
                  <strong>{item.value}</strong>
                  {item.unit ? <span>{item.unit}</span> : null}
                </dd>
              </div>
            ))}
          </dl>
        ) : null}
      </div>
    </aside>
  );
}
