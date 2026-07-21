import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { PluginActivityItem } from '../../../types/api';
import { activityProtocol } from '../plugin-protocol';
import type { PluginActions } from '../use-plugin-controller';
import { PluginHostPage } from './plugin-host-page';

const contribution: PluginActivityItem = {
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

describe('plugin host page', () => {
  beforeEach(() => {
    appState.activeProduct = 'plugin';
    appState.activePluginKey = contribution.key;
    appState.pluginCatalog = {
      schemaVersion: 1,
      available: true,
      generation: 2,
      revision: 'b'.repeat(64),
      items: [contribution],
    };
    appState.pluginContentByKey = {
      [contribution.key]: {
        key: contribution.key,
        packageId: contribution.packageId,
        skill: contribution.skill,
        registryRevision: 'b'.repeat(64),
        sha256: contribution.sha256,
        mediaType: 'text/html',
        html: '<!doctype html><html><head><title>Science</title></head><body><script>void 0</script></body></html>',
      },
    };
    appState.pluginContentStatus = 'ready';
    appState.pluginContentError = null;
    appState.pluginRuntimeError = null;
    appState.pluginContextProposal = null;
  });

  afterEach(() => cleanup());

  it('renders package HTML in a script-only opaque-origin sandbox with host CSP', () => {
    const actions = createPluginActions();
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('Science 插件内容');

    expect(iframe).toHaveAttribute('sandbox', 'allow-scripts');
    expect(iframe).not.toHaveAttribute('sandbox', expect.stringContaining('allow-same-origin'));
    expect(iframe).toHaveAttribute('referrerpolicy', 'no-referrer');
    expect(iframe.getAttribute('srcdoc')).toContain('Content-Security-Policy');
    expect(iframe.getAttribute('srcdoc')).toContain("connect-src 'none'");
    expect(actions.loadActivityContent).toHaveBeenCalledWith(contribution.key);
  });

  it('accepts bounded proposals only from the active iframe and presents a host-owned review', async () => {
    const actions = createPluginActions();
    actions.proposeContext.mockImplementation((proposal) => {
      appState.pluginContextProposal = proposal;
    });
    render(<PluginHostPage actions={actions} />);
    const iframe = screen.getByTitle('Science 插件内容') as HTMLIFrameElement;

    window.dispatchEvent(
      new MessageEvent('message', {
        source: iframe.contentWindow,
        data: {
          protocol: activityProtocol,
          type: 'context.propose',
          payload: {
            title: 'Review research context',
            summary: 'Compare CRISPR evidence.',
            prompt: 'Compare the selected sources.',
            fields: [{ label: 'Source', value: 'PubMed' }],
          },
        },
      })
    );

    await waitFor(() => expect(screen.getByRole('dialog', { name: 'Review research context' })).toBeInTheDocument());
    expect(screen.getByText('a3s-use-science')).toBeInTheDocument();
    expect(screen.getByText('Compare the selected sources.')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '在 Code 中使用' }));
    expect(actions.acceptContextProposal).toHaveBeenCalledOnce();
  });
});

function createPluginActions() {
  return {
    refreshActivities: vi.fn(async () => undefined),
    loadActivityContent: vi.fn(async () => undefined),
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
