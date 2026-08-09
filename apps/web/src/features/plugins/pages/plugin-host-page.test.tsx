import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { PluginActivityItem } from '../../../types/api';
import { activityProtocol } from '../plugin-protocol';
import type { PluginActions } from '../use-plugin-controller';
import { PluginHostPage } from './plugin-host-page';

const initialRevision = 'b'.repeat(64);
const initialDocumentUrl = activityDocumentUrl(2, initialRevision);
const contribution: PluginActivityItem = {
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
  documentUrl: initialDocumentUrl,
};

const channels: FakeMessageChannel[] = [];

describe('plugin host page', () => {
  beforeEach(() => {
    channels.length = 0;
    vi.stubGlobal('MessageChannel', FakeMessageChannel);
    appState.activeProduct = 'plugin';
    appState.activePluginKey = contribution.key;
    appState.pluginCatalog = {
      schemaVersion: 1,
      available: true,
      generation: 2,
      revision: initialRevision,
      items: [contribution],
    };
    appState.pluginRuntimeError = null;
    appState.pluginContextProposal = null;
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('loads only the generation-bound server document and opens a dedicated MessagePort', async () => {
    const actions = createPluginActions();
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;

    expect(iframe).toHaveAttribute('sandbox', 'allow-scripts');
    expect(iframe).not.toHaveAttribute('sandbox', expect.stringContaining('allow-same-origin'));
    expect(iframe).toHaveAttribute('referrerpolicy', 'no-referrer');
    expect(iframe).toHaveAttribute('data-status', 'loading');
    expect(iframe).toHaveAttribute('src', initialDocumentUrl);
    expect(iframe).not.toHaveAttribute('srcdoc');

    const postMessage = vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);

    expect(channels).toHaveLength(1);
    expect(channels[0].port1.started).toBe(true);
    expect(postMessage).toHaveBeenCalledWith(
      {
        protocol: 'a3s.activity.v3',
        type: 'host.init',
        payload: {
          theme: 'light',
          locale: navigator.language || 'zh-CN',
          packageId: contribution.packageId,
          key: contribution.key,
          generation: 2,
          revision: initialRevision,
        },
      },
      '*',
      [channels[0].port2]
    );
    expect(screen.getByText('正在启动隔离视图…')).toBeInTheDocument();

    act(() => channels[0].port1.emit({ protocol: activityProtocol, type: 'activity.ready' }));
    await waitFor(() => expect(screen.queryByText('正在启动隔离视图…')).not.toBeInTheDocument());
    expect(iframe).toHaveAttribute('data-status', 'ready');
  });

  it('ignores ambient window messages and accepts bounded proposals only through the active port', async () => {
    const actions = createPluginActions();
    actions.proposeContext.mockImplementation((proposal) => {
      appState.pluginContextProposal = proposal;
    });
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const proposal = {
      protocol: activityProtocol,
      type: 'context.propose',
      payload: {
        title: 'Review research context',
        summary: 'Compare CRISPR evidence.',
        prompt: 'Compare the selected sources.',
        fields: [{ label: 'Source', value: 'PubMed' }],
        usePackageSkill: true,
      },
    };

    window.dispatchEvent(new MessageEvent('message', { source: iframe.contentWindow, data: proposal }));
    expect(actions.proposeContext).not.toHaveBeenCalled();

    act(() => channels[0].port1.emit(proposal));
    await waitFor(() => expect(screen.getByRole('dialog', { name: 'Review research context' })).toBeInTheDocument());
    expect(actions.proposeContext).toHaveBeenCalledWith(
      expect.objectContaining({
        sourceKey: contribution.key,
        sourceGeneration: 2,
        sourceRevision: initialRevision,
        sourceDocumentUrl: initialDocumentUrl,
      })
    );
    expect(screen.getByText('a3s-use-science')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '在当前会话中使用' }));
    expect(actions.acceptContextProposal).toHaveBeenCalledOnce();
  });

  it('closes the capability when the document misses its ready deadline', () => {
    vi.useFakeTimers();
    const actions = createPluginActions();
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);

    act(() => vi.advanceTimersByTime(10_000));

    expect(channels[0].port1.closed).toBe(true);
    expect(screen.getByRole('alert')).toHaveTextContent('插件未在 10 秒内完成初始化');
  });

  it('terminates the frame after self-navigation and never accepts its old port again', async () => {
    const actions = createPluginActions();
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const oldPort = channels[0].port1;

    fireEvent.load(iframe);

    await waitFor(() => expect(screen.queryByTitle('科研 插件内容')).not.toBeInTheDocument());
    expect(screen.getByRole('alert')).toHaveTextContent('插件尝试离开已校验文档');
    expect(oldPort.closed).toBe(true);
    act(() =>
      oldPort.emit({
        protocol: activityProtocol,
        type: 'context.propose',
        payload: { title: 'Stale', summary: 'Stale.', prompt: 'Do not accept.' },
      })
    );
    expect(actions.proposeContext).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: '重新启动视图' }));
    const replacement = await screen.findByTitle('科研 插件内容');
    expect(replacement).not.toBe(iframe);
  });

  it('drains the prior port and replaces the iframe when the Registry generation changes', async () => {
    const actions = createPluginActions();
    render(<PluginHostPage actions={actions} />);
    const firstFrame = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(firstFrame.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(firstFrame);
    const oldPort = channels[0].port1;
    const nextRevision = 'c'.repeat(64);
    const nextDocumentUrl = activityDocumentUrl(3, nextRevision);

    act(() => {
      appState.pluginCatalog = {
        ...appState.pluginCatalog,
        generation: 3,
        revision: nextRevision,
        items: [
          {
            ...contribution,
            version: '2.0.0',
            sha256: 'd'.repeat(64),
            documentUrl: nextDocumentUrl,
          },
        ],
      };
    });

    await waitFor(() => expect(screen.getByTitle('科研 插件内容')).not.toBe(firstFrame));
    expect(screen.getByTitle('科研 插件内容')).toHaveAttribute('src', nextDocumentUrl);
    expect(oldPort.closed).toBe(true);
    act(() => oldPort.emit({ protocol: activityProtocol, type: 'activity.error', message: 'stale error' }));
    expect(appState.pluginRuntimeError).toBeNull();
  });

  it('serializes state mutations and returns correlated v3 results on the active port', async () => {
    const firstResponse = deferred<Response>();
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockImplementationOnce(() => firstResponse.promise)
      .mockResolvedValueOnce(
        jsonResponse({ schemaVersion: 1, operation: 'get', found: true, value: { query: 'CRISPR' } })
      );
    vi.stubGlobal('fetch', fetchMock);
    render(<PluginHostPage actions={createPluginActions()} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const port = channels[0].port1;

    act(() => {
      port.emit({
        protocol: activityProtocol,
        type: 'state.set',
        requestId: 'set-1',
        key: 'draft/current',
        value: { query: 'CRISPR' },
      });
      port.emit({
        protocol: activityProtocol,
        type: 'state.get',
        requestId: 'get-1',
        key: 'draft/current',
      });
    });

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(1));
    expect(fetchMock.mock.calls[0][0]).toBe(
      `/api/v1/plugins/activities/science%3Aresearch/state?generation=2&revision=${initialRevision}`
    );
    expect(JSON.parse(String(fetchMock.mock.calls[0][1]?.body))).toEqual({
      operation: 'set',
      key: 'draft/current',
      value: { query: 'CRISPR' },
    });
    firstResponse.resolve(jsonResponse({ schemaVersion: 1, operation: 'set', stored: true }));

    await waitFor(() => expect(fetchMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(port.posted).toHaveLength(2));
    expect(port.posted).toEqual([
      {
        protocol: activityProtocol,
        type: 'state.result',
        requestId: 'set-1',
        payload: { schemaVersion: 1, operation: 'set', stored: true },
      },
      {
        protocol: activityProtocol,
        type: 'state.result',
        requestId: 'get-1',
        payload: { schemaVersion: 1, operation: 'get', found: true, value: { query: 'CRISPR' } },
      },
    ]);
  });

  it('bounds the state queue and aborts the active request when its generation is retired', async () => {
    let activeSignal: AbortSignal | null | undefined;
    const fetchMock = vi.fn<typeof fetch>().mockImplementation(
      (_input, init) =>
        new Promise<Response>((_resolve, reject) => {
          activeSignal = init?.signal;
          activeSignal?.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')), {
            once: true,
          });
        })
    );
    vi.stubGlobal('fetch', fetchMock);
    render(<PluginHostPage actions={createPluginActions()} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const port = channels[0].port1;

    act(() => {
      for (let index = 0; index < 33; index += 1) {
        port.emit({
          protocol: activityProtocol,
          type: 'state.get',
          requestId: `get-${index}`,
          key: 'draft/current',
        });
      }
    });

    await waitFor(() => expect(fetchMock).toHaveBeenCalledOnce());
    expect(port.posted).toEqual([
      {
        protocol: activityProtocol,
        type: 'state.error',
        requestId: 'get-32',
        code: 'busy',
        message: '插件状态请求队列已满。',
      },
    ]);

    const nextRevision = 'c'.repeat(64);
    act(() => {
      appState.pluginCatalog = {
        ...appState.pluginCatalog,
        generation: 3,
        revision: nextRevision,
        items: [
          {
            ...contribution,
            version: '2.0.0',
            sha256: 'd'.repeat(64),
            documentUrl: activityDocumentUrl(3, nextRevision),
          },
        ],
      };
    });

    await waitFor(() => expect(port.closed).toBe(true));
    expect(activeSignal?.aborted).toBe(true);
    expect(port.posted).toHaveLength(1);
  });

  it('reports bounded state failures but silently drains a server-rejected stale port', async () => {
    const fetchMock = vi
      .fn<typeof fetch>()
      .mockResolvedValueOnce(errorResponse(503, 'Registry is converging.'))
      .mockResolvedValueOnce(errorResponse(410, 'Generation is stale.'));
    vi.stubGlobal('fetch', fetchMock);
    const actions = createPluginActions();
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('科研 插件内容') as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const port = channels[0].port1;

    act(() => port.emit({ protocol: activityProtocol, type: 'state.get', requestId: 'get-1', key: 'draft/current' }));
    await waitFor(() => expect(port.posted).toHaveLength(1));
    expect(port.posted[0]).toEqual({
      protocol: activityProtocol,
      type: 'state.error',
      requestId: 'get-1',
      code: 'unavailable',
      message: 'Registry is converging.',
    });

    act(() => port.emit({ protocol: activityProtocol, type: 'state.get', requestId: 'get-2', key: 'draft/current' }));
    await waitFor(() => expect(port.closed).toBe(true));
    expect(port.posted).toHaveLength(1);
    expect(actions.refreshActivities).toHaveBeenCalledOnce();
  });
});

function activityDocumentUrl(generation: number, revision: string): string {
  return `/api/v1/plugins/activities/science%3Aresearch/document?generation=${generation}&revision=${revision}`;
}

function createPluginActions() {
  return {
    refreshActivities: vi.fn(async () => undefined),
    refreshMarketplace: vi.fn(async () => undefined),
    planOperation: vi.fn(async () => undefined),
    applyReviewedOperation: vi.fn(async () => undefined),
    dismissOperationReview: vi.fn(),
    setPackageEnabled: vi.fn(async () => undefined),
    proposeContext: vi.fn(),
    dismissContextProposal: vi.fn(),
    acceptContextProposal: vi.fn(),
  } satisfies PluginActions;
}

class FakeMessagePort {
  onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
  onmessageerror: (() => void) | null = null;
  started = false;
  closed = false;
  posted: unknown[] = [];

  start() {
    this.started = true;
  }

  close() {
    this.closed = true;
    this.onmessage = null;
    this.onmessageerror = null;
  }

  emit(data: unknown) {
    if (!this.closed) this.onmessage?.({ data } as MessageEvent<unknown>);
  }

  postMessage(data: unknown) {
    if (this.closed) throw new Error('port is closed');
    this.posted.push(data);
  }
}

class FakeMessageChannel {
  readonly port1 = new FakeMessagePort();
  readonly port2 = new FakeMessagePort();

  constructor() {
    channels.push(this);
  }
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

function jsonResponse(data: unknown): Response {
  return new Response(JSON.stringify({ code: 200, data }), {
    status: 200,
    headers: { 'Content-Type': 'application/json' },
  });
}

function errorResponse(status: number, message: string): Response {
  return new Response(JSON.stringify({ code: status, message }), {
    status,
    headers: { 'Content-Type': 'application/json' },
  });
}
