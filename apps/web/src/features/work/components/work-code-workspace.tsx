import { ArrowLeft, FileCode2, LoaderCircle, RefreshCw, Save, Sparkles, X } from 'lucide-react';
import { lazy, Suspense, useEffect, useState } from 'react';
import { useSnapshot } from 'valtio';
import { Button, Dialog, IconButton, InlineNotice, StateView } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import { MonacoCodeEditor } from '../../workspace/components/monaco-code-editor';
import type { WorkCodeActions } from '../use-work-code-controller';
import type { WorkAgentRequest } from '../work-agent-request';
import { localPathBasename, workFileExtension } from '../work-local-files';
import { WorkFileIcon } from './work-file-icon';

const StreamingMarkdown = lazy(() => import('../../tasks/components/streaming-markdown'));

export function WorkCodeWorkspace({
  actions,
  rootPath,
  assistantOpen,
  onBack,
  onToggleAssistant,
  onAgentRequest,
}: {
  actions: WorkCodeActions;
  rootPath: string;
  assistantOpen: boolean;
  onBack: () => void;
  onToggleAssistant: () => void;
  onAgentRequest: (request: WorkAgentRequest) => void | Promise<void>;
}) {
  const state = useSnapshot(appState);
  const [intelligenceStatus, setIntelligenceStatus] = useState('代码导航连接中');
  const tab = actions.activeTab;
  const dark =
    state.theme === 'dark' || (state.theme === 'system' && document.documentElement.dataset.theme === 'dark');
  const dirty = Boolean(tab && tab.content !== tab.draft);
  const fileStatus = !tab
    ? '未打开文件'
    : tab.loading
      ? '正在读取'
      : tab.loadError
        ? '读取失败'
        : tab.saving
          ? '正在保存'
          : dirty
            ? '未保存'
            : '已保存';

  useEffect(() => setIntelligenceStatus('代码导航连接中'), [tab?.path]);

  return (
    <section className='work-code-workspace work-editor-shell' aria-label='代码文件详情'>
      <header className='work-editor-header work-code-detail-header'>
        <button type='button' className='work-editor-back' aria-label='返回办公文件' onClick={onBack}>
          <ArrowLeft size={17} />
        </button>
        <span className='work-code-detail-icon'>
          <WorkFileIcon path={tab?.path ?? ''} size={17} />
        </span>
        <div className='work-editor-identity work-code-detail-identity'>
          <strong title={tab?.path}>{tab ? localPathBasename(tab.path) : '代码文件'}</strong>
          <span title={tab?.path}>
            {tab ? workFileExtension(tab.path).toLocaleUpperCase() || 'TEXT' : 'TEXT'}
            <i aria-hidden='true'>·</i>
            <span>{fileStatus}</span>
          </span>
        </div>
        <div className='work-editor-header-actions'>
          {tab && (
            <button
              type='button'
              className='work-local-save-button'
              aria-label={tab.saving ? '正在保存代码文件' : '保存代码文件'}
              disabled={tab.loading || tab.saving || !dirty}
              onClick={() => void actions.saveFile(tab.path)}
            >
              {tab.saving ? <LoaderCircle className='spin' size={14} /> : <Save size={15} />}
              {tab.saving ? '保存中' : '保存'}
            </button>
          )}
          <button
            type='button'
            className={`work-editor-ai-button ${assistantOpen ? 'active' : ''}`}
            aria-label={assistantOpen ? '关闭 AI 助手' : '打开 AI 助手'}
            aria-pressed={assistantOpen}
            onClick={onToggleAssistant}
          >
            <Sparkles size={15} />
            AI 助手
          </button>
        </div>
      </header>
      <main className='work-code-detail-body'>
        {!tab ? (
          <StateView
            className='work-code-state'
            size='compact'
            icon={<FileCode2 size={22} />}
            title='没有打开的文件'
            description='返回办公文件后选择一个代码或文本文件。'
          />
        ) : (
          <section className='work-code-panel' aria-label={`编辑 ${localPathBasename(tab.path)}`}>
            {tab.loading ? (
              <EditorState icon={<LoaderCircle className='spin' size={18} />} title='正在读取文件…' />
            ) : tab.loadError ? (
              <EditorState
                icon={<RefreshCw size={18} />}
                title='无法读取文件'
                description={tab.loadError}
                action={() => void actions.openFile({ path: tab.path, isBinary: false })}
              />
            ) : (
              <div className={`work-code-surface ${isMarkdown(tab.path) ? 'markdown-split' : ''}`}>
                <section
                  className='work-code-source-pane'
                  aria-label={isMarkdown(tab.path) ? 'Markdown 编辑区' : '代码编辑区'}
                >
                  <MonacoCodeEditor
                    key={tab.path}
                    path={tab.path}
                    value={tab.draft}
                    location={tab.location}
                    readOnly={false}
                    dark={dark}
                    workspaceRoot={rootPath}
                    sessionId={state.activeSessionId}
                    savedDocument={tab.content === tab.draft}
                    onChange={(value) => actions.updateDraft(tab.path, value)}
                    onSave={() => void actions.saveFile(tab.path)}
                    onClose={() => actions.closeTab(tab.path)}
                    onNavigate={actions.openFile}
                    onStatusChange={setIntelligenceStatus}
                    onAssistantRequest={(request) =>
                      onAgentRequest({
                        workspaceRoot: rootPath,
                        paths: [tab.path],
                        instruction: request.instruction,
                        selection: request.selection,
                      })
                    }
                  />
                </section>
                {isMarkdown(tab.path) && (
                  <section className='work-markdown-preview' aria-label='Markdown 实时预览'>
                    <header>实时预览</header>
                    <div className='execution-markdown'>
                      <Suspense fallback={<p>{tab.draft}</p>}>
                        <StreamingMarkdown content={tab.draft} streaming={false} />
                      </Suspense>
                    </div>
                  </section>
                )}
              </div>
            )}
            <output className='work-code-statusbar' aria-label='编辑器状态'>
              <span className='work-code-statusbar-group'>
                <span>
                  {isMarkdown(tab.path) ? 'Markdown' : workFileExtension(tab.path).toLocaleUpperCase() || 'Plain Text'}
                </span>
                <span>{tab.draft.split(/\r?\n/).length} 行</span>
                {isMarkdown(tab.path) && <span>左侧编辑 · 右侧实时预览</span>}
              </span>
              <span className='work-code-statusbar-group'>
                <span>{tab.content === tab.draft ? '已保存' : '未保存'}</span>
                <span>UTF-8</span>
                <span>LF</span>
                {!tab.loading && !tab.loadError && <span>{intelligenceStatus}</span>}
              </span>
            </output>
          </section>
        )}
      </main>
      {actions.conflict && (
        <InlineNotice
          className='work-code-conflict'
          tone='warning'
          role='alert'
          title='文件已在其他应用中修改'
          actions={
            <>
              <Button tone='quiet' onClick={() => void actions.resolveConflict('reload')}>
                载入磁盘版本
              </Button>
              <Button tone='primary' onClick={() => void actions.resolveConflict('overwrite')}>
                保留当前编辑
              </Button>
              <IconButton label='关闭保存冲突提示' onClick={actions.dismissConflict}>
                <X size={14} />
              </IconButton>
            </>
          }
        >
          <code>{actions.conflict.path}</code>
        </InlineNotice>
      )}
      {actions.closeRequest && (
        <Dialog
          title='放弃未保存的更改？'
          description={actions.closeRequest.message}
          onClose={actions.dismissCloseRequest}
          footer={
            <>
              <Button tone='quiet' onClick={actions.dismissCloseRequest}>
                继续编辑
              </Button>
              <Button tone='danger' onClick={actions.confirmCloseRequest}>
                放弃更改
              </Button>
            </>
          }
        >
          <InlineNotice className='work-library-operation-warning' tone='danger' role='alert'>
            此操作无法撤销。
          </InlineNotice>
        </Dialog>
      )}
    </section>
  );
}

function EditorState({
  icon,
  title,
  description,
  action,
}: {
  icon: React.ReactNode;
  title: string;
  description?: string;
  action?: () => void;
}) {
  return (
    <StateView
      className='work-code-state'
      size='compact'
      tone={action ? 'danger' : 'neutral'}
      role={action ? 'alert' : 'status'}
      icon={icon}
      title={title}
      description={description}
      actions={action && <Button onClick={action}>重试</Button>}
    />
  );
}

function isMarkdown(path: string): boolean {
  return ['md', 'markdown', 'mdx'].includes(workFileExtension(path));
}
