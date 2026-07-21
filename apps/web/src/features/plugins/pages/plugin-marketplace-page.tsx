import {
  AlertTriangle,
  CheckCircle2,
  Download,
  PackageCheck,
  RefreshCw,
  Search,
  ShieldCheck,
  Store,
  Trash2,
  Upload,
} from 'lucide-react';
import { useEffect, useState } from 'react';
import { useSnapshot } from 'valtio';
import { Button, Dialog } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import type { PluginMarketplaceItem, PluginOperationAction, PluginOperationRequest } from '../../../types/api';
import type { PluginActions } from '../use-plugin-controller';

export function PluginMarketplacePage({ actions }: { actions: PluginActions }) {
  const state = useSnapshot(appState);
  const [query, setQuery] = useState('');

  useEffect(() => {
    if (appState.pluginMarketplaceStatus === 'idle') void actions.refreshMarketplace();
  }, [actions.refreshMarketplace]);

  const normalizedQuery = query.trim().toLocaleLowerCase();
  const items = (state.pluginMarketplace?.items ?? []).filter((item) =>
    [item.displayName, item.componentId, item.registryName, item.channel]
      .join(' ')
      .toLocaleLowerCase()
      .includes(normalizedQuery)
  );
  const busy = state.pluginOperationStatus === 'loading';

  return (
    <section className='plugin-marketplace-page' aria-label='插件市场'>
      <header className='plugin-page-header marketplace'>
        <div>
          <span className='plugin-page-icon'>
            <Store size={17} />
          </span>
          <div>
            <h1>插件市场</h1>
            <p>从已配置的 TUF 注册表安装和管理摘要绑定的 A3S Use 插件。</p>
          </div>
        </div>
        <Button loading={state.pluginMarketplaceStatus === 'loading'} onClick={() => void actions.refreshMarketplace()}>
          <RefreshCw size={14} />
          刷新已签名目录
        </Button>
      </header>

      <div className='plugin-marketplace-layout'>
        <aside className='plugin-registry-panel' aria-label='注册表状态'>
          <h2>可信注册表</h2>
          <p>只有通过 TUF 元数据验证的包会出现在目录中。</p>
          {state.pluginMarketplaceStatus === 'loading' && !state.pluginMarketplace && (
            <div className='plugin-registry-empty'>正在验证注册表…</div>
          )}
          {state.pluginMarketplace?.registries.map((registry) => (
            <article className={`plugin-registry ${registry.verified ? 'verified' : 'error'}`} key={registry.name}>
              <span>{registry.verified ? <ShieldCheck size={15} /> : <AlertTriangle size={15} />}</span>
              <div>
                <strong>{registry.name}</strong>
                <small>{registry.verified ? 'TUF 元数据已验证' : registry.configured ? '验证失败' : '尚未配置'}</small>
                <code>{registry.url}</code>
                {registry.error && <p>{registry.error}</p>}
              </div>
            </article>
          ))}
          {state.pluginMarketplace && state.pluginMarketplace.registries.length === 0 && (
            <div className='plugin-registry-empty'>尚未配置 TUF 插件注册表。</div>
          )}
          {state.pluginMarketplace?.verifiedAt && (
            <small className='plugin-verified-at'>
              目录验证于 {formatTimestamp(state.pluginMarketplace.verifiedAt)}
            </small>
          )}
        </aside>

        <div className='plugin-catalog-panel'>
          <label className='plugin-search-field'>
            <Search size={15} />
            <span className='sr-only'>搜索插件</span>
            <input
              value={query}
              placeholder='搜索插件、发布者或通道'
              onChange={(event) => setQuery(event.target.value)}
            />
          </label>

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
          {state.pluginMarketplaceStatus === 'loading' && !state.pluginMarketplace && (
            <div className='plugin-catalog-message'>正在验证并读取插件目录…</div>
          )}
          {state.pluginMarketplace && items.length === 0 && (
            <div className='plugin-catalog-message'>
              {normalizedQuery ? '没有符合搜索条件的插件。' : '可信注册表中暂无适用于当前平台的插件。'}
            </div>
          )}
          <div className='plugin-catalog-list'>
            {items.map((item) => (
              <MarketplaceCard
                key={`${item.registryName}:${item.componentId}:${item.channel}`}
                item={item as PluginMarketplaceItem}
                busy={busy}
                onPlan={(action) => void actions.planOperation(operationRequest(action, item as PluginMarketplaceItem))}
                onToggle={(enabled) => void actions.setPackageEnabled(item.componentId, enabled)}
              />
            ))}
          </div>
        </div>
      </div>

      {state.pluginOperationReview && <OperationReviewDialog actions={actions} />}
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
  return (
    <article className='plugin-marketplace-card'>
      <span className='plugin-package-logo' aria-hidden='true'>
        {item.displayName.slice(0, 1).toLocaleUpperCase()}
      </span>
      <div className='plugin-package-content'>
        <header>
          <div>
            <h2>{item.displayName}</h2>
            <code>{item.componentId}</code>
          </div>
          {item.installed && (
            <span className={`plugin-installed-badge ${item.enabled ? 'enabled' : ''}`}>
              <PackageCheck size={12} />
              {item.enabled ? '已启用' : '已停用'}
            </span>
          )}
        </header>
        <p>
          {item.registryName} · {item.channel} · {item.target}
        </p>
        <div className='plugin-package-integrity'>
          <span>v{item.version}</span>
          <span>{formatBytes(item.length)}</span>
          <span title={item.sha256}>SHA-256 {item.sha256.slice(0, 12)}…</span>
        </div>
        <footer>
          {!item.installed ? (
            <Button tone='primary' disabled={busy} onClick={() => onPlan('install')}>
              <Download size={14} />
              审核并安装
            </Button>
          ) : (
            <>
              <Button disabled={busy} onClick={() => onPlan('upgrade')}>
                <Upload size={14} />
                审核升级
              </Button>
              <Button disabled={busy} onClick={() => onToggle(!item.enabled)}>
                {item.enabled ? '停用' : '启用'}
              </Button>
              <Button tone='danger' disabled={busy} onClick={() => onPlan('uninstall')}>
                <Trash2 size={14} />
                卸载
              </Button>
            </>
          )}
        </footer>
      </div>
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
      title={`审核插件${actionLabel}计划`}
      description='计划由当前 a3s 可执行文件以 --dry-run 生成；应用时必须匹配下方摘要。'
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
          <span>应用请求只携带此计划的 SHA-256 摘要；注册表或目标发生变化时会失败关闭。</span>
        </div>
        <dl>
          <div>
            <dt>组件</dt>
            <dd>{review.request.componentId}</dd>
          </div>
          <div>
            <dt>计划摘要</dt>
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
        <section>
          <h3>将执行的操作</h3>
          {review.plan.plans.map((plan, index) => (
            <article key={`${plan.component}:${plan.action}:${index}`}>
              <span>{plan.mutates ? <CheckCircle2 size={14} /> : <PackageCheck size={14} />}</span>
              <div>
                <strong>{plan.action}</strong>
                <p>{plan.message}</p>
                <small>{plan.source}</small>
              </div>
            </article>
          ))}
        </section>
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

function operationLabel(action: PluginOperationAction): string {
  if (action === 'install') return '安装';
  if (action === 'upgrade') return '升级';
  return '卸载';
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
