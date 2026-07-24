import type { ProviderInfo } from '../../../../types/api';
import { KeyValueEditor } from '../config/key-value-editor';
import { SettingsDisclosure } from '../config/settings-disclosure';
import { SettingsField } from '../config/settings-field';
import { SettingsSecretField, SettingsTextField } from '../config/settings-fields';
import { SettingsRow } from '../config/settings-row';

export function ProviderConnectionEditor({
  provider,
  onChange,
}: {
  provider: ProviderInfo;
  onChange(provider: ProviderInfo): void;
}) {
  const label = provider.name || 'Provider';
  return (
    <div className='provider-connection-editor'>
      <header className='provider-workspace-heading'>
        <div>
          <strong>连接</strong>
          <span>API Key 和 Base URL 直接用于这个 Provider 下的模型。</span>
        </div>
      </header>
      <div className='provider-connection-grid'>
        <SettingsField label='Provider 名称'>
          <SettingsTextField
            label={`${label} 名称`}
            value={provider.name}
            placeholder='openai'
            onChange={(name) => onChange({ ...provider, name })}
          />
        </SettingsField>
        <SettingsField label='API Key'>
          <SettingsSecretField
            label={`${label} API Key`}
            value={provider.apiKey}
            onChange={(apiKey) => onChange({ ...provider, apiKey })}
          />
        </SettingsField>
        <SettingsField label='Base URL'>
          <SettingsTextField
            type='url'
            label={`${label} Base URL`}
            value={provider.baseUrl}
            placeholder='使用 Provider 默认地址'
            onChange={(baseUrl) => onChange({ ...provider, baseUrl: baseUrl || null })}
          />
        </SettingsField>
      </div>
      <SettingsDisclosure title='高级连接选项' description='请求头与会话标识透传；通常无需配置。'>
        <SettingsRow label='会话 ID Header' description='将运行时 session id 注入指定请求头。'>
          <SettingsTextField
            label={`${label} 会话 ID Header`}
            value={provider.sessionIdHeader}
            placeholder='例如 X-Session-ID'
            onChange={(sessionIdHeader) => onChange({ ...provider, sessionIdHeader: sessionIdHeader || null })}
          />
        </SettingsRow>
        <SettingsRow label='自定义 Header' description='添加该 Provider 要求的请求头。' vertical>
          <KeyValueEditor
            label={`${label} Header`}
            value={provider.headers ?? {}}
            onChange={(headers) => onChange({ ...provider, headers })}
          />
        </SettingsRow>
      </SettingsDisclosure>
    </div>
  );
}
