import {
  Activity,
  Archive,
  ArrowRight,
  CheckCircle2,
  ChevronDown,
  Database,
  Gauge,
  Hash,
  History,
  LoaderCircle,
  MapPinned,
  Plane,
  Search,
  ShieldCheck,
  TriangleAlert,
  Trophy,
} from 'lucide-react';
import { useSnapshot } from 'valtio';
import { taskWeather } from '../../../components/scene/task-weather';
import { labState, type RunSortieSnapshot } from '../../../state/lab-state';
import type { BenchRunResult } from '../../../types/bench';
import { HANGAR_AIRFRAME_OPTIONS, HANGAR_PILOT_OPTIONS } from '../../hangar/hangar-configuration';
import type { ResultController } from '../use-result-controller';
import { CampaignReportList } from './campaign-report-list';

interface ResultsWorkspaceProps {
  actions: ResultController;
}

export function ResultsWorkspace({ actions }: ResultsWorkspaceProps) {
  const state = useSnapshot(labState);
  const record = state.results.record;
  const currentSortie =
    record?.run_id === state.run.runId ? state.run.sortie : record?.run_id ? state.results.sortie : undefined;
  const pendingRunId = state.run.result ? undefined : state.run.runId?.trim();
  const pendingSortie = pendingRunId ? state.run.sortie : undefined;
  const pendingAirframe = pendingSortie
    ? HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === pendingSortie.rosterEntry.airframeId)
    : undefined;

  return (
    <main className='workspace-page results-workspace'>
      <header className='workspace-page-header results-hall-header'>
        <div>
          <p className='workspace-eyebrow'>AFTER ACTION / LOCAL RECORD HALL</p>
          <h1>战绩大厅</h1>
          <p>回看每次智能体出击的评分、任务身份与完整性校验链。</p>
        </div>
        <div className='record-hall-actions'>
          <span className='record-vault-status'>
            <i /> LOCAL RECORD VAULT
          </span>
          <button
            className='secondary-action'
            onClick={() => void actions.loadLatest()}
            disabled={state.results.loading}
          >
            {state.results.loading ? <LoaderCircle className='spin' size={15} /> : <Trophy size={15} />}
            调取最新战报
          </button>
        </div>
      </header>

      <div className='results-layout'>
        <aside className='result-query-rail'>
          <div className='rail-section current-run-section'>
            <span className='rail-label'>LATEST DEBRIEF</span>
            <h2>当前战报</h2>
            {state.run.result ? (
              <button className='current-result-card' onClick={() => void actions.openCurrentRun()}>
                <span>
                  <small>
                    {state.run.sortie
                      ? `${state.run.sortie.task.name} · ${state.run.sortie.rosterEntry.callsign}`
                      : (state.run.result.task_id ?? state.run.result.task_reference ?? '正在解析任务')}
                  </small>
                  <strong>{state.run.result.run_id}</strong>
                </span>
                <span className='current-score'>{state.run.result.score ?? stageLabel(state.run.result.status)}</span>
                <ArrowRight size={15} />
              </button>
            ) : pendingRunId && pendingSortie ? (
              <button className='current-result-card' onClick={() => void actions.openCurrentRun()}>
                <span>
                  <small>
                    {pendingSortie.task.name} · {pendingSortie.rosterEntry.callsign} ·{' '}
                    {pendingAirframe?.displayName ?? pendingSortie.rosterEntry.airframeId}
                  </small>
                  <strong title={pendingRunId}>{pendingRunId}</strong>
                </span>
                <span className='current-score is-pending'>
                  {state.run.trackingStatus === 'tracking_stopped' ? '跟踪已停止 · 待精确核验' : '待精确核验'}
                </span>
                <ArrowRight size={15} />
              </button>
            ) : (
              <p className='quiet-empty'>完成一次智能体评测后，本轮战报会在这里待命。</p>
            )}
          </div>

          <CampaignReportList
            roster={state.campaign.snapshot?.roster ?? []}
            members={state.campaign.members}
            status={state.campaign.status}
            selectedRunId={record?.run_id}
            loading={state.results.loading}
            onOpenResult={(runId) => void actions.loadResult(runId)}
          />

          <details className='rail-section history-query'>
            <summary>
              <History size={15} aria-hidden='true' />
              <span>
                <small>RECORD ARCHIVE</small>
                <strong>历史战报</strong>
              </span>
              <ChevronDown size={14} aria-hidden='true' />
            </summary>
            <div className='history-query-body'>
              <p>使用 Bench 生成的 `local-…` 编号精确调取一份旧战报。</p>
              <label className='stacked-field'>
                <span>战报编号</span>
                <input
                  value={state.results.runId}
                  onChange={(event) => actions.setRunId(event.target.value)}
                  placeholder='local-…'
                  onKeyDown={(event) => {
                    if (event.key === 'Enter') void actions.loadResult();
                  }}
                />
              </label>
              <button
                className='primary-action'
                onClick={() => void actions.loadResult()}
                disabled={state.results.loading}
              >
                <Search size={15} />
                调取历史战报
              </button>
            </div>
          </details>

          <div className='rail-note'>
            <ShieldCheck size={15} aria-hidden='true' />
            <span>战报库为只读视图。评分、身份与摘要均来自本机 Bench，界面不会改写记录。</span>
          </div>
        </aside>

        <section className='result-document' aria-live='polite'>
          {state.results.loading ? <ResultLoading /> : null}
          {!state.results.loading && state.results.error ? <ResultError message={state.results.error} /> : null}
          {!state.results.loading && !state.results.error && !record ? <ResultEmpty /> : null}
          {!state.results.loading && !state.results.error && record ? (
            record.status === 'completed' && record.score && record.task_id ? (
              <ResultRecord record={record} sortie={currentSortie} />
            ) : (
              <ResultProjection record={record} sortie={currentSortie} />
            )
          ) : null}
        </section>
      </div>
    </main>
  );
}

function ResultLoading() {
  return (
    <div className='result-state'>
      <LoaderCircle className='spin' size={25} />
      <strong>正在解码本地战报</strong>
      <span>记录始终保留在本机战报库。</span>
    </div>
  );
}

function ResultError({ message }: { message: string }) {
  return (
    <div className='result-state result-state-error'>
      <Database size={26} />
      <strong>战报读取失败</strong>
      <span>{message}</span>
    </div>
  );
}

function ResultEmpty() {
  return (
    <div className='result-state'>
      <Archive size={28} />
      <strong>选择一份战报</strong>
      <span>调取最新战报，或在历史战报中输入运行编号。</span>
    </div>
  );
}

const projectionStages = [
  'planned',
  'runtime_ready',
  'inputs_resolved',
  'candidate_running',
  'candidate_completed',
  'judging',
  'completed',
] as const;

function ResultProjection({ record, sortie }: { record: BenchRunResult; sortie?: Readonly<RunSortieSnapshot> }) {
  const failed = record.status === 'failed';
  const currentStage = projectionStages.indexOf(record.status as (typeof projectionStages)[number]);

  return (
    <article className={`result-projection ${failed ? 'is-failed' : ''}`}>
      <header className='record-header'>
        <span className='record-status'>
          {failed ? <TriangleAlert size={14} /> : <Activity size={14} />}
          {stageLabel(record.status)}
        </span>
        <span>MISSION DEBRIEF</span>
        <span className='record-run-id'>{record.run_id}</span>
      </header>

      <div className='projection-hero'>
        <div className='projection-signal' aria-hidden='true'>
          <span />
          <i />
        </div>
        <div>
          <span>{failed ? 'MISSION ABORTED' : 'DEBRIEF IN PROGRESS'}</span>
          <h2>{failed ? '本次行动未能完成' : '战报仍在生成'}</h2>
          <p>
            {failed
              ? '公开战报仅披露行动中止状态；详细诊断仍保留在本机运行日志中。'
              : 'Bench 正在写入本轮公开战报。裁定阶段完成后，再次调取即可查看评分与完整性摘要。'}
          </p>
        </div>
      </div>

      <section className='projection-reference'>
        <span>MISSION REFERENCE</span>
        <code>{record.task_reference ?? record.task_id ?? 'Not included in this projection'}</code>
      </section>

      {!failed ? (
        <ol className='projection-track' aria-label='战报生成阶段'>
          {projectionStages.map((stage, index) => {
            const state = index < currentStage ? 'complete' : index === currentStage ? 'active' : 'waiting';
            return (
              <li className={`is-${state}`} key={stage}>
                <span />
                <small>{stageLabel(stage)}</small>
              </li>
            );
          })}
        </ol>
      ) : null}

      {sortie ? <SortieManifest sortie={sortie} /> : null}

      <footer className='record-footer'>
        <ShieldCheck size={15} />
        <span>战绩大厅只读取公开战报，不访问 Candidate 或 Judge 的私有诊断。</span>
      </footer>
    </article>
  );
}

function ResultRecord({ record, sortie }: { record: BenchRunResult; sortie?: Readonly<RunSortieSnapshot> }) {
  const totalTokens = record.model_usage?.total_tokens;
  return (
    <article className='result-record'>
      <header className='record-header'>
        <span className='record-status'>
          <CheckCircle2 size={14} /> 已归档
        </span>
        <span>LOCAL / EVALUATION RECORD</span>
        <span className='record-run-id'>{record.run_id}</span>
      </header>

      <div className='score-deck'>
        <div className='score-hero'>
          <span>综合战绩</span>
          <strong>{record.score}</strong>
          <small>{record.primary_metric ?? 'score'}</small>
        </div>
        <div className='score-context'>
          <span>评测任务</span>
          <strong>{record.task_id}</strong>
          <dl>
            <div>
              <dt>运行环境</dt>
              <dd>{record.runtime_provider ?? '—'}</dd>
            </div>
            <div>
              <dt>模型核心</dt>
              <dd>{record.model ?? 'Adapter managed'}</dd>
            </div>
            <div>
              <dt>规则状态</dt>
              <dd>{record.governance_status}</dd>
            </div>
          </dl>
        </div>
        <div className='score-radar' aria-hidden='true'>
          <span />
          <span />
          <span />
          <i />
        </div>
      </div>

      {sortie ? <SortieManifest sortie={sortie} /> : null}

      <section className='record-section'>
        <div className='record-section-heading'>
          <Hash size={14} />
          <span>身份校验链</span>
        </div>
        <div className='identity-grid'>
          <IdentityValue label='智能体身份' value={record.candidate_identity} />
          <IdentityValue label='裁定者身份' value={record.judge_identity} />
          <IdentityValue label='任务锁摘要' value={record.task_lock_digest} />
          <IdentityValue label='智能体锁摘要' value={record.candidate_lock_digest} />
          <IdentityValue label='战报完整性摘要' value={record.result_digest} wide />
        </div>
      </section>

      <section className='record-section usage-section'>
        <div className='record-section-heading'>
          <Gauge size={14} />
          <span>资源消耗</span>
        </div>
        {totalTokens !== undefined ? (
          <div className='usage-values'>
            <strong>{Intl.NumberFormat('zh-CN').format(totalTokens)}</strong>
            <span>总 tokens</span>
            <i />
            <span>输入 {Intl.NumberFormat('zh-CN').format(record.model_usage?.prompt_tokens ?? 0)}</span>
            <span>输出 {Intl.NumberFormat('zh-CN').format(record.model_usage?.completion_tokens ?? 0)}</span>
            {record.model_usage?.cache_read_tokens != null ? (
              <span>缓存读取 {Intl.NumberFormat('zh-CN').format(record.model_usage.cache_read_tokens)}</span>
            ) : null}
            {record.model_usage?.cache_write_tokens != null ? (
              <span>缓存写入 {Intl.NumberFormat('zh-CN').format(record.model_usage.cache_write_tokens)}</span>
            ) : null}
            {record.model_usage?.tool_calls_count != null ? (
              <span>工具调用 {Intl.NumberFormat('zh-CN').format(record.model_usage.tool_calls_count)}</span>
            ) : null}
          </div>
        ) : (
          <p className='quiet-empty'>该 Candidate 未报告模型用量。</p>
        )}
      </section>

      <footer className='record-footer'>
        <ShieldCheck size={15} />
        <span>公开战报不包含私有 Judge diagnostics 或 Candidate 源路径。</span>
      </footer>
    </article>
  );
}

function SortieManifest({ sortie }: { sortie: Readonly<RunSortieSnapshot> }) {
  const airframe = HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === sortie.rosterEntry.airframeId);
  const pilot = HANGAR_PILOT_OPTIONS.find((option) => option.id === sortie.rosterEntry.pilotId);
  const weather = taskWeather(sortie.task.id);

  return (
    <section className='record-section sortie-manifest'>
      <div className='record-section-heading'>
        <Plane size={14} />
        <span>出击数据快照</span>
      </div>
      <div className='identity-grid'>
        <IdentityValue label='作战地图' value={`${sortie.task.name} · ${sortie.task.id}`} />
        <IdentityValue label='天气环境' value={weather.labelZh} />
        <IdentityValue
          label='飞机与呼号'
          value={`${airframe?.displayName ?? sortie.rosterEntry.airframeId} · ${sortie.rosterEntry.callsign}`}
        />
        <IdentityValue label='智能体飞行员' value={pilot?.displayName ?? sortie.rosterEntry.pilotId} />
        <IdentityValue
          label={sortie.input.locked ? '实际 Candidate Lock' : '实际 Candidate'}
          value={sortie.input.candidate}
        />
        <IdentityValue
          label={sortie.input.locked ? '实际 Task Lock' : '模型核心'}
          value={sortie.input.locked ? sortie.input.task : (sortie.input.model ?? 'Adapter managed')}
        />
        <IdentityValue label='Effort / 可视挂载' value={`${sortie.rosterEntry.effort.toUpperCase()} · visual only`} />
        <IdentityValue label='输入模式' value={sortie.input.locked ? 'Immutable lock files' : 'Resolved by Bench'} />
      </div>
      <p className='sortie-manifest-note'>
        <MapPinned size={12} aria-hidden='true' />
        地图、飞机和执行输入在部署时冻结；评分与摘要仍以 Bench 公开战报为唯一权威来源。
      </p>
    </section>
  );
}

function IdentityValue({ label, value, wide }: { label: string; value?: string; wide?: boolean }) {
  return (
    <div className={`identity-value ${wide ? 'is-wide' : ''}`}>
      <span>{label}</span>
      <code title={value}>{value ?? 'Not included in this projection'}</code>
    </div>
  );
}

function stageLabel(stage: BenchRunResult['status']): string {
  const labels: Record<BenchRunResult['status'], string> = {
    idle: '等待任务',
    planned: '等待部署',
    running: 'Bench 执行中',
    runtime_ready: '航电就绪',
    inputs_resolved: '任务装载',
    candidate_running: '智能体出击',
    candidate_completed: '行动完成',
    judging: '裁定中',
    completed: '战报归档',
    failed: '行动中止',
  };
  return labels[stage];
}
