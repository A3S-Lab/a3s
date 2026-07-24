import { SettingsDisclosure } from '../config/settings-disclosure';
import { SettingsNumberField } from '../config/settings-fields';

export function ModelRuntimeSettings({
  thinkingBudget,
  timeoutMs,
  onThinkingBudgetChange,
  onTimeoutChange,
}: {
  thinkingBudget?: number | null;
  timeoutMs?: number | null;
  onThinkingBudgetChange(value: number | null): void;
  onTimeoutChange(value: number | null): void;
}) {
  const overrideCount = Number(thinkingBudget != null) + Number(timeoutMs != null);

  return (
    <div className='model-runtime-disclosure'>
      <SettingsDisclosure
        title='高级运行参数'
        description='仅在需要覆盖运行时默认值时设置推理预算和请求超时。'
        badge={
          <span className={`model-runtime-status ${overrideCount ? 'configured' : ''}`}>
            {overrideCount ? `${overrideCount} 项已覆盖` : '使用默认值'}
          </span>
        }
      >
        <div className='model-runtime-grid'>
          <div className='model-runtime-field'>
            <div>
              <strong>推理预算</strong>
              <span>单次请求最多可使用的思考 token。</span>
            </div>
            <SettingsNumberField
              label='推理预算'
              value={thinkingBudget}
              min={1}
              placeholder='运行时默认'
              suffix='tokens'
              onChange={onThinkingBudgetChange}
            />
          </div>
          <div className='model-runtime-field'>
            <div>
              <strong>请求超时</strong>
              <span>模型请求等待多久后停止。</span>
            </div>
            <SettingsNumberField
              label='LLM API 请求超时'
              value={timeoutMs == null ? null : timeoutMs / 1_000}
              min={0.1}
              step={0.1}
              placeholder='运行时默认'
              suffix='秒'
              onChange={(seconds) => onTimeoutChange(seconds == null ? null : seconds * 1_000)}
            />
          </div>
        </div>
      </SettingsDisclosure>
    </div>
  );
}
