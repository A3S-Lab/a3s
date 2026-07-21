import { Cloud, LogOut } from 'lucide-react';
import { useState } from 'react';
import { useSnapshot } from 'valtio';
import { Button, StatusBadge } from '../../../../design-system/primitives';
import { appState } from '../../../../state/app-state';
import type { SettingsActions } from '../../settings-actions';
import { SettingsSection } from '../config/settings-section';

export function A3sOsAccount({ actions }: { actions: SettingsActions }) {
  const state = useSnapshot(appState);
  const [accountBusy, setAccountBusy] = useState(false);
  const signedIn = Boolean(state.osAccount?.signedIn);

  const runAccountAction = async () => {
    if (accountBusy) return;
    setAccountBusy(true);
    try {
      if (signedIn) await actions.logout();
      else await actions.loginWithOs();
    } catch {
      // The controller keeps the current account state and reports the error.
    } finally {
      setAccountBusy(false);
    }
  };

  return (
    <SettingsSection title='A3S OS' description='连接远程 A3S 能力；OAuth 凭据只由本机 CLI 持有，不写入浏览器。'>
      <div className='a3s-os-account-summary'>
        <img src='/logo.png' alt='' />
        <div className='account-provider-copy'>
          <strong>{state.osAccount?.label || state.osAccount?.origin || '未配置 A3S OS'}</strong>
          <span>
            {signedIn
              ? state.osAccount?.runtimeToolActive
                ? '授权有效，Runtime 工具可用'
                : '授权有效，Runtime 工具尚未启用'
              : state.osAccount?.configured
                ? '服务地址已配置，等待授权'
                : '请先在本机配置 A3S OS 地址'}
          </span>
        </div>
        <StatusBadge tone={signedIn ? 'success' : 'neutral'}>{signedIn ? '已连接' : '未连接'}</StatusBadge>
      </div>

      <div className='account-capability-grid'>
        <Capability label='内置技能' active={Boolean(state.osAccount?.builtinSkillActive)} />
        <Capability label='OS 能力技能' active={Boolean(state.osAccount?.capabilitySkillActive)} />
        <Capability label='Runtime 工具' active={Boolean(state.osAccount?.runtimeToolActive)} />
      </div>

      <div className='account-section-actions'>
        <span>
          {!state.osAccount?.configured && state.health?.configPath
            ? `配置文件：${state.health.configPath}`
            : '授权状态由本机 A3S Code 服务校验'}
        </span>
        <Button
          tone={signedIn ? 'danger' : 'primary'}
          loading={accountBusy}
          disabled={!signedIn && !state.osAccount?.configured}
          onClick={() => {
            void runAccountAction();
          }}
        >
          {signedIn ? <LogOut size={15} /> : <Cloud size={15} />}
          {signedIn ? '退出连接' : '授权连接'}
        </Button>
      </div>
    </SettingsSection>
  );
}

function Capability({ label, active }: { label: string; active: boolean }) {
  return (
    <div>
      <span>{label}</span>
      <strong className={active ? 'available' : ''}>{active ? '可用' : '未启用'}</strong>
    </div>
  );
}
