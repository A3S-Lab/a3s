import { documentContentLayoutProperties } from './work-document-section';
import { downloadBlob, fileNameWithoutExtension, safeFileName } from './work-file-download';
import { createWorkArtifact } from './work-templates';
import type { WorkArtifact } from './work-types';

export async function importWorkDocumentFile(file: File, extension: string): Promise<WorkArtifact> {
  let html: string;
  if (extension === 'docx') {
    const mammoth = await import('mammoth');
    const arrayBuffer = await file.arrayBuffer();
    const { applyDocxSectionsToHtml, prepareDocxImport, readDocxLayout } = await import('./work-docx-import');
    const prepared = await prepareDocxImport(arrayBuffer).catch(() => null);
    const conversionBuffer = prepared?.conversionBuffer ?? arrayBuffer;
    const input = {
      arrayBuffer: conversionBuffer,
      buffer: conversionBuffer,
    } as unknown as Parameters<typeof mammoth.convertToHtml>[0];
    const result = await mammoth.convertToHtml(input, {
      styleMap: ["br[type='page'] => hr.work-page-break[data-page-break='true']:fresh"],
    });
    html = prepared
      ? applyDocxSectionsToHtml(
          result.value,
          prepared.sections,
          prepared.captionMarkers,
          prepared.changeMarkers,
          prepared.commentMarkers,
          prepared.fieldMarkers,
          prepared.citationMarkers,
          prepared.bibliography
        )
      : result.value;
    const layout = prepared
      ? documentContentLayoutProperties(prepared.sections[0].layout)
      : await readDocxLayout(arrayBuffer).catch(() => ({ pageSize: 'a4' as const }));
    const { analyzeDocxCompatibility } = await import('./work-office-diagnostics');
    const artifact = createWorkArtifact('blank-document');
    artifact.title = fileNameWithoutExtension(file.name);
    artifact.content = {
      type: 'document',
      html,
      ...layout,
      ...(prepared?.trackChanges ? { trackChanges: true } : {}),
      ...(prepared?.commentMarkers.comments.length ? { comments: prepared.commentMarkers.comments } : {}),
      ...(prepared?.bibliography ? { bibliography: prepared.bibliography } : {}),
    };
    artifact.compatibility = await analyzeDocxCompatibility(file, result.messages);
    return artifact;
  }

  const source = await file.text();
  html = extension === 'html' || extension === 'htm' ? source : textToHtml(source);
  const artifact = createWorkArtifact('blank-document');
  artifact.title = fileNameWithoutExtension(file.name);
  artifact.content = { type: 'document', html, pageSize: 'a4' };
  return artifact;
}

export async function exportWorkDocumentArtifact(artifact: WorkArtifact): Promise<void> {
  downloadBlob(await createWorkDocumentBlob(artifact), `${safeFileName(artifact.title)}.docx`);
}

export async function createWorkDocumentBlob(artifact: WorkArtifact): Promise<Blob> {
  if (artifact.content.type !== 'document') throw new Error('当前文件不是文档。');
  const { createDocxBlob } = await import('./work-docx-export');
  return createDocxBlob(artifact.content);
}

function textToHtml(source: string): string {
  const blocks = source
    .replace(/\r\n?/g, '\n')
    .split(/\n{2,}/)
    .map((block) => block.trim())
    .filter(Boolean);
  return blocks
    .map((block) => {
      if (block.startsWith('# ')) return `<h1>${escapeHtml(block.slice(2))}</h1>`;
      if (block.startsWith('## ')) return `<h2>${escapeHtml(block.slice(3))}</h2>`;
      return `<p>${escapeHtml(block).replace(/\n/g, '<br>')}</p>`;
    })
    .join('');
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;');
}
