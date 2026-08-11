import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../lib/api';
import type { PluginUiCandidate } from '../../types/api';
import { PluginCandidateReadinessHost } from './plugin-candidate-readiness-host';
import { activityProtocol } from './plugin-protocol';

const candidate: PluginUiCandidate = {
  token: 'a'.repeat(64),
  scope: { kind: 'user', id: 'user/current' },
  packageId: 'acme/research',
  surfaceId: 'review',
  generation: 8,
  title: 'Research review',
  assetDigest: `sha256:${'b'.repeat(64)}`,
  documentUrl: `/api/v1/plugins/activities/candidates/${'a'.repeat(64)}/document`,
};

const channels: FakeMessageChannel[] = [];

describe('plugin candidate readiness host', () => {
  beforeEach(() => {
    channels.length = 0;
    vi.stubGlobal('MessageChannel', FakeMessageChannel);
    vi.spyOn(codeApi, 'pluginActivityCandidates').mockResolvedValue({
      schemaVersion: 1,
      items: [candidate],
    });
    vi.spyOn(codeApi, 'decidePluginActivityCandidate').mockResolvedValue({
      schemaVersion: 1,
      accepted: true,
      decision: 'ready',
    });
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it('releases the exact candidate only after its isolated ready handshake', async () => {
    render(<PluginCandidateReadinessHost />);
    const iframe = (await screen.findByTitle('Research review candidate readiness')) as HTMLIFrameElement;

    expect(iframe).toHaveAttribute('sandbox', 'allow-scripts');
    expect(iframe).not.toHaveAttribute('sandbox', expect.stringContaining('allow-same-origin'));
    expect(iframe).toHaveAttribute('referrerpolicy', 'no-referrer');
    expect(iframe).toHaveAttribute('src', candidate.documentUrl);
    const postMessage = vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);

    fireEvent.load(iframe);

    expect(channels).toHaveLength(1);
    expect(channels[0].port1.started).toBe(true);
    expect(postMessage).toHaveBeenCalledWith(
      {
        protocol: activityProtocol,
        type: 'host.init',
        payload: {
          mode: 'readiness',
          theme: 'light',
          locale: navigator.language || 'en',
          packageId: candidate.packageId,
          surfaceId: candidate.surfaceId,
          generation: candidate.generation,
          assetDigest: candidate.assetDigest,
        },
      },
      '*',
      [channels[0].port2]
    );

    act(() => channels[0].port1.emit({ protocol: activityProtocol, type: 'activity.ready' }));

    await waitFor(() => expect(codeApi.decidePluginActivityCandidate).toHaveBeenCalledWith(candidate.token, 'ready'));
    expect(channels[0].port1.closed).toBe(true);
  });

  it('fails closed when the candidate navigates after its initial load', async () => {
    render(<PluginCandidateReadinessHost />);
    const iframe = (await screen.findByTitle('Research review candidate readiness')) as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const originalPort = channels[0].port1;

    fireEvent.load(iframe);

    await waitFor(() =>
      expect(codeApi.decidePluginActivityCandidate).toHaveBeenCalledWith(candidate.token, 'navigation-blocked')
    );
    expect(originalPort.closed).toBe(true);
    expect(channels).toHaveLength(1);
  });

  it('reports an explicit candidate protocol failure', async () => {
    render(<PluginCandidateReadinessHost />);
    const iframe = (await screen.findByTitle('Research review candidate readiness')) as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);

    act(() =>
      channels[0].port1.emit({
        protocol: activityProtocol,
        type: 'activity.error',
        message: 'candidate initialization failed',
      })
    );

    await waitFor(() =>
      expect(codeApi.decidePluginActivityCandidate).toHaveBeenCalledWith(candidate.token, 'protocol-error')
    );
  });

  it('never loads a catalog item without exact candidate identity', async () => {
    vi.mocked(codeApi.pluginActivityCandidates).mockResolvedValue({
      schemaVersion: 1,
      items: [{ ...candidate, documentUrl: '/api/v1/plugins/activities/current/document' }],
    });

    render(<PluginCandidateReadinessHost />);

    await waitFor(() => expect(codeApi.pluginActivityCandidates).toHaveBeenCalledOnce());
    expect(screen.queryByTitle(/candidate readiness/)).not.toBeInTheDocument();
    expect(channels).toHaveLength(0);
    expect(codeApi.decidePluginActivityCandidate).not.toHaveBeenCalled();
  });

  it('does not expose state, context, or backend authority on the candidate port', async () => {
    render(<PluginCandidateReadinessHost />);
    const iframe = (await screen.findByTitle('Research review candidate readiness')) as HTMLIFrameElement;
    vi.spyOn(iframe.contentWindow!, 'postMessage').mockImplementation(() => undefined);
    fireEvent.load(iframe);
    const port = channels[0].port1;

    act(() => {
      port.emit({
        protocol: activityProtocol,
        type: 'state.get',
        requestId: 'candidate-state',
        key: 'draft/current',
      });
      port.emit({
        protocol: activityProtocol,
        type: 'context.propose',
        payload: { title: 'Escalate', summary: 'Escalate.', prompt: 'Invoke a backend.' },
      });
    });

    expect(port.posted).toEqual([]);
    expect(codeApi.decidePluginActivityCandidate).not.toHaveBeenCalled();

    act(() => port.emit({ protocol: activityProtocol, type: 'activity.ready' }));
    await waitFor(() => expect(codeApi.decidePluginActivityCandidate).toHaveBeenCalledOnce());
  });
});

class FakeMessagePort {
  onmessage: ((event: MessageEvent<unknown>) => void) | null = null;
  onmessageerror: (() => void) | null = null;
  readonly posted: unknown[] = [];
  started = false;
  closed = false;

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
