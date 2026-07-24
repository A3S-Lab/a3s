import { describe, expect, it } from 'vitest';
import { normalizeCollapsedMarkdownTables } from './markdown-normalization';

describe('Markdown normalization', () => {
  it('restores row boundaries for a collapsed GFM table', () => {
    expect(
      normalizeCollapsedMarkdownTables(
        '| # | 任务 | 验收 | 估时 | | --- | --- | --- | --- | | 0.1 | workspace | CI 通过 | 1d | | 0.2 | 日志 | 可检索 | 2d |'
      )
    ).toBe(
      [
        '| # | 任务 | 验收 | 估时 |',
        '| --- | --- | --- | --- |',
        '| 0.1 | workspace | CI 通过 | 1d |',
        '| 0.2 | 日志 | 可检索 | 2d |',
      ].join('\n')
    );
  });

  it('leaves valid tables, prose, and fenced examples unchanged', () => {
    const valid = ['| 项目 | 状态 |', '| --- | --- |', '| 构建 | 通过 |'].join('\n');
    const prose = '使用 alpha | beta 表示二选一。';
    const fenced = ['```md', '| A | B | | --- | --- | | 1 | 2 |', '```'].join('\n');

    expect(normalizeCollapsedMarkdownTables(valid)).toBe(valid);
    expect(normalizeCollapsedMarkdownTables(prose)).toBe(prose);
    expect(normalizeCollapsedMarkdownTables(fenced)).toBe(fenced);
  });

  it('does not mistake an empty cell for a row boundary', () => {
    const collapsed = '| A |  | C | | --- | --- | --- | | 1 |  | 3 |';

    expect(normalizeCollapsedMarkdownTables(collapsed)).toBe(
      ['| A |  | C |', '| --- | --- | --- |', '| 1 |  | 3 |'].join('\n')
    );
  });

  it('repairs aligned short delimiters and boundaries collapsed without spaces', () => {
    const collapsed = '| 左侧 | 右侧 || :-- | --: || 中文 | 42 |';

    expect(normalizeCollapsedMarkdownTables(collapsed)).toBe(
      ['| 左侧 | 右侧 |', '| :-- | --: |', '| 中文 | 42 |'].join('\n')
    );
  });

  it('separates prose from a collapsed table and supports an empty streaming body', () => {
    expect(normalizeCollapsedMarkdownTables('结果如下： | 项目 | 状态 | | --- | --- |')).toBe(
      ['结果如下：', '| 项目 | 状态 |', '| --- | --- |'].join('\n')
    );
  });

  it('pads a model-generated delimiter row that has fewer columns than its header', () => {
    const malformed = ['| 目录 | 性质 | 删除依据 |', '|---|---|', '| `.cache/` | 临时目录 | 已确认可删除 |'].join('\n');

    expect(normalizeCollapsedMarkdownTables(malformed)).toBe(
      ['| 目录 | 性质 | 删除依据 |', '|---|---|---|', '| `.cache/` | 临时目录 | 已确认可删除 |'].join('\n')
    );
  });
});
