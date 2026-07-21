import { downloadBlob, fileNameWithoutExtension, safeFileName } from './work-file-download';
import { createWorkArtifact } from './work-templates';
import type { WorkArtifact } from './work-types';

let presentationRuntimePromise: Promise<typeof import('pptxgenjs').default> | null = null;

declare global {
  interface Window {
    PptxGenJS?: typeof import('pptxgenjs').default;
  }
}

export async function importWorkPresentationFile(file: File): Promise<WorkArtifact> {
  const { importPptxPresentation } = await import('./work-pptx-import');
  const imported = await importPptxPresentation(file);
  const artifact = createWorkArtifact('blank-presentation');
  artifact.title = fileNameWithoutExtension(file.name);
  artifact.content = imported.content;
  artifact.compatibility = imported.compatibility;
  return artifact;
}

export async function exportWorkPresentationArtifact(artifact: WorkArtifact): Promise<void> {
  downloadBlob(await createWorkPresentationBlob(artifact), `${safeFileName(artifact.title)}.pptx`);
}

export async function createWorkPresentationBlob(artifact: WorkArtifact): Promise<Blob> {
  if (artifact.content.type !== 'presentation') throw new Error('当前文件不是演示文稿。');
  const PptxGenJS = await loadPresentationRuntime();
  const { createPptxBlob } = await import('./work-pptx-export');
  return createPptxBlob(artifact, PptxGenJS);
}

function loadPresentationRuntime(): Promise<typeof import('pptxgenjs').default> {
  if (window.PptxGenJS) return Promise.resolve(window.PptxGenJS);
  if (presentationRuntimePromise) return presentationRuntimePromise;
  const runtime = new Promise<typeof import('pptxgenjs').default>((resolve, reject) => {
    const existing = document.querySelector<HTMLScriptElement>('script[data-a3s-pptx-runtime]');
    const script = existing ?? document.createElement('script');
    const finish = () => {
      if (window.PptxGenJS) resolve(window.PptxGenJS);
      else reject(new Error('PowerPoint export runtime did not initialize'));
    };
    script.addEventListener('load', finish, { once: true });
    script.addEventListener('error', () => reject(new Error('PowerPoint export runtime could not be loaded')), {
      once: true,
    });
    if (!existing) {
      script.src = '/vendor/pptxgen.bundle.js';
      script.dataset.a3sPptxRuntime = 'true';
      document.head.append(script);
    }
  });
  presentationRuntimePromise = runtime.catch((error) => {
    presentationRuntimePromise = null;
    throw error;
  });
  return presentationRuntimePromise;
}
