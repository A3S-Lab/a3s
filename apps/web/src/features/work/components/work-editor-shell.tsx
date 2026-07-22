import {
  AlertTriangle,
  ArrowLeft,
  Cloud,
  CloudOff,
  Download,
  Eye,
  FileDown,
  FileText,
  FileType2,
  FolderOutput,
  History,
  Info,
  Pencil,
  Presentation,
  Printer,
  Save,
  Sheet,
  Sparkles,
  Star,
} from 'lucide-react';
import { lazy, Suspense, useEffect, useState } from 'react';
import { DocumentEditor } from '../editors/document-editor';
import { OfficeTextField } from '../editors/office-controls';
import { PresentationEditor } from '../editors/presentation-editor';
import type { WorkOfficeFileAction } from '../editors/work-office-chrome';
import type { WorkActions } from '../use-work-controller';
import type { WorkEditorAgentRequest } from '../work-agent-request';
import { localPathParent } from '../work-local-files';
import type { WorkArtifactContent, WorkArtifactKind, WorkPresentationPrintLayout } from '../work-types';
import { workArtifactExtension, workArtifactKindLabel } from '../work-types';
import { WorkCompatibilityDialog } from './work-compatibility-dialog';
import { WorkLocalFileConflictDialog, WorkLocalSaveDialog } from './work-local-save-dialog';
import { WorkPdfExportSurface } from './work-pdf-export-surface';
import { WorkPrintPreviewDialog } from './work-print-preview-dialog';
import { WorkVersionHistoryDialog } from './work-version-history-dialog';

const SpreadsheetEditor = lazy(() =>
  import('../editors/spreadsheet-editor').then((module) => ({ default: module.SpreadsheetEditor }))
);
const PdfViewer = lazy(() => import('../editors/pdf-viewer').then((module) => ({ default: module.PdfViewer })));

export function WorkEditorShell({
  actions,
  copilotOpen = false,
  onToggleCopilot,
  onAgentRequest,
  defaultLocalDirectory = '',
  onPickLocalDirectory,
  onLocalFileSaved,
}: {
  actions: WorkActions;
  copilotOpen?: boolean;
  onToggleCopilot?: () => void;
  onAgentRequest?: (request: WorkEditorAgentRequest) => void | Promise<void>;
  defaultLocalDirectory?: string;
  onPickLocalDirectory?: () => Promise<string | null>;
  onLocalFileSaved?: () => void;
}) {
  const artifact = actions.activeArtifact;
  const [preview, setPreview] = useState(false);
  const [showPrintPreview, setShowPrintPreview] = useState(false);
  const [showVersions, setShowVersions] = useState(false);
  const [showLocalSaveAs, setShowLocalSaveAs] = useState(false);
  const [compatibilityMode, setCompatibilityMode] = useState<'review' | 'native' | 'pdf' | 'local' | 'local-as' | null>(
    null
  );
  const [presentationPrintLayout, setPresentationPrintLayout] = useState<WorkPresentationPrintLayout>('slides');
  const [pendingPdfPageIndexes, setPendingPdfPageIndexes] = useState<number[] | null>(null);
  const saveBoundLocalFile = async (force = false) => {
    const saved = await actions.saveLocalFile({ force });
    if (saved) onLocalFileSaved?.();
    return saved;
  };
  const requestBoundLocalSave = () => {
    if (!artifact || artifact.kind === 'pdf' || !actions.activeLocalBinding) return;
    if (artifact.compatibility?.issues.length) {
      setCompatibilityMode('local');
      return;
    }
    void saveBoundLocalFile();
  };
  const requestPrimarySave = () => {
    if (!artifact || artifact.kind === 'pdf') return;
    if (actions.activeLocalBinding) requestBoundLocalSave();
    else void actions.saveNow();
  };
  const requestLocalSaveAs = () => {
    if (!artifact || artifact.kind === 'pdf') return;
    if (artifact.compatibility?.issues.length) setCompatibilityMode('local-as');
    else setShowLocalSaveAs(true);
  };

  useEffect(() => {
    setPreview(false);
    setPresentationPrintLayout('slides');
    setShowPrintPreview(false);
    setPendingPdfPageIndexes(null);
  }, [artifact?.id]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        event.repeat ||
        event.altKey ||
        !(event.metaKey || event.ctrlKey) ||
        !artifact ||
        artifact.kind === 'pdf'
      ) {
        return;
      }
      const key = event.key.toLowerCase();
      if (key === 'p' && !event.shiftKey) {
        event.preventDefault();
        setShowPrintPreview(true);
      } else if (key === 's') {
        event.preventDefault();
        if (event.shiftKey) requestLocalSaveAs();
        else requestPrimarySave();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [actions, artifact]);

  useEffect(() => {
    const path = actions.activeLocalBinding?.path;
    if (!path || artifact?.kind === 'pdf') return;
    void actions.checkLocalFile();
    const onFocus = () => void actions.checkLocalFile();
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [
    actions.activeLocalBinding?.fingerprint,
    actions.activeLocalBinding?.path,
    actions.checkLocalFile,
    artifact?.kind,
  ]);

  if (!artifact) return null;

  const updateContent = (content: WorkArtifactContent) => {
    actions.updateArtifact((current) => ({ ...current, content }));
  };
  const requestPdfExport = (pageIndexes: number[]) => {
    if (artifact.compatibility?.issues.length) {
      setPendingPdfPageIndexes(pageIndexes);
      setShowPrintPreview(false);
      setCompatibilityMode('pdf');
      return;
    }
    void actions.exportPdf({ pageIndexes });
  };
  const requestNativeExport = () => {
    if (artifact.compatibility?.issues.length) setCompatibilityMode('native');
    else void actions.exportArtifact();
  };
  const officeFileActions: readonly WorkOfficeFileAction[] =
    artifact.kind === 'pdf'
      ? []
      : [
          {
            id: 'save',
            label: '保存',
            icon: <Save size={16} />,
            shortcut: 'Cmd/Ctrl+S',
            disabled: actions.saveState === 'saving' || actions.localSaveState === 'saving',
            onSelect: requestPrimarySave,
          },
          {
            id: 'save-as',
            label: '另存为',
            icon: <FolderOutput size={16} />,
            shortcut: 'Cmd/Ctrl+Shift+S',
            onSelect: requestLocalSaveAs,
          },
          {
            id: 'print',
            label: '打印',
            icon: <Printer size={16} />,
            shortcut: 'Cmd/Ctrl+P',
            separatorBefore: true,
            onSelect: () => setShowPrintPreview(true),
          },
          {
            id: 'export',
            label: `导出 ${workArtifactExtension(artifact.kind).toUpperCase()}`,
            icon: <Download size={16} />,
            disabled: actions.exporting,
            onSelect: requestNativeExport,
          },
          ...(artifact.source
            ? [
                {
                  id: 'download-source',
                  label: '下载原文件',
                  icon: <FileDown size={16} />,
                  onSelect: () => void actions.downloadSource(),
                },
              ]
            : []),
          {
            id: 'versions',
            label: '版本记录',
            icon: <History size={16} />,
            separatorBefore: true,
            onSelect: () => setShowVersions(true),
          },
        ];

  return (
    <section className={`work-editor-shell ${artifact.kind}`}>
      <header className='work-editor-header'>
        <button
          type='button'
          className='work-editor-back'
          aria-label='返回办公文件中心'
          onClick={() => void actions.closeArtifact()}
        >
          <ArrowLeft size={17} />
        </button>
        <span className={`work-file-kind-icon ${artifact.kind}`}>
          <WorkKindIcon kind={artifact.kind} />
        </span>
        <div className='work-editor-identity'>
          <OfficeTextField
            value={artifact.title}
            aria-label='文件名'
            onChange={(event) => {
              const title = event.target.value;
              actions.updateArtifact((current) => ({ ...current, title }));
            }}
            onBlur={() => {
              if (artifact.title.trim()) return;
              actions.updateArtifact((current) => ({
                ...current,
                title: `无标题${workArtifactKindLabel(current.kind)}`,
              }));
            }}
          />
          <span>
            {workArtifactExtension(artifact.kind).toUpperCase()}
            <i aria-hidden='true'>·</i>
            <SaveStatus state={actions.saveState} storageMode={actions.storageMode} />
          </span>
        </div>
        <div className='work-editor-header-actions'>
          <button
            type='button'
            className={`work-icon-button ${artifact.favorite ? 'active' : ''}`}
            aria-label={artifact.favorite ? '取消收藏' : '收藏'}
            aria-pressed={artifact.favorite}
            title={artifact.favorite ? '取消收藏' : '收藏'}
            onClick={() => actions.toggleFavorite(artifact.id)}
          >
            <Star size={16} fill={artifact.favorite ? 'currentColor' : 'none'} />
          </button>
          {artifact.kind !== 'pdf' && (
            <button
              type='button'
              className='work-local-save-button'
              aria-label={actions.activeLocalBinding ? '保存到原本地文件' : '保存到 A3S'}
              title={actions.activeLocalBinding?.path}
              disabled={actions.saveState === 'saving' || actions.localSaveState === 'saving'}
              onClick={requestPrimarySave}
            >
              <Save size={15} />
              {actions.saveState === 'saving' || actions.localSaveState === 'saving' ? '正在保存…' : '保存'}
            </button>
          )}
          {artifact.kind !== 'pdf' && (
            <fieldset className='work-preview-switch'>
              <legend className='sr-only'>编辑或预览</legend>
              <button
                type='button'
                className={!preview ? 'active' : ''}
                aria-pressed={!preview}
                onClick={() => setPreview(false)}
              >
                <Pencil size={14} />
                编辑
              </button>
              <button
                type='button'
                className={preview ? 'active' : ''}
                aria-pressed={preview}
                onClick={() => setPreview(true)}
              >
                <Eye size={15} />
                预览
              </button>
            </fieldset>
          )}
          {onToggleCopilot && (
            <button
              type='button'
              className={`work-editor-ai-button ${copilotOpen ? 'active' : ''}`}
              aria-label={copilotOpen ? '关闭 AI 助手' : '打开 AI 助手'}
              aria-pressed={copilotOpen}
              onClick={onToggleCopilot}
            >
              <Sparkles size={15} />
              AI 助手
            </button>
          )}
          {artifact.kind === 'pdf' && (
            <>
              {artifact.source && (
                <button
                  type='button'
                  className='work-icon-button'
                  aria-label={`下载原始文件 ${artifact.source.name}`}
                  title={`下载原始文件：${artifact.source.name}`}
                  onClick={() => void actions.downloadSource()}
                >
                  <FileDown size={16} />
                </button>
              )}
              <button
                type='button'
                className='work-icon-button'
                aria-label='查看版本历史'
                onClick={() => setShowVersions(true)}
              >
                <History size={16} />
              </button>
              <button
                type='button'
                className='work-export-button'
                disabled={actions.exporting}
                onClick={() => void actions.downloadSource()}
              >
                <Download size={15} />
                {actions.exporting ? '正在下载…' : '下载 PDF'}
              </button>
            </>
          )}
        </div>
      </header>

      {actions.activeLocalBinding && actions.localSaveState === 'conflict' ? (
        <output className='work-source-copy-banner local-binding conflict'>
          <AlertTriangle size={14} />
          <span>
            本地文件已在外部修改：
            <strong title={actions.activeLocalBinding.path}>{actions.activeLocalBinding.path}</strong>
          </span>
        </output>
      ) : artifact.source ? (
        <output className='work-source-copy-banner'>
          <Info size={14} />
          <span>
            副本：<strong>{artifact.source.name}</strong>
            {' · 原文件不受影响'}
          </span>
        </output>
      ) : null}

      {artifact.compatibility?.issues.length ? (
        <button type='button' className='work-compatibility-banner' onClick={() => setCompatibilityMode('review')}>
          <AlertTriangle size={15} />
          <span>
            此 {artifact.compatibility.sourceFormat} 文件有 {artifact.compatibility.issues.length} 条兼容性提示
          </span>
          <strong>查看详情</strong>
        </button>
      ) : null}

      <div className='work-editor-body'>
        {artifact.content.type === 'document' && (
          <DocumentEditor
            key={artifact.id}
            content={artifact.content}
            preview={preview}
            saveStatus={workSaveStatusText(actions.saveState, actions.storageMode)}
            fileActions={officeFileActions}
            onChange={updateContent}
            onAgentRequest={onAgentRequest}
          />
        )}
        {artifact.content.type === 'spreadsheet' && (
          <Suspense fallback={<output className='work-editor-loading'>正在准备表格编辑器…</output>}>
            <SpreadsheetEditor
              key={artifact.id}
              content={artifact.content}
              preview={preview}
              saveStatus={workSaveStatusText(actions.saveState, actions.storageMode)}
              fileActions={officeFileActions}
              onChange={updateContent}
              onAgentRequest={onAgentRequest}
            />
          </Suspense>
        )}
        {artifact.content.type === 'presentation' && (
          <PresentationEditor
            key={artifact.id}
            content={artifact.content}
            preview={preview}
            saveStatus={workSaveStatusText(actions.saveState, actions.storageMode)}
            fileActions={officeFileActions}
            onChange={updateContent}
            onAgentRequest={onAgentRequest}
            onStartSlideshow={() => setPreview(true)}
          />
        )}
        {artifact.content.type === 'pdf' && (
          <Suspense fallback={<output className='work-editor-loading'>正在准备 PDF 预览器…</output>}>
            <PdfViewer
              key={artifact.id}
              fileName={artifact.source?.name ?? `${artifact.title}.pdf`}
              loadSource={actions.sourceBlob}
              saveLabel={actions.activeLocalBinding ? '保存并写回本地' : '保存到 A3S'}
              sourceKey={artifact.id}
              onSave={actions.savePdfSource}
            />
          </Suspense>
        )}
      </div>
      <WorkPdfExportSurface artifact={artifact} presentationLayout={presentationPrintLayout} />
      {showPrintPreview && artifact.kind !== 'pdf' && (
        <WorkPrintPreviewDialog
          artifact={artifact}
          presentationLayout={presentationPrintLayout}
          exportingPdf={actions.exportingPdf}
          onPresentationLayoutChange={setPresentationPrintLayout}
          onClose={() => setShowPrintPreview(false)}
          onExportPdf={requestPdfExport}
          onPrint={async () => {
            if (await actions.saveNow()) window.print();
          }}
          onReviewCompatibility={
            artifact.compatibility?.issues.length
              ? () => {
                  setShowPrintPreview(false);
                  setCompatibilityMode('review');
                }
              : undefined
          }
        />
      )}
      {compatibilityMode && artifact.compatibility && (
        <WorkCompatibilityDialog
          report={artifact.compatibility}
          mode={
            compatibilityMode === 'review'
              ? 'review'
              : compatibilityMode === 'local' || compatibilityMode === 'local-as'
                ? 'save'
                : 'export'
          }
          busy={actions.exporting || actions.exportingPdf}
          onClose={() => {
            setCompatibilityMode(null);
            if (compatibilityMode === 'pdf') setPendingPdfPageIndexes(null);
          }}
          onConfirm={
            compatibilityMode !== 'review'
              ? async () => {
                  if (compatibilityMode === 'pdf') {
                    await actions.exportPdf({
                      pageIndexes: pendingPdfPageIndexes ?? undefined,
                    });
                    setPendingPdfPageIndexes(null);
                  } else if (compatibilityMode === 'native') {
                    await actions.exportArtifact();
                  } else if (compatibilityMode === 'local') {
                    await saveBoundLocalFile();
                  } else {
                    setShowLocalSaveAs(true);
                  }
                  setCompatibilityMode(null);
                }
              : undefined
          }
        />
      )}
      {showLocalSaveAs && (
        <WorkLocalSaveDialog
          artifact={artifact}
          defaultDirectory={
            defaultLocalDirectory ||
            (actions.activeLocalBinding ? localPathParent(actions.activeLocalBinding.path) : '')
          }
          onClose={() => setShowLocalSaveAs(false)}
          onPickDirectory={onPickLocalDirectory ?? (async () => null)}
          onSave={async (directory, fileName, allowOverwrite) => {
            const result = await actions.saveLocalFileAs(directory, fileName, { allowOverwrite });
            if (result === 'saved') onLocalFileSaved?.();
            return result;
          }}
        />
      )}
      {actions.localConflict && (
        <WorkLocalFileConflictDialog
          conflict={actions.localConflict}
          onClose={actions.dismissLocalConflict}
          onSaveAs={() => {
            actions.dismissLocalConflict();
            setShowLocalSaveAs(true);
          }}
          onOverwrite={() => saveBoundLocalFile(true)}
        />
      )}
      {showVersions && <WorkVersionHistoryDialog actions={actions} onClose={() => setShowVersions(false)} />}
    </section>
  );
}

function WorkKindIcon({ kind }: { kind: WorkArtifactKind }) {
  if (kind === 'spreadsheet') return <Sheet size={17} />;
  if (kind === 'presentation') return <Presentation size={17} />;
  if (kind === 'pdf') return <FileType2 size={17} />;
  return <FileText size={17} />;
}

function SaveStatus({
  state,
  storageMode,
}: {
  state: WorkActions['saveState'];
  storageMode: WorkActions['storageMode'];
}) {
  if (state === 'error') {
    return (
      <>
        <CloudOff size={11} />
        保存失败
      </>
    );
  }
  return (
    <>
      <Cloud size={11} />
      {workSaveStatusText(state, storageMode)}
    </>
  );
}

function workSaveStatusText(state: WorkActions['saveState'], storageMode: WorkActions['storageMode']): string {
  if (state === 'error') return '保存失败';
  if (state === 'saving') return '正在保存';
  if (state === 'dirty') return '等待保存';
  return storageMode === 'server' ? '已保存到 A3S' : '已保存到此设备';
}
