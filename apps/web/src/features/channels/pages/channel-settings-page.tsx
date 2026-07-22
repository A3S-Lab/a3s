import { MessageCircleMore, MessageSquareText } from 'lucide-react';
import { useId } from 'react';
import { useSnapshot } from 'valtio';
import { Tabs, type TabItem } from '../../../design-system/primitives';
import { appState, navigateSettingsChannel } from '../../../state/app-state';
import type { ChannelSettingsTab } from '../../settings/settings-state';
import { WeixinRemotePage } from '../../weixin-remote/pages/weixin-remote-page';
import type { WeixinRemoteActions } from '../../weixin-remote/use-weixin-remote-controller';
import { FeishuChannelPage } from './feishu-channel-page';

const channels: Array<{ id: ChannelSettingsTab; label: string }> = [
  { id: 'weixin', label: '微信' },
  { id: 'feishu', label: '飞书' },
];

export function ChannelSettingsPage({ weixinActions }: { weixinActions?: WeixinRemoteActions }) {
  const state = useSnapshot(appState);
  const tabListId = useId();
  const items: readonly TabItem<ChannelSettingsTab>[] = channels.map((channel) => ({
    ...channel,
    icon: channel.id === 'weixin' ? <MessageCircleMore size={15} /> : <MessageSquareText size={15} />,
    panelId: `${tabListId}-${channel.id}-panel`,
  }));

  return (
    <section className='channel-settings-page' aria-label='渠道设置'>
      <Tabs
        ariaLabel='渠道'
        value={state.settingsChannel}
        items={items}
        variant='line'
        className='channel-settings-tabs'
        onChange={navigateSettingsChannel}
      />

      <section
        id={`${tabListId}-weixin-panel`}
        className='channel-settings-panel'
        role='tabpanel'
        aria-label='微信'
        hidden={state.settingsChannel !== 'weixin'}
      >
        {weixinActions ? (
          <WeixinRemotePage actions={weixinActions} embedded />
        ) : (
          <p className='channel-settings-controller-unavailable'>微信渠道控制器当前不可用。</p>
        )}
      </section>

      <section
        id={`${tabListId}-feishu-panel`}
        className='channel-settings-panel'
        role='tabpanel'
        aria-label='飞书'
        hidden={state.settingsChannel !== 'feishu'}
      >
        <FeishuChannelPage />
      </section>
    </section>
  );
}
