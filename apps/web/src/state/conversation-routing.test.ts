import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  appState,
  closeSettings,
  navigateConversation,
  navigateSettings,
  navigateWorkHome,
  syncShellRouteFromLocation,
} from './app-state';

describe('conversation navigation', () => {
  beforeEach(() => {
    appState.activeProduct = 'work';
    appState.settingsOpen = false;
    appState.workRoute = 'home';
    appState.conversationSessionId = null;
    window.history.replaceState(null, '', '#home');
  });

  afterEach(() => {
    vi.restoreAllMocks();
    window.history.replaceState(null, '', '#home');
  });

  it('adds Home-to-conversation navigation to browser history', () => {
    const pushState = vi.spyOn(window.history, 'pushState');

    navigateConversation('task/alpha');

    expect(pushState).toHaveBeenCalledWith(null, '', '#conversation/task%2Falpha');
    expect(window.location.hash).toBe('#conversation/task%2Falpha');
    expect(appState.workRoute).toBe('conversation');
    expect(appState.conversationSessionId).toBe('task/alpha');
  });

  it('restores shell state when browser history returns to a conversation', () => {
    window.history.replaceState(null, '', '#conversation/task-from-history');

    const route = syncShellRouteFromLocation();

    expect(route.conversationSessionId).toBe('task-from-history');
    expect(appState.activeProduct).toBe('work');
    expect(appState.workRoute).toBe('conversation');
    expect(appState.conversationSessionId).toBe('task-from-history');
  });

  it('returns to Home without leaving a stale conversation identity', () => {
    navigateConversation('task-1');

    navigateWorkHome({ history: 'replace' });

    expect(window.location.hash).toBe('#home');
    expect(appState.workRoute).toBe('home');
    expect(appState.conversationSessionId).toBeNull();
  });

  it('returns from settings to the conversation that opened it', () => {
    navigateConversation('task-settings');
    navigateSettings('general');

    closeSettings();

    expect(window.location.hash).toBe('#conversation/task-settings');
    expect(appState.workRoute).toBe('conversation');
    expect(appState.conversationSessionId).toBe('task-settings');
  });
});
