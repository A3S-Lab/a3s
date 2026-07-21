import { useSnapshot } from 'valtio';
import type { CodeActions } from '../features/code/use-code-controller';
import { appState } from '../state/app-state';
import { ActivityBar } from './activity-bar';
import { CommandPalette } from './shell/command-palette';
import { TaskLibrary } from '../features/tasks/components/task-library';
import { TasksPage } from '../features/tasks/pages/tasks-page';
import { SettingsDialog } from '../features/settings/components/settings-dialog';
import { RefreshCw, WifiOff } from 'lucide-react';
import { Button } from '../design-system/primitives';
import { MemoryPage } from '../features/memory/pages/memory-page';
import { WorkProduct } from '../features/work/pages/work-product';
import type { PluginActions } from '../features/plugins/use-plugin-controller';
import { PluginHostPage } from '../features/plugins/pages/plugin-host-page';
import { PluginMarketplacePage } from '../features/plugins/pages/plugin-marketplace-page';

export function AppShell({ actions, pluginActions }: { actions: CodeActions; pluginActions?: PluginActions }) {
  const state = useSnapshot(appState);

  return (
    <main className='app-shell'>
      <ActivityBar />
      {state.activeProduct === 'code' && state.codeSurface === 'tasks' && state.sidebarOpen && (
        <TaskLibrary actions={actions} />
      )}
      <section className='product-workspace'>
        {state.activeProduct === 'plugin' ? (
          pluginActions ? (
            <PluginHostPage actions={pluginActions} />
          ) : null
        ) : state.activeProduct === 'plugins' ? (
          pluginActions ? (
            <PluginMarketplacePage actions={pluginActions} />
          ) : null
        ) : state.activeProduct === 'work' ? (
          <WorkProduct actions={actions} />
        ) : state.codeSurface === 'memory' ? (
          <MemoryPage actions={actions} />
        ) : (
          <TasksPage actions={actions} />
        )}
      </section>
      {state.settingsOpen && <SettingsDialog actions={actions} />}
      {state.serviceStatus !== 'connected' && (
        <output className='service-connection-banner' aria-live='polite'>
          <WifiOff size={16} />
          <div>
            <strong>{state.serviceStatus === 'checking' ? '正在重新连接…' : '本地服务连接已中断'}</strong>
            <span>
              {state.serviceError ? `${state.serviceError} ` : ''}当前页面内容可能已过期；未保存的编辑仍保留在浏览器中。
            </span>
          </div>
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
        </output>
      )}
      {state.activeProduct === 'code' && state.commandPaletteOpen && <CommandPalette actions={actions} />}
    </main>
  );
}
