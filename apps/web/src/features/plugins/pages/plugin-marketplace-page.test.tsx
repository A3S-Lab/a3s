import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { PluginMarketplaceItem } from '../../../types/api';
import type { PluginActions } from '../use-plugin-controller';
import { PluginMarketplacePage } from './plugin-marketplace-page';

const marketplaceItem: PluginMarketplaceItem = {
  componentId: 'use/a3s/science',
  packageId: 'a3s/science',
  displayName: 'Science',
  registryName: 'a3s',
  registryUrl: 'https://packages.a3s.dev',
  version: '1.2.3',
  channel: 'stable',
  target: 'darwin-arm64',
  archiveName: 'science.tar.gz',
  length: 2048,
  sha256: 'a'.repeat(64),
  signedPlanDigest: 'b'.repeat(64),
  installed: false,
  enabled: false,
};

describe('plugin marketplace page', () => {
  beforeEach(() => {
    appState.pluginMarketplaceStatus = 'ready';
    appState.pluginMarketplaceError = null;
    appState.pluginOperationStatus = 'idle';
    appState.pluginOperationError = null;
    appState.pluginOperationReview = null;
    appState.pluginMarketplace = {
      schemaVersion: 1,
      verifiedAt: '2026-07-21T00:00:00Z',
      registries: [
        {
          name: 'a3s',
          url: 'https://packages.a3s.dev',
          configured: true,
          verified: true,
          metadata: {
            rootVersion: 1,
            timestampVersion: 2,
            snapshotVersion: 3,
            targetsVersion: 4,
            packageTargets: 1,
          },
        },
      ],
      items: [marketplaceItem],
    };
  });

  afterEach(() => cleanup());

  it('shows verified registry provenance and plans an exact signed package install', () => {
    const actions = createPluginActions();
    render(<PluginMarketplacePage actions={actions} />);

    expect(screen.getByText('TUF 元数据已验证')).toBeInTheDocument();
    expect(screen.getByText('SHA-256 aaaaaaaaaaaa…')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '审核并安装' }));
    expect(actions.planOperation).toHaveBeenCalledWith({
      action: 'install',
      componentId: 'use/a3s/science',
      version: '1.2.3',
      channel: 'stable',
    });
  });

  it('requires a second explicit confirmation for the digest-bound dry-run plan', async () => {
    const actions = createPluginActions();
    appState.pluginOperationReview = {
      request: { action: 'install', componentId: 'use/a3s/science', version: '1.2.3', channel: 'stable' },
      plan: {
        dryRun: true,
        planSchemaVersion: 1,
        planCommand: 'a3s install use/a3s/science --dry-run',
        planDigest: 'c'.repeat(64),
        plans: [
          {
            component: 'use/a3s/science',
            action: 'install',
            source: 'registry:a3s',
            mutates: true,
            message: 'Install signed Science package.',
          },
        ],
      },
    };
    render(<PluginMarketplacePage actions={actions} />);

    expect(screen.getByRole('dialog', { name: '审核插件安装计划' })).toBeInTheDocument();
    expect(screen.getByText('c'.repeat(64))).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '确认安装' }));
    await waitFor(() => expect(actions.applyReviewedOperation).toHaveBeenCalledOnce());
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
