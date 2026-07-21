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
});
