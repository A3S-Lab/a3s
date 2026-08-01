import { describe, expect, it } from 'vitest';
import { skillMentionRanges, slashCommandRanges } from './slash-command-highlight';

describe('slash command highlighting', () => {
  it('highlights a known command without including its argument', () => {
    expect(slashCommandRanges('/goal 所有测试通过')).toEqual([{ from: 0, to: 5 }]);
  });

  it('does not style inline, URL, or unknown slash tokens as commands', () => {
    expect(slashCommandRanges('请执行 /goal 所有测试通过')).toEqual([]);
    expect(slashCommandRanges('https://streamdown.ai/ /unknown')).toEqual([]);
  });

  it('highlights Codex-style Skill mentions', () => {
    expect(skillMentionRanges('请用 $review 和（$a3s-office）')).toEqual([
      { from: 3, to: 10 },
      { from: 13, to: 24 },
    ]);
  });
});
