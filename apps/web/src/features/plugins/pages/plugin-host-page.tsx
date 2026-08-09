import { AlertTriangle, Box, LoaderCircle, RefreshCw, ShieldCheck, Store } from 'lucide-react';
import { useCallback, useLayoutEffect, useRef, useState } from 'react';
import { useSnapshot } from 'valtio';
import { Button, Dialog, PageHeader, StateView, StatusBadge } from '../../../design-system/primitives';
import { appState, navigatePlugins } from '../../../state/app-state';
import { resolveActivityDocument, type PluginActivityDocumentIdentity } from '../plugin-activity-document';
import { activityHostInit, parsePluginMessage } from '../plugin-protocol';
import type { PluginActions } from '../use-plugin-controller';

type FrameStatus = 'loading' | 'ready' | 'error';
const ACTIVITY_READY_TIMEOUT_MS = 10_000;

export function PluginHostPage({ actions }: { actions: PluginActions }) {
  const state = useSnapshot(appState);
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const portRef = useRef<MessagePort | null>(null);
  const frameLoadCountRef = useRef(0);
  const loadedFrameTokenRef = useRef<string | null>(null);
  const readyTimeoutRef = useRef<number | null>(null);
  const [frameStatus, setFrameStatus] = useState<FrameStatus>('loading');
  const [blockedToken, setBlockedToken] = useState<string | null>(null);
  const [frameGeneration, setFrameGeneration] = useState(0);
  const key = state.activePluginKey;
  const contribution = state.pluginCatalog.items.find((item) => item.key === key && item.enabled);
  const { identity: documentIdentity, error: documentError } = currentActivityDocument(key);
  const identityToken = documentIdentity?.token ?? null;
  const frameToken = documentIdentity ? `${documentIdentity.token}:${frameGeneration}` : null;

  const clearReadyTimeout = useCallback(() => {
    if (readyTimeoutRef.current !== null) {
      window.clearTimeout(readyTimeoutRef.current);
      readyTimeoutRef.current = null;
    }
  }, []);

  const releasePort = useCallback(
    (port: MessagePort) => {
      port.onmessage = null;
      port.onmessageerror = null;
      port.close();
      if (portRef.current === port) {
        portRef.current = null;
        clearReadyTimeout();
      }
    },
    [clearReadyTimeout]
  );

  const closeActivePort = useCallback(() => {
    const port = portRef.current;
    if (port) releasePort(port);
    else clearReadyTimeout();
  }, [clearReadyTimeout, releasePort]);

  useLayoutEffect(() => {
    closeActivePort();
    frameLoadCountRef.current = 0;
    loadedFrameTokenRef.current = null;
    setFrameStatus('loading');
    setBlockedToken(null);
    appState.pluginRuntimeError = null;
    return closeActivePort;
  }, [closeActivePort, frameGeneration, identityToken, key]);

  if (!key || !contribution) {
    return (
      <PluginStateView
        icon={<LoaderCircle className='spin' size={22} />}
        title='正在解析插件…'
        message={state.pluginCatalogError ?? '正在核对已安装插件及其资产摘要。'}
        role='status'
      />
    );
  }

  const retryFrame = () => {
    closeActivePort();
    frameLoadCountRef.current = 0;
    loadedFrameTokenRef.current = null;
    appState.pluginRuntimeError = null;
    setFrameStatus('loading');
    setBlockedToken(null);
    setFrameGeneration((value) => value + 1);
  };

  const failFrame = (message: string, block = false) => {
    closeActivePort();
    setFrameStatus('error');
    appState.pluginRuntimeError = message;
    if (block && frameToken) setBlockedToken(frameToken);
  };

  const handleFrameLoad = () => {
    if (!documentIdentity || !frameToken) return;
    if (loadedFrameTokenRef.current !== frameToken) {
      closeActivePort();
      frameLoadCountRef.current = 0;
      loadedFrameTokenRef.current = frameToken;
    }
    frameLoadCountRef.current += 1;
    if (frameLoadCountRef.current > 1) {
      failFrame('插件尝试离开已校验文档，宿主已终止该视图。', true);
      return;
    }

    const contentWindow = iframeRef.current?.contentWindow;
    if (!contentWindow) {
      failFrame('无法建立插件通信通道。');
      return;
    }

    closeActivePort();
    const channel = new MessageChannel();
    const port = channel.port1;
    portRef.current = port;
    port.onmessage = (event: MessageEvent<unknown>) => {
      if (portRef.current !== port) return;
      if (!isCurrentActivityDocument(documentIdentity)) {
        releasePort(port);
        return;
      }
      const message = parsePluginMessage(event.data, documentIdentity);
      if (!message) return;
      if (message.type === 'ready') {
        clearReadyTimeout();
        setFrameStatus('ready');
        appState.pluginRuntimeError = null;
      } else if (message.type === 'context') {
        actions.proposeContext(message.proposal);
      } else {
        releasePort(port);
        setFrameStatus('error');
        appState.pluginRuntimeError = message.message;
      }
    };
    port.onmessageerror = () => {
      if (portRef.current !== port) return;
      if (!isCurrentActivityDocument(documentIdentity)) {
        releasePort(port);
        return;
      }
      releasePort(port);
      setFrameStatus('error');
      appState.pluginRuntimeError = '插件通信通道发生协议错误。';
    };
    port.start();

    readyTimeoutRef.current = window.setTimeout(() => {
      if (portRef.current !== port) return;
      releasePort(port);
      if (!isCurrentActivityDocument(documentIdentity)) return;
      setFrameStatus('error');
      appState.pluginRuntimeError = '插件未在 10 秒内完成初始化。';
    }, ACTIVITY_READY_TIMEOUT_MS);

    try {
      contentWindow.postMessage(
        activityHostInit(
          documentElementTheme(),
          navigator.language || 'zh-CN',
          contribution.packageId,
          documentIdentity
        ),
        '*',
        [channel.port2]
      );
    } catch {
      channel.port2.close();
      failFrame('无法建立插件通信通道。');
    }
  };

  return (
    <section className='plugin-host-page' aria-label={`${contribution.title} 插件`}>
      <PageHeader
        className='plugin-page-header'
        icon={<Box size={17} />}
        title={contribution.title}
        description={contribution.description || `${contribution.packageId} 提供的工作台视图`}
        actions={
          <>
            <StatusBadge tone='success'>
              <ShieldCheck size={13} /> 已校验 · {contribution.version}
            </StatusBadge>
            <Button tone='quiet' onClick={navigatePlugins}>
              <Store size={14} />
              市场
            </Button>
          </>
        }
      />

      <div className='plugin-frame-shell'>
        {!documentIdentity && !documentError && (
          <PluginStateView
            icon={<LoaderCircle className='spin' size={22} />}
            title='正在加载插件文档'
            message='宿主正在核对 Registry generation、revision 和文档身份。'
            role='status'
          />
        )}
        {!documentIdentity && documentError && (
          <PluginStateView
            icon={<AlertTriangle size={22} />}
            title='无法加载插件文档'
            message={documentError}
            tone='danger'
            role='alert'
            action={
              <Button tone='primary' onClick={() => void actions.refreshActivities()}>
                <RefreshCw size={14} />
                重试
              </Button>
            }
          />
        )}
        {documentIdentity && (
          <>
            {blockedToken !== frameToken && (
              <iframe
                key={frameToken}
                ref={iframeRef}
                className='plugin-frame'
                title={`${contribution.title} 插件内容`}
                sandbox='allow-scripts'
                referrerPolicy='no-referrer'
                data-status={frameStatus}
                src={documentIdentity.url}
                onLoad={handleFrameLoad}
              />
            )}
            {frameStatus === 'loading' && blockedToken !== frameToken && (
              <StateView
                className='plugin-frame-overlay'
                size='compact'
                role='status'
                icon={<LoaderCircle className='spin' size={20} />}
                title='正在启动隔离视图…'
              />
            )}
            {(frameStatus === 'error' || state.pluginRuntimeError) && (
              <StateView
                className='plugin-frame-overlay'
                size='compact'
                tone='danger'
                role='alert'
                icon={<AlertTriangle size={22} />}
                title='插件视图报告了错误'
                description={state.pluginRuntimeError ?? '插件未能完成初始化。'}
                actions={
                  <Button onClick={retryFrame}>
                    <RefreshCw size={14} />
                    重新启动视图
                  </Button>
                }
              />
            )}
          </>
        )}
      </div>

      {proposalMatchesDocument(state.pluginContextProposal, documentIdentity) && (
        <ContextReviewDialog actions={actions} contributionTitle={contribution.title} skill={contribution.skill} />
      )}
    </section>
  );
}

function ContextReviewDialog({
  actions,
  contributionTitle,
  skill,
}: {
  actions: PluginActions;
  contributionTitle: string;
  skill: string;
}) {
  const state = useSnapshot(appState);
  const proposal = state.pluginContextProposal;
  if (!proposal) return null;
  return (
    <Dialog
      title={proposal.title}
      description={`${contributionTitle} 请求将以下内容交给当前 AI 会话。`}
      onClose={actions.dismissContextProposal}
      footer={
        <>
          <Button onClick={actions.dismissContextProposal}>取消</Button>
          <Button tone='primary' onClick={actions.acceptContextProposal}>
            在当前会话中使用
          </Button>
        </>
      }
      className='plugin-review-dialog'
    >
      <div className='plugin-context-review'>
        <div className='plugin-review-assurance'>
          <ShieldCheck size={15} />
          {proposal.usePackageSkill ? (
            <span>
              只会附加你确认的上下文，以及宿主从同一签名包验证的 <code>{skill}</code> Skill。
            </span>
          ) : (
            <span>只会附加你确认的上下文，不会附加该包的专业 Skill；AI 将使用当前可用的通用能力。</span>
          )}
        </div>
        <section>
          <h3>摘要</h3>
          <p>{proposal.summary}</p>
        </section>
        {proposal.fields.length > 0 && (
          <dl>
            {proposal.fields.map((field) => (
              <div key={`${field.label}:${field.value}`}>
                <dt>{field.label}</dt>
                <dd>{field.value}</dd>
              </div>
            ))}
          </dl>
        )}
        <section>
          <h3>将加入输入框的指令</h3>
          <pre>{proposal.prompt}</pre>
        </section>
      </div>
    </Dialog>
  );
}

function PluginStateView({
  icon,
  title,
  message,
  action,
  tone = 'info',
  role,
}: {
  icon: React.ReactNode;
  title: string;
  message: string;
  action?: React.ReactNode;
  tone?: 'info' | 'danger';
  role?: 'alert' | 'status';
}) {
  return (
    <StateView
      className='plugin-state-view'
      icon={icon}
      title={title}
      description={message}
      actions={action}
      tone={tone}
      role={role}
    />
  );
}

function documentElementTheme(): 'light' | 'dark' {
  return document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light';
}

function currentActivityDocument(key: string | null): {
  identity: PluginActivityDocumentIdentity | null;
  error: string | null;
} {
  if (!key) return { identity: null, error: null };
  const contribution = appState.pluginCatalog.items.find((item) => item.key === key && item.enabled);
  if (!contribution) return { identity: null, error: null };
  try {
    return { identity: resolveActivityDocument(appState.pluginCatalog, contribution), error: null };
  } catch {
    return { identity: null, error: '插件文档身份与当前 Registry 目录不一致。' };
  }
}

function isCurrentActivityDocument(identity: PluginActivityDocumentIdentity): boolean {
  if (appState.activePluginKey !== identity.key) return false;
  const catalog = appState.pluginCatalog;
  if (catalog.generation !== identity.generation || catalog.revision !== identity.revision) return false;
  const contribution = catalog.items.find((item) => item.key === identity.key && item.enabled);
  if (!contribution) return false;
  try {
    const current = resolveActivityDocument(catalog, contribution);
    return current.token === identity.token && current.url === identity.url;
  } catch {
    return false;
  }
}

function proposalMatchesDocument(
  proposal: Readonly<{
    sourceKey: string;
    sourceGeneration: number;
    sourceRevision: string;
    sourceDocumentUrl: string;
  }> | null,
  identity: PluginActivityDocumentIdentity | null
): boolean {
  return Boolean(
    proposal &&
      identity &&
      proposal.sourceKey === identity.key &&
      proposal.sourceGeneration === identity.generation &&
      proposal.sourceRevision === identity.revision &&
      proposal.sourceDocumentUrl === identity.url
  );
}
