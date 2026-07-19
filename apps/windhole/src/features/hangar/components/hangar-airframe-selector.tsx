import { Check, ChevronRight } from 'lucide-react';
import type { BuiltInAircraftId } from '../../../components/scene/aircraft-blueprint';
import { HANGAR_AIRFRAME_OPTIONS, type HangarAirframeRole } from '../hangar-configuration';

export interface HangarAirframeSelectorProps {
  value: BuiltInAircraftId;
  disabled?: boolean;
  onChange: (airframeId: BuiltInAircraftId) => void;
}

const ROLE_LABELS: Record<HangarAirframeRole, string> = {
  'air-dominance': '制空',
  'carrier-multirole': '舰载多用途',
  'strike-multirole': '隐身打击',
  experimental: '试验平台',
};

export function HangarAirframeSelector({ value, disabled = false, onChange }: HangarAirframeSelectorProps) {
  const selected = HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === value) ?? HANGAR_AIRFRAME_OPTIONS[0];

  return (
    <section className='hangar-airframe-selector' aria-label='机体选择'>
      <header>
        <div>
          <span>01 / AIRFRAME SELECT</span>
          <strong>选择试验机体</strong>
        </div>
        <p>{selected.description}</p>
      </header>
      <div className='hangar-airframe-rail' role='radiogroup' aria-label='机体型号'>
        {HANGAR_AIRFRAME_OPTIONS.map((option, index) => {
          const isSelected = option.id === value;
          return (
            <button
              key={option.id}
              type='button'
              className={isSelected ? 'is-selected' : ''}
              aria-pressed={isSelected}
              aria-label={`选择 ${option.displayName}，${ROLE_LABELS[option.role]}`}
              onClick={() => onChange(option.id)}
              disabled={disabled}
            >
              <span className='hangar-airframe-rail__index'>{String(index + 1).padStart(2, '0')}</span>
              <span className='hangar-airframe-rail__copy'>
                <strong>{option.displayName}</strong>
                <small>{ROLE_LABELS[option.role]}</small>
              </span>
              <code>{option.id.toUpperCase()}</code>
              {isSelected ? <Check size={14} aria-hidden='true' /> : <ChevronRight size={13} aria-hidden='true' />}
            </button>
          );
        })}
      </div>
    </section>
  );
}
