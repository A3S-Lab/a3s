import { FileDiff, LoaderCircle, X } from 'lucide-react';
import { useLayoutEffect, useRef } from 'react';
import { useSnapshot } from 'valtio';
import { appState } from '../../../state/app-state';
import type { WorkspaceActions } from '../workspace-actions';
import { isFileEditorTabDirty } from '../workspace-state';
import { workspaceEditorTabLabels } from './workspace-editor-tab-label';
import { WorkspaceFileIcon } from './workspace-file-icon';

export function WorkspaceEditorTabs({ actions }: { actions: WorkspaceActions }) {
  const state = useSnapshot(appState);
  const activeRef = useRef<HTMLDivElement>(null);
  const tabRefs = useRef(new Map<string, HTMLButtonElement>());
  const requestedFocusIdRef = useRef<string | null>(null);
  useLayoutEffect(() => {
    activeRef.current?.scrollIntoView?.({ block: 'nearest', inline: 'nearest' });
    const requestedFocusId = requestedFocusIdRef.current;
    if (!requestedFocusId || requestedFocusId !== state.activeEditorTabId) return;
    const target = tabRefs.current.get(requestedFocusId);
    if (!target) return;
    requestedFocusIdRef.current = null;
    target.focus({ preventScroll: true });
  }, [state.activeEditorTabId]);
  if (!state.editorTabs.length) return null;
  const labels = workspaceEditorTabLabels(state.editorTabs, state.workspaceRoot);

  const activateFromKeyboard = (tabId: string) => {
    requestedFocusIdRef.current = tabId;
    actions.activateEditorTab(tabId);
    const target = tabRefs.current.get(tabId);
    if (!target) return;
    requestedFocusIdRef.current = null;
    target.focus({ preventScroll: true });
  };

  return (
    <div className='workspace-editor-tabs' role='tablist' aria-label='已打开的编辑器' aria-orientation='horizontal'>
      {state.editorTabs.map((tab, index) => {
        const active = tab.id === state.activeEditorTabId;
        const dirty = tab.kind === 'file' && isFileEditorTabDirty(tab);
        const display = labels.get(tab.id);
        if (!display) return null;
        return (
          <div
            ref={active ? activeRef : undefined}
            className={`workspace-editor-tab ${active ? 'active' : ''} ${dirty ? 'dirty' : ''} ${display.detail ? 'disambiguated' : ''}`}
            key={tab.id}
          >
            <button
              ref={(element) => {
                if (element) tabRefs.current.set(tab.id, element);
                else tabRefs.current.delete(tab.id);
              }}
              type='button'
              className='workspace-editor-tab-trigger'
              role='tab'
              aria-label={`${display.ariaLabel}${dirty ? '，未保存' : ''}`}
              aria-selected={active}
              aria-controls='workspace-editor-active-panel'
              tabIndex={active ? 0 : -1}
              title={display.title}
              onClick={() => actions.activateEditorTab(tab.id)}
              onAuxClick={(event) => {
                if (event.button === 1) actions.closeEditorTab(tab.id);
              }}
              onKeyDown={(event) => {
                let targetIndex: number | null = null;
                if (event.key === 'ArrowLeft') targetIndex = index - 1;
                if (event.key === 'ArrowRight') targetIndex = index + 1;
                if (event.key === 'Home') targetIndex = 0;
                if (event.key === 'End') targetIndex = state.editorTabs.length - 1;
                if (targetIndex !== null) {
                  event.preventDefault();
                  const next = state.editorTabs[(targetIndex + state.editorTabs.length) % state.editorTabs.length];
                  activateFromKeyboard(next.id);
                  return;
                }
                if (event.key === 'Delete') {
                  event.preventDefault();
                  actions.closeEditorTab(tab.id);
                }
              }}
            >
              {tab.kind === 'diff' ? (
                <FileDiff className={`workspace-tab-icon ${tab.staged ? 'staged' : 'working'}`} size={14} />
              ) : (
                <WorkspaceFileIcon path={tab.path} size={14} />
              )}
              <span className='workspace-tab-label'>
                <span className='workspace-tab-name'>{display.name}</span>
                {display.detail && <small className='workspace-tab-detail'>{display.detail}</small>}
              </span>
            </button>
            {tab.loading ? (
              <LoaderCircle className='workspace-tab-loading spin' size={12} />
            ) : (
              <button
                type='button'
                className='workspace-tab-close'
                aria-label={`关闭 ${display.ariaLabel}${dirty ? '，未保存' : ''}`}
                tabIndex={active ? 0 : -1}
                onClick={() => actions.closeEditorTab(tab.id)}
              >
                <span className='workspace-tab-dirty-dot' aria-hidden='true' />
                <X className='workspace-tab-close-icon' size={12} />
              </button>
            )}
          </div>
        );
      })}
    </div>
  );
}
