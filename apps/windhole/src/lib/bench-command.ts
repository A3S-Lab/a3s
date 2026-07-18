import type { StartBenchRunInput } from '../types/bench';

export function buildBenchCommand(input: StartBenchRunInput): string {
  const parts = [
    'a3s',
    'bench',
    'run',
    shellDisplayArgument(input.task),
    '--agent',
    shellDisplayArgument(input.candidate),
  ];
  if (input.model) parts.push('--model', shellDisplayArgument(input.model));
  if (input.locked) parts.push('--locked');
  parts.push('--json');
  return parts.join(' ');
}

export function shellDisplayArgument(value: string): string {
  if (/^[a-zA-Z0-9_./:@+-]+$/u.test(value)) return value;
  return `'${value.replaceAll("'", "'\\''")}'`;
}
