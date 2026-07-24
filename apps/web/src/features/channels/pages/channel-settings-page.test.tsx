import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { WeixinRemoteActions } from '../../weixin-remote/use-weixin-remote-controller';
import { createWeixinRemoteState } from '../../weixin-remote/weixin-remote-state';
import { ChannelSettingsPage } from './channel-settings-page';

const weixinActions: WeixinRemoteActions = {
  refresh: vi.fn(async () => undefined),
  refreshTargets: vi.fn(async () => undefined),
  startLogin: vi.fn(async () => true),
  submitVerification: vi.fn(async () => true),
  retryLoginPolling: vi.fn(),
  cancelLogin: vi.fn(async () => undefined),
  dismissLogin: vi.fn(),
  pause: vi.fn(async () => undefined),
  resume: vi.fn(async () => undefined),
  disconnect: vi.fn(async () => undefined),
};

describe('ChannelSettingsPage', () => {
  beforeEach(() => {
    Object.assign(appState, createWeixinRemoteState(), {
      settingsOpen: true,
      settingsTab: 'channels',
      settingsChannel: 'weixin',
    });
    appState.weixinCapabilityStatus = 'unavailable';
    appState.weixinCapability = {
      schemaVersion: 1,
      state: 'unavailable',
      protocolMode: 'disabled',
      productionEntitled: false,
      supportedScopes: [],
      releaseBlockers: [{ code: 'ilink_entitlement_missing', message: 'Entitlement required.' }],
    };
    window.history.replaceState(null, '', '#settings/channels/weixin');
  });

  afterEach(() => {
    cleanup();
    Object.assign(appState, createWeixinRemoteState(), {
      settingsOpen: false,
      settingsTab: 'general',
      settingsChannel: 'weixin',
    });
    window.history.replaceState(null, '', '#code/conversation');
  });

  it('switches the channel workspace without changing the outer Settings page', async () => {
    const { container } = render(<ChannelSettingsPage weixinActions={weixinActions} />);

    expect(screen.getByRole('button', { name: '微信' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('heading', { name: '微信渠道尚未就绪' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: '微信' }).parentElement).toHaveClass('channel-settings-page');
    expect(container.querySelector('.channel-provider-workspace')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '飞书' }));

    expect(appState.settingsTab).toBe('channels');
    expect(appState.settingsChannel).toBe('feishu');
    expect(window.location.hash).toBe('#settings/channels/feishu');
    await waitFor(() => expect(screen.getByRole('button', { name: '飞书' })).toHaveAttribute('aria-pressed', 'true'));
    expect(screen.getByRole('heading', { name: '飞书渠道' })).toBeInTheDocument();
    expect(screen.getByText('敬请期待')).toBeInTheDocument();

    fireEvent.keyDown(screen.getByRole('button', { name: '飞书' }), { key: 'ArrowLeft' });

    expect(appState.settingsTab).toBe('channels');
    expect(appState.settingsChannel).toBe('weixin');
    expect(window.location.hash).toBe('#settings/channels/weixin');
    await waitFor(() => expect(screen.getByRole('button', { name: '微信' })).toHaveAttribute('aria-pressed', 'true'));
    expect(screen.getByRole('button', { name: '微信' })).toHaveFocus();
  });
});
