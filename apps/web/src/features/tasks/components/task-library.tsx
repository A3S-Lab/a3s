import { useDebounce } from 'ahooks';
import { ChevronDown, CirclePlus, Search } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { useSnapshot } from 'valtio';
import { ProductSidebar, SidebarNavIcon } from '../../../components/product-sidebar';
import { CollectionState, IconButton, SearchField } from '../../../design-system/primitives';
import { appState, sessionTitle } from '../../../state/app-state';
import type { CodeSession } from '../../../types/api';
import type { TaskActions } from '../task-actions';
import { TaskLibraryItem } from './task-library-item';

export function TaskLibrary({
  actions,
  title = '会话',
  label = '会话列表',
  itemLabel = '会话',
  onNewConversation,
  onSelectSession,
}: {
  actions: TaskActions;
  title?: string;
  label?: string;
  itemLabel?: string;
  onNewConversation?: () => void;
  onSelectSession?: (session: CodeSession) => void | Promise<void>;
}) {
  const state = useSnapshot(appState);
  const [searchOpen, setSearchOpen] = useState(Boolean(state.searchQuery));
  const [tasksExpanded, setTasksExpanded] = useState(true);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const searchTriggerRef = useRef<HTMLButtonElement>(null);
  const query = useDebounce(state.searchQuery.trim().toLowerCase(), { wait: 160 });
  const sessions = state.sessions.filter(
    (session) =>
      !query ||
      sessionTitle(session, state.sessionTitles).toLowerCase().includes(query) ||
      session.workspace.toLowerCase().includes(query)
  );
  useEffect(() => {
    if (searchOpen) searchInputRef.current?.focus();
  }, [searchOpen]);
  return (
    <ProductSidebar
      className='task-library'
      label={label}
      title={title}
      headerActions={
        <IconButton
          ref={searchTriggerRef}
          label={searchOpen ? `关闭${itemLabel}搜索` : `搜索${itemLabel}`}
          selected={searchOpen}
          aria-expanded={searchOpen}
          onClick={() => {
            if (searchOpen) appState.searchQuery = '';
            else setTasksExpanded(true);
            setSearchOpen(!searchOpen);
          }}
        >
          <Search size={15} />
        </IconButton>
      }
      onCollapse={() => {
        appState.sidebarOpen = false;
      }}
    >
      <button
        type='button'
        className={`sidebar-nav-item task-library-new ${state.activeSessionId ? '' : 'active'}`}
        aria-current={state.activeSessionId ? undefined : 'page'}
        onClick={() => {
          if (onNewConversation) onNewConversation();
          else actions.newConversation();
          closeCompactTaskLibrary();
        }}
      >
        <SidebarNavIcon tone='blue'>
          <CirclePlus size={15} />
        </SidebarNavIcon>
        <span className='sidebar-nav-label'>新建{itemLabel}</span>
      </button>
      {searchOpen && (
        <SearchField
          ref={searchInputRef}
          className='task-library-search'
          size='compact'
          label={`搜索${itemLabel}`}
          clearLabel={`清除${itemLabel}搜索`}
          value={state.searchQuery}
          placeholder={`搜索${itemLabel}`}
          onValueChange={(value) => {
            appState.searchQuery = value;
          }}
          onKeyDown={(event) => {
            if (event.key !== 'Escape') return;
            event.preventDefault();
            appState.searchQuery = '';
            setSearchOpen(false);
            searchTriggerRef.current?.focus();
          }}
        />
      )}
      <section className='task-list' aria-label={`${itemLabel}列表`}>
        <button
          type='button'
          className='task-list-label'
          aria-expanded={tasksExpanded}
          onClick={() => setTasksExpanded(!tasksExpanded)}
        >
          <span>
            {itemLabel} ({sessions.length})
          </span>
          <ChevronDown size={13} />
        </button>
        {tasksExpanded && sessions.length ? (
          <div className='task-list-items'>
            {sessions.map((session) => {
              const title = sessionTitle(session, state.sessionTitles);
              return (
                <TaskLibraryItem
                  key={session.sessionId}
                  session={session}
                  title={title}
                  active={state.activeSessionId === session.sessionId}
                  running={state.streamingSessionId === session.sessionId}
                  onSelect={() => {
                    if (onSelectSession) void onSelectSession(session as CodeSession);
                    else void actions.selectSession(session.sessionId);
                    closeCompactTaskLibrary();
                  }}
                  itemLabel={itemLabel}
                  onRename={(name) => actions.renameSession(session.sessionId, name)}
                  onDelete={
                    state.streamingSessionId === session.sessionId
                      ? undefined
                      : () => actions.removeSession(session.sessionId)
                  }
                />
              );
            })}
          </div>
        ) : tasksExpanded ? (
          <CollectionState className='task-list-empty' role='status'>
            {query ? `没有匹配的${itemLabel}` : `暂无${itemLabel}`}
          </CollectionState>
        ) : null}
      </section>
    </ProductSidebar>
  );
}

function closeCompactTaskLibrary(): void {
  if (window.innerWidth <= 620) appState.sidebarOpen = false;
}
