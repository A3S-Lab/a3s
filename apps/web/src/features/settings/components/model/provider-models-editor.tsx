import { Check, Pencil, Plus, Star, Trash2 } from 'lucide-react';
import { useState } from 'react';
import { Button, StateView } from '../../../../design-system/primitives';
import type { ModelInfo, ProviderInfo } from '../../../../types/api';
import { createModel } from './model-catalog';
import { ModelEditorDialog } from './model-editor';

export function ProviderModelsEditor({
  provider,
  defaultModel,
  onDefaultModelChange,
  onChange,
}: {
  provider: ProviderInfo;
  defaultModel: string;
  onDefaultModelChange(defaultModel: string): void;
  onChange(provider: ProviderInfo): void;
}) {
  const [addingModel, setAddingModel] = useState<ModelInfo | null>(null);
  const [editingModelIndex, setEditingModelIndex] = useState<number | null>(null);
  const editingModel = editingModelIndex === null ? null : provider.models[editingModelIndex];
  const providerName = provider.name || 'Provider';

  const updateModel = (index: number, model: ModelInfo) => {
    onChange({
      ...provider,
      models: provider.models.map((item, modelIndex) => (modelIndex === index ? model : item)),
    });
  };
  const removeModel = (index: number) => {
    onChange({ ...provider, models: provider.models.filter((_, modelIndex) => modelIndex !== index) });
  };

  return (
    <section className='provider-models-editor'>
      <header className='provider-workspace-heading'>
        <div>
          <strong>模型</strong>
          <span>
            {provider.models.length ? `${provider.models.length} 个可用模型` : '添加这个 Provider 可用的模型。'}
          </span>
        </div>
        <Button tone='secondary' size='compact' onClick={() => setAddingModel(createModel(provider.models))}>
          <Plus size={13} /> 添加模型
        </Button>
      </header>

      {provider.models.length ? (
        <div className='provider-model-table-wrap'>
          <table className='provider-model-table'>
            <thead>
              <tr>
                <th className='provider-model-default-column'>默认</th>
                <th>模型</th>
                <th className='provider-model-capability-column'>能力</th>
                <th className='provider-model-limit-column'>上下文 / 输出</th>
                <th className='provider-model-actions-column'>操作</th>
              </tr>
            </thead>
            <tbody>
              {provider.models.map((model, index) => {
                const reference = `${provider.name}/${model.id}`;
                const isDefault = reference === defaultModel;
                return (
                  <tr key={`${model.id}:${index}`}>
                    <td>
                      <button
                        type='button'
                        className={`provider-model-default${isDefault ? ' selected' : ''}`}
                        aria-label={isDefault ? `${reference} 是默认模型` : `将 ${reference} 设为默认模型`}
                        title={isDefault ? '默认模型' : '设为默认模型'}
                        onClick={() => onDefaultModelChange(reference)}
                      >
                        {isDefault ? <Check size={13} /> : <Star size={13} />}
                      </button>
                    </td>
                    <td>
                      <strong>{model.name?.trim() || model.id || '未命名模型'}</strong>
                      <code>{model.id || '尚未填写模型 ID'}</code>
                    </td>
                    <td className='provider-model-capability-column'>{capabilityLabel(model)}</td>
                    <td className='provider-model-limit-column'>
                      {formatLimit(model.limit?.context)} / {formatLimit(model.limit?.output)}
                    </td>
                    <td>
                      <div className='provider-model-row-actions'>
                        <Button
                          tone='quiet'
                          size='compact'
                          aria-label={`编辑模型 ${reference}`}
                          onClick={() => setEditingModelIndex(index)}
                        >
                          <Pencil size={13} />
                        </Button>
                        <Button
                          tone='quiet'
                          size='compact'
                          className='provider-model-delete'
                          aria-label={`删除模型 ${reference}`}
                          onClick={() => removeModel(index)}
                        >
                          <Trash2 size={13} />
                        </Button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      ) : (
        <StateView
          className='provider-model-empty'
          size='compact'
          title='还没有模型'
          description='添加模型后，它会出现在默认模型和任务模型选择器中。'
          actions={
            <Button tone='secondary' onClick={() => setAddingModel(createModel(provider.models))}>
              <Plus size={13} /> 添加第一个模型
            </Button>
          }
        />
      )}

      {addingModel && (
        <ModelEditorDialog
          initialModel={addingModel}
          providerName={provider.name}
          existingModelIds={provider.models.map((model) => model.id)}
          title={`为 ${providerName} 添加模型`}
          onClose={() => setAddingModel(null)}
          onSave={(model) => {
            onChange({ ...provider, models: [...provider.models, model] });
            setAddingModel(null);
          }}
        />
      )}
      {editingModel && editingModelIndex !== null && (
        <ModelEditorDialog
          initialModel={editingModel}
          providerName={provider.name}
          existingModelIds={provider.models.filter((_, index) => index !== editingModelIndex).map((model) => model.id)}
          title={`编辑 ${editingModel.name?.trim() || editingModel.id || '模型'}`}
          onClose={() => setEditingModelIndex(null)}
          onSave={(model) => {
            updateModel(editingModelIndex, model);
            setEditingModelIndex(null);
          }}
        />
      )}
    </section>
  );
}

function capabilityLabel(model: ModelInfo): string {
  const capabilities = [
    model.reasoning ? '推理' : null,
    model.toolCall !== false ? '工具' : null,
    model.attachment ? '附件' : null,
  ].filter(Boolean);
  return capabilities.join(' · ') || '标准';
}

function formatLimit(value?: number): string {
  if (!value) return '—';
  return new Intl.NumberFormat('zh-CN', { notation: value >= 10_000 ? 'compact' : 'standard' }).format(value);
}
