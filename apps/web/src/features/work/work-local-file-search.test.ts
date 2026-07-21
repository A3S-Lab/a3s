import { describe, expect, it, vi } from 'vitest';
import type { WorkspaceEntry } from '../../types/api';
import { searchWorkLocalFiles } from './work-local-file-search';

const entry = (path: string, directory = false): WorkspaceEntry => ({
  name: path.split(/[\\/]/).at(-1) ?? path,
  path,
  isDirectory: directory,
  isFile: !directory,
  size: directory ? 0 : 128,
  mtimeMs: 10,
  extension: directory ? null : path.split('.').at(-1),
  isBinary: false,
});

describe('Work recursive local-file search', () => {
  it('finds matching files and folders breadth-first with their real metadata', async () => {
    const readDir = vi.fn(async (path: string) => {
      if (path === '/docs') return [entry('/docs/Reports', true), entry('/docs/Plan.docx')];
      if (path === '/docs/Reports') {
        return [entry('/docs/Reports/Archive', true), entry('/docs/Reports/Quarterly Plan.xlsx')];
      }
      if (path === '/docs/Reports/Archive') return [entry('/docs/Reports/Archive/Old Plan.pdf')];
      return [];
    });

    const result = await searchWorkLocalFiles(readDir, '/docs', 'plan');

    expect(result.entries.map((candidate) => candidate.path)).toEqual([
      '/docs/Plan.docx',
      '/docs/Reports/Quarterly Plan.xlsx',
      '/docs/Reports/Archive/Old Plan.pdf',
    ]);
    expect(result.scannedDirectories).toBe(3);
    expect(result.scannedEntries).toBe(5);
    expect(result.truncated).toBe(false);
  });

  it('stays inside the selected root, skips unreadable descendants, and reports bounded results', async () => {
    const readDir = vi.fn(async (path: string) => {
      if (path === '/docs') {
        return [
          entry('/docs/Match 1.docx'),
          entry('/docs/Private', true),
          entry('/outside/Escape', true),
          entry('/docs/Match 2.docx'),
        ];
      }
      if (path === '/docs/Private') throw new Error('permission denied');
      return [];
    });

    const result = await searchWorkLocalFiles(readDir, '/docs', 'match', { maxResults: 1 });

    expect(result.entries.map((candidate) => candidate.path)).toEqual(['/docs/Match 1.docx']);
    expect(result.unreadableDirectories).toBe(0);
    expect(result.truncated).toBe(true);
    expect(readDir).not.toHaveBeenCalledWith('/outside/Escape');
  });

  it('continues after an unreadable nested folder without hiding the partial-result condition', async () => {
    const readDir = vi.fn(async (path: string) => {
      if (path === '/docs') return [entry('/docs/Private', true), entry('/docs/Public', true)];
      if (path === '/docs/Private') throw new Error('permission denied');
      if (path === '/docs/Public') return [entry('/docs/Public/Meeting Notes.docx')];
      return [];
    });

    const result = await searchWorkLocalFiles(readDir, '/docs', 'notes');

    expect(result.entries.map((candidate) => candidate.path)).toEqual(['/docs/Public/Meeting Notes.docx']);
    expect(result.unreadableDirectories).toBe(1);
    expect(result.truncated).toBe(true);
  });
});
