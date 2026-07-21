import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createWorkArtifact } from '../work-templates';
import { WorkLocalFileConflictDialog, WorkLocalSaveDialog } from './work-local-save-dialog';

describe('Work local save dialogs', () => {
  afterEach(cleanup);

  it('requires explicit replacement after Save As finds an existing file', async () => {
    const artifact = createWorkArtifact('blank-document');
    artifact.title = 'Launch plan';
    const onSave = vi.fn().mockResolvedValueOnce('exists').mockResolvedValueOnce('saved');
    const onClose = vi.fn();

    render(
      <WorkLocalSaveDialog
        artifact={artifact}
        defaultDirectory='/docs'
        onClose={onClose}
        onPickDirectory={vi.fn()}
        onSave={onSave}
      />
    );

    expect(screen.getByLabelText('本地文件名')).toHaveValue('Launch plan.docx');
    fireEvent.click(screen.getByRole('button', { name: '保存' }));

    await screen.findByText('此位置已有同名文件。再次确认后才会替换它。');
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: '替换文件' }));

    await waitFor(() => expect(onClose).toHaveBeenCalled());
    expect(onSave).toHaveBeenNthCalledWith(1, '/docs', 'Launch plan.docx', false);
    expect(onSave).toHaveBeenNthCalledWith(2, '/docs', 'Launch plan.docx', true);
  });

  it('offers Save As before an explicit external-version overwrite', async () => {
    const onSaveAs = vi.fn();
    const onOverwrite = vi.fn().mockResolvedValue(true);

    render(
      <WorkLocalFileConflictDialog
        conflict={{ path: '/docs/Plan.docx', missing: false }}
        onClose={vi.fn()}
        onSaveAs={onSaveAs}
        onOverwrite={onOverwrite}
      />
    );

    expect(screen.getByText('/docs/Plan.docx')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '另存为' }));
    expect(onSaveAs).toHaveBeenCalled();
    expect(onOverwrite).not.toHaveBeenCalled();
  });
});
