import { cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CodeActions } from '../features/code/use-code-controller';
import { appState } from '../state/app-state';
import { AppShell } from './app-shell';

vi.mock('../features/work/pages/work-product', () => ({
  WorkProduct: () => <div>Work</div>,
}));

vi.mock('./activity-bar', () => ({
  ActivityBar: () => <nav aria-label='产品导航' />,
}));

vi.mock('./shell/command-palette', () => ({
  CommandPalette: () => null,
}));

describe('App shell route synchronization', () => {
  beforeEach(() => {
    appState.activeProduct = 'work';
    appState.workRoute = 'home';
    appState.conversationSessionId = null;
    appState.activeSessionId = 'task-old';
    appState.sessions = [session('task-old'), session('task-from-history')];
    appState.settingsOpen = false;
    appState.commandPaletteOpen = false;
    appState.serviceStatus = 'connected';
    window.history.replaceState(null, '', '#home');
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    window.history.replaceState(null, '', '#home');
  });

  it('selects the session addressed by browser Back or a copied hash', async () => {
    const selectSession = vi.fn(async (sessionId: string) => {
      appState.activeSessionId = sessionId;
    });
    render(<AppShell actions={{ selectSession } as unknown as CodeActions} />);

    window.history.pushState(null, '', '#conversation/task-from-history');
    window.dispatchEvent(new PopStateEvent('popstate'));

    await waitFor(() => expect(selectSession).toHaveBeenCalledWith('task-from-history'));
    expect(appState.workRoute).toBe('conversation');
    expect(appState.conversationSessionId).toBe('task-from-history');
    expect(appState.activeSessionId).toBe('task-from-history');
  });
});

function session(sessionId: string) {
  return {
    sessionId,
    workspace: '/repo',
    cwd: '/repo',
    followDefaultModel: true,
    permissionMode: 'default',
    state: 'idle',
    createdAt: 1,
  };
}
