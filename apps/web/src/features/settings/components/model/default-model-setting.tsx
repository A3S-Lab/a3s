import { ModelCombobox } from '../../../../design-system/primitives';
import type { CatalogModel } from '../../../../types/api';

export function DefaultModelSetting({
  models,
  providerCount,
  value,
  savedValue,
  onChange,
}: {
  models: CatalogModel[];
  providerCount: number;
  value: string;
  savedValue: string;
  onChange(value: string): void;
}) {
  const current = models.find((model) => model.id === value);
  return (
    <div className='model-default-control'>
      <span className='model-default-label'>
        <strong>默认模型</strong>
        <small>{current ? `${current.source} · ${current.name}` : '请先添加模型'}</small>
      </span>
      <ModelCombobox
        compact
        models={models}
        value={value}
        defaultModel={savedValue}
        disabled={!models.length}
        label={`设置默认模型，${providerCount} 个 Provider`}
        onChange={onChange}
      />
    </div>
  );
}
