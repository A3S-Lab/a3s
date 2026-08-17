import { RefreshCw, WifiOff } from 'lucide-react';
import { useEffect } from 'react';
import { useSnapshot } from 'valtio';
import { Button, InlineNotice } from '../design-system/primitives';
import type { CodeActions } from '../features/code/use-code-controller';
import { KnowledgePage } from '../features/knowledge/pages/knowledge-page';
import type { KnowledgeActions } from '../features/knowledge/use-knowledge-controller';
import { MemoryPage } from '../features/memory/pages/memory-page';
import { PluginHostPage } from '../features/plugins/pages/plugin-host-page';
import { PluginMarketplacePage } from '../features/plugins/pages/plugin-marketplace-page';
import type { PluginActions } from '../features/plugins/use-plugin-controller';
import { SettingsDialog } from '../features/settings/components/settings-dialog';
import type { WeixinRemoteActions } from '../features/weixin-remote/use-weixin-remote-controller';
import { WorkProduct } from '../features/work/pages/work-product';
import { appState, syncShellRouteFromLocation } from '../state/app-state';
import { ActivityBar } from './activity-bar';
import { CommandPalette } from './shell/command-palette';

export function AppShell({
  actions,
  pluginActions,
  knowledgeActions,
  weixinActions,
}: {
  actions: CodeActions;
  pluginActions?: PluginActions;
  knowledgeActions?: KnowledgeActions;
  weixinActions?: WeixinRemoteActions;
}) {
  const state = useSnapshot(appState);

  useEffect(() => {
    let compact = isCompactViewport();
    if (compact) appState.sidebarOpen = false;
    const handleResize = () => {
      const nextCompact = isCompactViewport();
      if (nextCompact && !compact) appState.sidebarOpen = false;
      compact = nextCompact;
    };
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    const syncRoute = () => {
      syncShellRouteFromLocation();
    };
    window.addEventListener('popstate', syncRoute);
    window.addEventListener('hashchange', syncRoute);
    return () => {
      window.removeEventListener('popstate', syncRoute);
      window.removeEventListener('hashchange', syncRoute);
    };
  }, []);

  useEffect(() => {
    const sessionId = state.workRoute === 'conversation' ? state.conversationSessionId : null;
    if (
      !sessionId ||
      sessionId === state.activeSessionId ||
      !state.sessions.some((session) => session.sessionId === sessionId)
    ) {
      return;
    }
    void actions.selectSession(sessionId);
  }, [actions.selectSession, state.activeSessionId, state.conversationSessionId, state.sessions, state.workRoute]);

  return (
    <main className='app-shell'>
      <ActivityBar />
      <section className='product-workspace'>
        {state.activeProduct === 'plugin' ? (
          pluginActions ? (
            <PluginHostPage actions={pluginActions} />
          ) : null
        ) : state.activeProduct === 'plugins' ? (
          pluginActions ? (
            <PluginMarketplacePage actions={pluginActions} />
          ) : null
        ) : state.activeProduct === 'knowledge' ? (
          knowledgeActions ? (
            <KnowledgePage actions={knowledgeActions} />
          ) : null
        ) : state.activeProduct === 'memory' ? (
          <MemoryPage actions={actions} />
        ) : (
          <WorkProduct actions={actions} />
        )}
      </section>
      {state.settingsOpen && <SettingsDialog actions={actions} weixinActions={weixinActions} />}
      {state.serviceStatus !== 'connected' && (
        <InlineNotice
          className='service-connection-banner'
          tone='warning'
          role='status'
          icon={<WifiOff size={16} />}
          title={state.serviceStatus === 'checking' ? '正在重新连接…' : '本地服务连接已中断'}
          actions={
            <Button
              loading={state.serviceStatus === 'checking'}
              disabled={state.serviceStatus === 'checking'}
              onClick={() => {
                void actions.retryConnection();
              }}
            >
              <RefreshCw size={13} />
              重新连接
            </Button>
          }
        >
          <span>
            {state.serviceError ? `${state.serviceError} ` : ''}当前页面内容可能已过期；未保存的编辑仍保留在浏览器中。
          </span>
        </InlineNotice>
      )}
      {state.activeProduct === 'work' && state.commandPaletteOpen && <CommandPalette actions={actions} />}
    </main>
  );
}

function isCompactViewport(): boolean {
  return window.innerWidth <= 767;
}
