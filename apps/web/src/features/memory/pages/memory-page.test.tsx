import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { appState } from '../../../state/app-state';
import type { CodeActions } from '../../code/use-code-controller';
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
    expect(screen.getByRole('button', { name: '记忆设置' })).toHaveClass('ds-button', 'quiet');
    expect(screen.queryByRole('complementary', { name: '记忆详情' })).not.toBeInTheDocument();
    expect(document.querySelector('.memory-workbench')).not.toHaveClass('with-inspector');
    expect(screen.getAllByText('知识实体')).not.toHaveLength(0);
    expect(screen.getByText('语义关联')).toBeInTheDocument();
    const graph = screen.getByRole('region', { name: '记忆知识图谱' });
    fireEvent.click(within(graph).getByRole('button', { name: '浏览图谱节点' }));
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
    ).toHaveLength(2);
    expect(screen.getByText('由 LLM 在完整轮次结束后判断 · 置信度 96%。')).toBeInTheDocument();
    expect(screen.getByText('用户全局')).toBeInTheDocument();
    expect(screen.getAllByText('/Users/test/project')).toHaveLength(2);
    expect(screen.getByRole('meter', { name: '保留优先级' })).toBeInTheDocument();
    expect(screen.getByText('技术信息').closest('details')).not.toHaveAttribute('open');
    expect(screen.getAllByText('94%')).toHaveLength(2);

    fireEvent.click(
      within(screen.getByRole('complementary', { name: '记忆详情' })).getByRole('button', { name: /Codex/ })
    );
    await waitFor(() => expect(screen.getByRole('heading', { name: 'Codex' })).toBeInTheDocument());
    expect(screen.getByText('OpenAI Codex')).toBeInTheDocument();
    const entityInspector = screen.getByRole('complementary', { name: '实体详情' });
    expect(within(entityInspector).getByText('90%')).toBeInTheDocument();
    expect(within(entityInspector).getByText('关系')).toBeInTheDocument();
  });

  it('keeps advanced filters collapsed while surfacing active hidden filters', () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.queryByRole('heading', { name: '保留层级' })).not.toBeInTheDocument();
    const moreFilters = screen.getByRole('button', { name: '更多筛选' });
    fireEvent.click(moreFilters);
    expect(screen.getByRole('heading', { name: '保留层级' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /用户偏好/ }));
    fireEvent.click(screen.getByRole('button', { name: /更多筛选/ }));

    expect(screen.queryByRole('heading', { name: '保留层级' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /更多筛选.*1 项已启用/ })).toHaveAttribute('aria-expanded', 'false');
  });

  it('copies the selected memory content and reports feedback', async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText } });
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    const graph = screen.getByRole('region', { name: '记忆知识图谱' });
    fireEvent.click(within(graph).getByRole('button', { name: '浏览图谱节点' }));
    fireEvent.click(within(graph).getByRole('button', { name: /记忆：Prefer Rust/ }));
    fireEvent.click(await screen.findByRole('button', { name: '复制记忆' }));

    await waitFor(() =>
      expect(writeText).toHaveBeenCalledWith('Prefer Rust for systems services and keep APIs explicit.')
    );
    expect(appState.toast?.message).toBe('记忆内容已复制');
  });

  it('returns focus to the selected graph node after closing the inspector', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    const graph = screen.getByRole('region', { name: '记忆知识图谱' });
    fireEvent.click(within(graph).getByRole('button', { name: '浏览图谱节点' }));
    const memoryNode = within(graph).getByRole('button', { name: /记忆：Prefer Rust/ });
    memoryNode.focus();
    fireEvent.click(memoryNode);

    const close = await screen.findByRole('button', { name: '关闭详情' });
    close.focus();
    fireEvent.click(close);

    await waitFor(() => expect(memoryNode).toHaveFocus());
  });

  it('filters across graph entities, switches to the timeline, and recovers from no results', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    const search = screen.getByRole('searchbox', { name: '搜索记忆' });
    fireEvent.change(search, { target: { value: 'cargo' } });
    await waitFor(() =>
      expect(document.querySelector('.memory-visualization-context > span')).toHaveTextContent('1 / 3 条记忆')
    );
    fireEvent.click(screen.getByRole('button', { name: '浏览图谱节点' }));
    expect(screen.getByRole('button', { name: /记忆：Run focused cargo tests/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /记忆：Run focused cargo tests/ }));
    await waitFor(() =>
      expect(
        within(screen.getByRole('complementary', { name: '记忆详情' })).getByText(
          'Run focused cargo tests after changing the memory store.'
        )
      ).toBeInTheDocument()
    );

    fireEvent.click(screen.getByRole('tab', { name: '时间线' }));
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
      expect(document.querySelector('.memory-visualization-context > span')).toHaveTextContent('3 / 3 条记忆')
    );
  });

  it('keeps stale data visible when a refresh reports an error', () => {
    appState.memoryError = 'temporary read failure';
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.getByText(/正在显示上次成功加载的数据/)).toBeInTheDocument();
    expect(screen.getByText('记忆总数')).toBeInTheDocument();
  });

  it('does not invent an LLM reason for legacy or manually saved memory', async () => {
    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    fireEvent.click(screen.getByRole('tab', { name: '时间线' }));
    fireEvent.click(await screen.findByRole('button', { name: /Old shell experiment/ }));

    const inspector = await screen.findByRole('complementary', { name: '记忆详情' });
    expect(within(inspector).getByText('这条经历来自「上下文记录」。')).toBeInTheDocument();
    expect(
      within(inspector).getByText(
        '这条旧版或手动记忆没有保存独立的 LLM 判断理由，页面不会根据关键词或统计数据代为猜测。'
      )
    ).toBeInTheDocument();
  });

  it('renders an accessible loading state before the first response arrives', () => {
    Object.assign(appState, createMemoryState(), { memoryPhase: 'loading' });

    render(<MemoryPage actions={{ loadMemory: vi.fn() } as unknown as CodeActions} />);

    expect(screen.getByLabelText('正在加载记忆')).toHaveTextContent('正在整理记忆图谱');
    expect(screen.queryByRole('region', { name: '记忆知识图谱' })).not.toBeInTheDocument();
  });

  it('offers retry after an initial read error', () => {
    Object.assign(appState, createMemoryState(), {
      memoryPhase: 'error',
      memoryError: 'memory service unavailable',
    });
    const loadMemory = vi.fn();

    render(<MemoryPage actions={{ loadMemory } as unknown as CodeActions} />);

    expect(screen.getByRole('alert')).toHaveTextContent('memory service unavailable');
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

    expect(screen.getByRole('heading', { name: '记忆库还是空的' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '查看记忆设置' })).toBeInTheDocument();
    expect(screen.queryByRole('searchbox', { name: '搜索记忆' })).not.toBeInTheDocument();
  });
});
