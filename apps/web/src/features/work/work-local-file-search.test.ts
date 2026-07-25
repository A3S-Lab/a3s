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

  it('skips generated and metadata trees without excluding the OKF knowledge directory', async () => {
    const readDir = vi.fn(async (path: string) => {
      if (path === '/workspace') {
        return [
          entry('/workspace/.git', true),
          entry('/workspace/node_modules', true),
          entry('/workspace/target', true),
          entry('/workspace/dist', true),
          entry('/workspace/.cloud-h0-2-native-snapshot', true),
          entry('/workspace/.a3s', true),
          entry('/workspace/docs', true),
        ];
      }
      if (path === '/workspace/.a3s') return [entry('/workspace/.a3s/kb', true)];
      if (path === '/workspace/.a3s/kb') return [entry('/workspace/.a3s/kb/project-app.md')];
      if (path === '/workspace/docs') return [entry('/workspace/docs/app-guide.md')];
      return [entry(`${path}/irrelevant-app.js`)];
    });

    const result = await searchWorkLocalFiles(readDir, '/workspace', 'app');

    expect(result.entries.map((candidate) => candidate.path)).toEqual([
      '/workspace/docs/app-guide.md',
      '/workspace/.a3s/kb/project-app.md',
    ]);
    expect(readDir).not.toHaveBeenCalledWith('/workspace/.git');
    expect(readDir).not.toHaveBeenCalledWith('/workspace/node_modules');
    expect(readDir).not.toHaveBeenCalledWith('/workspace/target');
    expect(readDir).not.toHaveBeenCalledWith('/workspace/dist');
    expect(readDir).not.toHaveBeenCalledWith('/workspace/.cloud-h0-2-native-snapshot');
    expect(readDir).toHaveBeenCalledWith('/workspace/.a3s/kb');
    expect(result.truncated).toBe(false);
  });
});
