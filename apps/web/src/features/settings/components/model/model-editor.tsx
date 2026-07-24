import { useState } from 'react';
import { Button, Dialog } from '../../../../design-system/primitives';
import type { ModelInfo } from '../../../../types/api';
import { KeyValueEditor } from '../config/key-value-editor';
import { SettingsDisclosure } from '../config/settings-disclosure';
import { SettingsField } from '../config/settings-field';
import { SettingsNumberField, SettingsSecretField, SettingsTextField } from '../config/settings-fields';
import { SettingsRow } from '../config/settings-row';
import { SettingsSwitch } from '../config/settings-switch';

export function ModelEditorDialog({
  initialModel,
  providerName,
  existingModelIds,
  title,
  onClose,
  onSave,
}: {
  initialModel: ModelInfo;
  providerName: string;
  existingModelIds: string[];
  title: string;
  onClose(): void;
  onSave(model: ModelInfo): void;
}) {
  const [model, setModel] = useState<ModelInfo>(() => structuredClone(initialModel));
  const modelId = model.id.trim();
  const duplicateId = Boolean(modelId && existingModelIds.includes(modelId));
  const reference = `${providerName || 'provider'}/${modelId || 'model'}`;

  const save = () => {
    if (!modelId || duplicateId) return;
    onSave({
      ...model,
      id: modelId,
      name: model.name?.trim() || modelId,
    });
  };

  return (
    <Dialog
      className='model-editor-dialog'
      title={title}
      description='模型 ID 必填；能力、限额和连接覆盖按实际情况设置。'
      onClose={onClose}
      footer={
        <>
          <Button onClick={onClose}>取消</Button>
          <Button tone='primary' disabled={!modelId || duplicateId} onClick={save}>
            保存模型
          </Button>
        </>
      }
    >
      <div className='model-editor-form'>
        <div className='config-field-grid two'>
          <SettingsField label='模型 ID'>
            <SettingsTextField
              label={`${reference} 模型标识`}
              value={model.id}
              aria-invalid={!modelId || duplicateId}
              onChange={(id) => setModel({ ...model, id })}
            />
            {duplicateId && (
              <small className='model-editor-field-error' role='alert'>
                当前 Provider 已有同名模型。
              </small>
            )}
          </SettingsField>
          <SettingsField label='显示名称'>
            <SettingsTextField
              label={`${reference} 显示名称`}
              value={model.name}
              placeholder={modelId || '模型名称'}
              onChange={(name) => setModel({ ...model, name })}
            />
          </SettingsField>
        </div>

        <fieldset className='config-subsection-grid capabilities'>
          <legend className='config-visually-hidden'>模型能力</legend>
          <Capability
            label='文件附件'
            checked={Boolean(model.attachment)}
            onChange={(attachment) => setModel({ ...model, attachment })}
          />
          <Capability
            label='推理模式'
            checked={Boolean(model.reasoning)}
            onChange={(reasoning) => setModel({ ...model, reasoning })}
          />
          <Capability
            label='工具调用'
            checked={model.toolCall !== false}
            onChange={(toolCall) => setModel({ ...model, toolCall })}
          />
          <Capability
            label='Temperature'
            checked={model.temperature !== false}
            onChange={(temperature) => setModel({ ...model, temperature })}
          />
        </fieldset>

        <SettingsDisclosure title='能力与限额' description='模型家族、上下文窗口和最大输出。'>
          <div className='config-field-grid three'>
            <SettingsField label='模型家族'>
              <SettingsTextField
                label={`${reference} 模型家族`}
                value={model.family}
                placeholder='例如 claude-sonnet'
                onChange={(family) => setModel({ ...model, family })}
              />
            </SettingsField>
            <SettingsField label='上下文窗口'>
              <SettingsNumberField
                label={`${reference} 上下文窗口`}
                value={model.limit?.context}
                min={0}
                suffix='tokens'
                onChange={(context) => setModel({ ...model, limit: { ...model.limit, context: context ?? 0 } })}
              />
            </SettingsField>
            <SettingsField label='最大输出'>
              <SettingsNumberField
                label={`${reference} 最大输出`}
                value={model.limit?.output}
                min={0}
                suffix='tokens'
                onChange={(output) => setModel({ ...model, limit: { ...model.limit, output: output ?? 0 } })}
              />
            </SettingsField>
          </div>
        </SettingsDisclosure>

        <SettingsDisclosure title='模型级连接覆盖' description='留空时继承 Provider 的连接配置。'>
          <SettingsRow label='独立 Base URL'>
            <SettingsTextField
              type='url'
              label={`${reference} 独立 API 地址`}
              value={model.baseUrl}
              placeholder='继承 Provider'
              onChange={(baseUrl) => setModel({ ...model, baseUrl: baseUrl || null })}
            />
          </SettingsRow>
          <SettingsRow label='独立 API Key'>
            <SettingsSecretField
              label={`${reference} 独立 API Key`}
              value={model.apiKey}
              onChange={(apiKey) => setModel({ ...model, apiKey })}
            />
          </SettingsRow>
        </SettingsDisclosure>

        <SettingsDisclosure title='高级模型信息' description='发布日期、模态、成本和请求头。'>
          <SettingsRow label='发布日期'>
            <SettingsTextField
              label={`${reference} 发布日期`}
              value={model.releaseDate}
              placeholder='YYYY-MM-DD'
              onChange={(releaseDate) => setModel({ ...model, releaseDate: releaseDate || null })}
            />
          </SettingsRow>
          <SettingsRow label='会话 ID Header'>
            <SettingsTextField
              label={`${reference} 会话 ID Header`}
              value={model.sessionIdHeader}
              placeholder='继承 Provider'
              onChange={(sessionIdHeader) => setModel({ ...model, sessionIdHeader: sessionIdHeader || null })}
            />
          </SettingsRow>
          <SettingsRow label='输入模态'>
            <SettingsTextField
              label={`${reference} 输入模态`}
              value={(model.modalities?.input ?? []).join(', ')}
              placeholder='text, image'
              onChange={(value) =>
                setModel({
                  ...model,
                  modalities: { input: commaList(value), output: model.modalities?.output ?? [] },
                })
              }
            />
          </SettingsRow>
          <SettingsRow label='输出模态'>
            <SettingsTextField
              label={`${reference} 输出模态`}
              value={(model.modalities?.output ?? []).join(', ')}
              placeholder='text'
              onChange={(value) =>
                setModel({
                  ...model,
                  modalities: { input: model.modalities?.input ?? [], output: commaList(value) },
                })
              }
            />
          </SettingsRow>
          <div className='config-field-grid four'>
            {(['input', 'output', 'cacheRead', 'cacheWrite'] as const).map((key) => (
              <SettingsField label={costLabels[key]} key={key}>
                <SettingsNumberField
                  label={`${reference} ${costLabels[key]}`}
                  value={model.cost?.[key]}
                  min={0}
                  step={0.01}
                  onChange={(value) =>
                    setModel({
                      ...model,
                      cost: {
                        input: model.cost?.input ?? 0,
                        output: model.cost?.output ?? 0,
                        cacheRead: model.cost?.cacheRead ?? 0,
                        cacheWrite: model.cost?.cacheWrite ?? 0,
                        [key]: value ?? 0,
                      },
                    })
                  }
                />
              </SettingsField>
            ))}
          </div>
          <SettingsRow label='自定义 Header' vertical>
            <KeyValueEditor
              label={`${reference} Header`}
              value={model.headers ?? {}}
              onChange={(headers) => setModel({ ...model, headers })}
            />
          </SettingsRow>
        </SettingsDisclosure>
      </div>
    </Dialog>
  );
}

function Capability({ label, checked, onChange }: { label: string; checked: boolean; onChange(value: boolean): void }) {
  return (
    <div>
      <span>{label}</span>
      <SettingsSwitch label={label} checked={checked} onChange={onChange} />
    </div>
  );
}

function commaList(value: string) {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

const costLabels = {
  input: '输入成本 / M',
  output: '输出成本 / M',
  cacheRead: '缓存读取 / M',
  cacheWrite: '缓存写入 / M',
};
