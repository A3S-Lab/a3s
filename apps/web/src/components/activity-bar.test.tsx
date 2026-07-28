import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { createCodeShellState } from '../features/code/code-state';
import { appState } from '../state/app-state';
import type { PluginActivityItem } from '../types/api';
import { ActivityBar } from './activity-bar';

const sciencePlugin: PluginActivityItem = {
  key: 'science:research',
  packageId: 'use/a3s/science',
  route: 'science',
  version: '1.2.3',
  enabled: true,
  id: 'research',
  title: '科研',
  description: 'Explore scientific sources.',
  icon: 'flask-conical',
  skill: 'a3s-use-science',
  order: 120,
  sha256: 'a'.repeat(64),
  mediaType: 'text/html',
};

describe('A3S activity bar', () => {
  beforeEach(() => {
    appState.activeProduct = 'work';
    appState.activePluginKey = null;
    appState.pluginCatalog = {
      schemaVersion: 1,
      available: true,
      generation: 1,
      revision: 'b'.repeat(64),
      items: [],
    };
    appState.activeSessionId = null;
    appState.composerValue = '';
    appState.composerContextFiles = [];
    appState.composerSkills = [];
    window.history.replaceState(null, '', '#home');
  });

  afterEach(() => {
    cleanup();
    appState.settingsOpen = false;
  });

  it('selects the unified Work surface as the first and default destination on a fresh URL', () => {
    window.history.replaceState(null, '', '/');
    Object.assign(appState, createCodeShellState());
    render(<ActivityBar />);

    const buttons = screen.getAllByRole('button');
    expect(buttons[0]).toHaveAttribute('aria-label', '工作');
    expect(buttons[1]).toHaveAttribute('aria-label', '知识');
    expect(buttons[0]).toHaveAttribute('aria-current', 'page');
    expect(appState.activeProduct).toBe('work');
    expect(window.location.hash).toBe('#home');
  });

  it('keeps one unified Work entry while loading enabled plugin contributions', async () => {
    appState.pluginCatalog.items = [
      sciencePlugin,
      { ...sciencePlugin, key: 'search:find', route: 'search', id: 'find', title: 'Search', order: 20, icon: 'search' },
      { ...sciencePlugin, key: 'hidden:view', route: 'hidden', id: 'view', title: 'Hidden', enabled: false },
    ];
    render(<ActivityBar />);

    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual([
      '工作',
      '知识',
      'Search',
      '科研',
      '记忆',
      '市场',
      '设置',
    ]);
    expect(screen.queryByRole('button', { name: /科学/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Hidden' })).not.toBeInTheDocument();
    const productSection = screen.getByRole('button', { name: '工作' }).parentElement;
    expect(productSection).toHaveClass('activity-products');
    expect(productSection).toContainElement(screen.getByRole('button', { name: '知识' }));
    expect(productSection).toContainElement(screen.getByRole('button', { name: '科研' }));

    fireEvent.click(screen.getByRole('button', { name: '科研' }));
    expect(appState.activeProduct).toBe('plugin');
    expect(appState.activePluginKey).toBe('science:research');
    expect(window.location.hash).toBe('#plugin/science%3Aresearch');
    await waitFor(() => expect(screen.getByRole('button', { name: '科研' })).toHaveAttribute('aria-current', 'page'));

    fireEvent.click(screen.getByRole('button', { name: '工作' }));
    expect(appState.activeProduct).toBe('work');
    expect(window.location.hash).toBe('#home');
    await waitFor(() => expect(screen.getByRole('button', { name: '工作' })).toHaveAttribute('aria-current', 'page'));
  });

  it('opens Knowledge as the built-in destination immediately below Work', async () => {
    appState.sidebarOpen = false;
    render(<ActivityBar />);
    const productButtons = screen
      .getByRole('button', { name: '工作' })
      .parentElement?.querySelectorAll<HTMLButtonElement>('.activity-button');

    expect([...productButtons!].map((button) => button.getAttribute('aria-label'))).toEqual(['工作', '知识']);
    fireEvent.click(screen.getByRole('button', { name: '知识' }));

    expect(appState.activeProduct).toBe('knowledge');
    expect(appState.sidebarOpen).toBe(true);
    expect(window.location.hash).toBe('#knowledge');
    await waitFor(() => expect(screen.getByRole('button', { name: '知识' })).toHaveAttribute('aria-current', 'page'));
  });

  it('preserves the unified Work draft while visiting Knowledge', () => {
    appState.activeProduct = 'work';
    appState.activeSessionId = null;
    appState.composerValue = '请概览当前文件夹的内容，说明主要文件、用途和最近值得关注的变化。不要修改文件。';
    appState.composerContextFiles = ['Reports'];
    appState.draftsByTask = {};

    render(<ActivityBar />);
    fireEvent.click(screen.getByRole('button', { name: '知识' }));
    fireEvent.click(screen.getByRole('button', { name: '工作' }));

    expect(appState.activeProduct).toBe('work');
    expect(appState.composerValue).toContain('概览当前文件夹');
    expect(appState.composerContextFiles).toEqual(['Reports']);
  });

  it('opens the signed plugin marketplace as a system entry', async () => {
    render(<ActivityBar />);
    fireEvent.click(screen.getByRole('button', { name: '市场' }));

    expect(appState.activeProduct).toBe('plugins');
    expect(window.location.hash).toBe('#plugins');
    await waitFor(() => expect(screen.getByRole('button', { name: '市场' })).toHaveAttribute('aria-current', 'page'));
  });

  it('keeps settings in the system section', async () => {
    render(<ActivityBar />);
    const systemSection = screen.getByRole('button', { name: '设置' }).parentElement;
    expect(systemSection).toHaveClass('activity-system');
    expect(systemSection).toContainElement(screen.getByRole('button', { name: '记忆' }));
    expect(systemSection).toContainElement(screen.getByRole('button', { name: '市场' }));
    expect(systemSection?.querySelectorAll('.activity-button')).toHaveLength(3);
    expect(screen.queryByRole('button', { name: '账户与连接' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '设置' })).toHaveAttribute('data-activity-tooltip', '设置');
    fireEvent.click(screen.getByRole('button', { name: '设置' }));
    expect(appState.settingsOpen).toBe(true);
    expect(appState.settingsTab).toBe('general');
    await waitFor(() => expect(screen.getByRole('button', { name: '工作' })).toHaveAttribute('aria-current', 'page'));
    expect(screen.getByRole('button', { name: '设置' })).toHaveClass('active');
    expect(screen.getByRole('button', { name: '设置' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('opens Memory at its top-level route and restores the unified Work surface', async () => {
    render(<ActivityBar />);

    fireEvent.click(screen.getByRole('button', { name: '记忆' }));
    expect(appState.activeProduct).toBe('memory');
    expect(window.location.hash).toBe('#memory');
    await waitFor(() => expect(screen.getByRole('button', { name: '记忆' })).toHaveAttribute('aria-current', 'page'));
    expect(screen.getByRole('button', { name: '工作' })).not.toHaveAttribute('aria-current');

    fireEvent.click(screen.getByRole('button', { name: '工作' }));
    expect(appState.activeProduct).toBe('work');
    expect(window.location.hash).toBe('#home');
  });
});
