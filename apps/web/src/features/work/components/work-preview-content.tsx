import { FileQuestion } from 'lucide-react';
import { lazy, Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import { StateView } from '../../../design-system/primitives';
import { formatApiError } from '../../../state/app-state';
import type { WorkspaceEntry } from '../../../types/api';
import { PresentationEditor } from '../editors/presentation-editor';
import { loadWorkQuickLook, type WorkQuickLookContent } from '../work-quick-look-loader';
import type { WorkArtifact } from '../work-types';
import { WorkDocumentPreview } from './work-document-pages';
import { WorkFileIcon } from './work-file-icon';

const SpreadsheetEditor = lazy(() =>
  import('../editors/spreadsheet-editor').then((module) => ({ default: module.SpreadsheetEditor }))
);
const PdfViewer = lazy(() => import('../editors/pdf-viewer').then((module) => ({ default: module.PdfViewer })));
const ignoreArtifactChange = () => undefined;

export function WorkPreviewContent({
  entry,
  reloadKey = 0,
  loadPreview = loadWorkQuickLook,
}: {
  entry: WorkspaceEntry;
  reloadKey?: number;
  loadPreview?: (entry: WorkspaceEntry) => Promise<WorkQuickLookContent>;
}) {
  const [preview, setPreview] = useState<WorkQuickLookContent | null>(null);
  const [error, setError] = useState('');

  useEffect(() => {
    let current = true;
    setPreview(null);
    setError('');
    void loadPreview(entry)
      .then((content) => {
        if (current) setPreview(content);
      })
      .catch((loadError) => {
        if (current) setError(formatApiError(loadError));
      });
    return () => {
      current = false;
    };
  }, [entry, loadPreview, reloadKey]);

  if (error) {
    return (
      <StateView
        className='work-quick-look-state'
        size='compact'
        tone='danger'
        role='alert'
        icon={<FileQuestion size={30} />}
        title='无法生成预览'
        description={error}
      />
    );
  }
  if (!preview) {
    return <StateView className='work-quick-look-state' size='compact' role='status' title='正在生成预览…' />;
  }
  return <PreviewContent entry={entry} preview={preview} />;
}

function PreviewContent({ entry, preview }: { entry: WorkspaceEntry; preview: WorkQuickLookContent }) {
  if (preview.kind === 'directory') {
    return (
      <StateView
        className='work-quick-look-state directory'
        size='compact'
        icon={<WorkFileIcon path={entry.path} directory size={32} />}
        title={entry.name}
        description='这个文件夹没有可预览的入口页面。'
      />
    );
  }
  if (preview.kind === 'unsupported') {
    return (
      <StateView
        className='work-quick-look-state unsupported'
        size='compact'
        icon={<FileQuestion size={38} />}
        title='没有内容预览'
        description={preview.reason}
      />
    );
  }
  if (preview.kind === 'text') {
    return (
      <article className='work-quick-look-text' aria-label='文本文件预览'>
        <pre>{preview.text || '（空文件）'}</pre>
      </article>
    );
  }
  if (preview.kind === 'image') return <PreviewImage blob={preview.blob} name={entry.name} />;
  if (preview.kind === 'pdf') return <PreviewPdf blob={preview.blob} />;
  return <PreviewArtifact artifact={preview.artifact} />;
}

function PreviewImage({ blob, name }: { blob: Blob; name: string }) {
  const url = useMemo(() => URL.createObjectURL(blob), [blob]);
  useEffect(() => () => URL.revokeObjectURL(url), [url]);
  return (
    <figure className='work-quick-look-image'>
      <img src={url} alt={name} />
    </figure>
  );
}

function PreviewPdf({ blob }: { blob: Blob }) {
  const loadSource = useCallback(async () => blob, [blob]);
  return (
    <Suspense
      fallback={
        <StateView className='work-quick-look-state' size='compact' role='status' title='正在准备 PDF 预览器…' />
      }
    >
      <PdfViewer loadSource={loadSource} />
    </Suspense>
  );
}

function PreviewArtifact({ artifact }: { artifact: WorkArtifact }) {
  return (
    <section className={`work-quick-look-artifact ${artifact.kind}`} aria-label={`${artifact.title} 预览`}>
      {artifact.compatibility?.issues.length ? (
        <output className='work-quick-look-compatibility'>
          此文件有 {artifact.compatibility.issues.length} 条兼容性提示；预览不会保存转换结果。
        </output>
      ) : null}
      {artifact.content.type === 'document' && <WorkDocumentPreview content={artifact.content} />}
      {artifact.content.type === 'spreadsheet' && (
        <Suspense
          fallback={
            <StateView className='work-quick-look-state' size='compact' role='status' title='正在准备表格预览器…' />
          }
        >
          <SpreadsheetEditor content={artifact.content} preview onChange={ignoreArtifactChange} />
        </Suspense>
      )}
      {artifact.content.type === 'presentation' && (
        <PresentationEditor content={artifact.content} preview onChange={ignoreArtifactChange} />
      )}
    </section>
  );
}
