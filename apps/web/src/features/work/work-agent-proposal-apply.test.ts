import { describe, expect, it } from 'vitest';
import type { WorkAgentProposalChange } from './work-agent-proposal';
import {
  applyPresentationAgentProposalChanges,
  applySpreadsheetAgentProposalChanges,
} from './work-agent-proposal-apply';
import { createWorkArtifact } from './work-templates';

describe('Work agent proposal application', () => {
  it('applies approved spreadsheet values and formulas while skipping stale cells', () => {
    const artifact = createWorkArtifact('blank-spreadsheet');
    if (artifact.content.type !== 'spreadsheet') throw new Error('Spreadsheet fixture is invalid');
    const sheet = artifact.content.sheets[0];
    sheet.data = [
      [
        { v: '旧内容', m: '旧内容' },
        { f: '=A1', v: 1 },
      ],
    ];

    const outcome = applySpreadsheetAgentProposalChanges(artifact.content, sheet.id!, [
      change('A1', '预算!A1', '旧内容', '新内容'),
      change('B1', '预算!B1', '=WRONG', '=A1*2'),
      change('C1', '预算!C1', '', '=SUM(A1:B1)'),
    ]);

    expect(artifact.content.sheets[0].data?.[0]?.[0]).toMatchObject({ v: '旧内容' });
    expect(outcome.content.sheets[0].data?.[0]?.[0]).toMatchObject({ v: '新内容', m: '新内容' });
    expect(outcome.content.sheets[0].data?.[0]?.[1]).toMatchObject({ f: '=A1', v: 1 });
    expect(outcome.content.sheets[0].data?.[0]?.[2]).toEqual({ f: '=SUM(A1:B1)' });
    expect(outcome.result.appliedTargetIds).toEqual(['A1', 'C1']);
    expect(outcome.result.conflicts).toEqual([
      expect.objectContaining({ targetId: 'B1', message: expect.stringContaining('已发生变化') }),
    ]);
  });

  it('applies presentation text, table, and notes changes without overwriting stale content', () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') throw new Error('Presentation fixture is invalid');
    const slide = artifact.content.slides[0];
    const textElement = slide.elements[0];
    textElement.text = '原始标题';
    slide.notes = '原始备注';
    slide.elements.push({
      id: 'table-1',
      type: 'table',
      x: 10,
      y: 30,
      width: 80,
      height: 40,
      text: '',
      fontSize: 14,
      color: '#000000',
      fill: '#ffffff',
      bold: false,
      align: 'left',
      table: { rows: [['原值', '保留']] },
    });

    const outcome = applyPresentationAgentProposalChanges(artifact.content, slide.id, [
      change(`text:${textElement.id}`, '标题', '原始标题', '建议标题'),
      change('table:table-1:0:0', '表格单元格', '原值', '新值'),
      change('notes', '演讲者备注', '原始备注', '建议备注'),
      change('table:table-1:0:1', '另一个单元格', '过期值', '不应写入'),
    ]);

    const updatedSlide = outcome.content.slides[0];
    expect(slide.elements[0].text).toBe('原始标题');
    expect(updatedSlide.elements[0]).toMatchObject({ text: '建议标题', textRuns: undefined });
    expect(updatedSlide.elements.at(-1)?.table?.rows[0]).toEqual(['新值', '保留']);
    expect(updatedSlide.notes).toBe('建议备注');
    expect(outcome.result.appliedTargetIds).toEqual([`text:${textElement.id}`, 'table:table-1:0:0', 'notes']);
    expect(outcome.result.conflicts).toEqual([expect.objectContaining({ targetId: 'table:table-1:0:1' })]);
  });
});

function change(id: string, label: string, before: string, after: string): WorkAgentProposalChange {
  return { id, label, before, after, reason: '测试建议' };
}
