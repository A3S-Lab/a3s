import { Check, Cpu, Gauge, SlidersHorizontal } from 'lucide-react';
import type { PilotIdentity } from '../../../components/scene/pilot-profile';
import type { ReasoningEffort } from '../../../components/scene/weapon-loadout';
import {
  candidateReferenceStatus,
  createHangarDraft,
  HANGAR_PILOT_OPTIONS,
  type HangarDraft,
} from '../hangar-configuration';

export interface HangarConfiguratorProps {
  draft: Readonly<HangarDraft>;
  disabled?: boolean;
  onChange: (draft: HangarDraft) => void;
}

const EFFORT_OPTIONS: readonly { id: ReasoningEffort; label: string; detail: string; slots: number }[] = [
  { id: 'none', label: '无', detail: 'Clean', slots: 0 },
  { id: 'minimal', label: '极低', detail: 'Self defense', slots: 1 },
  { id: 'low', label: '低', detail: 'Light', slots: 2 },
  { id: 'medium', label: '中', detail: 'Balanced', slots: 3 },
  { id: 'high', label: '高', detail: 'Heavy', slots: 4 },
  { id: 'xhigh', label: '极高', detail: 'Full combat', slots: 5 },
];
const WEAPON_SLOT_IDS = ['slot-1', 'slot-2', 'slot-3', 'slot-4', 'slot-5'] as const;
const PILOT_PORTRAIT_CODES = {
  a3s: 'A3',
  codex: 'CX',
  claude: 'CL',
  generic: 'T0',
} as const satisfies Record<PilotIdentity, string>;

export function HangarConfigurator({ draft, disabled = false, onChange }: HangarConfiguratorProps) {
  const draftCandidateStatus = candidateReferenceStatus(draft.candidate);

  function updateDraft<Key extends keyof HangarDraft>(field: Key, value: HangarDraft[Key]): void {
    onChange({ ...draft, [field]: value });
  }

  function selectPilot(pilotId: PilotIdentity): void {
    onChange(createHangarDraft(pilotId, draft.airframeId));
  }

  return (
    <section className='hangar-configurator' aria-label='飞行员与武器配置'>
      <header className='hangar-section-heading'>
        <div>
          <span>PILOT &amp; LOADOUT</span>
          <h2>飞行员与武器等级</h2>
        </div>
        <Cpu size={17} aria-hidden='true' />
      </header>

      <fieldset className='hangar-control-group' disabled={disabled}>
        <legend>
          <span>02</span>
          选择飞行员
        </legend>
        <div className='hangar-pilot-options' role='radiogroup' aria-label='智能体飞行员'>
          {HANGAR_PILOT_OPTIONS.map((option) => {
            const selected = option.id === draft.pilotId;
            const candidateStatus = candidateReferenceStatus(option.candidate);
            return (
              <button
                key={option.id}
                type='button'
                className={selected ? 'is-selected' : ''}
                data-pilot={option.id}
                aria-pressed={selected}
                aria-label={`选择 ${option.displayName}`}
                onClick={() => selectPilot(option.id)}
              >
                <span className='hangar-pilot-options__avatar' aria-hidden='true'>
                  <i className='pilot-portrait__suit' />
                  <i className='pilot-portrait__harness' />
                  <i className='pilot-portrait__helmet' />
                  <i className='pilot-portrait__visor' />
                  <b>{PILOT_PORTRAIT_CODES[option.id]}</b>
                </span>
                <span className='hangar-pilot-options__copy'>
                  <strong>{option.displayName}</strong>
                  <small className={`is-${candidateStatus.kind}`} title={candidateStatus.message}>
                    {option.candidate || '需配置本地或 OCI Adapter'}
                  </small>
                  <span className='hangar-pilot-options__attire' aria-hidden='true'>
                    <i />
                    <i />
                    <i />
                  </span>
                </span>
                {selected ? <Check size={13} aria-hidden='true' /> : null}
              </button>
            );
          })}
        </div>
      </fieldset>

      <fieldset className='hangar-control-group' disabled={disabled}>
        <legend>
          <span>03</span>
          Effort 与可视挂载
        </legend>
        <div className='hangar-effort-options' role='radiogroup' aria-label='Effort 等级'>
          {EFFORT_OPTIONS.map((option) => {
            const selected = option.id === draft.effort;
            return (
              <button
                key={option.id}
                type='button'
                className={selected ? 'is-selected' : ''}
                aria-pressed={selected}
                aria-label={`Effort ${option.label}`}
                onClick={() => updateDraft('effort', option.id)}
              >
                <Gauge size={13} aria-hidden='true' />
                <strong>{option.label}</strong>
                <small>{option.detail}</small>
                <span className='hangar-effort-options__slots' aria-hidden='true'>
                  {WEAPON_SLOT_IDS.map((slotId, index) => (
                    <i key={slotId} className={index < option.slots ? 'is-filled' : ''} />
                  ))}
                </span>
              </button>
            );
          })}
        </div>
        <p className='hangar-selection-note'>Effort 只控制机库与三维评测场景中的可视武器挂载，不会写入 Bench 参数。</p>
      </fieldset>

      <details className='hangar-advanced-config'>
        <summary>
          <SlidersHorizontal size={14} aria-hidden='true' />
          <span>
            <strong>高级配置</strong>
            <small>Candidate · Model · Callsign</small>
          </span>
        </summary>
        <fieldset disabled={disabled}>
          <div className='hangar-field-grid'>
            <label>
              <span>模型</span>
              <input
                value={draft.model}
                onChange={(event) => updateDraft('model', event.target.value)}
                placeholder='provider/model'
              />
            </label>
            <label>
              <span>呼号</span>
              <input
                value={draft.callsign}
                onChange={(event) => updateDraft('callsign', event.target.value)}
                placeholder='TEST-01'
              />
            </label>
            <label className='hangar-field-grid__wide'>
              <span>Candidate</span>
              <input
                value={draft.candidate}
                onChange={(event) => updateDraft('candidate', event.target.value)}
                placeholder='./candidate'
              />
              <span
                className={`hangar-candidate-status is-${draftCandidateStatus.kind}`}
                role={draftCandidateStatus.deployable ? 'status' : 'alert'}
              >
                {draftCandidateStatus.message}
              </span>
            </label>
          </div>
        </fieldset>
      </details>
    </section>
  );
}
