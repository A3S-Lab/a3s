import { Copy, FileDiff, Files, LoaderCircle, PanelRightClose, X, XCircle } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useSnapshot } from 'valtio';
import { appState, showToast } from '../../../state/app-state';
import type { WorkspaceActions } from '../workspace-actions';
import { isFileEditorTabDirty, type WorkspaceEditorTab, workspaceRelativePath } from '../workspace-state';
import { WorkspaceContextMenu, type WorkspaceContextMenuItem } from './workspace-context-menu';
import { WorkspaceFileIcon } from './workspace-file-icon';

interface TabContextMenuState {
  tabId: string;
  x: number;
  y: number;
}

export function WorkspaceEditorTabs({ actions }: { actions: WorkspaceActions }) {
  const state = useSnapshot(appState);
  const activeRef = useRef<HTMLDivElement>(null);
  const [contextMenu, setContextMenu] = useState<TabContextMenuState | null>(null);
  useEffect(() => {
    activeRef.current?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
  }, [state.activeEditorTabId]);
  if (!state.editorTabs.length) return null;
  const contextualTab = contextMenu ? state.editorTabs.find((tab) => tab.id === contextMenu.tabId) : undefined;

  return (
    <>
      <div className='workspace-editor-tabs' role='tablist' aria-label='已打开的编辑器'>
        {state.editorTabs.map((tab, index) => {
          const active = tab.id === state.activeEditorTabId;
          const dirty = tab.kind === 'file' && isFileEditorTabDirty(tab);
          const relativePath = workspaceRelativePath(tab.path, state.workspaceRoot);
          const label =
            tab.kind === 'diff'
              ? `${basename(tab.path)} ${tab.staged ? '（已暂存）' : '（工作树）'}`
              : basename(tab.path);
          return (
            <div
              ref={active ? activeRef : undefined}
              className={`workspace-editor-tab ${active ? 'active' : ''} ${dirty ? 'dirty' : ''} ${
                contextMenu?.tabId === tab.id ? 'contextual' : ''
              }`}
              key={tab.id}
              role='tab'
              aria-selected={active}
              aria-controls='workspace-editor-active-panel'
              aria-haspopup='menu'
              tabIndex={active ? 0 : -1}
              title={
                tab.kind === 'diff' ? `${relativePath} · ${tab.staged ? '已暂存差异' : '工作树差异'}` : relativePath
              }
              onClick={() => actions.activateEditorTab(tab.id)}
              onAuxClick={(event) => {
                if (event.button === 1) actions.closeEditorTab(tab.id);
              }}
              onContextMenu={(event) => {
                event.preventDefault();
                event.currentTarget.focus();
                setContextMenu({ tabId: tab.id, x: event.clientX, y: event.clientY });
              }}
              onKeyDown={(event) => {
                if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
                  event.preventDefault();
                  const bounds = event.currentTarget.getBoundingClientRect();
                  setContextMenu({
                    tabId: tab.id,
                    x: bounds.left + Math.min(bounds.width, 160),
                    y: bounds.bottom,
                  });
                  return;
                }
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  actions.activateEditorTab(tab.id);
                }
                if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
                  event.preventDefault();
                  const direction = event.key === 'ArrowRight' ? 1 : -1;
                  const next =
                    state.editorTabs[(index + direction + state.editorTabs.length) % state.editorTabs.length];
                  actions.activateEditorTab(next.id);
                  requestAnimationFrame(() => activeRef.current?.focus());
                }
              }}
            >
              {tab.kind === 'diff' ? (
                <FileDiff className={`workspace-tab-icon ${tab.staged ? 'staged' : 'working'}`} size={14} />
              ) : (
                <WorkspaceFileIcon path={tab.path} size={14} />
              )}
              <span>{label}</span>
              {tab.loading ? (
                <LoaderCircle className='workspace-tab-loading spin' size={12} />
              ) : (
                <button
                  type='button'
                  className='workspace-tab-close'
                  aria-label={`关闭 ${label}${dirty ? '，未保存' : ''}`}
                  onClick={(event) => {
                    event.stopPropagation();
                    actions.closeEditorTab(tab.id);
                  }}
                >
                  <span className='workspace-tab-dirty-dot' aria-hidden='true' />
                  <X className='workspace-tab-close-icon' size={12} />
                </button>
              )}
            </div>
          );
        })}
      </div>
      {contextMenu && contextualTab && (
        <WorkspaceContextMenu
          label={`${basename(contextualTab.path)} 标签页操作`}
          x={contextMenu.x}
          y={contextMenu.y}
          items={tabContextMenuItems(
            contextualTab as WorkspaceEditorTab,
            state.editorTabs as readonly WorkspaceEditorTab[],
            state.workspaceRoot,
            actions
          )}
          onClose={() => setContextMenu(null)}
        />
      )}
    </>
  );
}

function tabContextMenuItems(
  tab: WorkspaceEditorTab,
  tabs: readonly WorkspaceEditorTab[],
  workspaceRoot: string,
  actions: WorkspaceActions
): WorkspaceContextMenuItem[] {
  const index = tabs.findIndex((candidate) => candidate.id === tab.id);
  const otherIds = tabs.filter((candidate) => candidate.id !== tab.id).map((candidate) => candidate.id);
  const rightIds = tabs.slice(index + 1).map((candidate) => candidate.id);
  const relativePath = workspaceRelativePath(tab.path, workspaceRoot);
  const absolutePath = absoluteWorkspacePath(tab.path, workspaceRoot);
  const closeShortcut = primaryShortcut('W');
  return [
    {
      id: 'close',
      label: '关闭',
      icon: <X size={14} />,
      shortcut: closeShortcut.label,
      ariaKeyShortcut: closeShortcut.aria,
      onSelect: () => actions.closeEditorTabs([tab.id]),
    },
    {
      id: 'close-others',
      label: '关闭其他标签页',
      icon: <Files size={14} />,
      disabled: otherIds.length === 0,
      onSelect: () => actions.closeEditorTabs(otherIds),
    },
    {
      id: 'close-right',
      label: '关闭右侧标签页',
      icon: <PanelRightClose size={14} />,
      disabled: rightIds.length === 0,
      onSelect: () => actions.closeEditorTabs(rightIds),
    },
    {
      id: 'close-all',
      label: '关闭全部标签页',
      icon: <XCircle size={14} />,
      onSelect: () => actions.closeEditorTabs(tabs.map((candidate) => candidate.id)),
    },
    {
      id: 'copy-path',
      label: '复制路径',
      icon: <Copy size={14} />,
      separatorBefore: true,
      onSelect: () => void copyPath(absolutePath),
    },
    {
      id: 'copy-relative-path',
      label: '复制相对路径',
      icon: <Copy size={14} />,
      onSelect: () => void copyPath(relativePath),
    },
  ];
}

function primaryShortcut(key: string): { label: string; aria: string } {
  const applePlatform =
    typeof navigator !== 'undefined' && /Mac|iPhone|iPad|iPod/u.test(`${navigator.platform} ${navigator.userAgent}`);
  return applePlatform ? { label: `⌘${key}`, aria: `Meta+${key}` } : { label: `Ctrl+${key}`, aria: `Control+${key}` };
}

async function copyPath(path: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(path);
    showToast('路径已复制', 'success');
  } catch {
    showToast('无法复制路径', 'error');
  }
}

function absoluteWorkspacePath(path: string, root: string): string {
  if (path.startsWith('/') || /^[A-Za-z]:[\\/]/.test(path)) return path;
  const separator = root.includes('\\') && !root.includes('/') ? '\\' : '/';
  return `${root.replace(/[\\/]$/, '')}${separator}${path}`;
}

function basename(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() || path;
}
