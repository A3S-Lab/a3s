import type { ToolFileChange } from './tool-call-projection';

type DiffLineKind = 'context' | 'added' | 'removed';

interface DiffLine {
  kind: DiffLineKind;
  text: string;
  oldLine?: number;
  newLine?: number;
}

const MAX_LCS_CELLS = 250_000;

export function ToolCallFileDiff({ change }: { change: ToolFileChange }) {
  const lines = unifiedDiffLines(change.original, change.modified);
  const additions = lines.filter((line) => line.kind === 'added').length;
  const deletions = lines.filter((line) => line.kind === 'removed').length;

  return (
    <section
      className={`tool-call-file-diff${change.compacted ? ' compacted' : ''}`}
      aria-label={`文件差异 ${change.path}`}
    >
      <header className='tool-call-file-diff-summary'>
        <span>{additions + deletions} 行变更</span>
        <span className='added'>+{additions}</span>
        <span className='removed'>−{deletions}</span>
      </header>
      {change.compacted && (
        <small className='tool-call-file-diff-notice'>大型变更仅显示首尾预览，完整内容请打开文件查看。</small>
      )}
      <div className='tool-call-file-diff-lines'>
        <table aria-label={`${change.path} 行级差异`}>
          <tbody>
            {lines.map((line, index) => (
              <tr className={`tool-call-diff-row ${line.kind}`} key={`${line.kind}-${index}`}>
                <td className='tool-call-diff-line-number old-line' aria-label='原行号'>
                  {line.oldLine}
                </td>
                <td className='tool-call-diff-line-number new-line' aria-label='新行号'>
                  {line.newLine}
                </td>
                <td className='tool-call-diff-marker' aria-hidden='true'>
                  {line.kind === 'added' ? '+' : line.kind === 'removed' ? '−' : ' '}
                </td>
                <td className='tool-call-diff-code'>
                  <code>{line.text || ' '}</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function unifiedDiffLines(original: string, modified: string): DiffLine[] {
  const before = sourceLines(original);
  const after = sourceLines(modified);
  const operations: Array<{ kind: DiffLineKind; text: string }> = [];
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) {
    operations.push({ kind: 'context', text: before[prefix] });
    prefix += 1;
  }

  let beforeEnd = before.length;
  let afterEnd = after.length;
  while (beforeEnd > prefix && afterEnd > prefix && before[beforeEnd - 1] === after[afterEnd - 1]) {
    beforeEnd -= 1;
    afterEnd -= 1;
  }

  operations.push(...middleDiff(before.slice(prefix, beforeEnd), after.slice(prefix, afterEnd)));
  for (let index = beforeEnd; index < before.length; index += 1) {
    operations.push({ kind: 'context', text: before[index] });
  }

  let oldLine = 1;
  let newLine = 1;
  return operations.map((operation) => {
    if (operation.kind === 'added') {
      return { ...operation, newLine: newLine++ };
    }
    if (operation.kind === 'removed') {
      return { ...operation, oldLine: oldLine++ };
    }
    return { ...operation, oldLine: oldLine++, newLine: newLine++ };
  });
}

function middleDiff(before: readonly string[], after: readonly string[]): Array<{ kind: DiffLineKind; text: string }> {
  if (!before.length) return after.map((text) => ({ kind: 'added', text }));
  if (!after.length) return before.map((text) => ({ kind: 'removed', text }));
  if (before.length * after.length > MAX_LCS_CELLS) {
    return [
      ...before.map((text) => ({ kind: 'removed' as const, text })),
      ...after.map((text) => ({ kind: 'added' as const, text })),
    ];
  }

  const table = Array.from({ length: before.length + 1 }, () => new Uint32Array(after.length + 1));
  for (let beforeIndex = before.length - 1; beforeIndex >= 0; beforeIndex -= 1) {
    for (let afterIndex = after.length - 1; afterIndex >= 0; afterIndex -= 1) {
      table[beforeIndex][afterIndex] =
        before[beforeIndex] === after[afterIndex]
          ? table[beforeIndex + 1][afterIndex + 1] + 1
          : Math.max(table[beforeIndex + 1][afterIndex], table[beforeIndex][afterIndex + 1]);
    }
  }

  const operations: Array<{ kind: DiffLineKind; text: string }> = [];
  let beforeIndex = 0;
  let afterIndex = 0;
  while (beforeIndex < before.length && afterIndex < after.length) {
    if (before[beforeIndex] === after[afterIndex]) {
      operations.push({ kind: 'context', text: before[beforeIndex] });
      beforeIndex += 1;
      afterIndex += 1;
    } else if (table[beforeIndex + 1][afterIndex] >= table[beforeIndex][afterIndex + 1]) {
      operations.push({ kind: 'removed', text: before[beforeIndex++] });
    } else {
      operations.push({ kind: 'added', text: after[afterIndex++] });
    }
  }
  while (beforeIndex < before.length) operations.push({ kind: 'removed', text: before[beforeIndex++] });
  while (afterIndex < after.length) operations.push({ kind: 'added', text: after[afterIndex++] });
  return operations;
}

function sourceLines(value: string): string[] {
  if (!value) return [];
  const lines = value.replace(/\r\n?/g, '\n').split('\n');
  if (lines.at(-1) === '') lines.pop();
  return lines;
}
