import {
  Activity,
  ChevronDown,
  CircleStop,
  LockKeyhole,
  Plane,
  Play,
  Settings2,
  ShieldCheck,
  Swords,
  TriangleAlert,
} from 'lucide-react';
import { useSnapshot } from 'valtio';
import { campaignDeploymentStatus } from '../features/bench/campaign-deployment-status';
import { CampaignRosterStrip } from '../features/bench/components/campaign-roster-strip';
import { DeploymentScopePicker } from '../features/bench/components/deployment-scope-picker';
import { taskDeploymentStatus } from '../features/bench/task-deployment-status';
import { type BenchController, isBenchDoctorReady } from '../features/bench/use-bench-controller';
import {
  type CandidateReferenceStatus,
  candidateRunStatus,
  HANGAR_AIRFRAME_OPTIONS,
  HANGAR_PILOT_OPTIONS,
} from '../features/hangar/hangar-configuration';
import { activateHangarRosterEntry } from '../features/hangar/hangar-roster-state';
import {
  campaignSnapshotMatchesConfiguration,
  isEvaluationActive,
  isSingleRunActive,
  labState,
} from '../state/lab-state';
import type { BenchCampaignMemberStatus, BenchCampaignStatus, BenchTask } from '../types/bench';
import type { EvaluationEffort } from '../types/evaluation';

interface TaskRunControlsProps {
  actions: BenchController;
  task: BenchTask;
}

const EFFORT_OPTIONS: ReadonlyArray<{
  value: EvaluationEffort;
  label: string;
  loadout: string;
  strength: number;
}> = [
  { value: 'none', label: '空载', loadout: 'CLEAN', strength: 0 },
  { value: 'minimal', label: '警戒', loadout: 'SELF DEF', strength: 1 },
  { value: 'low', label: '轻装', loadout: 'LIGHT A/A', strength: 2 },
  { value: 'medium', label: '均衡', loadout: 'BALANCED', strength: 3 },
  { value: 'high', label: '重装', loadout: 'HEAVY A/A', strength: 4 },
  { value: 'xhigh', label: '满载', loadout: 'FULL COMBAT', strength: 5 },
];

interface DeploymentReadiness {
  deployable: boolean;
  kind: CandidateReferenceStatus['kind'] | 'campaign' | 'locked';
  label: string;
  message: string;
}

export function TaskRunControls({ actions, task }: TaskRunControlsProps) {
  const state = useSnapshot(labState);
  const trackingStopped = state.run.trackingStatus === 'tracking_stopped';
  const singleRunActive = isSingleRunActive(state.run.stage, state.run.trackingStatus);
  const campaignActive = state.campaign.status === 'running';
  const evaluationActive = isEvaluationActive(state.run.stage, state.campaign.status, state.run.trackingStatus);
  const configuredEntry = state.hangar.roster.find((entry) => entry.id === state.hangar.activeEntryId);
  const activeEntry = singleRunActive && state.run.sortie ? state.run.sortie.rosterEntry : configuredEntry;
  const sortieMatchesSelection = Boolean(
    state.run.sortie &&
      configuredEntry &&
      state.run.sortie.task.id === task.id &&
      sameRosterConfiguration(state.run.sortie.rosterEntry, configuredEntry)
  );
  const showTrackingStopped = trackingStopped && Boolean(state.run.sortie);
  const displayedStage = singleRunActive || sortieMatchesSelection ? state.run.stage : 'idle';
  const displayedScore = singleRunActive || sortieMatchesSelection ? state.run.result?.score : undefined;
  const locked = singleRunActive && state.run.sortie ? state.run.sortie.input.locked : state.runConfig.locked;
  const deploymentScope = locked ? 'single' : state.runConfig.deploymentScope;
  const candidateValue =
    singleRunActive && state.run.sortie
      ? state.run.sortie.input.candidate
      : locked
        ? state.runConfig.candidateLock
        : (activeEntry?.candidate ?? '');
  const currentEffort = EFFORT_OPTIONS.find((option) => option.value === activeEntry?.effort) ?? EFFORT_OPTIONS[0];
  const activeAirframe = HANGAR_AIRFRAME_OPTIONS.find((option) => option.id === activeEntry?.airframeId);
  const activePilot = HANGAR_PILOT_OPTIONS.find((option) => option.id === activeEntry?.pilotId);
  const readiness = locked
    ? lockedDeploymentReadiness(state.runConfig.candidateLock, state.runConfig.taskLock)
    : candidateDeploymentReadiness(candidateRunStatus(activeEntry?.candidate ?? '', activeEntry?.model ?? ''));
  const taskStatus = locked
    ? { deployable: true, message: 'Task Lock 将由 Bench 在部署时校验。' }
    : taskDeploymentStatus(task, state.connection.doctor);
  const taskReady = taskStatus.deployable;
  const benchReady = state.connection.mode === 'live' && isBenchDoctorReady(state.connection.doctor);
  const campaignReadiness = campaignDeploymentStatus({
    connectionMode: state.connection.mode,
    doctor: state.connection.doctor,
    locked,
    roster: state.hangar.roster,
    task,
  });
  const visibleReadiness: DeploymentReadiness =
    deploymentScope === 'campaign'
      ? {
          deployable: campaignReadiness.deployable,
          kind: 'campaign',
          label: campaignReadiness.deployable ? '编队已就绪' : '编队存在阻塞',
          message: campaignReadiness.message,
        }
      : readiness;
  const campaignMatchesSelection = campaignSnapshotMatchesConfiguration(
    state.campaign.snapshot,
    task.id,
    state.hangar.roster
  );
  const showCampaignState = Boolean(
    deploymentScope === 'campaign' && state.campaign.snapshot && (campaignActive || campaignMatchesSelection)
  );
  const campaignRoster =
    campaignActive && state.campaign.snapshot
      ? state.campaign.snapshot.roster.map((sortie) => sortie.rosterEntry)
      : state.hangar.roster;
  const campaignMembers = showCampaignState ? state.campaign.members : [];
  const campaignCounts = countCampaignMembers(campaignMembers);
  const campaignTotal = showCampaignState ? campaignRoster.length : state.hangar.roster.length;
  const campaignTrackingStopped = showCampaignState && state.campaign.status === 'tracking_stopped';
  const showStopCampaignTracking = deploymentScope === 'campaign' && campaignActive;
  const launchDisabled =
    deploymentScope === 'campaign'
      ? evaluationActive || !campaignReadiness.deployable
      : !activeEntry || evaluationActive || !readiness.deployable || !taskReady || !benchReady;
  const progress =
    deploymentScope === 'campaign'
      ? campaignProgress(showCampaignState ? state.campaign.status : 'idle', campaignCounts.terminal, campaignTotal)
      : runProgress(displayedStage, showTrackingStopped);
  const statusLabel =
    deploymentScope === 'campaign'
      ? campaignStatusLabel(showCampaignState ? state.campaign.status : 'idle', campaignCounts, campaignTotal)
      : runStatusLabel(displayedStage, displayedScore, showTrackingStopped);
  const progressLabel =
    progress === undefined
      ? campaignTrackingStopped
        ? '状态未知'
        : showTrackingStopped
          ? '跟踪已停止'
          : '实时状态'
      : `${progress}%`;
  const progressClassName = `sortie-progress${
    deploymentScope === 'single' && singleRunActive ? ' is-indeterminate' : ''
  }${showTrackingStopped || campaignTrackingStopped ? ' is-tracking-stopped' : ''}`;

  return (
    <section className='task-run-control' aria-label='出击准备'>
      <div className='sortie-status-line'>
        <span className={`task-run-status run-stage-${displayedStage}`}>
          <Activity size={11} aria-hidden='true' />
          {statusLabel}
        </span>
        <span>{progressLabel}</span>
      </div>
      <span className={progressClassName} aria-hidden='true'>
        <i style={progress === undefined ? undefined : { width: `${progress}%` }} />
      </span>

      <details className='sortie-configuration'>
        <summary>
          {activeEntry ? (
            <span className='active-sortie-combination' data-roster-entry-id={activeEntry.id}>
              <Plane size={14} aria-hidden='true' />
              <span>
                <small>{evaluationActive ? '已锁定出击组合' : '当前出击组合'}</small>
                <strong>{activeEntry.callsign}</strong>
              </span>
              <span>
                {activeAirframe?.displayName ?? activeEntry.airframeId} ·{' '}
                {activePilot?.displayName ?? activeEntry.pilotId}
              </span>
            </span>
          ) : (
            <span className='active-sortie-combination is-missing'>请先在机库选择一组飞机与飞行员。</span>
          )}
          <span className='sortie-configuration-caption'>
            <small>出击配置</small>
            <strong>
              {deploymentScope === 'campaign' ? '全编队' : '单机'} · {currentEffort.label}
            </strong>
          </span>
          <ChevronDown size={13} aria-hidden='true' />
        </summary>

        <div className='sortie-configuration-body'>
          <DeploymentScopePicker
            scope={deploymentScope}
            rosterSize={state.hangar.roster.length}
            locked={locked}
            disabled={evaluationActive}
            onChange={actions.setDeploymentScope}
          />

          {deploymentScope === 'campaign' ? (
            <CampaignRosterStrip
              roster={campaignRoster}
              campaignMembers={campaignMembers}
              activeEntryId={state.hangar.activeEntryId}
              disabled={evaluationActive}
              onSelectEntry={(entryId) => activateHangarRosterEntry(entryId)}
            />
          ) : null}

          <div className='loadout-heading'>
            <span>
              <Swords size={12} aria-hidden='true' /> 战术挂载
            </span>
            <output>{currentEffort.loadout}</output>
          </div>
          <fieldset className='effort-selector'>
            <legend className='sr-only'>选择努力程度与武器挂载</legend>
            {EFFORT_OPTIONS.map((option) => (
              <button
                className={option.value === activeEntry?.effort ? 'is-active' : ''}
                onClick={() => actions.setEffort(option.value)}
                aria-pressed={option.value === activeEntry?.effort}
                aria-label={`${option.label}，${option.loadout}`}
                disabled={evaluationActive}
                key={option.value}
              >
                <span>{option.label}</span>
                <i aria-hidden='true'>
                  {Array.from({ length: 5 }, (_, index) => (
                    <b className={index < option.strength ? 'is-filled' : ''} key={`${option.value}-${index}`} />
                  ))}
                </i>
              </button>
            ))}
          </fieldset>

          <details className='task-run-settings'>
            <summary>
              <Settings2 size={12} aria-hidden='true' />
              <span>高级配置</span>
              <code>{candidateValue || '未配置 Candidate'}</code>
              <ChevronDown size={12} aria-hidden='true' />
            </summary>
            <div className='task-run-fields'>
              <label>
                <span>{locked ? 'CANDIDATE LOCK' : 'CANDIDATE'}</span>
                <input
                  value={candidateValue}
                  onChange={(event) =>
                    locked ? actions.setCandidateLock(event.target.value) : actions.setCandidate(event.target.value)
                  }
                  disabled={evaluationActive}
                  placeholder={locked ? './candidate.lock.json' : './candidate'}
                />
              </label>
              <label>
                <span>{locked ? 'TASK LOCK' : 'MODEL / OPTIONAL'}</span>
                <input
                  value={
                    singleRunActive && state.run.sortie
                      ? locked
                        ? state.run.sortie.input.task
                        : (state.run.sortie.input.model ?? '')
                      : locked
                        ? state.runConfig.taskLock
                        : (activeEntry?.model ?? '')
                  }
                  onChange={(event) =>
                    locked ? actions.setTaskLock(event.target.value) : actions.setModel(event.target.value)
                  }
                  disabled={evaluationActive}
                  placeholder={locked ? './task.lock.json' : 'provider/model'}
                />
              </label>
              <label className='task-lock-toggle'>
                <input
                  type='checkbox'
                  checked={locked}
                  onChange={(event) => actions.setLocked(event.target.checked)}
                  disabled={evaluationActive}
                />
                <LockKeyhole size={11} aria-hidden='true' />
                <span>使用可复现锁文件</span>
              </label>
            </div>
          </details>
        </div>
      </details>

      {activeEntry ? (
        <div
          className={`candidate-deployment-status is-${visibleReadiness.kind} ${
            visibleReadiness.deployable ? 'is-deployable' : 'is-blocked'
          }`}
          role={visibleReadiness.deployable ? 'status' : 'alert'}
        >
          {visibleReadiness.deployable ? (
            <ShieldCheck size={13} aria-hidden='true' />
          ) : (
            <TriangleAlert size={13} aria-hidden='true' />
          )}
          <span>
            <strong>{visibleReadiness.label}</strong>
            <small>{visibleReadiness.message}</small>
          </span>
        </div>
      ) : null}

      {locked ? (
        <div className='locked-map-preview-note' role='note'>
          <LockKeyhole size={12} aria-hidden='true' />
          <span>
            <strong>当前地图与天气仅为场景预览</strong>
            <small>实际评测任务由 Task Lock 解析，并在 Bench 返回真实 Task ID 后绑定。</small>
          </span>
        </div>
      ) : null}

      <button
        className={`task-run-button${showStopCampaignTracking ? ' is-stop-tracking' : ''}`}
        onClick={() => {
          if (showStopCampaignTracking) {
            actions.stopCampaignTracking();
            return;
          }
          void (deploymentScope === 'campaign' ? actions.startCampaign() : actions.startRun());
        }}
        disabled={showStopCampaignTracking ? false : launchDisabled}
        title={
          showStopCampaignTracking
            ? `只停止此界面的状态轮询，不会取消已经提交的 Bench 进程。${campaignRunDetail(
                campaignCounts,
                campaignTotal
              )}`
            : launchDisabled && !evaluationActive
              ? deploymentScope === 'campaign'
                ? campaignReadiness.message
                : launchDetail(
                    Boolean(activeEntry),
                    taskReady,
                    taskStatus.message,
                    readiness,
                    benchReady,
                    state.connection.message
                  )
              : undefined
        }
      >
        <span className='launch-icon'>
          {showStopCampaignTracking ? (
            <CircleStop size={14} aria-hidden='true' />
          ) : (
            <Play size={13} fill='currentColor' aria-hidden='true' />
          )}
        </span>
        <span>
          <strong>
            {showStopCampaignTracking
              ? '停止前端跟踪'
              : deploymentScope === 'campaign'
                ? campaignLaunchLabel(evaluationActive, campaignReadiness.deployable, state.hangar.roster.length)
                : launchLabel(
                    evaluationActive,
                    Boolean(activeEntry),
                    taskReady,
                    readiness,
                    benchReady,
                    state.connection.mode
                  )}
          </strong>
          <small>
            {showStopCampaignTracking
              ? `不会取消 Bench 进程 · ${campaignRunDetail(campaignCounts, campaignTotal)}`
              : evaluationActive
                ? deploymentScope === 'campaign'
                  ? campaignRunDetail(campaignCounts, campaignTotal)
                  : runStageDetail(state.run.stage)
                : deploymentScope === 'campaign'
                  ? campaignReadiness.message
                  : launchDetail(
                      Boolean(activeEntry),
                      taskReady,
                      taskStatus.message,
                      readiness,
                      benchReady,
                      state.connection.message
                    )}
          </small>
        </span>
        <kbd>{showStopCampaignTracking ? 'STOP' : '↵'}</kbd>
      </button>
    </section>
  );
}

function candidateDeploymentReadiness(status: CandidateReferenceStatus): DeploymentReadiness {
  const label =
    status.kind === 'bundled'
      ? 'Candidate 就绪'
      : status.kind === 'local' || status.kind === 'oci'
        ? 'Candidate 待 Bench 校验'
        : status.kind === 'missing'
          ? '需配置 Candidate Adapter'
          : 'Candidate 引用不可用';
  return { ...status, label };
}

function lockedDeploymentReadiness(candidateLock: string, taskLock: string): DeploymentReadiness {
  const candidateMissing = !candidateLock.trim();
  const taskMissing = !taskLock.trim();
  if (!candidateMissing && !taskMissing) {
    return {
      deployable: true,
      kind: 'locked',
      label: '锁文件就绪',
      message: 'Candidate Lock 与 Task Lock 将由 Bench 在部署时校验。',
    };
  }

  const missingLabel =
    candidateMissing && taskMissing ? 'Candidate Lock 与 Task Lock' : candidateMissing ? 'Candidate Lock' : 'Task Lock';
  return {
    deployable: false,
    kind: 'locked',
    label: '需配置锁文件',
    message: `请先填写 ${missingLabel} 文件。`,
  };
}

function launchLabel(
  running: boolean,
  hasActiveEntry: boolean,
  taskReady: boolean,
  readiness: DeploymentReadiness,
  benchReady: boolean,
  connectionMode: string
): string {
  if (running) return '评测进行中';
  if (!hasActiveEntry) return '编队未就绪';
  if (!readiness.deployable) return readiness.kind === 'locked' ? '需配置锁文件' : '需配置 Adapter';
  if (!taskReady) return '地图不可部署';
  if (!benchReady) return connectionMode === 'checking' ? 'Bench 自检中' : 'Bench 未就绪';
  return '部署评测';
}

function launchDetail(
  hasActiveEntry: boolean,
  taskReady: boolean,
  taskMessage: string,
  readiness: DeploymentReadiness,
  benchReady: boolean,
  connectionMessage: string
): string {
  if (!hasActiveEntry) return '请先在机库选择出击组合';
  if (!readiness.deployable) return readiness.message;
  if (!taskReady) return taskMessage;
  if (!benchReady) return connectionMessage;
  return '确认地图与挂载后开始';
}

function sameRosterConfiguration(
  left: Readonly<NonNullable<(typeof labState.run)['sortie']>['rosterEntry']>,
  right: Readonly<NonNullable<(typeof labState.run)['sortie']>['rosterEntry']>
): boolean {
  return (
    left.id === right.id &&
    left.airframeId === right.airframeId &&
    left.pilotId === right.pilotId &&
    left.candidate === right.candidate &&
    left.model === right.model &&
    left.effort === right.effort &&
    left.callsign === right.callsign
  );
}

function runStatusLabel(stage: string, score?: string, trackingStopped = false): string {
  if (trackingStopped) return '跟踪已停止 · Job 状态未知';
  if (stage === 'completed') return `任务完成 · ${score ?? '—'}`;
  if (stage === 'failed') return '部署失败';
  if (stage === 'idle') return '待命';
  if (stage === 'judging' || stage === 'candidate_completed') return '结果判定';
  return '执行中';
}

function runStageDetail(stage: string): string {
  if (stage === 'judging' || stage === 'candidate_completed') return '正在核验任务结果';
  if (stage === 'inputs_resolved') return '装载任务输入';
  if (stage === 'runtime_ready') return '运行环境就绪';
  if (stage === 'running') return 'Bench Job 正在执行';
  return '智能体正在执行任务';
}

interface CampaignCounts {
  completed: number;
  failed: number;
  running: number;
  queued: number;
  trackingStopped: number;
  terminal: number;
}

function countCampaignMembers(members: readonly Readonly<{ status: BenchCampaignMemberStatus }>[]): CampaignCounts {
  const counts: CampaignCounts = {
    completed: 0,
    failed: 0,
    running: 0,
    queued: 0,
    trackingStopped: 0,
    terminal: 0,
  };
  for (const member of members) {
    if (member.status === 'completed') counts.completed += 1;
    if (member.status === 'failed') counts.failed += 1;
    if (member.status === 'running' || member.status === 'starting') counts.running += 1;
    if (member.status === 'queued') counts.queued += 1;
    if (member.status === 'tracking_stopped') counts.trackingStopped += 1;
  }
  counts.terminal = counts.completed + counts.failed;
  return counts;
}

function campaignProgress(status: BenchCampaignStatus, terminal: number, total: number): number | undefined {
  if (status === 'tracking_stopped') return undefined;
  if (status === 'idle' || total < 1) return 0;
  return Math.min(100, Math.round((terminal / total) * 100));
}

function campaignStatusLabel(status: BenchCampaignStatus, counts: CampaignCounts, total: number): string {
  if (status === 'running') return `编队执行中 · ${counts.terminal}/${total} 返回`;
  if (status === 'completed') return `编队完成 · ${counts.completed}/${total}`;
  if (status === 'completed_with_failures') return `编队返航 · ${counts.completed} 完成 / ${counts.failed} 失败`;
  if (status === 'failed') return `编队部署失败 · ${counts.failed}/${total}`;
  if (status === 'tracking_stopped') return `跟踪已停止 · ${counts.trackingStopped} 架状态未知`;
  return `编队待命 · ${total} 架`;
}

function campaignRunDetail(counts: CampaignCounts, total: number): string {
  return `真实终态 ${counts.terminal}/${total} · ${counts.trackingStopped} 跟踪已停止 · ${counts.running} 执行中 · ${counts.queued} 等待跑道`;
}

function campaignLaunchLabel(active: boolean, deployable: boolean, rosterSize: number): string {
  if (active) return '编队评测进行中';
  if (!deployable) return '编队不可部署';
  return `部署编队 · ${rosterSize}`;
}

export function runProgress(stage: string, trackingStopped = false): number | undefined {
  if (trackingStopped) return undefined;
  if (stage === 'idle') return 0;
  if (stage === 'completed' || stage === 'failed') return 100;
  return undefined;
}
