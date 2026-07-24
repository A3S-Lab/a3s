import { Server, Trash2 } from 'lucide-react';
import { Button } from '../../../../design-system/primitives';
import type { ProviderInfo } from '../../../../types/api';
import { ProviderConnectionEditor } from './provider-connection-editor';
import { ProviderModelsEditor } from './provider-models-editor';

export function ProviderEditor({
  provider,
  defaultModel,
  onDefaultModelChange,
  onChange,
  onRemove,
}: {
  provider: ProviderInfo;
  defaultModel: string;
  onDefaultModelChange(defaultModel: string): void;
  onChange(provider: ProviderInfo): void;
  onRemove(): void;
}) {
  const name = provider.name || '未命名 Provider';
  return (
    <div className='model-provider-editor'>
      <header className='model-provider-detail-header'>
        <span className='model-provider-detail-icon' aria-hidden='true'>
          <Server size={16} />
        </span>
        <div>
          <strong>{name}</strong>
          <span>{provider.baseUrl || '使用 Provider 默认 API 地址'}</span>
        </div>
        <Button tone='quiet' className='model-provider-remove' aria-label={`删除 Provider ${name}`} onClick={onRemove}>
          <Trash2 size={13} /> 删除
        </Button>
      </header>

      <ProviderConnectionEditor provider={provider} onChange={onChange} />
      <ProviderModelsEditor
        provider={provider}
        defaultModel={defaultModel}
        onDefaultModelChange={onDefaultModelChange}
        onChange={onChange}
      />
    </div>
  );
}
