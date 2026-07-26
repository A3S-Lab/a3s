import { BookOpen, CheckCircle2, DatabaseZap, LoaderCircle, X } from 'lucide-react';
import { type FormEvent, useEffect, useMemo, useState } from 'react';
import { Button, IconButton, InlineNotice } from '../../../design-system/primitives';
import { codeApi } from '../../../lib/api';
import { appState, formatApiError, navigateKnowledgeBase, showToast } from '../../../state/app-state';
import type {
  KnowledgeCompilationPolicy,
  KnowledgeSourceSelectionPreview,
  PersonalKnowledgeBase,
} from '../../../types/api';

export function WorkKnowledgeBasePanel({
  workspaceRoot,
  paths,
  onClose,
}: {
  workspaceRoot: string;
  paths: readonly string[];
  onClose: () => void;
}) {
  const [preview, setPreview] = useState<KnowledgeSourceSelectionPreview | null>(null);
  const [name, setName] = useState('');
  const [description, setDescription] = useState('');
  const [policy, setPolicy] = useState<KnowledgeCompilationPolicy>('manual');
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [queueing, setQueueing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [created, setCreated] = useState<PersonalKnowledgeBase | null>(null);
  const requestPaths = useMemo(() => [...paths], [paths]);
  const requestKey = requestPaths.join('\n');
  const compilationActive = created?.compilation?.phase === 'queued' || created?.compilation?.phase === 'running';

  useEffect(() => {
    const controller = new AbortController();
    setPreview(null);
    setLoading(true);
    setError(null);
    void codeApi
      .previewKnowledgeBaseSelection({ workspace: workspaceRoot, paths: requestPaths }, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return;
        setPreview(result);
        setName((current) => current || result.suggestedName);
      })
      .catch((previewError) => {
        if (!controller.signal.aborted) setError(formatApiError(previewError));
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [requestKey, requestPaths, workspaceRoot]);

  const create = async (event: FormEvent) => {
    event.preventDefault();
    if (!preview || !name.trim() || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const mutation = await codeApi.createKnowledgeBaseFromSelection({
        workspace: workspaceRoot,
        paths: requestPaths,
        name: name.trim(),
        description: description.trim() || undefined,
        compilationPolicy: policy,
      });
      rememberKnowledgeBase(mutation.knowledgeBase, workspaceRoot);
      setCreated(mutation.knowledgeBase);
      showToast('知识库已创建，来源已准备但尚未编译。', 'success');
    } catch (createError) {
      setError(formatApiError(createError));
    } finally {
      setSubmitting(false);
    }
  };

  const compileNow = async () => {
    if (!created || queueing) return;
    setQueueing(true);
    setError(null);
    try {
      const mutation = await codeApi.requestKnowledgeCompilation(created.id, workspaceRoot);
      rememberKnowledgeBase(mutation.knowledgeBase, workspaceRoot);
      setCreated(mutation.knowledgeBase);
      showToast(mutation.changed ? '知识编译已加入队列。' : '知识编译已在处理中。', 'success');
    } catch (compileError) {
      setError(formatApiError(compileError));
    } finally {
      setQueueing(false);
    }
  };

  return (
    <aside className='work-knowledge-builder' aria-label='从所选项目创建知识库'>
      <IconButton className='work-knowledge-builder-close' label='关闭知识库创建面板' onClick={onClose}>
        <X size={15} />
      </IconButton>
      {created ? (
        <div className='work-knowledge-builder-success'>
          <span className='work-knowledge-builder-mark success' aria-hidden='true'>
            <CheckCircle2 size={19} />
          </span>
          <div>
            <strong>{created.name} 已创建</strong>
            <p>
              来源已准备，尚未编译。创建与编译是两个独立步骤；编译器只会把结果写入 <code>wiki/</code>。
            </p>
          </div>
          <span className={`knowledge-compilation-chip phase-${created.compilation?.phase ?? 'source_ready'}`}>
            {created.compilation?.phase === 'queued' ? '等待编译' : '来源已准备'}
          </span>
          <div className='work-knowledge-builder-actions'>
            <Button tone='primary' loading={queueing} disabled={compilationActive} onClick={() => void compileNow()}>
              <DatabaseZap size={14} />
              {created.compilation?.phase === 'queued'
                ? '已加入队列'
                : created.compilation?.phase === 'running'
                  ? '编译中'
                  : '立即编译'}
            </Button>
            <Button onClick={() => navigateKnowledgeBase(created.id, workspaceRoot)}>
              <BookOpen size={14} />
              在知识中查看
            </Button>
          </div>
        </div>
      ) : (
        <form className='work-knowledge-builder-form' onSubmit={(event) => void create(event)}>
          <span className='work-knowledge-builder-mark' aria-hidden='true'>
            {loading ? <LoaderCircle className='spin' size={18} /> : <DatabaseZap size={18} />}
          </span>
          <div className='work-knowledge-builder-copy'>
            <strong>创建知识库</strong>
            <p>
              {preview
                ? `${preview.selectedCount} 个选择已整理为 ${preview.sourceRootCount} 个来源根 · ${preview.fileCount} 个文件 · ${formatBytes(preview.bytes)}`
                : '正在检查所选文件与文件夹…'}
            </p>
            <small>创建与编译分开：本步骤只准备来源，不会立即编译。</small>
          </div>
          <label>
            <span>名称</span>
            <input
              maxLength={80}
              value={name}
              aria-label='知识库名称'
              placeholder='知识库名称'
              onChange={(event) => setName(event.target.value)}
            />
          </label>
          <label className='work-knowledge-description'>
            <span>描述（可选）</span>
            <input
              maxLength={280}
              value={description}
              aria-label='知识库描述'
              placeholder='这份知识包含什么'
              onChange={(event) => setDescription(event.target.value)}
            />
          </label>
          <label className='work-knowledge-auto-policy'>
            <input
              type='checkbox'
              checked={policy === 'smart_auto'}
              onChange={(event) => setPolicy(event.target.checked ? 'smart_auto' : 'manual')}
            />
            <span>
              <strong>原文件变化后智能编译</strong>
              <small>稳定 5 秒且静默 30 秒后触发；两次自动编译至少间隔 10 分钟</small>
            </span>
          </label>
          <Button tone='primary' type='submit' loading={submitting} disabled={loading || !preview || !name.trim()}>
            创建知识库
          </Button>
        </form>
      )}
      {error && (
        <InlineNotice className='work-knowledge-builder-error' tone='danger' role='alert'>
          {error}
        </InlineNotice>
      )}
    </aside>
  );
}

function rememberKnowledgeBase(base: PersonalKnowledgeBase, workspaceRoot: string): void {
  appState.knowledgeWorkspace = workspaceRoot;
  const catalog = appState.personalKnowledgeBases;
  if (!catalog || catalog.workspaceRoot !== workspaceRoot) {
    appState.personalKnowledgeBases = null;
    appState.knowledgeStatus = 'idle';
    return;
  }
  const items = catalog.items.filter((item) => item.id !== base.id);
  items.push(base);
  items.sort(
    (left, right) =>
      Number(right.pinned) - Number(left.pinned) ||
      right.updatedAt.localeCompare(left.updatedAt) ||
      left.name.localeCompare(right.name)
  );
  appState.personalKnowledgeBases = { ...catalog, items, total: items.length };
}

function formatBytes(value: number): string {
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MiB`;
}
