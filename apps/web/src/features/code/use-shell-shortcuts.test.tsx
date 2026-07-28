import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../state/app-state';
import { useShellShortcuts } from './use-shell-shortcuts';

describe('shell location synchronization', () => {
  afterEach(() => {
    cleanup();
    appState.settingsOpen = false;
    appState.updateInstalling = false;
    appState.commandPaletteOpen = false;
    appState.fileQuickOpenOpen = false;
    appState.sidebarOpen = true;
    appState.activeProduct = 'work';
    appState.settingsChannel = 'weixin';
    appState.activeSessionId = null;
    appState.composerValue = '';
    appState.workspaceRoot = '';
    window.history.replaceState(null, '', '#home');
  });

  it('opens Memory as a top-level product without switching the unified session draft', () => {
    appState.activeProduct = 'work';
    appState.activeSessionId = null;
    appState.composerValue = '保留当前工作草稿';
    appState.draftsByTask = {
      __new_task__: {
        content: '保留当前工作草稿',
        contextFiles: [],
        skillNames: [],
      },
    };
    window.history.replaceState(null, '', '#memory');

    renderHook(() => useShellShortcuts(() => undefined));

    expect(appState.activeProduct).toBe('memory');
    expect(appState.activeSessionId).toBeNull();
    expect(appState.composerValue).toBe('保留当前工作草稿');
    expect(appState.draftsByTask.__new_task__?.content).toBe('保留当前工作草稿');
  });

  it.each(['#code/activity', '#work/home'])('does not retain the removed %s route', (route) => {
    renderHook(() => useShellShortcuts(() => undefined));
    window.location.hash = route;
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    expect(appState.activeProduct).toBe('work');
    expect(appState.taskView).toBe('conversation');
    expect(window.location.hash).toBe('#home');
  });

  it('updates system pages and returns to the unified home route', () => {
    renderHook(() => useShellShortcuts(() => undefined));
    window.location.hash = '#settings';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    expect(appState.settingsOpen).toBe(true);
    window.location.hash = '#home';
    window.dispatchEvent(new HashChangeEvent('hashchange'));
    expect(appState.activeProduct).toBe('work');
    expect(appState.taskView).toBe('conversation');
  });

  it('opens the standalone Knowledge route from browser navigation', () => {
    renderHook(() => useShellShortcuts(() => undefined));
    window.location.hash = '#knowledge';
    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(appState.settingsOpen).toBe(false);
    expect(appState.activeProduct).toBe('knowledge');
  });

  it('does not retain the removed standalone Weixin route', () => {
    appState.activeSessionId = 'task-keep';
    appState.composerValue = 'keep this draft';
    window.history.replaceState(null, '', '#weixin');

    renderHook(() => useShellShortcuts(() => undefined));

    expect(appState.activeProduct).toBe('work');
    expect(appState.settingsOpen).toBe(false);
    expect(window.location.hash).toBe('#home');
    expect(appState.activeSessionId).toBe('task-keep');
    expect(appState.composerValue).toBe('keep this draft');
  });

  it('opens a nested Feishu channel route inside Settings', () => {
    window.history.replaceState(null, '', '#settings/channels/feishu');

    renderHook(() => useShellShortcuts(() => undefined));

    expect(appState.settingsOpen).toBe(true);
    expect(appState.settingsTab).toBe('channels');
    expect(appState.settingsChannel).toBe('feishu');
    expect(window.location.hash).toBe('#settings/channels/feishu');
  });

  it('opens Help inside Settings from the question-mark shortcut', () => {
    renderHook(() => useShellShortcuts(() => undefined));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: '?', cancelable: true }));

    expect(appState.settingsOpen).toBe(true);
    expect(appState.settingsTab).toBe('help');
    expect(window.location.hash).toBe('#settings/help');
  });

  it('does not retain the removed standalone Help route', () => {
    renderHook(() => useShellShortcuts(() => undefined));
    window.location.hash = '#help';
    window.dispatchEvent(new HashChangeEvent('hashchange'));

    expect(appState.settingsOpen).toBe(false);
    expect(appState.activeProduct).toBe('work');
    expect(window.location.hash).toBe('#home');
  });

  it('does not let global shortcuts dismiss settings while an update is installing', () => {
    const newTask = vi.fn();
    appState.settingsOpen = true;
    window.history.replaceState(null, '', '#settings/general');
    appState.updateInstalling = true;
    renderHook(() => useShellShortcuts(newTask));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'n', ctrlKey: true, cancelable: true }));

    expect(appState.settingsOpen).toBe(true);
    expect(newTask).not.toHaveBeenCalled();
  });

  it('leaves formatting shortcuts to content-editable task input', () => {
    appState.sidebarOpen = true;
    renderHook(() => useShellShortcuts(() => undefined));
    const editor = document.createElement('div');
    editor.contentEditable = 'true';
    document.body.append(editor);

    editor.dispatchEvent(new KeyboardEvent('keydown', { key: 'b', ctrlKey: true, bubbles: true, cancelable: true }));

    expect(appState.sidebarOpen).toBe(true);
    editor.remove();
  });

  it('lets the Monaco editor toggle the task sidebar with the shell shortcut', () => {
    appState.sidebarOpen = true;
    renderHook(() => useShellShortcuts(() => undefined));
    const monaco = document.createElement('div');
    monaco.className = 'monaco-editor';
    const input = document.createElement('div');
    input.contentEditable = 'true';
    input.addEventListener('keydown', (event) => event.stopPropagation());
    monaco.append(input);
    document.body.append(monaco);
    const event = new KeyboardEvent('keydown', {
      key: 'b',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });

    input.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(appState.sidebarOpen).toBe(false);
    monaco.remove();
  });

  it('opens file quick open from Monaco before its key handler stops propagation', () => {
    appState.activeSessionId = 'task-1';
    appState.workspaceRoot = '/repo';
    appState.commandPaletteOpen = true;
    renderHook(() => useShellShortcuts(() => undefined));
    const monaco = document.createElement('div');
    monaco.className = 'monaco-editor';
    const input = document.createElement('textarea');
    input.addEventListener('keydown', (event) => event.stopPropagation());
    monaco.append(input);
    document.body.append(monaco);
    const event = new KeyboardEvent('keydown', {
      key: 'p',
      ctrlKey: true,
      bubbles: true,
      cancelable: true,
    });

    input.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(appState.commandPaletteOpen).toBe(false);
    expect(appState.fileQuickOpenOpen).toBe(true);
    monaco.remove();
  });

  it('preserves the browser print shortcut when no active workspace can open a file', () => {
    renderHook(() => useShellShortcuts(() => undefined));
    const event = new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, cancelable: true });

    window.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
    expect(appState.fileQuickOpenOpen).toBe(false);
  });
});
