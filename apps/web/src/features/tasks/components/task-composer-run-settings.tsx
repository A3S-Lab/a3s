import { ChevronUp, SlidersHorizontal, X } from 'lucide-react';
import { useSnapshot } from 'valtio';
import { Popover } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import type { TaskActions } from '../task-actions';
import { TaskComposerTrailingControls } from './task-composer-controls';
import { TaskComposerResearchMode } from './task-composer-research-mode';

export function TaskComposerRunSettings({ actions, disabled = false }: { actions: TaskActions; disabled?: boolean }) {
  const state = useSnapshot(appState);
  const deepResearch = state.composerMode === 'deepResearch';

  return (
    <Popover
      label={deepResearch ? '运行设置，深度研究已开启' : '运行设置'}
      panelLabel='运行设置面板'
      className={`composer-run-settings${deepResearch ? ' research-active' : ''}`}
      panelClassName='composer-run-settings-panel'
      placement='top-end'
      disabled={disabled}
      trigger={(triggerProps, { open }) => (
        <button
          {...triggerProps}
          className={`composer-quick-trigger composer-run-settings-trigger${open ? ' active' : ''}`}
        >
          <SlidersHorizontal size={15} />
          <span>运行设置</span>
          {deepResearch && <i aria-hidden='true'>研究</i>}
          <ChevronUp className='composer-run-settings-chevron' size={12} />
        </button>
      )}
    >
      {(close) => (
        <>
          <header>
            <span>
              <strong>运行设置</strong>
              <small>默认配置适合大多数任务，需要时再调整</small>
            </span>
            <button type='button' aria-label='关闭运行设置' onClick={close}>
              <X size={14} />
            </button>
          </header>
          <div className='composer-run-settings-row'>
            <span>任务方式</span>
            <TaskComposerResearchMode disabled={disabled} />
          </div>
          <div className='composer-run-settings-row'>
            <span>模型与推理</span>
            <TaskComposerTrailingControls actions={actions} />
          </div>
        </>
      )}
    </Popover>
  );
}
