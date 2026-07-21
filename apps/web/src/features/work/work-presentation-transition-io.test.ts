import JSZip from 'jszip';
import PptxGenJS from 'pptxgenjs';
import { describe, expect, it } from 'vitest';
import { importWorkFile } from './work-file-io';
import { createPptxBlob } from './work-pptx-export';
import { createWorkArtifact } from './work-templates';

describe('Work presentation transition interoperability', () => {
  it('writes, imports, and reopens editable basic slide transitions', async () => {
    const artifact = createWorkArtifact('strategy-deck');
    if (artifact.content.type !== 'presentation') return;
    artifact.content.slides[0].transition = {
      type: 'push',
      speed: 'slow',
      direction: 'right',
      advanceOnClick: false,
      advanceAfterMs: 3000,
    };
    artifact.content.slides[1].transition = {
      type: 'split',
      speed: 'fast',
      direction: 'out',
      orientation: 'horizontal',
      advanceOnClick: true,
    };

    const blob = await createPptxBlob(artifact, PptxGenJS);
    const archive = await JSZip.loadAsync(await blob.arrayBuffer());
    const firstSlide = await archive.file('ppt/slides/slide1.xml')?.async('text');
    const secondSlide = await archive.file('ppt/slides/slide2.xml')?.async('text');
    expect(firstSlide).toContain('<p:transition spd="slow" advClick="0" advTm="3000"><p:push dir="r"/></p:transition>');
    expect(secondSlide).toContain(
      '<p:transition spd="fast" advClick="1"><p:split orient="horz" dir="out"/></p:transition>'
    );

    const reopened = await importWorkFile(
      new File([blob], 'Transitions.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );
    expect(reopened.content.type).toBe('presentation');
    if (reopened.content.type !== 'presentation') return;
    expect(reopened.content.slides[0].transition).toEqual(artifact.content.slides[0].transition);
    expect(reopened.content.slides[1].transition).toEqual(artifact.content.slides[1].transition);
    expect(reopened.compatibility?.issues.find((issue) => issue.code === 'pptx.transition')).toMatchObject({
      severity: 'info',
      message: expect.stringContaining('editable'),
    });
  });
});
