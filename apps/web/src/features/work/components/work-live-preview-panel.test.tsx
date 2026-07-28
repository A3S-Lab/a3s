import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../../lib/api';
import type { PreviewDescriptor, WorkspaceChangeEvent } from '../../../types/api';
import { WorkLivePreviewPanel } from './work-live-preview-panel';

describe('Work live preview panel', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('isolates static sites and reloads them after debounced workspace changes', async () => {
    let onChange: ((event: WorkspaceChangeEvent) => void) | undefined;
    const closeWatch = vi.fn();
    vi.spyOn(codeApi, 'createPreview').mockResolvedValue(staticPreview());
    vi.spyOn(codeApi, 'stopPreview').mockResolvedValue({ id: 'static-preview', stopped: true });
    vi.spyOn(codeApi, 'watchWorkspace').mockImplementation((_root, listener) => {
      onChange = listener;
      return closeWatch;
    });

    const view = render(
      <WorkLivePreviewPanel
        target='/repo/site/index.html'
        width={620}
        onWidthChange={vi.fn()}
        onTargetChange={vi.fn()}
        onClose={vi.fn()}
      />
    );

    const initialFrame = await screen.findByTitle('index.html 实时预览');
    expect(initialFrame).toHaveAttribute('src', '/preview/static-preview/');
    expect(initialFrame.getAttribute('sandbox')).toContain('allow-scripts');
    expect(initialFrame.getAttribute('sandbox')).not.toContain('allow-same-origin');
    fireEvent.load(initialFrame);
    expect(screen.getByText('实时同步中')).toBeInTheDocument();

    act(() => {
      onChange?.({ type: 'workspace_change', kind: 'modify', paths: ['/repo/site/app.js'] });
    });
    expect(screen.getByText('检测到文件变化')).toBeInTheDocument();
    await waitFor(() => expect(screen.getByText(/1 次更新/)).toBeInTheDocument(), { timeout: 1000 });
    expect(screen.getByTitle('index.html 实时预览')).not.toBe(initialFrame);

    view.unmount();
    expect(closeWatch).toHaveBeenCalledTimes(1);
    expect(codeApi.stopPreview).toHaveBeenCalledWith('static-preview');
  });

  it('keeps a different-port local app functional and normalizes address submissions', async () => {
    vi.spyOn(codeApi, 'createPreview').mockResolvedValue(localUrlPreview());
    vi.spyOn(codeApi, 'stopPreview').mockResolvedValue({ id: 'url-preview', stopped: true });
    const onTargetChange = vi.fn();

    render(
      <WorkLivePreviewPanel
        target='localhost:4173/'
        width={620}
        onWidthChange={vi.fn()}
        onTargetChange={onTargetChange}
        onClose={vi.fn()}
      />
    );

    const frame = await screen.findByTitle('localhost 实时预览');
    expect(codeApi.createPreview).toHaveBeenCalledWith('http://localhost:4173/');
    expect(frame.getAttribute('sandbox')).toContain('allow-same-origin');
    const address = screen.getByRole('textbox', { name: '预览文件路径或本地地址' });
    fireEvent.change(address, { target: { value: 'localhost:5173/dashboard' } });
    fireEvent.submit(address.closest('form')!);
    expect(onTargetChange).toHaveBeenCalledWith('http://localhost:5173/dashboard');
  });
});

function staticPreview(): PreviewDescriptor {
  return {
    id: 'static-preview',
    kind: 'staticSite',
    title: 'index.html',
    source: {
      type: 'path',
      path: '/repo/site/index.html',
      rootPath: '/repo/site',
      name: 'index.html',
      size: 120,
      mtimeMs: 1,
      isDirectory: false,
      isBinary: false,
    },
    contentUrl: '/preview/static-preview/',
    watchRoot: '/repo/site',
    createdAt: 1,
    expiresAt: 2,
    capabilities: { liveReload: true, responsive: true, navigation: true, openExternal: true },
  };
}

function localUrlPreview(): PreviewDescriptor {
  return {
    id: 'url-preview',
    kind: 'localUrl',
    title: 'localhost',
    source: { type: 'url', url: 'http://localhost:4173/' },
    contentUrl: 'http://localhost:4173/',
    watchRoot: null,
    createdAt: 1,
    expiresAt: 2,
    capabilities: { liveReload: false, responsive: true, navigation: true, openExternal: true },
  };
}
