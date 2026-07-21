import {
  AlertTriangle,
  CheckCircle2,
  FlaskConical,
  PackageCheck,
  Plus,
  Puzzle,
  RefreshCw,
  Search,
  ShieldCheck,
  Trash2,
  Upload,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useSnapshot } from 'valtio';
import { Button, Dialog, IconButton } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import type { PluginMarketplaceItem, PluginOperationAction, PluginOperationRequest } from '../../../types/api';
import type { PluginActions } from '../use-plugin-controller';

type MarketplaceSection = 'catalog' | 'sources';
type CatalogView = 'all' | 'installed';
type ChannelFilter = 'all' | PluginMarketplaceItem['channel'];

export function PluginMarketplacePage({ actions }: { actions: PluginActions }) {
  const state = useSnapshot(appState);
  const [section, setSection] = useState<MarketplaceSection>('catalog');
  const [view, setView] = useState<CatalogView>('all');
  const [channel, setChannel] = useState<ChannelFilter>('all');
  const [query, setQuery] = useState('');

  useEffect(() => {
    if (appState.pluginMarketplaceStatus === 'idle') void actions.refreshMarketplace();
  }, [actions.refreshMarketplace]);

  const items = useMemo(
    () => (state.pluginMarketplace?.items ?? []).map((item) => item as PluginMarketplaceItem),
    [state.pluginMarketplace?.items]
  );
  const installedCount = items.filter((item) => item.installed).length;
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const filteredItems = items.filter((item) => {
    if (view === 'installed' && !item.installed) return false;
    if (channel !== 'all' && item.channel !== channel) return false;
    return [item.displayName, item.componentId, item.registryName, item.channel]
      .join(' ')
      .toLocaleLowerCase()
      .includes(normalizedQuery);
  });
  const busy = state.pluginOperationStatus === 'loading';

  const showInstalled = () => {
    setSection('catalog');
    setView('installed');
    setChannel('all');
    setQuery('');
  };

  return (
    <section className='plugin-marketplace-page' aria-label='插件市场'>
      <header className='plugin-marketplace-toolbar'>
        <nav className='plugin-marketplace-sections' aria-label='插件市场区域'>
          <button type='button' aria-pressed={section === 'catalog'} onClick={() => setSection('catalog')}>
            <Puzzle size={14} /> 插件
          </button>
          <button type='button' aria-pressed={section === 'sources'} onClick={() => setSection('sources')}>
            <ShieldCheck size={14} /> 来源
          </button>
        </nav>
        <div className='plugin-marketplace-tools'>
          {section === 'catalog' && (
            <label className='plugin-search-field'>
              <Search size={14} />
              <span className='sr-only'>搜索插件</span>
              <input value={query} placeholder='搜索插件' onChange={(event) => setQuery(event.target.value)} />
            </label>
          )}
          <button type='button' className='plugin-installed-filter' onClick={showInstalled}>
            <PackageCheck size={14} /> 已安装 <span>{installedCount}</span>
          </button>
          <Button
            loading={state.pluginMarketplaceStatus === 'loading'}
            onClick={() => void actions.refreshMarketplace()}
          >
            <RefreshCw size={14} /> 刷新
          </Button>
        </div>
      </header>

      <div className='plugin-marketplace-scroll'>
        <MarketplaceMessages />

        {section === 'sources' ? (
          <RegistryCatalog />
        ) : (
          <>
            <section className='plugin-directory-section' aria-labelledby='plugin-directory-title'>
              <h2 className='sr-only' id='plugin-directory-title'>
                插件目录
              </h2>
              <div className='plugin-directory-tabs' role='tablist' aria-label='插件目录视图'>
                <button
                  id='plugin-view-all'
                  type='button'
                  role='tab'
                  aria-selected={view === 'all'}
                  aria-controls='plugin-directory-panel'
                  onClick={() => setView('all')}
                >
                  全部插件
                </button>
                <button
                  id='plugin-view-installed'
                  type='button'
                  role='tab'
                  aria-selected={view === 'installed'}
                  aria-controls='plugin-directory-panel'
                  onClick={() => setView('installed')}
                >
                  已安装
                </button>
              </div>

              <fieldset className='plugin-channel-filters'>
                <legend className='sr-only'>版本类型</legend>
                {(['all', 'stable', 'beta', 'nightly'] as const).map((candidate) => (
                  <button
                    key={candidate}
                    type='button'
                    aria-pressed={channel === candidate}
                    onClick={() => setChannel(candidate)}
                  >
                    {channelLabel(candidate)}
                  </button>
                ))}
              </fieldset>

              {state.pluginMarketplaceStatus === 'loading' && !state.pluginMarketplace && (
                <div className='plugin-catalog-message'>正在加载插件…</div>
              )}
              {state.pluginMarketplace && filteredItems.length === 0 && (
                <div className='plugin-catalog-message'>
                  {normalizedQuery
                    ? '没有符合搜索条件的插件。'
                    : view === 'installed'
                      ? '尚未安装插件。'
                      : '当前筛选条件下暂无插件。'}
                </div>
              )}
              <div
                className='plugin-catalog-list'
                id='plugin-directory-panel'
                role='tabpanel'
                aria-labelledby={`plugin-view-${view}`}
              >
                {filteredItems.map((item) => (
                  <MarketplaceCard
                    key={`${item.registryName}:${item.componentId}:${item.channel}`}
                    item={item}
                    busy={busy}
                    onPlan={(action) => void actions.planOperation(operationRequest(action, item))}
                    onToggle={(enabled) => void actions.setPackageEnabled(item.componentId, enabled)}
                  />
                ))}
              </div>
            </section>
          </>
        )}
      </div>

      {state.pluginOperationReview && <OperationReviewDialog actions={actions} />}
    </section>
  );
}

function MarketplaceMessages() {
  const state = useSnapshot(appState);
  return (
    <>
      {state.pluginMarketplaceStatus === 'error' && (
        <div className='plugin-catalog-message error' role='alert'>
          <AlertTriangle size={18} />
          <div>
            <strong>无法读取插件目录</strong>
            <p>{state.pluginMarketplaceError}</p>
          </div>
        </div>
      )}
      {state.pluginOperationError && (
        <div className='plugin-catalog-message error' role='alert'>
          <AlertTriangle size={18} />
          <div>
            <strong>插件操作失败</strong>
            <p>{state.pluginOperationError}</p>
          </div>
        </div>
      )}
    </>
  );
}

function RegistryCatalog() {
  const state = useSnapshot(appState);
  return (
    <section className='plugin-source-section' aria-labelledby='plugin-source-title'>
      <div className='plugin-section-title'>
        <div>
          <h1 id='plugin-source-title'>插件来源</h1>
          <p>只有验证通过的来源才会显示在插件列表中。</p>
        </div>
        {state.pluginMarketplace?.verifiedAt && (
          <span>上次检查 {formatTimestamp(state.pluginMarketplace.verifiedAt)}</span>
        )}
      </div>
      {state.pluginMarketplaceStatus === 'loading' && !state.pluginMarketplace && (
        <div className='plugin-catalog-message'>正在检查来源…</div>
      )}
      <div className='plugin-source-grid'>
        {state.pluginMarketplace?.registries.map((registry) => (
          <article className={`plugin-registry ${registry.verified ? 'verified' : 'error'}`} key={registry.name}>
            <span>{registry.verified ? <ShieldCheck size={18} /> : <AlertTriangle size={18} />}</span>
            <div>
              <header>
                <strong>{registry.name}</strong>
                <small>{registry.verified ? '已验证' : registry.configured ? '验证失败' : '未配置'}</small>
              </header>
              <details className='plugin-registry-details'>
                <summary>技术信息</summary>
                <code>{registry.url}</code>
                {registry.metadata && (
                  <dl>
                    <div>
                      <dt>目录版本</dt>
                      <dd>v{registry.metadata.targetsVersion}</dd>
                    </div>
                    <div>
                      <dt>插件数</dt>
                      <dd>{registry.metadata.packageTargets}</dd>
                    </div>
                    {registry.hostTarget && (
                      <div>
                        <dt>平台</dt>
                        <dd>{registry.hostTarget}</dd>
                      </div>
                    )}
                  </dl>
                )}
              </details>
              {registry.error && <p>{registry.error}</p>}
            </div>
          </article>
        ))}
      </div>
      {state.pluginMarketplace && state.pluginMarketplace.registries.length === 0 && (
        <div className='plugin-catalog-message'>还没有可用的插件来源。</div>
      )}
    </section>
  );
}

function MarketplaceCard({
  item,
  busy,
  onPlan,
  onToggle,
}: {
  item: PluginMarketplaceItem;
  busy: boolean;
  onPlan: (action: PluginOperationAction) => void;
  onToggle: (enabled: boolean) => void;
}) {
  const isResearch = item.packageId === 'a3s/science';
  return (
    <article className='plugin-marketplace-card'>
      <header>
        <span className={`plugin-package-logo ${isResearch ? 'research' : ''}`} aria-hidden='true'>
          {isResearch ? <FlaskConical size={19} /> : <Puzzle size={18} />}
        </span>
        <div className='plugin-card-identity'>
          <h2>{item.displayName}</h2>
          <span>{item.registryName}</span>
        </div>
        {!item.installed ? (
          <IconButton
            className='plugin-card-install'
            label={`查看并安装 ${item.displayName}`}
            disabled={busy}
            onClick={() => onPlan('install')}
          >
            <Plus size={16} />
          </IconButton>
        ) : (
          <span className={`plugin-installed-badge ${item.enabled ? 'enabled' : ''}`}>
            <PackageCheck size={12} /> {item.enabled ? '已启用' : '已停用'}
          </span>
        )}
      </header>

      <p className='plugin-card-description'>{marketplaceDescription(item)}</p>

      <div className='plugin-package-integrity'>
        <span>v{item.version}</span>
        <span>{channelLabel(item.channel)}</span>
        <span>{formatBytes(item.length)}</span>
      </div>
      {item.installed && (
        <footer>
          <Button disabled={busy} onClick={() => onPlan('upgrade')}>
            <Upload size={13} /> 升级
          </Button>
          <Button disabled={busy} onClick={() => onToggle(!item.enabled)}>
            {item.enabled ? '停用' : '启用'}
          </Button>
          <IconButton label={`卸载 ${item.displayName}`} disabled={busy} onClick={() => onPlan('uninstall')}>
            <Trash2 size={14} />
          </IconButton>
        </footer>
      )}
    </article>
  );
}

function OperationReviewDialog({ actions }: { actions: PluginActions }) {
  const state = useSnapshot(appState);
  const review = state.pluginOperationReview;
  if (!review) return null;
  const mutates = review.plan.plans.some((plan) => plan.mutates);
  const actionLabel = operationLabel(review.request.action);
  return (
    <Dialog
      title={`确认${actionLabel}`}
      description='请确认以下更改。'
      onClose={actions.dismissOperationReview}
      closeDisabled={state.pluginOperationStatus === 'loading'}
      className='plugin-operation-dialog'
      footer={
        <>
          <Button disabled={state.pluginOperationStatus === 'loading'} onClick={actions.dismissOperationReview}>
            取消
          </Button>
          <Button
            tone={review.request.action === 'uninstall' ? 'danger' : 'primary'}
            loading={state.pluginOperationStatus === 'loading'}
            disabled={!mutates}
            onClick={() => void actions.applyReviewedOperation()}
          >
            {mutates ? `确认${actionLabel}` : '无需更改'}
          </Button>
        </>
      }
    >
      <div className='plugin-operation-review'>
        <div className='plugin-review-assurance'>
          <ShieldCheck size={15} />
          <span>内容发生变化时，A3S 不会继续操作。</span>
        </div>
        <section>
          <h3>将进行</h3>
          {review.plan.plans.map((plan, index) => (
            <article key={`${plan.component}:${plan.action}:${index}`}>
              <span>{plan.mutates ? <CheckCircle2 size={14} /> : <PackageCheck size={14} />}</span>
              <div>
                <strong>{planActionLabel(plan.action)}</strong>
                <p>{plan.message}</p>
              </div>
            </article>
          ))}
        </section>
        <details className='plugin-operation-details'>
          <summary>技术信息</summary>
          <dl>
            <div>
              <dt>组件</dt>
              <dd>{review.request.componentId}</dd>
            </div>
            <div>
              <dt>校验值</dt>
              <dd>
                <code>{review.plan.planDigest}</code>
              </dd>
            </div>
            <div>
              <dt>命令</dt>
              <dd>
                <code>{review.plan.planCommand}</code>
              </dd>
            </div>
          </dl>
        </details>
      </div>
    </Dialog>
  );
}

function operationRequest(action: PluginOperationAction, item: PluginMarketplaceItem): PluginOperationRequest {
  if (action === 'install') {
    return { action, componentId: item.componentId, version: item.version, channel: item.channel };
  }
  return { action, componentId: item.componentId };
}

function marketplaceDescription(item: PluginMarketplaceItem): string {
  if (item.packageId === 'a3s/science') {
    return '跨学科科研工作台；生命科学场景可按需调用已验证的专业数据源。';
  }
  return `由 ${item.registryName} 发布。安装前会显示具体更改。`;
}

function operationLabel(action: PluginOperationAction): string {
  if (action === 'install') return '安装';
  if (action === 'upgrade') return '升级';
  return '卸载';
}

function channelLabel(channel: ChannelFilter): string {
  if (channel === 'all') return '全部';
  if (channel === 'stable') return '稳定版';
  if (channel === 'beta') return '测试版';
  return '每日版';
}

function planActionLabel(action: string): string {
  if (action === 'install') return '安装';
  if (action === 'upgrade') return '升级';
  if (action === 'uninstall') return '卸载';
  return '更新';
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}

function formatTimestamp(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? value : date.toLocaleString();
}
