import { BrainCircuit, Database, Focus, ListTree, Network, RefreshCw, Settings2 } from 'lucide-react';
import { useEffect, useMemo } from 'react';
import { useSnapshot } from 'valtio';
import { Button } from '../../../design-system/primitives';
import { appState, navigateSettings } from '../../../state/app-state';
import type { CodeActions } from '../../code/use-code-controller';
import { clearMemoryFilters, MemoryFiltersPanel } from '../components/memory-filters';
import { MemoryGraph } from '../components/memory-graph';
import { MemoryInspector } from '../components/memory-inspector';
import { MemorySummary } from '../components/memory-summary';
import { MemoryTimeline } from '../components/memory-timeline';
import { relativeMemoryTime } from '../memory-format';
import { filterMemoryEntries, projectMemoryGraph } from '../memory-projection';

export function MemoryPage({ actions }: { actions: CodeActions }) {
  const state = useSnapshot(appState);
  const data = state.memoryData ? appState.memoryData : null;

  useEffect(() => {
    if (appState.memoryPhase === 'idle') void actions.loadMemory();
  }, [actions]);

  const filters = useMemo(
    () => ({
      query: state.memoryQuery,
      types: [...state.memoryTypeFilters],
      sources: [...state.memorySourceFilters],
      tiers: [...state.memoryTierFilters],
      signals: [...state.memorySignalFilters],
      lifecycle: [...state.memoryLifecycleFilters],
      timeRange: state.memoryTimeRange,
    }),
    [
      state.memoryLifecycleFilters,
      state.memoryQuery,
      state.memorySignalFilters,
      state.memorySourceFilters,
      state.memoryTierFilters,
      state.memoryTimeRange,
      state.memoryTypeFilters,
    ]
  );
  const entries = useMemo(() => (data ? filterMemoryEntries(data, filters) : []), [data, filters]);
  const selectedMemoryId = state.memoryInspector?.kind === 'memory' ? state.memoryInspector.id : undefined;
  const selectedEntityId = state.memoryInspector?.kind === 'entity' ? state.memoryInspector.id : undefined;
  const graph = useMemo(
    () => (data ? projectMemoryGraph(data, entries, state.memoryGraphScope, selectedMemoryId, selectedEntityId) : null),
    [data, entries, selectedEntityId, selectedMemoryId, state.memoryGraphScope]
  );
  const visibleMemoryIds = useMemo(() => new Set(entries.map((entry) => entry.id)), [entries]);
  const inspectorOpen = Boolean(
    data &&
      state.memoryInspector &&
      (state.memoryInspector.kind === 'memory'
        ? data.entries.some((entry) => entry.id === state.memoryInspector?.id)
        : data.graph.entities.some((entity) => entity.id === state.memoryInspector?.id))
  );

  return (
    <section className='memory-page' aria-label='记忆工作区'>
      <header className='memory-page-header'>
        <div className='memory-page-title'>
          <span className='memory-title-mark'>
            <BrainCircuit size={19} />
          </span>
          <div>
            <h1>记忆</h1>
            <p>了解 A3S 记住了什么、这些内容如何关联，以及哪些记忆值得长期保留。</p>
          </div>
        </div>
        <div className='memory-page-actions'>
          {state.memoryLastLoadedAt && (
            <span className='memory-updated-at'>
              更新于 {relativeMemoryTime(new Date(state.memoryLastLoadedAt).toISOString())}
            </span>
          )}
          <Button tone='quiet' className='memory-settings-action' onClick={() => navigateSettings('context')}>
            <Settings2 size={14} /> 记忆设置
          </Button>
          <Button
            tone='secondary'
            loading={state.memoryRefreshing}
            disabled={state.memoryPhase === 'loading' || state.memoryRefreshing}
            onClick={() => {
              void actions.loadMemory(true);
            }}
          >
            {!state.memoryRefreshing && <RefreshCw size={14} />}
            刷新
          </Button>
        </div>
      </header>

      {state.memoryPhase === 'loading' && !data && <MemoryLoadingState />}
      {state.memoryPhase === 'error' && !data && (
        <MemoryErrorState
          error={state.memoryError}
          onRetry={() => {
            void actions.loadMemory(true);
          }}
        />
      )}
      {data && (
        <>
          {state.memoryError && (
            <output className='memory-stale-notice'>
              <span>刷新失败，正在显示上次成功加载的数据。{state.memoryError}</span>
              <button type='button' onClick={() => void actions.loadMemory(true)}>
                重试
              </button>
            </output>
          )}
          {data.stats.entries === 0 ? (
            <MemoryEmptyState />
          ) : (
            <>
              <MemorySummary data={data} visibleCount={entries.length} />
              <section className={`memory-workbench ${inspectorOpen ? 'with-inspector' : ''}`}>
                <MemoryFiltersPanel data={data} visibleCount={entries.length} />
                <main className='memory-visualization'>
                  <header className='memory-visualization-toolbar'>
                    <div className='memory-view-switcher' role='tablist' aria-label='记忆视图'>
                      <button
                        type='button'
                        role='tab'
                        aria-selected={state.memoryView === 'graph'}
                        className={state.memoryView === 'graph' ? 'active' : ''}
                        onClick={() => {
                          appState.memoryView = 'graph';
                        }}
                      >
                        <Network size={14} /> 图谱
                      </button>
                      <button
                        type='button'
                        role='tab'
                        aria-selected={state.memoryView === 'timeline'}
                        className={state.memoryView === 'timeline' ? 'active' : ''}
                        onClick={() => {
                          appState.memoryView = 'timeline';
                        }}
                      >
                        <ListTree size={14} /> 时间线
                      </button>
                    </div>
                    <div className='memory-visualization-context'>
                      <span>
                        {entries.length} / {data.stats.entries} 条记忆
                      </span>
                      {state.memoryView === 'graph' && (
                        <fieldset className='memory-scope-switcher'>
                          <legend>图谱显示范围</legend>
                          <button
                            type='button'
                            className={state.memoryGraphScope === 'balanced' ? 'active' : ''}
                            aria-pressed={state.memoryGraphScope === 'balanced'}
                            title='突出最重要的记忆与关联，适合日常浏览'
                            onClick={() => {
                              appState.memoryGraphScope = 'balanced';
                            }}
                          >
                            <Focus size={14} /> 精选图谱
                          </button>
                          <button
                            type='button'
                            className={state.memoryGraphScope === 'complete' ? 'active' : ''}
                            aria-pressed={state.memoryGraphScope === 'complete'}
                            title='查看当前筛选结果中的全局关系'
                            onClick={() => {
                              appState.memoryGraphScope = 'complete';
                            }}
                          >
                            <Database size={14} /> 全局图谱
                          </button>
                        </fieldset>
                      )}
                    </div>
                  </header>
                  {entries.length === 0 ? (
                    <MemoryNoResults />
                  ) : state.memoryView === 'graph' && graph ? (
                    <MemoryGraph
                      graph={graph}
                      selectedMemoryId={selectedMemoryId}
                      selectedEntityId={selectedEntityId}
                      onSelectMemory={(id) => {
                        appState.memoryInspector = { kind: 'memory', id };
                      }}
                      onSelectEntity={(id) => {
                        appState.memoryInspector = { kind: 'entity', id };
                      }}
                      onClearSelection={() => {
                        appState.memoryInspector = null;
                      }}
                    />
                  ) : (
                    <MemoryTimeline
                      data={data}
                      entries={entries}
                      selectedMemoryId={selectedMemoryId}
                      onSelectMemory={(id) => {
                        appState.memoryInspector = { kind: 'memory', id };
                      }}
                    />
                  )}
                </main>
                {inspectorOpen && (
                  <>
                    <button
                      type='button'
                      className='memory-inspector-scrim'
                      aria-label='关闭详情面板'
                      onClick={() => {
                        appState.memoryInspector = null;
                      }}
                    />
                    <MemoryInspector
                      data={data}
                      selection={appState.memoryInspector}
                      visibleMemoryIds={visibleMemoryIds}
                      onClear={() => {
                        appState.memoryInspector = null;
                      }}
                      onSelectMemory={(id) => {
                        appState.memoryInspector = { kind: 'memory', id };
                      }}
                      onSelectEntity={(id) => {
                        appState.memoryInspector = { kind: 'entity', id };
                      }}
                    />
                  </>
                )}
              </section>
              <footer className='memory-root-path'>
                <Database size={12} />
                <span>本地记忆库</span>
                <code title={data.root}>{data.root}</code>
              </footer>
            </>
          )}
        </>
      )}
    </section>
  );
}

function MemoryLoadingState() {
  return (
    <output className='memory-loading' aria-label='正在加载记忆'>
      <div className='memory-loading-summary'>
        {[0, 1, 2, 3].map((item) => (
          <i key={item} />
        ))}
      </div>
      <div className='memory-loading-workbench'>
        <i />
        <i />
        <i />
      </div>
      <span>正在整理记忆图谱…</span>
    </output>
  );
}

function MemoryErrorState({ error, onRetry }: { error: string | null; onRetry: () => void }) {
  return (
    <div className='memory-state-card' role='alert'>
      <span>
        <BrainCircuit size={22} />
      </span>
      <h2>无法读取记忆库</h2>
      <p>{error || '本地服务没有返回可用的记忆数据。'}</p>
      <Button onClick={onRetry}>
        <RefreshCw size={14} />
        重新加载
      </Button>
    </div>
  );
}

function MemoryEmptyState() {
  return (
    <div className='memory-state-card memory-empty'>
      <span>
        <BrainCircuit size={24} />
      </span>
      <h2>记忆库还是空的</h2>
      <p>A3S 会在完成任务时提取稳定的偏好、知识和流程。形成第一条记忆后，它会在这里进入图谱和时间线。</p>
      <Button tone='secondary' onClick={() => navigateSettings('context')}>
        <Settings2 size={14} />
        查看记忆设置
      </Button>
    </div>
  );
}

function MemoryNoResults() {
  return (
    <div className='memory-no-results'>
      <BrainCircuit size={22} />
      <h2>没有符合条件的记忆</h2>
      <p>调整关键词、时间或保留状态后再试。</p>
      <button type='button' onClick={clearMemoryFilters}>
        清除全部筛选
      </button>
    </div>
  );
}
