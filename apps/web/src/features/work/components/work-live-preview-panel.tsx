import {
  AlertTriangle,
  ExternalLink,
  LoaderCircle,
  Monitor,
  Radio,
  RefreshCw,
  Smartphone,
  Tablet,
  X,
  ZoomIn,
  ZoomOut,
} from 'lucide-react';
import { type FormEvent, type PointerEvent as ReactPointerEvent, useEffect, useMemo, useRef, useState } from 'react';
import { codeApi } from '../../../lib/api';
import { formatApiError } from '../../../state/app-state';
import type { PreviewDescriptor, WorkspaceChangeEvent, WorkspaceEntry } from '../../../types/api';
import { localPathBasename } from '../work-local-files';
import { WorkPreviewContent } from './work-preview-content';

type PreviewDevice = 'desktop' | 'tablet' | 'mobile';
type PreviewStatus = 'connecting' | 'ready' | 'updating' | 'paused' | 'error';

const deviceWidths: Record<PreviewDevice, number | null> = {
  desktop: null,
  tablet: 768,
  mobile: 390,
};

export function WorkLivePreviewPanel({
  target,
  width,
  onWidthChange,
  onTargetChange,
  onClose,
}: {
  target: string;
  width: number;
  onWidthChange: (width: number) => void;
  onTargetChange: (target: string) => void;
  onClose: () => void;
}) {
  const [draftTarget, setDraftTarget] = useState(target);
  const [descriptor, setDescriptor] = useState<PreviewDescriptor | null>(null);
  const [error, setError] = useState('');
  const [status, setStatus] = useState<PreviewStatus>('connecting');
  const [autoRefresh, setAutoRefresh] = useState(true);
  const [reloadKey, setReloadKey] = useState(0);
  const [changeCount, setChangeCount] = useState(0);
  const [lastUpdated, setLastUpdated] = useState<Date | null>(null);
  const [device, setDevice] = useState<PreviewDevice>('desktop');
  const [zoom, setZoom] = useState(1);
  const [retryNonce, setRetryNonce] = useState(0);
  const resizeStart = useRef<{ clientX: number; width: number } | null>(null);

  useEffect(() => setDraftTarget(target), [target]);
  useEffect(() => {
    let current = true;
    setDescriptor(null);
    setError('');
    setStatus('connecting');
    setChangeCount(0);
    setLastUpdated(null);
    const requestTarget = normalizeTarget(target);
    void codeApi
      .createPreview(requestTarget)
      .then((created) => {
        if (!current) {
          void codeApi.stopPreview(created.id).catch(() => undefined);
          return;
        }
        setDescriptor(created);
        setStatus(created.kind === 'staticSite' || created.kind === 'localUrl' ? 'connecting' : 'ready');
        setLastUpdated(new Date());
      })
      .catch((requestError) => {
        if (!current) return;
        setError(formatApiError(requestError));
        setStatus('error');
      });
    return () => {
      current = false;
    };
  }, [target, retryNonce]);
  useEffect(
    () => () => {
      if (descriptor) void codeApi.stopPreview(descriptor.id).catch(() => undefined);
    },
    [descriptor]
  );
  useEffect(() => {
    if (!descriptor?.watchRoot || !descriptor.capabilities.liveReload || !autoRefresh) {
      if (descriptor && !autoRefresh) setStatus('paused');
      return;
    }
    let timer: ReturnType<typeof setTimeout> | undefined;
    const closeWatch = codeApi.watchWorkspace(descriptor.watchRoot, (event) => {
      if (!isRelevantChange(descriptor, event)) return;
      setStatus('updating');
      if (timer) clearTimeout(timer);
      timer = setTimeout(() => {
        setReloadKey((value) => value + 1);
        setChangeCount((value) => value + 1);
        setLastUpdated(new Date());
        setStatus(descriptor.kind === 'staticSite' ? 'connecting' : 'ready');
      }, 320);
    });
    return () => {
      closeWatch();
      if (timer) clearTimeout(timer);
    };
  }, [autoRefresh, descriptor]);
  useEffect(() => {
    const onPointerMove = (event: PointerEvent) => {
      const start = resizeStart.current;
      if (!start) return;
      onWidthChange(clampPanelWidth(start.width + start.clientX - event.clientX));
    };
    const onPointerUp = () => {
      resizeStart.current = null;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', onPointerUp);
    return () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', onPointerUp);
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
    };
  }, [onWidthChange]);

  const entry = useMemo(() => descriptorEntry(descriptor), [descriptor]);
  const responsive = descriptor?.capabilities.responsive ?? false;
  const viewportWidth = deviceWidths[device];
  const iframeSandbox = descriptor ? sandboxFor(descriptor) : undefined;
  const submitTarget = (event: FormEvent) => {
    event.preventDefault();
    const next = normalizeTarget(draftTarget);
    if (next && next !== target) onTargetChange(next);
    else if (next) setRetryNonce((value) => value + 1);
  };
  const refresh = () => {
    setStatus(descriptor?.kind === 'staticSite' || descriptor?.kind === 'localUrl' ? 'connecting' : 'updating');
    setReloadKey((value) => value + 1);
    setLastUpdated(new Date());
  };

  return (
    <aside className='work-live-preview-panel' style={{ width }} aria-label='实时预览面板'>
      <hr
        className='work-live-preview-resizer'
        tabIndex={0}
        aria-label='调整实时预览宽度'
        aria-orientation='vertical'
        aria-valuemin={380}
        aria-valuemax={Math.round(Math.max(480, window.innerWidth * 0.72))}
        aria-valuenow={Math.round(width)}
        onKeyDown={(event) => {
          if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
          event.preventDefault();
          onWidthChange(clampPanelWidth(width + (event.key === 'ArrowLeft' ? 24 : -24)));
        }}
        onPointerDown={(event: ReactPointerEvent) => {
          resizeStart.current = { clientX: event.clientX, width };
          document.body.style.cursor = 'col-resize';
          document.body.style.userSelect = 'none';
        }}
      />
      <header className='work-live-preview-header'>
        <div className='work-live-preview-title'>
          <span className={`work-live-preview-signal ${status}`} aria-hidden='true'>
            {status === 'connecting' || status === 'updating' ? (
              <LoaderCircle className='spin' size={14} />
            ) : status === 'error' ? (
              <AlertTriangle size={14} />
            ) : (
              <Radio size={14} />
            )}
          </span>
          <div>
            <strong>实时预览</strong>
            <small>{statusLabel(status, autoRefresh)}</small>
          </div>
        </div>
        <button type='button' aria-label='关闭实时预览' onClick={onClose}>
          <X size={16} />
        </button>
      </header>
      <div className='work-live-preview-toolbar'>
        <form className='work-live-preview-address' onSubmit={submitTarget}>
          <span aria-hidden='true' />
          <input
            aria-label='预览文件路径或本地地址'
            value={draftTarget}
            spellCheck={false}
            onChange={(event) => setDraftTarget(event.target.value)}
          />
        </form>
        <button type='button' aria-label='刷新预览' disabled={!descriptor} onClick={refresh}>
          <RefreshCw className={status === 'updating' || status === 'connecting' ? 'spin' : ''} size={14} />
        </button>
        <button
          type='button'
          aria-label='在新窗口打开预览'
          disabled={!descriptor?.capabilities.openExternal}
          onClick={() => descriptor && window.open(descriptor.contentUrl, '_blank', 'noopener,noreferrer')}
        >
          <ExternalLink size={14} />
        </button>
      </div>
      <div className='work-live-preview-controls'>
        <fieldset disabled={!responsive}>
          <legend className='sr-only'>预览设备尺寸</legend>
          {(
            [
              ['desktop', '桌面', <Monitor size={14} key='desktop' />],
              ['tablet', '平板', <Tablet size={14} key='tablet' />],
              ['mobile', '手机', <Smartphone size={14} key='mobile' />],
            ] as const
          ).map(([id, label, icon]) => (
            <button
              key={id}
              type='button'
              className={device === id ? 'active' : ''}
              aria-label={`${label}预览`}
              aria-pressed={device === id}
              onClick={() => setDevice(id)}
            >
              {icon}
            </button>
          ))}
        </fieldset>
        <div className='work-live-preview-zoom'>
          <button
            type='button'
            aria-label='缩小预览'
            disabled={!responsive || zoom <= 0.5}
            onClick={() => setZoom((value) => Math.max(0.5, value - 0.1))}
          >
            <ZoomOut size={13} />
          </button>
          <span>{Math.round(zoom * 100)}%</span>
          <button
            type='button'
            aria-label='放大预览'
            disabled={!responsive || zoom >= 1.5}
            onClick={() => setZoom((value) => Math.min(1.5, value + 0.1))}
          >
            <ZoomIn size={13} />
          </button>
        </div>
        {descriptor?.capabilities.liveReload && (
          <button
            type='button'
            className={`work-live-preview-auto ${autoRefresh ? 'active' : ''}`}
            aria-label={autoRefresh ? '暂停自动刷新' : '开启自动刷新'}
            aria-pressed={autoRefresh}
            onClick={() => {
              setAutoRefresh((value) => !value);
              if (!autoRefresh) setStatus('ready');
            }}
          >
            <Radio size={12} />
            自动刷新
          </button>
        )}
      </div>
      <main className='work-live-preview-stage'>
        {error ? (
          <section className='work-live-preview-error' role='alert'>
            <AlertTriangle size={24} />
            <strong>无法打开预览</strong>
            <p>{error}</p>
            <button type='button' onClick={() => setRetryNonce((value) => value + 1)}>
              重试
            </button>
          </section>
        ) : !descriptor ? (
          <output className='work-live-preview-loading'>
            <LoaderCircle className='spin' size={22} />
            <span>正在准备安全预览环境…</span>
          </output>
        ) : descriptor.kind === 'staticSite' || descriptor.kind === 'localUrl' ? (
          <div className='work-live-preview-canvas'>
            <div
              className={`work-live-preview-device ${device}`}
              style={{
                width: viewportWidth ? `${viewportWidth}px` : '100%',
                height: `${100 / zoom}%`,
                transform: `scale(${zoom})`,
              }}
            >
              <iframe
                key={`${descriptor.id}-${reloadKey}`}
                src={descriptor.contentUrl}
                title={`${descriptor.title} 实时预览`}
                sandbox={iframeSandbox}
                referrerPolicy='no-referrer'
                onLoad={() => setStatus(autoRefresh ? 'ready' : 'paused')}
              />
            </div>
          </div>
        ) : entry ? (
          <section className='work-live-preview-document' key={`${descriptor.id}-${reloadKey}`}>
            <WorkPreviewContent entry={entry} reloadKey={reloadKey} />
          </section>
        ) : null}
      </main>
      <footer className='work-live-preview-footer'>
        <span title={descriptor?.source.type === 'path' ? descriptor.source.path : descriptor?.contentUrl}>
          {descriptor?.source.type === 'path' ? localPathBasename(descriptor.source.path) : descriptor?.title || '预览'}
        </span>
        <span>
          {changeCount > 0 ? `${changeCount} 次更新 · ` : ''}
          {lastUpdated
            ? `更新于 ${lastUpdated.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`
            : '等待更新'}
        </span>
      </footer>
    </aside>
  );
}

function descriptorEntry(descriptor: PreviewDescriptor | null): WorkspaceEntry | null {
  if (!descriptor || descriptor.source.type !== 'path') return null;
  const source = descriptor.source;
  return {
    name: source.name,
    path: source.path,
    isDirectory: source.isDirectory,
    isFile: !source.isDirectory,
    size: source.size,
    mtimeMs: source.mtimeMs,
    extension: source.name.includes('.') ? source.name.split('.').pop() : null,
    isBinary: source.isBinary,
  };
}

function isRelevantChange(descriptor: PreviewDescriptor, event: WorkspaceChangeEvent): boolean {
  if (descriptor.source.type !== 'path') return false;
  const source = normalizePath(descriptor.source.path);
  const root = normalizePath(descriptor.watchRoot ?? descriptor.source.rootPath);
  return event.paths.some((path) => {
    const changed = normalizePath(path);
    return descriptor.kind === 'staticSite' ? changed === root || changed.startsWith(`${root}/`) : changed === source;
  });
}

function normalizePath(path: string): string {
  return path.replaceAll('\\', '/').replace(/\/$/, '');
}

function normalizeTarget(target: string): string {
  const trimmed = target.trim();
  if (/^(localhost|127\.\d+\.\d+\.\d+|\[::1\])(?::\d+)?(?:\/|$)/i.test(trimmed)) {
    return `http://${trimmed}`;
  }
  return trimmed;
}

function sandboxFor(descriptor: PreviewDescriptor): string {
  const base = 'allow-scripts allow-forms allow-modals allow-popups allow-downloads';
  if (descriptor.kind !== 'localUrl') return base;
  try {
    return new URL(descriptor.contentUrl, window.location.href).origin === window.location.origin
      ? base
      : `${base} allow-same-origin`;
  } catch {
    return base;
  }
}

function statusLabel(status: PreviewStatus, autoRefresh: boolean): string {
  if (status === 'connecting') return '正在连接';
  if (status === 'updating') return '检测到文件变化';
  if (status === 'paused' || !autoRefresh) return '自动刷新已暂停';
  if (status === 'error') return '需要处理';
  return '实时同步中';
}

function clampPanelWidth(width: number): number {
  return Math.max(380, Math.min(width, Math.max(480, window.innerWidth * 0.72)));
}
