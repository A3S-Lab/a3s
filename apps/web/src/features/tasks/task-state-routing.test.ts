import { afterEach, describe, expect, it } from 'vitest';
import { createTaskState } from './task-state';

describe('task state route authority', () => {
  afterEach(() => {
    localStorage.removeItem('a3s-code-web.task-drafts');
    localStorage.removeItem('a3s-work.active-session');
  });

  it('uses the deep-linked session and its draft instead of the previously active task', () => {
    localStorage.setItem('a3s-work.active-session', 'stored-task');
    localStorage.setItem(
      'a3s-code-web.task-drafts',
      JSON.stringify({
        'stored-task': { content: 'stored draft', contextFiles: [] },
        'route-task': { content: 'route draft', contextFiles: ['src/app.ts'] },
      })
    );

    const state = createTaskState('route-task');

    expect(state.activeSessionId).toBe('route-task');
    expect(state.composerValue).toBe('route draft');
    expect(state.composerContextFiles).toEqual(['src/app.ts']);
  });
});
