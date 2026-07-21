import { createWorkSlideTransition } from '../work-presentation-transition';
import type {
  WorkSlideTransition,
  WorkSlideTransitionDirection,
  WorkSlideTransitionSpeed,
  WorkSlideTransitionType,
} from '../work-types';

export function PresentationTransitionPanel({
  transition,
  onChange,
  onApplyToAll,
}: {
  transition: WorkSlideTransition | undefined;
  onChange: (transition: WorkSlideTransition | undefined) => void;
  onApplyToAll: () => void;
}) {
  const update = (patch: Partial<WorkSlideTransition>) => {
    if (transition) onChange({ ...transition, ...patch });
  };
  return (
    <section className='work-presentation-transition-panel' aria-label='幻灯片切换设置'>
      <strong>切换</strong>
      <label>
        <span>效果</span>
        <select
          aria-label='幻灯片切换效果'
          value={transition?.type ?? 'none'}
          onChange={(event) => {
            const type = event.target.value;
            onChange(
              type === 'none' ? undefined : createWorkSlideTransition(type as WorkSlideTransitionType, transition)
            );
          }}
        >
          <option value='none'>无</option>
          <option value='fade'>淡化</option>
          <option value='push'>推进</option>
          <option value='wipe'>擦除</option>
          <option value='split'>分割</option>
          <option value='cut'>切换</option>
        </select>
      </label>
      {(transition?.type === 'push' || transition?.type === 'wipe') && (
        <label>
          <span>方向</span>
          <select
            aria-label='切换方向'
            value={transition.direction}
            onChange={(event) => update({ direction: event.target.value as WorkSlideTransitionDirection })}
          >
            <option value='left'>向左</option>
            <option value='right'>向右</option>
            <option value='up'>向上</option>
            <option value='down'>向下</option>
          </select>
        </label>
      )}
      {transition?.type === 'split' && (
        <>
          <label>
            <span>方向</span>
            <select
              aria-label='切换方向'
              value={transition.direction}
              onChange={(event) => update({ direction: event.target.value as WorkSlideTransitionDirection })}
            >
              <option value='out'>向外</option>
              <option value='in'>向内</option>
            </select>
          </label>
          <label>
            <span>分割方式</span>
            <select
              aria-label='分割方式'
              value={transition.orientation}
              onChange={(event) => update({ orientation: event.target.value as 'horizontal' | 'vertical' })}
            >
              <option value='horizontal'>水平</option>
              <option value='vertical'>垂直</option>
            </select>
          </label>
        </>
      )}
      <label>
        <span>速度</span>
        <select
          aria-label='切换速度'
          disabled={!transition}
          value={transition?.speed ?? 'medium'}
          onChange={(event) => update({ speed: event.target.value as WorkSlideTransitionSpeed })}
        >
          <option value='fast'>快速</option>
          <option value='medium'>中速</option>
          <option value='slow'>慢速</option>
        </select>
      </label>
      <label className='toggle'>
        <input
          type='checkbox'
          aria-label='单击鼠标后换片'
          disabled={!transition}
          checked={transition?.advanceOnClick ?? true}
          onChange={(event) => update({ advanceOnClick: event.target.checked })}
        />
        <span>单击鼠标后</span>
      </label>
      <label className='toggle'>
        <input
          type='checkbox'
          aria-label='自动换片'
          disabled={!transition}
          checked={transition?.advanceAfterMs !== undefined}
          onChange={(event) => update({ advanceAfterMs: event.target.checked ? 5000 : undefined })}
        />
        <span>自动换片</span>
      </label>
      <label>
        <span>秒数</span>
        <input
          type='number'
          aria-label='自动换片秒数'
          min={0.25}
          max={3600}
          step={0.25}
          disabled={!transition || transition.advanceAfterMs === undefined}
          value={transition?.advanceAfterMs === undefined ? '' : transition.advanceAfterMs / 1000}
          onChange={(event) =>
            update({
              advanceAfterMs: Math.max(250, Math.min(3_600_000, Number(event.target.value) * 1000)),
            })
          }
        />
      </label>
      <button type='button' onClick={onApplyToAll}>
        应用切换效果到全部幻灯片
      </button>
    </section>
  );
}
