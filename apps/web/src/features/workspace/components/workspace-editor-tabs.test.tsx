import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { WorkspaceActions } from '../workspace-actions';
import type { WorkspaceEditorTab } from '../workspace-state';
import { WorkspaceEditorTabs } from './workspace-editor-tabs';

const tabs: WorkspaceEditorTab[] = [
  fileTab('/repo/src/app.ts'),
  fileTab('/repo/src/routes.ts'),
  fileTab('/repo/README.md'),
];

describe('WorkspaceEditorTabs context menu', () => {
  beforeEach(() => {
    appState.workspaceRoot = '/repo';
    appState.editorTabs = tabs.map((tab) => ({ ...tab }));
    appState.activeEditorTabId = tabs[0].id;
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('offers Chinese tab operations and closes the tabs to the right as one safe request', () => {
    const closeEditorTabs = vi.fn();
    render(
      <WorkspaceEditorTabs actions={{ closeEditorTabs, activateEditorTab: vi.fn() } as unknown as WorkspaceActions} />
    );

    fireEvent.contextMenu(screen.getByRole('tab', { name: /routes\.ts/ }), {
      clientX: 120,
      clientY: 32,
    });

    const menu = screen.getByRole('menu', { name: 'routes.ts 标签页操作' });
    expect(menu).toHaveTextContent('关闭');
    expect(menu).toHaveTextContent('关闭其他标签页');
    expect(menu).toHaveTextContent('关闭右侧标签页');
    expect(menu).toHaveTextContent('关闭全部标签页');
    expect(menu).toHaveTextContent('复制相对路径');
    expect(screen.getByRole('menuitem', { name: '关闭' })).toHaveAttribute(
      'aria-keyshortcuts',
      expect.stringMatching(/^(Meta|Control)\+W$/)
    );
    fireEvent.click(screen.getByRole('menuitem', { name: '关闭右侧标签页' }));

    expect(closeEditorTabs).toHaveBeenCalledWith([tabs[2].id]);
  });

  it('copies a workspace-relative path and supports keyboard dismissal with focus restoration', async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(
      <WorkspaceEditorTabs
        actions={{ closeEditorTabs: vi.fn(), activateEditorTab: vi.fn() } as unknown as WorkspaceActions}
      />
    );
    const tab = screen.getByRole('tab', { name: /app\.ts/ });
    tab.focus();
    fireEvent.keyDown(tab, { key: 'F10', shiftKey: true });

    expect(screen.getByRole('menuitem', { name: '关闭' })).toHaveFocus();
    fireEvent.click(screen.getByRole('menuitem', { name: '复制相对路径' }));
    expect(writeText).toHaveBeenCalledWith('src/app.ts');

    fireEvent.keyDown(tab, { key: 'F10', shiftKey: true });
    const menu = screen.getByRole('menu', { name: 'app.ts 标签页操作' });
    fireEvent.keyDown(menu, { key: 'Escape' });
    expect(tab).toHaveFocus();
  });

  it('shares Home and End roving navigation with the system tab contract', () => {
    const activateEditorTab = vi.fn();
    render(
      <WorkspaceEditorTabs actions={{ closeEditorTabs: vi.fn(), activateEditorTab } as unknown as WorkspaceActions} />
    );

    const first = screen.getByRole('tab', { name: /app\.ts/ });
    const last = screen.getByRole('tab', { name: /README\.md/ });
    first.focus();
    fireEvent.keyDown(first, { key: 'End' });

    expect(last).toHaveFocus();
    expect(activateEditorTab).toHaveBeenLastCalledWith(tabs[2].id);

    fireEvent.keyDown(last, { key: 'Home' });
    expect(first).toHaveFocus();
    expect(activateEditorTab).toHaveBeenLastCalledWith(tabs[0].id);
  });
});

function fileTab(path: string): WorkspaceEditorTab {
  return {
    id: `file:${path}`,
    kind: 'file',
    path,
    content: '',
    draft: '',
    isBinary: false,
    location: null,
    loading: false,
    loadError: null,
    saving: false,
    configValidation: null,
  };
}
