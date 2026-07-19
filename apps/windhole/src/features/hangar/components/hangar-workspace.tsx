import { Boxes, PlaneTakeoff, ShieldCheck, TriangleAlert } from 'lucide-react';
import type { ReactNode } from 'react';
import {
  candidateRunStatus,
  HANGAR_AIRFRAME_OPTIONS,
  HANGAR_PILOT_OPTIONS,
  type HangarDraft,
  type HangarRosterEntry,
  MAX_HANGAR_ROSTER_SIZE,
} from '../hangar-configuration';
import { HangarAirframeSelector } from './hangar-airframe-selector';
import { HangarConfigurator } from './hangar-configurator';
import { HangarPreview, type HangarPreviewStatus } from './hangar-preview';
import { HangarRoster } from './hangar-roster';

export interface HangarWorkspaceProps {
  draft: Readonly<HangarDraft>;
  roster: readonly Readonly<HangarRosterEntry>[];
  selectedRosterId?: string;
  preview?: ReactNode;
  previewStatus?: HangarPreviewStatus;
  disabled?: boolean;
  error?: string;
  onDraftChange: (draft: HangarDraft) => void;
  onAddToRoster: (draft: HangarDraft) => void;
  onSelectRoster: (id: string) => void;
  onRemoveFromRoster: (id: string) => void;
  onUpdateRoster?: () => void;
  onResetPreview?: () => void;
  onDeploy?: () => void;
  deployReady?: boolean;
  updateReady?: boolean;
}

export function HangarWorkspace({
  draft,
  roster,
  selectedRosterId,
  preview,
  previewStatus,
  disabled = false,
  error,
  onDraftChange,
  onAddToRoster,
  onSelectRoster,
  onRemoveFromRoster,
  onUpdateRoster,
  onResetPreview,
  onDeploy,
  deployReady = true,
  updateReady = false,
}: HangarWorkspaceProps) {
  const airframe =
    HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === draft.airframeId) ?? HANGAR_AIRFRAME_OPTIONS[0];
  const pilot = HANGAR_PILOT_OPTIONS.find((option) => option.id === draft.pilotId) ?? HANGAR_PILOT_OPTIONS[3];
  const candidateStatus = candidateRunStatus(draft.candidate, draft.model);

  return (
    <main className='workspace-page hangar-workspace'>
      <header className='workspace-page-header hangar-workspace__header'>
        <div>
          <p className='workspace-eyebrow'>AGENT HANGAR / FORMATION SETUP</p>
          <h1>智能体机库</h1>
          <p>为每架试验机绑定机体、智能体飞行员、模型与可视 Effort 挂载。</p>
        </div>
        <div className='hangar-workspace__header-actions'>
          <div className='hangar-workspace__summary'>
            <span>
              <Boxes size={14} aria-hidden='true' /> 当前编队
            </span>
            <strong>
              {roster.length} / {MAX_HANGAR_ROSTER_SIZE}
            </strong>
          </div>
          {onDeploy ? (
            <button
              className='hangar-workspace__deploy'
              type='button'
              onClick={onDeploy}
              disabled={!roster.length || !deployReady}
              title={deployReady ? '使用当前活动组合进入地图' : '请先保存对当前组合的修改'}
            >
              <PlaneTakeoff size={16} aria-hidden='true' />
              <span>
                <strong>{deployReady ? '进入作战地图' : '组合尚未保存'}</strong>
                <small>{deployReady ? '使用当前组合' : '请先更新当前组合'}</small>
              </span>
            </button>
          ) : null}
        </div>
      </header>

      <div className='hangar-workspace__layout'>
        <div className='hangar-workspace__stage'>
          <HangarPreview
            airframe={airframe}
            pilot={pilot}
            draft={draft}
            status={previewStatus}
            onResetView={onResetPreview}
          >
            {preview}
          </HangarPreview>

          <aside
            className={`hangar-workspace__note ${candidateStatus.deployable ? 'is-ready' : 'is-blocked'}`}
            role={candidateStatus.deployable ? 'status' : 'alert'}
          >
            {candidateStatus.deployable ? (
              <ShieldCheck size={15} aria-hidden='true' />
            ) : (
              <TriangleAlert size={15} aria-hidden='true' />
            )}
            <span>{candidateStatus.message}</span>
          </aside>
        </div>

        <aside className='hangar-workspace__sidebar' aria-label='机库配置面板'>
          <HangarAirframeSelector
            value={draft.airframeId}
            disabled={disabled}
            onChange={(airframeId) => onDraftChange({ ...draft, airframeId })}
          />
          <HangarConfigurator draft={draft} disabled={disabled} onChange={onDraftChange} />
          {error ? <output className='hangar-workspace__error'>{error}</output> : null}
        </aside>
      </div>

      <div className='hangar-workspace__squadron-dock'>
        <HangarRoster
          roster={roster}
          selectedId={selectedRosterId}
          disabled={disabled}
          onAdd={() => onAddToRoster({ ...draft })}
          onSelect={onSelectRoster}
          onRemove={onRemoveFromRoster}
          addReady={candidateStatus.deployable}
          addStatus={candidateStatus.message}
          onUpdate={onUpdateRoster}
          updatePending={updateReady}
          updateReady={candidateStatus.deployable && updateReady}
        />
      </div>
    </main>
  );
}
