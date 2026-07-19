import { Check, Plus, Trash2, UsersRound } from 'lucide-react';
import {
  candidateReferenceStatus,
  HANGAR_AIRFRAME_OPTIONS,
  HANGAR_PILOT_OPTIONS,
  type HangarRosterEntry,
  MAX_HANGAR_ROSTER_SIZE,
} from '../hangar-configuration';

export interface HangarRosterProps {
  roster: readonly Readonly<HangarRosterEntry>[];
  selectedId?: string;
  disabled?: boolean;
  onAdd: () => void;
  onSelect: (id: string) => void;
  onRemove: (id: string) => void;
  addReady?: boolean;
  addStatus?: string;
  onUpdate?: () => void;
  updatePending?: boolean;
  updateReady?: boolean;
}

const CAPACITY_SLOT_IDS = ['bay-1', 'bay-2', 'bay-3', 'bay-4', 'bay-5'] as const;

export function HangarRoster({
  roster,
  selectedId,
  disabled = false,
  onAdd,
  onSelect,
  onRemove,
  addReady = true,
  addStatus,
  onUpdate,
  updateReady = false,
  updatePending = updateReady,
}: HangarRosterProps) {
  const atCapacity = roster.length >= MAX_HANGAR_ROSTER_SIZE;

  return (
    <section className='hangar-roster' aria-label='当前试验编队'>
      <header className='hangar-section-heading'>
        <div>
          <span>EVALUATION SQUADRON</span>
          <h2>当前编队</h2>
        </div>
        <meter className='sr-only' aria-label='编队容量' min={0} max={MAX_HANGAR_ROSTER_SIZE} value={roster.length}>
          {roster.length}/{MAX_HANGAR_ROSTER_SIZE}
        </meter>
        <div className='hangar-roster__capacity' aria-hidden='true'>
          {CAPACITY_SLOT_IDS.map((slotId, index) => (
            <i key={slotId} className={index < roster.length ? 'is-filled' : ''} />
          ))}
          <strong>
            {roster.length}/{MAX_HANGAR_ROSTER_SIZE}
          </strong>
        </div>
      </header>

      {roster.length === 0 ? (
        <div className='hangar-roster__empty'>
          <UsersRound size={20} aria-hidden='true' />
          <strong>编队尚未配置</strong>
          <span>从上方选择机体、飞行员和模型，然后加入编队。</span>
        </div>
      ) : (
        <ol className='hangar-roster__list'>
          {roster.map((entry, index) => {
            const airframe = HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === entry.airframeId);
            const pilot = HANGAR_PILOT_OPTIONS.find((option) => option.id === entry.pilotId);
            const candidateStatus = candidateReferenceStatus(entry.candidate);
            const selected = entry.id === selectedId;
            return (
              <li key={entry.id} className={selected ? 'is-selected' : ''}>
                <button
                  type='button'
                  className='hangar-roster__select'
                  aria-pressed={selected}
                  aria-label={`选择编队成员 ${entry.callsign}`}
                  onClick={() => onSelect(entry.id)}
                  disabled={disabled}
                >
                  <span className='hangar-roster__index'>{String(index + 1).padStart(2, '0')}</span>
                  <span className='hangar-roster__main'>
                    <span>
                      <strong>{entry.callsign || entry.id}</strong>
                      <small>{pilot?.displayName ?? entry.pilotId}</small>
                    </span>
                    <span>
                      <b>{airframe?.displayName ?? entry.airframeId}</b>
                      <code className={candidateStatus.deployable ? '' : 'is-blocked'} title={candidateStatus.message}>
                        {candidateStatus.deployable ? compactModel(entry.model) : '需配置 Adapter'}
                      </code>
                    </span>
                  </span>
                  <span className='hangar-roster__effort'>{entry.effort.toUpperCase()}</span>
                  {selected ? <Check size={14} aria-hidden='true' /> : null}
                </button>
                <button
                  type='button'
                  className='hangar-roster__remove'
                  aria-label={`从编队移除 ${entry.callsign}`}
                  onClick={() => onRemove(entry.id)}
                  disabled={disabled}
                >
                  <Trash2 size={14} aria-hidden='true' />
                </button>
              </li>
            );
          })}
        </ol>
      )}

      <div className='hangar-roster__actions'>
        {onUpdate ? (
          <button
            type='button'
            className={`hangar-roster__update ${updateReady ? 'is-primary' : ''}`}
            onClick={onUpdate}
            disabled={disabled || !updateReady}
            title={
              updatePending && !addReady
                ? addStatus
                : updateReady
                  ? '保存到当前选中的编队成员'
                  : '当前组合没有未保存修改'
            }
          >
            <Check size={15} aria-hidden='true' />
            {updateReady ? '更新当前组合' : updatePending ? '先完善当前组合' : '当前组合已同步'}
          </button>
        ) : null}
        <button
          type='button'
          className={`hangar-roster__add ${updatePending ? 'is-secondary' : ''}`}
          onClick={onAdd}
          disabled={disabled || atCapacity || !addReady}
          title={!addReady ? addStatus : undefined}
        >
          <Plus size={15} aria-hidden='true' />
          {atCapacity ? '机库已满' : addReady ? '加入新组合' : '先配置 Candidate Adapter'}
        </button>
      </div>
    </section>
  );
}

function compactModel(model: string): string {
  return model.split('/').at(-1) || 'adapter managed';
}
