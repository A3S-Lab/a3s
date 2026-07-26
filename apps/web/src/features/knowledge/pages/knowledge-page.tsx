import { FolderInput, Plus } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useSnapshot } from 'valtio';
import { SidebarProductOpenButton } from '../../../components/product-sidebar';
import { Button, SearchField } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import type { PersonalKnowledgeBase } from '../../../types/api';
import { KnowledgeSidebar } from '../components/knowledge-sidebar';
import type { KnowledgeActions } from '../use-knowledge-controller';
import { KnowledgeBaseInlineComposer, KnowledgeDirectory } from './knowledge-directory';
import { KnowledgeEditor } from './knowledge-editor';

export function KnowledgePage({ actions }: { actions: KnowledgeActions }) {
  const state = useSnapshot(appState);
  const [query, setQuery] = useState('');
  const [composer, setComposer] = useState<'create' | 'import' | null>(null);
  const [selectedBaseId, setSelectedBaseId] = useState<string | null>(() => appState.requestedKnowledgeBaseId);
  const knowledgeCount = state.personalKnowledgeBases?.total ?? 0;
  const catalogMatchesWorkspace =
    !state.knowledgeWorkspace || state.personalKnowledgeBases?.workspaceRoot === state.knowledgeWorkspace;
  const selectedBase = catalogMatchesWorkspace
    ? ((state.personalKnowledgeBases?.items.find((item) => item.id === selectedBaseId) as
        | PersonalKnowledgeBase
        | undefined) ?? null)
    : null;

  useEffect(() => {
    if (appState.knowledgeStatus === 'idle') void actions.refreshKnowledge();
  }, [actions.refreshKnowledge]);

  useEffect(() => {
    if (!selectedBaseId || state.knowledgeStatus !== 'ready' || !state.personalKnowledgeBases) return;
    if (!catalogMatchesWorkspace) return;
    if (selectedBase) {
      if (appState.requestedKnowledgeBaseId === selectedBaseId) appState.requestedKnowledgeBaseId = null;
      return;
    }
    setSelectedBaseId(null);
    if (appState.requestedKnowledgeBaseId === selectedBaseId) appState.requestedKnowledgeBaseId = null;
  }, [catalogMatchesWorkspace, selectedBase, selectedBaseId, state.knowledgeStatus, state.personalKnowledgeBases]);

  useEffect(() => {
    if (!state.requestedKnowledgeBaseId) return;
    setSelectedBaseId(state.requestedKnowledgeBaseId);
  }, [state.requestedKnowledgeBaseId]);

  useEffect(() => {
    const items = state.personalKnowledgeBases?.items ?? [];
    const compiling = items.some(
      (item) =>
        item.origin === 'selection' && (item.compilation?.phase === 'queued' || item.compilation?.phase === 'running')
    );
    const watchingSources = items.some(
      (item) => item.origin === 'selection' && item.compilation?.policy === 'smart_auto'
    );
    if (!compiling && !watchingSources) return;
    const interval = window.setInterval(() => void actions.refreshKnowledge(true), compiling ? 5_000 : 30_000);
    return () => window.clearInterval(interval);
  }, [actions.refreshKnowledge, state.personalKnowledgeBases?.items]);

  const showLibrary = () => {
    setSelectedBaseId(null);
    setQuery('');
  };

  return (
    <section className='knowledge-product' aria-label='知识'>
      {state.sidebarOpen && (
        <KnowledgeSidebar
          count={knowledgeCount}
          libraryActive={!selectedBase}
          refreshing={state.knowledgeStatus === 'loading'}
          onShowLibrary={showLibrary}
          onCollapse={() => {
            appState.sidebarOpen = false;
          }}
          onCreate={() => setComposer('create')}
          onImport={() => setComposer('import')}
          onRefresh={() => void actions.refreshKnowledge()}
        />
      )}

      {selectedBase ? (
        <KnowledgeEditor
          knowledgeBase={selectedBase}
          actions={actions}
          onBack={showLibrary}
          onRefreshKnowledge={() => void actions.refreshKnowledge(true)}
        />
      ) : (
        <section className='work-home knowledge-home'>
          <header className='work-home-header'>
            <div className='work-home-title'>
              {!state.sidebarOpen && (
                <SidebarProductOpenButton
                  title='知识'
                  className='work-sidebar-open-button'
                  onOpen={() => {
                    appState.sidebarOpen = true;
                  }}
                />
              )}
              <h1>我的知识库</h1>
            </div>
            <div className='work-home-header-actions'>
              <SearchField
                className='work-search'
                label='搜索知识库'
                value={query}
                placeholder='搜索知识库'
                onValueChange={setQuery}
              />
              <Button tone='secondary' onClick={() => setComposer('import')}>
                <FolderInput size={15} />
                导入知识库
              </Button>
              <Button tone='secondary' onClick={() => setComposer('create')}>
                <Plus size={15} />
                新建知识库
              </Button>
            </div>
          </header>

          {composer && (
            <KnowledgeBaseInlineComposer
              key={composer}
              mode={composer}
              actions={actions}
              onClose={() => setComposer(null)}
              onCreated={() => {
                setComposer(null);
                showLibrary();
              }}
              onImported={(item) => {
                setComposer(null);
                setQuery('');
                setSelectedBaseId(item.id);
              }}
            />
          )}

          <KnowledgeDirectory
            actions={actions}
            query={query}
            onCreate={() => setComposer('create')}
            onImport={() => setComposer('import')}
            onOpen={(item) => setSelectedBaseId(item.id)}
          />
        </section>
      )}
    </section>
  );
}
