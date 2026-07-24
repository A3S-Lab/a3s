import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ToolCallFileDiff } from './tool-call-file-diff';

describe('ToolCallFileDiff', () => {
  it('renders line-level additions, deletions, context, and both line-number columns', () => {
    const { container } = render(
      <ToolCallFileDiff
        change={{
          path: 'src/example.ts',
          original: 'alpha\nbeta\ngamma\n',
          modified: 'alpha\ninserted\nbeta changed\ngamma\n',
          compacted: false,
        }}
      />
    );

    expect(screen.getByText('+2')).toBeInTheDocument();
    expect(screen.getByText('−1')).toBeInTheDocument();
    expect(screen.getByRole('table', { name: 'src/example.ts 行级差异' })).toBeInTheDocument();
    expect(container.querySelectorAll('.tool-call-diff-row.context')).toHaveLength(2);
    expect(container.querySelector('.tool-call-diff-row.removed')).toHaveTextContent('beta');

    const added = container.querySelectorAll('.tool-call-diff-row.added');
    expect(added).toHaveLength(2);
    expect(added[0]).toHaveTextContent('inserted');
    expect(added[0].querySelector('.old-line')).toBeEmptyDOMElement();
    expect(added[0].querySelector('.new-line')).toHaveTextContent('2');
    expect(added[1].querySelector('.new-line')).toHaveTextContent('3');

    const finalContext = container.querySelectorAll('.tool-call-diff-row.context')[1];
    expect(finalContext).toHaveTextContent('gamma');
    expect(finalContext.querySelector('.old-line')).toHaveTextContent('3');
    expect(finalContext.querySelector('.new-line')).toHaveTextContent('4');
  });
});
