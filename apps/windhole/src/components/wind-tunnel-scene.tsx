import {
  AlertTriangle,
  CloudLightning,
  CloudRain,
  CloudSun,
  Crosshair,
  Hand,
  RotateCcw,
  Snowflake,
  Wind,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import * as THREE from 'three';
import { useSnapshot } from 'valtio';
import { candidateRunStatus } from '../features/hangar/hangar-configuration';
import {
  campaignSnapshotMatchesConfiguration,
  labState,
  type SingleRunTrackingStatus,
  singleRunSnapshotMatchesConfiguration,
} from '../state/lab-state';
import type { BenchRunStage, WindTunnelParameters } from '../types/bench';
import { AircraftHud, type AircraftHudStatus } from './aircraft-hud';
import type { AircraftHoverEvent } from './scene/aircraft-interaction';
import { campaignAircraftHudStatus } from './scene/campaign-aircraft-hud';
import { buildRosterFormation, type FormationAircraft, selectedFormationId } from './scene/flight-formation';
import { taskWeather, type WeatherId } from './scene/task-weather';
import { createWindTunnelRuntime, type WindTunnelRuntime } from './scene/wind-tunnel-runtime';

interface WindTunnelSceneProps {
  onActivateRosterEntry?: (entryId: string) => void;
}

export function WindTunnelScene({ onActivateRosterEntry }: WindTunnelSceneProps = {}) {
  const mountRef = useRef<HTMLDivElement>(null);
  const runtimeRef = useRef<WindTunnelRuntime | undefined>(undefined);
  const activateRosterEntryRef = useRef(onActivateRosterEntry);
  activateRosterEntryRef.current = onActivateRosterEntry;
  const state = useSnapshot(labState);
  const [renderError, setRenderError] = useState<string>();
  const [aircraftHover, setAircraftHover] = useState<AircraftHoverEvent>();
  const parametersRef = useRef<WindTunnelParameters>({ ...state.tunnel });
  parametersRef.current = { ...state.tunnel };

  const activeRosterEntry = state.hangar.roster.find((entry) => entry.id === state.hangar.activeEntryId);
  const formation = useMemo(
    () => buildRosterFormation(state.hangar.roster.map((entry) => ({ ...entry }))),
    [state.hangar.roster]
  );
  const configuredAircraftId = selectedFormationId(
    formation,
    activeRosterEntry?.candidate ?? '',
    state.hangar.activeEntryId
  );
  const [selectedAircraftId, setSelectedAircraftId] = useState(configuredAircraftId);
  const selectedAircraft = formation.find((aircraft) => aircraft.instanceId === selectedAircraftId) ?? formation[0];
  const hoveredAircraft = formation.find((aircraft) => aircraft.instanceId === aircraftHover?.id);
  const task = state.catalog.tasks.find((item) => item.id === state.catalog.selectedTaskId);
  const weather = taskWeather(state.catalog.selectedTaskId);
  const formationKey = formation
    .map(
      (aircraft) =>
        `${aircraft.instanceId}:${aircraft.candidate}:${aircraft.candidateLabel}:${aircraft.model}:${aircraft.effort}:${aircraft.configuration.airframe.airframe.id}:${aircraft.configuration.loadout.id}:${aircraft.pilot.id}`
    )
    .join('|');
  const rosterEntryIdsRef = useRef<ReadonlySet<string>>(new Set());
  rosterEntryIdsRef.current = new Set(state.hangar.roster.map((entry) => entry.id));
  const campaignMatchesScene = campaignSnapshotMatchesConfiguration(
    state.campaign.snapshot,
    state.catalog.selectedTaskId,
    state.hangar.roster
  );
  const hoveredCampaignMember =
    state.campaign.status === 'idle' || !campaignMatchesScene || !hoveredAircraft
      ? undefined
      : state.campaign.members.find((member) => member.rosterEntryId === hoveredAircraft.instanceId);
  const hoveredRosterEntry = state.hangar.roster.find((entry) => entry.id === hoveredAircraft?.instanceId);
  const hoveredSingleRun = singleRunSnapshotMatchesConfiguration(
    state.run.sortie,
    state.catalog.selectedTaskId,
    hoveredRosterEntry
  )
    ? state.run
    : undefined;

  const handleAircraftSelect = (entryId: string): void => {
    setSelectedAircraftId(entryId);
    if (labState.campaign.status !== 'running' && rosterEntryIdsRef.current.has(entryId)) {
      activateRosterEntryRef.current?.(entryId);
    }
  };

  useEffect(() => {
    const container = mountRef.current;
    if (!container) return;
    try {
      runtimeRef.current = createWindTunnelRuntime(container, {
        formation,
        selectedId: configuredAircraftId,
        taskId: state.catalog.selectedTaskId,
        taskCategory: task?.category,
        getParameters: () => parametersRef.current,
        onHover: setAircraftHover,
        onSelect: handleAircraftSelect,
      });
      setRenderError(undefined);
    } catch (error) {
      setRenderError(error instanceof Error ? error.message : 'WebGL 初始化失败');
    }

    return () => {
      runtimeRef.current?.dispose();
      runtimeRef.current = undefined;
    };
  }, []);

  useEffect(() => {
    setSelectedAircraftId(configuredAircraftId);
    runtimeRef.current?.syncFormation(formation, configuredAircraftId);
  }, [configuredAircraftId, formationKey]);

  useEffect(() => {
    runtimeRef.current?.setTask(state.catalog.selectedTaskId, task?.category);
  }, [state.catalog.selectedTaskId, task?.category]);

  return (
    <section className='tunnel-panel panel-frame' aria-label='A3S智能体评测三维可视化'>
      <div className='tunnel-toolbar'>
        <span className='tunnel-title'>
          <Crosshair size={14} aria-hidden='true' />
          A3S智能体评测
        </span>
        <span className='tunnel-axis'>{String(formation.length).padStart(2, '0')} AIRCRAFT</span>
        <span className={`flow-health ${state.tunnel.turbulence > 0.55 ? 'is-warning' : ''}`}>
          <i /> {state.tunnel.turbulence > 0.55 ? 'FLOW UNSTEADY' : 'FLOW STABLE'}
        </span>
      </div>

      <div className='tunnel-viewport'>
        <div className='three-mount' ref={mountRef} />
        {renderError ? (
          <div className='webgl-error'>
            <AlertTriangle size={24} aria-hidden='true' />
            <strong>无法启动 WebGL 试验区</strong>
            <span>{renderError}</span>
          </div>
        ) : null}

        <div className='viewport-reticle reticle-left' aria-hidden='true' />
        <div className='viewport-reticle reticle-right' aria-hidden='true' />
        <div className='view-corner view-corner-tl' aria-hidden='true' />
        <div className='view-corner view-corner-tr' aria-hidden='true' />
        <div className='view-corner view-corner-bl' aria-hidden='true' />
        <div className='view-corner view-corner-br' aria-hidden='true' />

        {selectedAircraft ? (
          <div
            className='specimen-label'
            style={{ '--aircraft-accent': colorStyle(selectedAircraft.profile.accentColor) } as React.CSSProperties}
          >
            <span>{`SELECTED SPECIMEN // ${selectedAircraft.profile.agentLabel.toUpperCase()}`}</span>
            <strong>{selectedAircraft.configuration.airframe.airframe.displayName}</strong>
            <small>{selectedAircraft.model || 'MODEL MANAGED BY ADAPTER'}</small>
            <div className='specimen-controls'>
              <Hand size={12} aria-hidden='true' />
              <span>点击选择 · 拖动旋转 · 数字键切换</span>
              <button
                onClick={() => runtimeRef.current?.resetAircraft(selectedAircraftId)}
                aria-label='复位当前飞机观察角度'
              >
                <RotateCcw size={12} aria-hidden='true' />
              </button>
            </div>
          </div>
        ) : null}

        {hoveredAircraft && aircraftHover ? (
          <AircraftHud
            open
            anchor={{ x: aircraftHover.x, y: aircraftHover.y }}
            placement={aircraftHover.placement}
            accentColor={colorStyle(hoveredAircraft.profile.accentColor)}
            aircraft={{
              model: hoveredAircraft.configuration.airframe.airframe.displayName,
              airframe: hoveredAircraft.model || 'ADAPTER MANAGED',
              manufacturer: hoveredAircraft.configuration.airframe.airframe.manufacturer,
            }}
            agent={{
              name: hoveredAircraft.profile.agentLabel,
              pilot: hoveredAircraft.pilot.displayName,
              livery: hoveredAircraft.pilot.marking.label,
            }}
            effort={{
              label: hoveredAircraft.effort.toUpperCase(),
              detail: hoveredAircraft.configuration.loadout.displayName,
            }}
            loadout={loadoutLabels(hoveredAircraft.configuration.loadout)}
            task={task ? { id: task.id, label: task.name } : undefined}
            status={
              hoveredCampaignMember
                ? campaignAircraftHudStatus(hoveredCampaignMember)
                : hoveredSingleRun
                  ? singleRunHudStatus(hoveredSingleRun, aircraftHover.hitPart)
                  : candidateHudStatus(hoveredAircraft, aircraftHover.hitPart)
            }
            telemetry={[
              { label: 'MACH', value: state.tunnel.mach.toFixed(2), tone: 'accent' },
              { label: 'AOA', value: signed(state.tunnel.angleOfAttack), unit: 'deg' },
              { label: 'DENSITY', value: state.tunnel.airDensity.toFixed(3), unit: 'kg/m³' },
            ]}
          />
        ) : null}

        <div className='weather-badge' title={weather.label}>
          {weatherIcon(weather.id)}
          <span>{`WEATHER / ${weather.labelZh}`}</span>
        </div>

        <div className='viewport-scale' aria-hidden='true'>
          <span>0</span>
          <i />
          <span>5 m</span>
        </div>
      </div>
    </section>
  );
}

function colorStyle(color: THREE.ColorRepresentation): string {
  return `#${new THREE.Color(color).getHexString()}`;
}

function loadoutLabels(loadout: FormationAircraft['configuration']['loadout']): string[] {
  if (loadout.stores.length === 0) return ['Clean configuration'];
  return loadout.stores.map((store) => {
    const kind = store.kind === 'short-range-aam' ? 'Short-range AAM' : 'Medium-range AAM';
    return `${store.quantity}× ${kind} · ${store.placement}`;
  });
}

function candidateHudStatus(aircraft: FormationAircraft, hitPart: string): AircraftHudStatus {
  const hoverDetail = `Hover target · ${hitPart}`;
  const readiness = candidateRunStatus(aircraft.candidate, aircraft.model);
  if (!readiness.deployable) {
    return { label: 'NEEDS CONFIG', tone: 'warning', detail: `${readiness.message} · ${hoverDetail}` };
  }

  return { label: 'READY', tone: 'ready', detail: hoverDetail };
}

interface SingleRunHudRecord {
  readonly stage: BenchRunStage;
  readonly trackingStatus?: SingleRunTrackingStatus;
  readonly result?: { readonly score?: string };
  readonly error?: string;
}

function singleRunHudStatus(run: SingleRunHudRecord, hitPart: string): AircraftHudStatus {
  if (run.trackingStatus === 'tracking_stopped') {
    return {
      label: 'TRACKING STOPPED',
      tone: 'warning',
      detail: '前端跟踪已停止，Bench Job 可能仍在运行',
    };
  }
  if (run.stage === 'completed') {
    const score = run.result?.score?.trim();
    return {
      label: 'COMPLETE',
      tone: 'success',
      detail: score ? `SCORE ${score}` : 'Bench 已完成，但战报未返回评分',
    };
  }
  if (run.stage === 'failed') {
    return { label: 'FAILED', tone: 'error', detail: run.error?.trim() || 'Bench 未返回失败详情' };
  }
  return {
    label: 'RUNNING',
    tone: 'running',
    detail: `${run.stage.replaceAll('_', ' ').toUpperCase()} · ${hitPart}`,
  };
}

function signed(value: number): string {
  return `${value >= 0 ? '+' : ''}${value.toFixed(1)}`;
}

function weatherIcon(weatherId: WeatherId): React.ReactNode {
  if (weatherId === 'clear') return <CloudSun size={13} aria-hidden='true' />;
  if (weatherId === 'hail') return <Snowflake size={13} aria-hidden='true' />;
  if (weatherId === 'typhoon') return <Wind size={13} aria-hidden='true' />;
  if (weatherId === 'thunderstorm' || weatherId === 'mixed') {
    return <CloudLightning size={13} aria-hidden='true' />;
  }
  return <CloudRain size={13} aria-hidden='true' />;
}
