import { LockKeyhole, Plane, UsersRound } from 'lucide-react';
import type { BenchDeploymentScope } from '../../../types/bench';

export type DeploymentScope = BenchDeploymentScope;

export interface DeploymentScopePickerProps {
  scope: DeploymentScope;
  rosterSize: number;
  locked: boolean;
  disabled?: boolean;
  onChange: (scope: DeploymentScope) => void;
}

export function DeploymentScopePicker({
  scope,
  rosterSize,
  locked,
  disabled = false,
  onChange,
}: DeploymentScopePickerProps) {
  const normalizedRosterSize = Math.max(0, Math.trunc(rosterSize));
  const campaignDisabled = disabled || locked || normalizedRosterSize === 0;
  const campaignUnavailableReason = locked
    ? '锁文件模式仅支持单机出击。'
    : normalizedRosterSize === 0
      ? '请先在机库组建编队。'
      : undefined;

  return (
    <fieldset className='deployment-scope-picker'>
      <legend className='sr-only'>选择部署战术</legend>
      <button
        type='button'
        className='deployment-scope-picker__option'
        aria-pressed={scope === 'single'}
        disabled={disabled}
        onClick={() => onChange('single')}
      >
        <span className='deployment-scope-picker__icon'>
          <Plane size={14} aria-hidden='true' />
        </span>
        <span>
          <strong>单机先锋</strong>
          <small>当前出击组合</small>
        </span>
      </button>

      <button
        type='button'
        className='deployment-scope-picker__option'
        aria-pressed={scope === 'campaign'}
        aria-describedby={campaignUnavailableReason ? 'campaign-scope-unavailable' : undefined}
        disabled={campaignDisabled}
        title={campaignUnavailableReason}
        onClick={() => onChange('campaign')}
      >
        <span className='deployment-scope-picker__icon'>
          {locked ? <LockKeyhole size={14} aria-hidden='true' /> : <UsersRound size={14} aria-hidden='true' />}
        </span>
        <span>
          <strong>全编队 · {normalizedRosterSize}</strong>
          <small>{locked ? '锁文件模式不可用' : '按机库顺序出击'}</small>
        </span>
      </button>

      {campaignUnavailableReason ? (
        <span className='sr-only' id='campaign-scope-unavailable'>
          {campaignUnavailableReason}
        </span>
      ) : null}
    </fieldset>
  );
}
