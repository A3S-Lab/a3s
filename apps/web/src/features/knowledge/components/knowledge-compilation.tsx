import { AlertTriangle, CheckCircle2, Clock3, DatabaseZap, LoaderCircle, PauseCircle } from 'lucide-react';
import type { ReactNode } from 'react';
import { useSnapshot } from 'valtio';
import { Button } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import type { KnowledgeCompilationSummary, PersonalKnowledgeBase } from '../../../types/api';
import type { KnowledgeActions } from '../use-knowledge-controller';

export function KnowledgeCompilationBadge({ compilation }: { compilation?: KnowledgeCompilationSummary }) {
  if (!compilation) return null;
  const presentation = compilationPresentation(compilation);
  return (
    <span className={`knowledge-compilation-chip phase-${compilation.phase}`} title={presentation.description}>
      {presentation.icon}
      {presentation.label}
    </span>
  );
}

export function KnowledgeCompilationControls({
  knowledgeBase,
  actions,
}: {
  knowledgeBase: PersonalKnowledgeBase;
  actions: KnowledgeActions;
}) {
  const state = useSnapshot(appState);
  const compilation = knowledgeBase.compilation;
  if (knowledgeBase.origin !== 'selection' || !compilation) return null;
  const presentation = compilationPresentation(compilation);
  const compiling = state.knowledgeOperationId === `compile:${knowledgeBase.id}`;
  const changingPolicy = state.knowledgeOperationId === `policy:${knowledgeBase.id}`;
  const active = compilation.phase === 'queued' || compilation.phase === 'running';

  return (
    <section className='knowledge-compilation-controls' aria-label='知识库更新设置'>
      <span className='knowledge-compilation-control-mark' aria-hidden='true'>
        <DatabaseZap size={16} />
      </span>
      <div className='knowledge-compilation-control-copy'>
        <div>
          <strong>更新知识库</strong>
          <KnowledgeCompilationBadge compilation={compilation} />
        </div>
        <p>
          {compilation.pausedReason || compilation.lastError || presentation.description}
          {compilation.pendingChanges && compilation.phase === 'succeeded' ? '；检测到来源变化，建议再次更新。' : ''}
        </p>
      </div>
      <label className='knowledge-compilation-policy'>
        <input
          type='checkbox'
          aria-label='原文件变化后自动更新知识库'
          checked={compilation.policy === 'smart_auto'}
          disabled={changingPolicy}
          onChange={(event) =>
            void actions.setCompilationPolicy(knowledgeBase.id, event.target.checked ? 'smart_auto' : 'manual')
          }
        />
        <span>
          <strong>自动更新</strong>
          <small>文件稳定后更新，且两次更新至少间隔 10 分钟</small>
        </span>
      </label>
      <Button
        tone='primary'
        loading={compiling}
        disabled={active || changingPolicy}
        onClick={() => void actions.requestCompilation(knowledgeBase.id)}
      >
        <DatabaseZap size={14} />
        {compilation.phase === 'succeeded' || compilation.phase === 'failed' ? '再次更新' : '立即更新'}
      </Button>
    </section>
  );
}

function compilationPresentation(compilation: KnowledgeCompilationSummary): {
  label: string;
  description: string;
  icon: ReactNode;
} {
  if (compilation.phase === 'queued') {
    return {
      label: '等待更新',
      description: '已加入队列，稍后会生成可搜索内容。',
      icon: <Clock3 size={10} />,
    };
  }
  if (compilation.phase === 'running') {
    return {
      label: '正在更新',
      description: '正在整理来源并建立可搜索内容。',
      icon: <LoaderCircle className='spin' size={10} />,
    };
  }
  if (compilation.phase === 'succeeded') {
    return {
      label: compilation.pendingChanges ? '来源有变化' : '已更新',
      description: '最近一次更新成功；如果后续更新失败，仍会保留当前可用版本。',
      icon: <CheckCircle2 size={10} />,
    };
  }
  if (compilation.phase === 'failed') {
    return {
      label: '更新失败',
      description: '最近一次更新失败，上一版可搜索内容仍然可用。',
      icon: <AlertTriangle size={10} />,
    };
  }
  if (compilation.phase === 'paused') {
    return {
      label: '自动更新已暂停',
      description: '来源变化规模较大或读取异常，需要检查后手动继续。',
      icon: <PauseCircle size={10} />,
    };
  }
  return {
    label: '可以更新',
    description: '来源已经准备好，更新后即可搜索和引用其中的内容。',
    icon: <Clock3 size={10} />,
  };
}
