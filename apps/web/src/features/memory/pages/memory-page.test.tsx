import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { CodeActions } from '../../code/use-code-controller';
import { evolutionTestData } from '../evolution-test-data';
import { createMemoryState } from '../memory-state';
import { memoryTestData } from '../memory-test-data';
import { MemoryPage } from './memory-page';

describe('MemoryPage', () => {
  beforeEach(() => {
    Object.assign(appState, createMemoryState(), {
      memoryPhase: 'ready',
      memoryData: memoryTestData(),
      memoryLastLoadedAt: Date.parse('2026-07-20T10:00:00Z'),
    });
  });

  afterEach(() => {
    cleanup();
    Object.assign(appState, createMemoryState());
  });

  it('renders the complete graph overview and opens memory and entity details', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.getByRole('heading', { name: '记忆' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '已保存' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('button', { name: '设置' })).toHaveClass('ds-button', 'quiet');
    expect(screen.queryByText('查看 A3S 记住的内容。')).not.toBeInTheDocument();
    expect(screen.queryByRole('region', { name: '记忆概览' })).not.toBeInTheDocument();
    expect(screen.queryByRole('complementary', { name: '记忆详情' })).not.toBeInTheDocument();
    expect(document.querySelector('.memory-workbench')).not.toHaveClass('with-inspector');
    const graph = screen.getByRole('region', { name: '记忆关联图' });
    expect(within(graph).queryByRole('complementary', { name: '关系图说明' })).not.toBeInTheDocument();
    fireEvent.click(within(graph).getByRole('button', { name: '浏览图中内容' }));
    expect(within(graph).getByRole('button', { name: /记忆：Prefer Rust/ })).toBeInTheDocument();

    fireEvent.click(within(graph).getByRole('button', { name: /记忆：Prefer Rust/ }));
    await waitFor(() =>
      expect(screen.getByText('Prefer Rust for systems services and keep APIs explicit.')).toBeInTheDocument()
    );
    expect(document.querySelector('.memory-workbench')).toHaveClass('with-inspector');
    expect(screen.getByRole('heading', { name: '为什么会记住' })).toBeInTheDocument();
    expect(
      within(screen.getByRole('complementary', { name: '记忆详情' })).getAllByText(
        'A stable language and API-style preference changes future implementation choices.'
      )
    ).toHaveLength(1);
    expect(screen.queryByText(/判断把握|把握 96%|保留优先级/)).not.toBeInTheDocument();
    expect(screen.queryByText('用户全局')).not.toBeInTheDocument();
    expect(screen.queryByText('/Users/test/project')).not.toBeInTheDocument();
    expect(screen.queryByText('更多信息')).not.toBeInTheDocument();
    expect(screen.queryByText('94%')).not.toBeInTheDocument();

    fireEvent.click(
      within(screen.getByRole('complementary', { name: '记忆详情' })).getByRole('button', { name: /Codex/ })
    );
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Codex' })).toBeInTheDocument());
    expect(screen.getByText('OpenAI Codex')).toBeInTheDocument();
    const entityInspector = screen.getByRole('complementary', { name: '相关内容详情' });
    expect(within(entityInspector).queryByText('90%')).not.toBeInTheDocument();
    expect(within(entityInspector).queryByText('关系')).not.toBeInTheDocument();
  });

  it('keeps advanced filters collapsed while surfacing active hidden filters', () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.queryByRole('heading', { name: '保存期限' })).not.toBeInTheDocument();
    const moreFilters = screen.getByRole('button', { name: '更多筛选' });
    fireEvent.click(moreFilters);
    expect(screen.getByRole('heading', { name: '保存期限' })).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: '整理方式' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /有冲突/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /偏好/ }));
    fireEvent.click(screen.getByRole('button', { name: /更多筛选/ }));

    expect(screen.queryByRole('heading', { name: '保存期限' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /更多筛选.*已选 1 项/ })).toHaveAttribute('aria-expanded', 'false');
  });

  it('copies the selected memory content and reports feedback', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    const graph = screen.getByRole('region', { name: '记忆关联图' });
    fireEvent.click(within(graph).getByRole('button', { name: '浏览图中内容' }));
    fireEvent.click(within(graph).getByRole('button', { name: /记忆：Prefer Rust/ }));
    fireEvent.click(await screen.findByRole('button', { name: '复制记忆' }));

    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith('Prefer Rust for systems services and keep APIs explicit.')
    );
    expect(appState.toast?.message).toBe('记忆内容已复制');
  });

  it('returns focus to the selected graph node after closing the inspector', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    const graph = screen.getByRole('region', { name: '记忆关联图' });
    fireEvent.click(within(graph).getByRole('button', { name: '浏览图中内容' }));
    const memoryNode = within(graph).getByRole('button', { name: /记忆：Prefer Rust/ });
    memoryNode.focus();
    fireEvent.click(memoryNode);

    const browserToggle = within(graph).getByRole('button', { name: '浏览图中内容' });
    expect(browserToggle).toHaveAttribute('aria-expanded', 'false');
    const close = await screen.findByRole('button', { name: '关闭详情' });
    close.focus();
    fireEvent.click(close);

    await waitFor(() => expect(browserToggle).toHaveFocus());
  });

  it('filters across graph entities, switches to the timeline, and recovers from no results', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    const search = screen.getByRole('searchbox', { name: '搜索记忆' });
    fireEvent.change(search, { target: { value: 'cargo' } });
    await waitFor(() =>
      expect(document.querySelector('.memory-visualization-context > span')).toHaveTextContent('1 / 3 条记忆')
    );
    fireEvent.click(screen.getByRole('button', { name: '浏览图中内容' }));
    expect(screen.getByRole('button', { name: /记忆：Run focused cargo tests/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /记忆：Run focused cargo tests/ }));
    await waitFor(() =>
      expect(
        within(screen.getByRole('complementary', { name: '记忆详情' })).getByText(
          'Run focused cargo tests after changing the memory store.'
        )
      ).toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole('tab', { name: '按时间' }));
    await waitFor(() => expect(screen.getByRole('region', { name: '记忆时间线' })).toBeInTheDocument());
    expect(screen.getByRole('button', { name: /Run focused cargo tests/ })).toHaveAttribute('aria-current', 'true');
    expect(
      within(screen.getByRole('complementary', { name: '记忆详情' })).getByText(
        'Run focused cargo tests after changing the memory store.'
      )
    ).toBeInTheDocument();

    fireEvent.change(search, { target: { value: 'not-a-memory' } });
    await waitFor(() => expect(screen.getByRole('heading', { name: '没有符合条件的记忆' })).toBeInTheDocument());
    fireEvent.click(screen.getByRole('button', { name: '清除全部筛选' }));
    await waitFor(() =>
      expect(document.querySelector('.memory-visualization-context > span')).toHaveTextContent('3 条记忆')
    );
  });

  it('keeps stale data visible when a refresh reports an error', () => {
    appState.memoryError = 'temporary read failure';
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.getByText('刷新失败，当前显示上次结果。')).toBeInTheDocument();
    expect(screen.getByRole('region', { name: '记忆关联图' })).toBeInTheDocument();
  });

  it('omits a reason when legacy or manually saved memory has none', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    fireEvent.click(screen.getByRole('tab', { name: '按时间' }));
    fireEvent.click(await screen.findByRole('button', { name: /Old shell experiment/ }));

    const inspector = await screen.findByRole('complementary', { name: '记忆详情' });
    expect(within(inspector).queryByRole('heading', { name: '为什么会记住' })).not.toBeInTheDocument();
    expect(within(inspector).getByText('对话')).toBeInTheDocument();
  });

  it('renders an accessible loading state before the first response arrives', () => {
    Object.assign(appState, createMemoryState(), { memoryPhase: 'loading' });

    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.getByLabelText('正在加载记忆')).toHaveTextContent('正在加载记忆');
    expect(screen.queryByRole('region', { name: '记忆关联图' })).not.toBeInTheDocument();
  });

  it('offers retry after an initial read error', () => {
    Object.assign(appState, createMemoryState(), {
      memoryPhase: 'error',
      memoryError: 'memory service unavailable',
    });
    const loadMemory = vi.fn();

    render(<MemoryPage actions={{ loadMemory } as unknown as CodeActions} />);

    expect(screen.getByRole('alert')).toHaveTextContent('暂时无法读取，请稍后重试。');
    expect(screen.getByText('暂时无法读取，请稍后重试。')).toHaveAttribute('title', 'memory service unavailable');
    fireEvent.click(screen.getByRole('button', { name: '重新加载' }));
    expect(loadMemory).toHaveBeenCalledWith(true);
  });

  it('explains how the first memory will appear when the store is empty', () => {
    const data = memoryTestData();
    data.entries = [];
    data.stats.entries = 0;
    data.graph.events = [];
    data.graph.entities = [];
    data.graph.relations = [];
    data.graph.facets = {};
    Object.assign(appState, createMemoryState(), {
      memoryPhase: 'ready',
      memoryData: data,
    });

    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.getByRole('heading', { name: '还没有记忆' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看记忆设置' })).toBeInTheDocument();
    expect(screen.queryByRole('searchbox', { name: '搜索记忆' })).not.toBeInTheDocument();
  });

  it('opens the autonomous evolution workbench and scans memory on demand', async () => {
    Object.assign(appState, {
      evolutionPhase: 'ready',
      evolutionData: evolutionTestData(),
      evolutionSelectedId: 'preference-concise-evidence',
    });
    const scanEvolution = vi.fn();
    render(
      <MemoryPage actions={{ loadMemory: vi.fn(), loadEvolution: vi.fn(), scanEvolution } as unknown as CodeActions} />
    );

    fireEvent.click(screen.getByRole('tab', { name: /学习/ }));

    expect(await screen.findByRole('heading', { name: '记忆' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '学习' })).toHaveAttribute('aria-selected', 'true');
    expect(screen.queryByRole('region', { name: '学习概览' })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '检查新内容' }));
    expect(scanEvolution).toHaveBeenCalledTimes(1);
  });

  it('loads the evolution catalog only when its tab first becomes active', async () => {
    const loadEvolution = vi.fn();
    render(<MemoryPage actions={{ loadMemory: vi.fn(), loadEvolution } as unknown as CodeActions} />);

    expect(loadEvolution).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('tab', { name: '学习' }));

    await waitFor(() => expect(loadEvolution).toHaveBeenCalledTimes(1));
  });
});
