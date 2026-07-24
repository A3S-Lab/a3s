import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import StreamingMarkdown from './streaming-markdown';

describe('StreamingMarkdown', () => {
  afterEach(cleanup);

  it('parses emphasis and strikethrough next to Chinese text', () => {
    render(<StreamingMarkdown content='中文**重点内容**以及~~废弃内容~~仍能正确渲染。' streaming={false} />);

    expect(screen.getByText('重点内容', { selector: '[data-streamdown="strong"]' })).toBeInTheDocument();
    expect(screen.getByText('废弃内容', { selector: 'del' })).toBeInTheDocument();
  });

  it('exposes streaming state without replacing semantic Markdown', () => {
    const { container } = render(<StreamingMarkdown content={'## 执行结果\n\n- 已完成'} streaming />);

    expect(screen.getByRole('heading', { name: '执行结果', level: 2 })).toBeInTheDocument();
    expect(screen.getByText('已完成')).toBeInTheDocument();
    expect(container.querySelector('.streaming-markdown-region')).toHaveAttribute('aria-busy', 'true');
  });

  it('renders aligned Chinese GFM tables after a streaming fragment completes', () => {
    render(
      <StreamingMarkdown
        content={['| 项目 | 说明 | 数量 |', '| :--- | :---: | ---: |', '| 构建 | 已通过 | 3 |'].join('\n')}
        streaming
      />
    );

    const table = screen.getByRole('table');
    expect(table).toHaveTextContent('构建');
    expect(screen.getAllByRole('columnheader')).toHaveLength(3);
    expect(screen.getAllByRole('row')).toHaveLength(2);
  });

  it('renders a table when the model omits one delimiter cell', () => {
    render(
      <StreamingMarkdown
        content={['| 目录 | 性质 | 删除依据 |', '|---|---|', '| `.cache/` | 临时目录 | 已确认可删除 |'].join('\n')}
        streaming={false}
      />
    );

    expect(screen.getByRole('table')).toHaveTextContent('已确认可删除');
    expect(screen.getAllByRole('columnheader')).toHaveLength(3);
    expect(screen.getAllByRole('cell')).toHaveLength(3);
  });
});
