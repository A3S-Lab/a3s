import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
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
  title: 'Science',
  description: 'Explore scientific sources.',
  icon: 'flask-conical',
  skill: 'a3s-use-science',
  order: 120,
  sha256: 'a'.repeat(64),
  mediaType: 'text/html',
};

describe('A3S activity bar', () => {
  beforeEach(() => {
    appState.activeProduct = 'code';
    appState.codeSurface = 'tasks';
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
    window.history.replaceState(null, '', '#code/conversation');
  });

  afterEach(() => {
    cleanup();
    appState.settingsOpen = false;
  });

  it('keeps Code first and replaces hardcoded products with enabled plugin contributions', async () => {
    appState.pluginCatalog.items = [
      sciencePlugin,
      { ...sciencePlugin, key: 'search:find', route: 'search', id: 'find', title: 'Search', order: 20, icon: 'search' },
      { ...sciencePlugin, key: 'hidden:view', route: 'hidden', id: 'view', title: 'Hidden', enabled: false },
    ];
    render(<ActivityBar />);

    expect(screen.getAllByRole('button').map((button) => button.getAttribute('aria-label'))).toEqual([
      '编码',
      'Search',
      'Science',
      '记忆',
      '插件市场',
      '设置',
    ]);
    expect(screen.queryByRole('button', { name: '办公' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /科学/ })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Hidden' })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: 'Science' }));
    expect(appState.activeProduct).toBe('plugin');
    expect(appState.activePluginKey).toBe('science:research');
    expect(window.location.hash).toBe('#plugin/science%3Aresearch');
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Science' })).toHaveAttribute('aria-current', 'page')
    );
  });

  it('opens the signed plugin marketplace as a system entry', async () => {
    render(<ActivityBar />);
    fireEvent.click(screen.getByRole('button', { name: '插件市场' }));

    expect(appState.activeProduct).toBe('plugins');
    expect(window.location.hash).toBe('#plugins');
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '插件市场' })).toHaveAttribute('aria-current', 'page')
    );
  });

  it('keeps settings in the system section', async () => {
    render(<ActivityBar />);
    expect(screen.queryByRole('button', { name: '账户与连接' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: '设置' })).toHaveAttribute('data-activity-tooltip', '设置');
    fireEvent.click(screen.getByRole('button', { name: '设置' }));
    expect(appState.settingsOpen).toBe(true);
    expect(appState.settingsTab).toBe('general');
    await waitFor(() => expect(screen.getByRole('button', { name: '编码' })).toHaveAttribute('aria-current', 'page'));
    expect(screen.getByRole('button', { name: '设置' })).toHaveClass('active');
    expect(screen.getByRole('button', { name: '设置' })).toHaveAttribute('aria-expanded', 'true');
  });

  it('opens Memory as a dedicated Code surface and restores Code tasks', async () => {
    render(<ActivityBar />);

    fireEvent.click(screen.getByRole('button', { name: '记忆' }));
    expect(appState.activeProduct).toBe('code');
    expect(appState.codeSurface).toBe('memory');
    expect(window.location.hash).toBe('#code/memory');
    await waitFor(() => expect(screen.getByRole('button', { name: '记忆' })).toHaveAttribute('aria-current', 'page'));
    expect(screen.getByRole('button', { name: '编码' })).not.toHaveAttribute('aria-current');

    fireEvent.click(screen.getByRole('button', { name: '编码' }));
    expect(appState.codeSurface).toBe('tasks');
    expect(window.location.hash).toBe('#code/conversation');
  });
});
