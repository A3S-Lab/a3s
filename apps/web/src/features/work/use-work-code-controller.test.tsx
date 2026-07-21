import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { codeApi } from '../../lib/api';
import { useWorkCodeController } from './use-work-code-controller';

describe('Work WebIDE controller', () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it('opens, edits, and safely saves arbitrary text code files', async () => {
    const readFile = vi.spyOn(codeApi, 'readFile').mockResolvedValue({ content: 'export const value = 1;\n' });
    const writeFile = vi.spyOn(codeApi, 'writeFile').mockResolvedValue({ success: true });
    const { result } = renderHook(() => useWorkCodeController('/repo'));

    await act(() => result.current.openFile({ path: '/repo/src/value.ts', isBinary: false }));
    expect(result.current.activeTab?.draft).toBe('export const value = 1;\n');

    act(() => result.current.updateDraft('/repo/src/value.ts', 'export const value = 2;\n'));
    await act(() => result.current.saveFile('/repo/src/value.ts'));

    expect(readFile).toHaveBeenLastCalledWith('/repo/src/value.ts');
    expect(writeFile).toHaveBeenCalledWith('/repo/src/value.ts', 'export const value = 2;\n');
    expect(result.current.activeTab?.content).toBe('export const value = 2;\n');
  });

  it('does not overwrite an externally changed file without an explicit conflict decision', async () => {
    vi.spyOn(codeApi, 'readFile')
      .mockResolvedValueOnce({ content: '# Original\n' })
      .mockResolvedValueOnce({ content: '# Changed elsewhere\n' });
    const writeFile = vi.spyOn(codeApi, 'writeFile').mockResolvedValue({ success: true });
    const { result } = renderHook(() => useWorkCodeController('/repo'));

    await act(() => result.current.openFile({ path: '/repo/README.md', isBinary: false }));
    act(() => result.current.updateDraft('/repo/README.md', '# Local draft\n'));
    await act(() => result.current.saveFile('/repo/README.md'));

    expect(writeFile).not.toHaveBeenCalled();
    expect(result.current.conflict).toMatchObject({ path: '/repo/README.md' });

    await act(() => result.current.resolveConflict('overwrite'));
    await waitFor(() => expect(result.current.conflict).toBeNull());
    expect(writeFile).toHaveBeenCalledWith('/repo/README.md', '# Local draft\n');
  });
});
