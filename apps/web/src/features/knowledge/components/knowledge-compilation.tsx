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
    <section className='knowledge-compilation-controls' aria-label='知识编译设置'>
      <span className='knowledge-compilation-control-mark' aria-hidden='true'>
        <DatabaseZap size={16} />
      </span>
      <div className='knowledge-compilation-control-copy'>
        <div>
          <strong>知识编译</strong>
          <KnowledgeCompilationBadge compilation={compilation} />
        </div>
        <p>
          {compilation.pausedReason || compilation.lastError || presentation.description}
          {compilation.pendingChanges && compilation.phase === 'succeeded' ? '；检测到新来源，建议重新编译。' : ''}
        </p>
      </div>
      <label className='knowledge-compilation-policy'>
        <input
          type='checkbox'
          aria-label='原文件变化后智能自动编译'
          checked={compilation.policy === 'smart_auto'}
          disabled={changingPolicy}
          onChange={(event) =>
            void actions.setCompilationPolicy(knowledgeBase.id, event.target.checked ? 'smart_auto' : 'manual')
          }
        />
        <span>
          <strong>智能自动编译</strong>
          <small>稳定 5 秒 + 静默 30 秒；至少间隔 10 分钟</small>
        </span>
      </label>
      <Button
        tone='primary'
        loading={compiling}
        disabled={active || changingPolicy}
        onClick={() => void actions.requestCompilation(knowledgeBase.id)}
      >
        <DatabaseZap size={14} />
        {compilation.phase === 'succeeded' || compilation.phase === 'failed' ? '重新编译' : '立即编译'}
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
      label: '等待编译',
      description: '已加入队列，等待独立知识编译器领取任务。',
      icon: <Clock3 size={10} />,
    };
  }
  if (compilation.phase === 'running') {
    return {
      label: '编译中',
      description: '独立知识编译器正在生成 OKF wiki。',
      icon: <LoaderCircle className='spin' size={10} />,
    };
  }
  if (compilation.phase === 'succeeded') {
    return {
      label: compilation.pendingChanges ? '有新来源' : '已编译',
      description: '最近一次知识编译已成功，旧版本会在失败时继续保留。',
      icon: <CheckCircle2 size={10} />,
    };
  }
  if (compilation.phase === 'failed') {
    return {
      label: '编译失败',
      description: '最近一次编译失败，上一版 wiki 未被覆盖。',
      icon: <AlertTriangle size={10} />,
    };
  }
  if (compilation.phase === 'paused') {
    return {
      label: '自动编译已暂停',
      description: '来源变化规模较大或读取异常，需要检查后手动继续。',
      icon: <PauseCircle size={10} />,
    };
  }
  return {
    label: '来源已准备',
    description: '知识库已经创建，但尚未运行知识编译。',
    icon: <Clock3 size={10} />,
  };
}
