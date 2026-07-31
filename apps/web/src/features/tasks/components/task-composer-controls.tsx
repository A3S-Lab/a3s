import { useSnapshot } from 'valtio';
import { appState } from '../../../state/app-state';
import type { TaskActions } from '../task-actions';
import { findTaskSession } from '../task-state';
import { TaskComposerContextControl } from './task-composer-context-control';
import { TaskComposerEffortControl } from './task-composer-effort-control';
import { TaskComposerModelControl } from './task-composer-model-control';

export function TaskComposerTrailingControls({ actions }: { actions: TaskActions }) {
  const state = useSnapshot(appState);
  const task = findTaskSession(state.sessions, state.activeSessionId);
  const controls = task ? state.sessionControls[task.sessionId] : undefined;

  return (
    <section className='composer-controls composer-trailing-controls' aria-label='上下文、推理强度和模型'>
      {controls?.context && task && (
        <TaskComposerContextControl
          context={controls.context}
          compacting={Boolean(state.contextCompacting[task.sessionId])}
          disabled={Boolean(state.streamingSessionId || state.taskConfigSaving || state.taskSubmissionState)}
          onCompact={() => actions.compactSession()}
        />
      )}
      <TaskComposerEffortControl actions={actions} />
      <TaskComposerModelControl actions={actions} />
    </section>
  );
}
