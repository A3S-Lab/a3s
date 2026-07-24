import { AlertTriangle, MessageCircleMore, MessageSquareText } from 'lucide-react';
import { type KeyboardEvent, useRef } from 'react';
import { useSnapshot } from 'valtio';
import { StateView } from '../../../design-system/primitives';
import { appState, navigateSettingsChannel } from '../../../state/app-state';
import type { ChannelSettingsTab } from '../../settings/settings-state';
import { WeixinRemotePage } from '../../weixin-remote/pages/weixin-remote-page';
import type { WeixinRemoteActions } from '../../weixin-remote/use-weixin-remote-controller';
import { FeishuChannelPage } from './feishu-channel-page';

const channels = [
  { id: 'weixin', label: '微信', description: '扫码绑定本机微信' },
  { id: 'feishu', label: '飞书', description: '开放平台应用接入' },
] as const satisfies ReadonlyArray<{ id: ChannelSettingsTab; label: string; description: string }>;

export function ChannelSettingsPage({ weixinActions }: { weixinActions?: WeixinRemoteActions }) {
  const state = useSnapshot(appState);
  const channelButtons = useRef<Array<HTMLButtonElement | null>>([]);
  const weixinStatus = channelStatusLabel(state);

  const selectChannel = (channel: ChannelSettingsTab) => {
    navigateSettingsChannel(channel);
  };
  const handleChannelKeyDown = (index: number, event: KeyboardEvent<HTMLButtonElement>) => {
    if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault();
    const step = event.key === 'ArrowRight' || event.key === 'ArrowDown' ? 1 : -1;
    const nextIndex = (index + step + channels.length) % channels.length;
    selectChannel(channels[nextIndex].id);
    channelButtons.current[nextIndex]?.focus();
  };

  return (
    <section className='channel-settings-page' aria-label='渠道设置'>
      <aside className='channel-provider-list' aria-label='渠道列表'>
        <header>
          <strong>渠道</strong>
          <span>选择一个渠道进行连接和管理。</span>
        </header>
        <nav aria-label='可用渠道'>
          {channels.map((channel, index) => {
            const selected = state.settingsChannel === channel.id;
            const Icon = channel.id === 'weixin' ? MessageCircleMore : MessageSquareText;
            return (
              <button
                ref={(element) => {
                  channelButtons.current[index] = element;
                }}
                type='button'
                className={selected ? 'selected' : ''}
                aria-label={channel.label}
                aria-current={selected ? 'page' : undefined}
                aria-pressed={selected}
                onClick={() => selectChannel(channel.id)}
                onKeyDown={(event) => handleChannelKeyDown(index, event)}
                key={channel.id}
              >
                <span className='channel-provider-icon' aria-hidden='true'>
                  <Icon size={15} />
                </span>
                <span className='channel-provider-copy'>
                  <strong>{channel.label}</strong>
                  <small>{channel.id === 'weixin' ? weixinStatus : '即将支持'}</small>
                </span>
              </button>
            );
          })}
        </nav>
      </aside>

      <section className='channel-settings-panel' aria-label='微信' hidden={state.settingsChannel !== 'weixin'}>
        {weixinActions ? (
          <WeixinRemotePage actions={weixinActions} embedded />
        ) : (
          <StateView
            className='channel-settings-controller-unavailable'
            size='compact'
            tone='danger'
            role='alert'
            icon={<AlertTriangle size={20} />}
            title='微信渠道控制器当前不可用'
            description='请重新连接本机 A3S Boot 后再试。'
          />
        )}
      </section>

      <section className='channel-settings-panel' aria-label='飞书' hidden={state.settingsChannel !== 'feishu'}>
        <FeishuChannelPage />
      </section>
    </section>
  );
}

function channelStatusLabel(state: {
  weixinCapabilityStatus: string;
  weixinAccount?: { bound?: boolean } | null;
}): string {
  if (state.weixinCapabilityStatus === 'loading') return '检查中';
  if (state.weixinCapabilityStatus === 'error' || state.weixinCapabilityStatus === 'unavailable') return '未就绪';
  if (state.weixinAccount?.bound) return '已连接';
  if (state.weixinCapabilityStatus === 'ready') return '未连接';
  return '未检查';
}
