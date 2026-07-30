import {
  Archive,
  ArrowRight,
  Check,
  ChevronDown,
  FlaskConical,
  LoaderCircle,
  RefreshCw,
  ShieldCheck,
  Sparkles,
  WandSparkles,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import { useSnapshot } from 'valtio';
import { Button, CollectionState, Dialog, InlineNotice } from '../../../design-system/primitives';
import { appState } from '../../../state/app-state';
import type {
  EvolutionCandidate,
  SkillOptimizationRun,
  SkillOptimizationRunSummary,
  SkillOptimizationStatus,
} from '../../../types/api';
import { formatEvolutionDate, skillOptimizationEditLabel, skillOptimizationStatusLabel } from '../evolution-format';

type OptimizationConfirmation = 'adopt' | 'dismiss';

export function SkillOptimizationPanel({
  candidate,
  summaries,
  candidateBusy,
  onOptimize,
  onLoad,
  onAdopt,
  onDismiss,
}: {
  candidate: EvolutionCandidate;
  summaries: SkillOptimizationRunSummary[];
  candidateBusy: boolean;
  onOptimize: (candidateId: string) => Promise<void>;
  onLoad: (runId: string, quiet?: boolean) => Promise<void>;
  onAdopt: (runId: string) => Promise<void>;
  onDismiss: (runId: string) => Promise<void>;
}) {
  const state = useSnapshot(appState);
  const orderedSummaries = useMemo(
    () => [...summaries].sort((left, right) => Date.parse(right.createdAt) - Date.parse(left.createdAt)),
    [summaries]
  );
  const runKey = orderedSummaries.map((summary) => summary.id).join(':');
  const [selectedRunId, setSelectedRunId] = useState<string | null>(orderedSummaries[0]?.id ?? null);
  const [confirmation, setConfirmation] = useState<OptimizationConfirmation | null>(null);

  useEffect(() => {
    setSelectedRunId((current) => {
      if (current && orderedSummaries.some((summary) => summary.id === current)) return current;
      return orderedSummaries[0]?.id ?? null;
    });
  }, [candidate.id, runKey]);

  const selectedSummary = orderedSummaries.find((summary) => summary.id === selectedRunId) ?? null;
  const snapshotRun = selectedRunId ? state.evolutionOptimizationRuns[selectedRunId] : undefined;
  const run = snapshotRun && selectedRunId ? appState.evolutionOptimizationRuns[selectedRunId] : null;
  const status = run?.status ?? selectedSummary?.status ?? null;
  const activeRunId = orderedSummaries.find((summary) => {
    const cachedStatus = state.evolutionOptimizationRuns[summary.id]?.status;
    const candidateStatus = cachedStatus ?? summary.status;
    return candidateStatus === 'queued' || candidateStatus === 'running';
  })?.id;
  const loading = selectedRunId === state.evolutionOptimizationLoadingId && !run;
  const optimizationBusy = selectedRunId === state.evolutionOptimizationBusyId;
  const canOptimize =
    candidate.state !== 'rejected' && !candidate.hasConflicts && candidate.instructions.length > 0 && !activeRunId;

  useEffect(() => {
    if (selectedRunId) void onLoad(selectedRunId);
  }, [onLoad, selectedRunId]);

  useEffect(() => {
    if (!activeRunId) return;
    const timer = window.setInterval(() => {
      void onLoad(activeRunId, true);
    }, 1500);
    return () => window.clearInterval(timer);
  }, [activeRunId, onLoad]);

  return (
    <section className='skill-optimization-panel' aria-label='Skill 自优化'>
      <header className='skill-optimization-header'>
        <div className='skill-optimization-title'>
          <span>
            <FlaskConical size={15} />
          </span>
          <div>
            <h3>Skill 自优化</h3>
            <p>隔离评测可复用指令；只有通过留出集门禁并由你采用后才会生效。</p>
          </div>
        </div>
        <Button
          tone={orderedSummaries.length ? 'secondary' : undefined}
          loading={candidateBusy || Boolean(activeRunId)}
          disabled={!canOptimize || candidateBusy}
          onClick={() => void onOptimize(candidate.id)}
        >
          {!candidateBusy &&
            !activeRunId &&
            (orderedSummaries.length ? <RefreshCw size={13} /> : <WandSparkles size={13} />)}
          {activeRunId ? '优化进行中' : orderedSummaries.length ? '再次优化' : '开始优化'}
        </Button>
      </header>

      {!canOptimize && !activeRunId && <OptimizationUnavailable candidate={candidate} />}

      {orderedSummaries.length === 0 ? (
        <OptimizationIntro />
      ) : (
        <>
          <div className='skill-optimization-runbar'>
            <div>
              <OptimizationStatus status={status ?? 'queued'} />
              <span>{selectedSummary ? formatEvolutionDate(selectedSummary.createdAt) : '刚刚'}</span>
            </div>
            {orderedSummaries.length > 1 && (
              <label>
                <span>运行记录</span>
                <select value={selectedRunId ?? ''} onChange={(event) => setSelectedRunId(event.target.value)}>
                  {orderedSummaries.map((summary, index) => (
                    <option key={summary.id} value={summary.id}>
                      {index === 0 ? '最新 · ' : ''}
                      {skillOptimizationStatusLabel(summary.status)} · {formatEvolutionDate(summary.createdAt)}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>

          {state.evolutionOptimizationError && selectedRunId && !run && !loading ? (
            <InlineNotice
              className='skill-optimization-error'
              tone='warning'
              role='alert'
              actions={
                <Button tone='quiet' onClick={() => void onLoad(selectedRunId)}>
                  重试
                </Button>
              }
            >
              <span title={state.evolutionOptimizationError}>无法读取这次优化的详情。</span>
            </InlineNotice>
          ) : loading || !run ? (
            <CollectionState className='skill-optimization-loading' role='status' icon={<LoaderCircle size={15} />}>
              正在读取评测记录
            </CollectionState>
          ) : (
            <OptimizationRunDetail
              run={run}
              busy={optimizationBusy}
              onAdopt={() => setConfirmation('adopt')}
              onDismiss={() => setConfirmation('dismiss')}
            />
          )}
        </>
      )}

      {confirmation && run && (
        <OptimizationConfirmationDialog
          action={confirmation}
          run={run}
          busy={optimizationBusy}
          onClose={() => setConfirmation(null)}
          onConfirm={async () => {
            if (confirmation === 'adopt') await onAdopt(run.id);
            else await onDismiss(run.id);
            if (!appState.evolutionOptimizationError) setConfirmation(null);
          }}
        />
      )}
    </section>
  );
}

function OptimizationIntro() {
  return (
    <div className='skill-optimization-intro'>
      <ol className='skill-optimization-steps' aria-label='优化流程'>
        <li>
          <b>1</b>隔离回放
        </li>
        <li>
          <b>2</b>训练集反思
        </li>
        <li>
          <b>3</b>留出集盲测
        </li>
      </ol>
      <p>默认生成 4 个任务，候选只允许有限的新增、精确替换或删除。整个过程不调用工具，也不会自动采用或发布。</p>
    </div>
  );
}

function OptimizationUnavailable({ candidate }: { candidate: EvolutionCandidate }) {
  const reason = candidate.hasConflicts
    ? '请先解决互相矛盾的学习来源。'
    : candidate.state === 'rejected'
      ? '请先重新考虑这项 Skill。'
      : candidate.instructions.length === 0
        ? '这项 Skill 还没有可优化的指令。'
        : null;
  if (!reason) return null;
  return (
    <InlineNotice className='skill-optimization-unavailable' tone='warning' role='note'>
      {reason}
    </InlineNotice>
  );
}

function OptimizationStatus({ status }: { status: SkillOptimizationStatus }) {
  const active = status === 'queued' || status === 'running';
  return (
    <strong className='skill-optimization-status' data-status={status}>
      {active ? <LoaderCircle size={12} /> : status === 'staged' || status === 'adopted' ? <Check size={12} /> : null}
      {skillOptimizationStatusLabel(status)}
    </strong>
  );
}

function OptimizationRunDetail({
  run,
  busy,
  onAdopt,
  onDismiss,
}: {
  run: SkillOptimizationRun;
  busy: boolean;
  onAdopt: () => void;
  onDismiss: () => void;
}) {
  const dismissible = run.status === 'staged' || run.status === 'rejected' || run.status === 'failed';
  if (run.status === 'queued' || run.status === 'running') {
    return <OptimizationProgress status={run.status} />;
  }

  return (
    <div className='skill-optimization-result'>
      {run.gate && <OptimizationScorecard run={run} />}
      {run.status === 'failed' && (
        <InlineNotice className='skill-optimization-error' tone='danger' role='alert'>
          {run.error || '优化进程意外结束；当前 Skill 没有发生变化。'}
        </InlineNotice>
      )}
      {run.proposal && <OptimizationProposal run={run} />}
      {run.scores.length > 0 && <OptimizationEvaluation run={run} />}
      {(run.status === 'staged' || dismissible) && (
        <div className='skill-optimization-actions'>
          {dismissible && (
            <Button tone='quiet' disabled={busy} onClick={onDismiss}>
              <Archive size={13} /> 归档结果
            </Button>
          )}
          {run.status === 'staged' && (
            <Button loading={busy} disabled={busy} onClick={onAdopt}>
              <Sparkles size={13} /> 采用为新版本
            </Button>
          )}
        </div>
      )}
      {run.status === 'adopted' && (
        <p className='skill-optimization-adopted'>
          <ShieldCheck size={14} /> 已保存为不可变的 Skill 第 {run.adoptedVersion ?? '新'} 版。
        </p>
      )}
    </div>
  );
}

function OptimizationProgress({ status }: { status: 'queued' | 'running' }) {
  return (
    <output className='skill-optimization-progress' aria-live='polite'>
      <div>
        <LoaderCircle size={17} />
        <span>
          <strong>{status === 'queued' ? '正在准备隔离评测' : '正在比较基线与候选 Skill'}</strong>
          <small>可以离开此页面，后台运行不会阻塞当前工作。</small>
        </span>
      </div>
      <i aria-hidden='true' />
      <div className='skill-optimization-progress-labels'>
        <span>基线回放</span>
        <span>有限编辑</span>
        <span>盲测门禁</span>
      </div>
    </output>
  );
}

function OptimizationScorecard({ run }: { run: SkillOptimizationRun }) {
  const gate = run.gate;
  if (!gate) return null;
  return (
    <div className='skill-optimization-scorecard' data-accepted={gate.accepted}>
      <fieldset className='skill-optimization-metrics' aria-label='优化评分'>
        <span>
          <small>基线均分</small>
          <strong>{gate.baselineScore.toFixed(1)}</strong>
        </span>
        <ArrowRight size={14} />
        <span>
          <small>候选均分</small>
          <strong>{gate.candidateScore.toFixed(1)}</strong>
        </span>
        <span className='delta'>
          <small>变化</small>
          <strong>{formatDelta(gate.improvement)}</strong>
        </span>
      </fieldset>
      <p>
        {gate.accepted ? <ShieldCheck size={13} /> : <FlaskConical size={13} />}
        {gate.reason}
      </p>
    </div>
  );
}

function OptimizationProposal({ run }: { run: SkillOptimizationRun }) {
  const proposal = run.proposal;
  if (!proposal) return null;
  return (
    <div className='skill-optimization-proposal'>
      <header>
        <div>
          <strong>{run.status === 'staged' ? '通过验证的候选' : '候选草案（未生效）'}</strong>
          <span>{run.edits.length} 项有限编辑</span>
        </div>
        <p>{proposal.summary}</p>
      </header>
      <ol>
        {proposal.instructions.map((instruction, index) => (
          <li key={`${index}-${instruction}`}>{instruction}</li>
        ))}
      </ol>
      <div className='skill-optimization-edits'>
        {run.edits.map((edit, index) => (
          <article key={`${edit.operation}-${index}`} data-operation={edit.operation}>
            <span>{skillOptimizationEditLabel(edit.operation)}</span>
            <div>
              {edit.target && <del>{edit.target}</del>}
              {edit.content && <ins>{edit.content}</ins>}
              <small>{edit.rationale}</small>
            </div>
          </article>
        ))}
      </div>
    </div>
  );
}

function OptimizationEvaluation({ run }: { run: SkillOptimizationRun }) {
  return (
    <details className='skill-optimization-evaluation'>
      <summary>
        <span>
          盲测明细 <i>{run.scores.length} 个留出任务</i>
        </span>
        <ChevronDown size={13} />
      </summary>
      <div>
        {run.scores.map((score) => {
          const task = run.tasks.find((item) => item.id === score.taskId);
          return (
            <article key={score.taskId}>
              <p>{task?.prompt ?? score.taskId}</p>
              <div>
                <span>基线 {score.baseline.toFixed(1)}</span>
                <ArrowRight size={11} />
                <span>候选 {score.candidate.toFixed(1)}</span>
                <strong data-positive={score.delta > 0}>{formatDelta(score.delta)}</strong>
              </div>
              <small>{score.rationale}</small>
            </article>
          );
        })}
      </div>
    </details>
  );
}

function OptimizationConfirmationDialog({
  action,
  run,
  busy,
  onClose,
  onConfirm,
}: {
  action: OptimizationConfirmation;
  run: SkillOptimizationRun;
  busy: boolean;
  onClose: () => void;
  onConfirm: () => Promise<void>;
}) {
  const adopting = action === 'adopt';
  return (
    <Dialog
      className='evolution-confirmation-dialog'
      title={adopting ? '采用这版 Skill 优化？' : '归档这次优化？'}
      description={
        adopting
          ? '候选指令会保存为新的不可变版本，并刷新使用它的当前对话。'
          : '评测记录会保留，当前 Skill 不会发生变化。'
      }
      onClose={onClose}
      closeDisabled={busy}
      footer={
        <>
          <Button tone='quiet' disabled={busy} onClick={onClose}>
            取消
          </Button>
          <Button tone={adopting ? undefined : 'secondary'} loading={busy} onClick={() => void onConfirm()}>
            {adopting ? <Sparkles size={13} /> : <Archive size={13} />}
            {adopting ? '确认采用' : '确认归档'}
          </Button>
        </>
      }
    >
      <div className='evolution-confirmation-copy'>
        <strong>{run.candidateTitle}</strong>
        <p>
          {run.gate
            ? `基线 ${run.gate.baselineScore.toFixed(1)} → 候选 ${run.gate.candidateScore.toFixed(1)}（${formatDelta(run.gate.improvement)}）`
            : '这次运行没有产生可采用的评分。'}
        </p>
      </div>
    </Dialog>
  );
}

function formatDelta(value: number): string {
  return `${value > 0 ? '+' : ''}${value.toFixed(1)}`;
}
