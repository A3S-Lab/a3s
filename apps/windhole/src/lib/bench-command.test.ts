import { describe, expect, it } from 'vitest';
import { buildBenchCommand, shellDisplayArgument } from './bench-command';

describe('buildBenchCommand', () => {
  it('mirrors the public A3S Bench run contract', () => {
    expect(
      buildBenchCommand({
        task: 'quick_file_edit',
        candidate: './candidate',
        model: 'openai/gpt-5',
        locked: false,
      })
    ).toBe('a3s bench run quick_file_edit --agent ./candidate --model openai/gpt-5 --json');
  });

  it('does not combine a model flag into a locked command', () => {
    expect(
      buildBenchCommand({
        task: './task.lock.json',
        candidate: './candidate.lock.json',
        locked: true,
      })
    ).toBe('a3s bench run ./task.lock.json --agent ./candidate.lock.json --locked --json');
  });
});

describe('shellDisplayArgument', () => {
  it('quotes whitespace and apostrophes for an accurate command preview', () => {
    expect(shellDisplayArgument("candidate's adapter")).toBe("'candidate'\\''s adapter'");
  });
});
