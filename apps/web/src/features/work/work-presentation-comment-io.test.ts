import JSZip from 'jszip';
import PptxGenJS from 'pptxgenjs';
import { describe, expect, it } from 'vitest';
import { importPptxPresentation } from './work-pptx-import';
import { createPptxBlob } from './work-pptx-export';
import { createWorkArtifact } from './work-templates';

describe('Work PPTX comment interoperability', () => {
  it('round-trips traditional slide comments, authors, dates, and positions', async () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Expected presentation content');
    artifact.content.slides[0].comments = [
      {
        id: 'slide-comment-one',
        author: 'Alice Reviewer',
        initials: 'AR',
        date: '2026-07-21T00:00:00.000Z',
        text: 'Verify the launch metric.',
        x: 25,
        y: 40,
      },
      {
        id: 'slide-comment-two',
        author: 'Bob',
        initials: 'B',
        date: '2026-07-21T01:00:00.000Z',
        text: 'Metric verified.',
        x: 75,
        y: 60,
      },
    ];

    const exported = await createPptxBlob(artifact, PptxGenJS);
    const archive = await JSZip.loadAsync(exported);
    const authorsXml = await archive.file('ppt/commentAuthors.xml')?.async('text');
    const commentsXml = await archive.file('ppt/comments/comment1.xml')?.async('text');
    const slideRelationships = await archive.file('ppt/slides/_rels/slide1.xml.rels')?.async('text');
    const presentationRelationships = await archive.file('ppt/_rels/presentation.xml.rels')?.async('text');
    const contentTypes = await archive.file('[Content_Types].xml')?.async('text');

    expect(authorsXml).toContain('name="Alice Reviewer"');
    expect(authorsXml).toContain('initials="AR"');
    expect(authorsXml).toContain('name="Bob"');
    expect(commentsXml).toContain('Verify the launch metric.');
    expect(commentsXml).toContain('Metric verified.');
    expect(commentsXml).toContain(`x="${Math.round(13.333 * 914_400 * 0.25)}"`);
    expect(commentsXml).toContain('y="2743200"');
    expect(slideRelationships).toContain('/relationships/comments');
    expect(slideRelationships).toContain('../comments/comment1.xml');
    expect(presentationRelationships).toContain('/relationships/commentAuthors');
    expect(contentTypes).toContain('/ppt/commentAuthors.xml');
    expect(contentTypes).toContain('/ppt/comments/comment1.xml');

    const reopened = await importPptxPresentation(
      new File([exported], 'Comment review.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );
    expect(reopened.content.slides[0].comments).toMatchObject([
      {
        author: 'Alice Reviewer',
        initials: 'AR',
        date: '2026-07-21T00:00:00.000Z',
        text: 'Verify the launch metric.',
        x: 25,
        y: 40,
      },
      {
        author: 'Bob',
        initials: 'B',
        date: '2026-07-21T01:00:00.000Z',
        text: 'Metric verified.',
        x: 75,
        y: 60,
      },
    ]);
    expect(reopened.compatibility.issues.find((issue) => issue.code === 'pptx.comments')).toMatchObject({
      severity: 'info',
    });
    expect(reopened.compatibility.issues.some((issue) => issue.code === 'pptx.comments.threaded')).toBe(false);
  });

  it('reports modern threaded comments instead of treating them as editable legacy comments', async () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Expected presentation content');
    artifact.content.slides[0].comments = [
      {
        id: 'source-comment',
        author: 'Alice',
        date: '2026-07-21T00:00:00.000Z',
        text: 'Legacy source',
        x: 50,
        y: 50,
      },
    ];
    const archive = await JSZip.loadAsync(await createPptxBlob(artifact, PptxGenJS));
    archive.file(
      'ppt/comments/comment1.xml',
      [
        '<p188:cmLst xmlns:p188="http://schemas.microsoft.com/office/powerpoint/2018/8/main">',
        '<p188:cm id="{00000000-0000-0000-0000-000000000001}" authorId="{00000000-0000-0000-0000-000000000002}">',
        '<p188:text>Threaded source</p188:text>',
        '</p188:cm>',
        '</p188:cmLst>',
      ].join('')
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const reopened = await importPptxPresentation(
      new File([source], 'Threaded comments.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );

    expect(reopened.content.slides[0].comments).toBeUndefined();
    expect(reopened.compatibility.issues.find((issue) => issue.code === 'pptx.comments.threaded')).toMatchObject({
      severity: 'warning',
    });
    expect(reopened.compatibility.issues.some((issue) => issue.code === 'pptx.comments')).toBe(false);
  });

  it('reports missing traditional comment metadata before normalizing it', async () => {
    const artifact = createWorkArtifact('blank-presentation');
    if (artifact.content.type !== 'presentation') throw new Error('Expected presentation content');
    artifact.content.slides[0].comments = [
      {
        id: 'malformed-comment',
        author: 'Alice',
        date: '2026-07-21T00:00:00.000Z',
        text: 'Metadata source',
        x: 30,
        y: 30,
      },
    ];
    const archive = await JSZip.loadAsync(await createPptxBlob(artifact, PptxGenJS));
    archive.remove('ppt/commentAuthors.xml');
    const commentsXml = await archive.file('ppt/comments/comment1.xml')?.async('text');
    if (!commentsXml) throw new Error('Expected slide comments part');
    archive.file(
      'ppt/comments/comment1.xml',
      commentsXml.replace('dt="2026-07-21T00:00:00.000Z"', 'dt="invalid"').replace(/(<p:pos x=")\d+/, '$1-1')
    );
    const source = await archive.generateAsync({ type: 'blob' });
    const reopened = await importPptxPresentation(
      new File([source], 'Malformed comments.pptx', {
        type: 'application/vnd.openxmlformats-officedocument.presentationml.presentation',
      })
    );

    expect(reopened.content.slides[0].comments?.[0]).toMatchObject({
      author: '未知审阅者',
      date: '',
      text: 'Metadata source',
      x: 50,
    });
    expect(reopened.compatibility.issues.find((issue) => issue.code === 'pptx.comments.metadata')).toMatchObject({
      severity: 'warning',
    });
  });
});
