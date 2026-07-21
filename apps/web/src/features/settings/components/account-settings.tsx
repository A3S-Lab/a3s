import type { SettingsActions } from '../settings-actions';
import { A3sOsAccount } from './account/a3s-os-account';
import { LocalModelAccounts } from './account/local-model-accounts';

export function AccountSettings({ actions }: { actions: SettingsActions }) {
  return (
    <div className='settings-config-page account-settings-page'>
      <A3sOsAccount actions={actions} />
      <LocalModelAccounts actions={actions} />
    </div>
  );
}
