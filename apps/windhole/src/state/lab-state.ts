import { proxy, ref } from 'valtio';
import { demoTasks } from '../data/demo-tasks';
import {
  createHangarDraft,
  DEFAULT_HANGAR_ROSTER,
  type HangarDraft,
  type HangarRosterEntry,
} from '../features/hangar/hangar-configuration';
import type {
  BenchCampaignMemberStatus,
  BenchCampaignStatus,
  BenchDeploymentScope,
  BenchDoctorResult,
  BenchHealth,
  BenchOperationResult,
  BenchRunResult,
  BenchRunStage,
  BenchTask,
  StartBenchRunInput,
  WindTunnelParameters,
  WorkspaceView,
} from '../types/bench';

export type ConnectionMode = 'checking' | 'live' | 'preview';
export type SingleRunTrackingStatus = 'tracking_stopped';

export type RunSortieTaskSnapshot = Readonly<Omit<BenchTask, 'tags'> & { tags?: readonly string[] }>;

export interface RunSortieSnapshot {
  readonly task: RunSortieTaskSnapshot;
  readonly rosterEntry: Readonly<HangarRosterEntry>;
  readonly input: Readonly<StartBenchRunInput>;
}

export interface RunCampaignSortieSnapshot {
  readonly rosterEntry: Readonly<HangarRosterEntry>;
  readonly input: Readonly<StartBenchRunInput>;
}

export interface RunCampaignSnapshot {
  readonly task: RunSortieTaskSnapshot;
  readonly roster: readonly RunCampaignSortieSnapshot[];
}

export interface LabRunState {
  mode?: 'live';
  stage: BenchRunStage;
  trackingStatus?: SingleRunTrackingStatus;
  trackingStoppedAt?: string;
  jobId?: string;
  runId?: string;
  startedAt?: string;
  completedAt?: string;
  sortie?: RunSortieSnapshot;
  result?: BenchRunResult;
  error?: string;
}

export interface LabCampaignMemberRun {
  readonly rosterEntryId: string;
  readonly sortie: RunCampaignSortieSnapshot;
  status: BenchCampaignMemberStatus;
  stage?: BenchRunStage;
  jobId?: string;
  runId?: string;
  startedAt?: string;
  completedAt?: string;
  result?: BenchRunResult;
  error?: string;
}

export interface LabCampaignState {
  generation: number;
  status: BenchCampaignStatus;
  startedAt?: string;
  completedAt?: string;
  snapshot?: RunCampaignSnapshot;
  members: LabCampaignMemberRun[];
  error?: string;
}

interface LabState {
  workspace: WorkspaceView;
  connection: {
    mode: ConnectionMode;
    message: string;
    health?: BenchHealth;
    doctor?: BenchDoctorResult;
  };
  catalog: {
    tasks: BenchTask[];
    selectedTaskId: string;
    query: string;
    category: string;
    includeBlocked: boolean;
  };
  runConfig: {
    candidateLock: string;
    deploymentScope: BenchDeploymentScope;
    locked: boolean;
    taskLock: string;
  };
  hangar: {
    draft: HangarDraft;
    roster: HangarRosterEntry[];
    activeEntryId: string;
  };
  tunnel: WindTunnelParameters;
  run: LabRunState;
  campaign: LabCampaignState;
  results: {
    runId: string;
    loading: boolean;
    record?: BenchRunResult;
    sortie?: RunSortieSnapshot;
    error?: string;
  };
  engineering: {
    activeOperation?: 'doctor' | 'check' | 'task-lock' | 'candidate-lock';
    taskSource: string;
    taskLockOutput: string;
    candidate: string;
    candidateModel: string;
    candidateLockOutput: string;
    lastOperation?: BenchOperationResult;
    error?: string;
  };
  notice?: {
    tone: 'info' | 'success' | 'error';
    message: string;
  };
}

export const defaultTunnelParameters: WindTunnelParameters = {
  mach: 0.82,
  angleOfAttack: 4,
  airDensity: 1.225,
  turbulence: 0.12,
  smokeVisible: true,
  paused: false,
};

export const labState = proxy<LabState>({
  workspace: 'lab',
  connection: {
    mode: 'checking',
    message: '正在连接本机 A3S Bench…',
  },
  catalog: {
    tasks: demoTasks,
    selectedTaskId: demoTasks[0].id,
    query: '',
    category: 'all',
    includeBlocked: false,
  },
  runConfig: {
    candidateLock: './candidate.lock.json',
    deploymentScope: 'single',
    locked: false,
    taskLock: './task.lock.json',
  },
  hangar: {
    draft: createHangarDraft('a3s'),
    roster: DEFAULT_HANGAR_ROSTER.map((entry) => ({ ...entry })),
    activeEntryId: DEFAULT_HANGAR_ROSTER[0].id,
  },
  tunnel: { ...defaultTunnelParameters },
  run: {
    stage: 'idle',
  },
  campaign: {
    generation: 0,
    status: 'idle',
    members: [],
  },
  results: {
    runId: '',
    loading: false,
  },
  engineering: {
    taskSource: './task',
    taskLockOutput: './task.lock.json',
    candidate: './candidate',
    candidateModel: '',
    candidateLockOutput: './candidate.lock.json',
  },
});

export function selectedTask(): BenchTask | undefined {
  return labState.catalog.tasks.find((task) => task.id === labState.catalog.selectedTaskId);
}

export function createRunSortieSnapshot(
  task: RunSortieTaskSnapshot,
  rosterEntry: Readonly<HangarRosterEntry>,
  input: Readonly<StartBenchRunInput>
): RunSortieSnapshot {
  const taskSnapshot = freezeTaskSnapshot(task);
  const rosterEntrySnapshot = Object.freeze({ ...rosterEntry });
  const inputSnapshot = Object.freeze({ ...input });
  return ref(
    Object.freeze({
      task: taskSnapshot,
      rosterEntry: rosterEntrySnapshot,
      input: inputSnapshot,
    })
  );
}

export function createRunCampaignSnapshot(
  task: RunSortieTaskSnapshot,
  roster: readonly Readonly<HangarRosterEntry>[]
): RunCampaignSnapshot {
  const taskSnapshot = freezeTaskSnapshot(task);
  const rosterSnapshot = Object.freeze(
    roster.map((entry) => {
      const rosterEntry = Object.freeze({ ...entry });
      const input = Object.freeze({
        task: taskSnapshot.id,
        candidate: entry.candidate.trim(),
        model: entry.model.trim() || undefined,
        locked: false,
      } satisfies StartBenchRunInput);
      return Object.freeze({ rosterEntry, input });
    })
  );
  return ref(Object.freeze({ task: taskSnapshot, roster: rosterSnapshot }));
}

export function isCampaignActive(status: BenchCampaignStatus = labState.campaign.status): boolean {
  return status === 'running';
}

export function campaignSnapshotMatchesConfiguration(
  snapshot: Readonly<RunCampaignSnapshot> | undefined,
  taskId: string,
  roster: readonly Readonly<HangarRosterEntry>[]
): boolean {
  return (
    snapshot?.task.id === taskId &&
    snapshot.roster.length === roster.length &&
    snapshot.roster.every((sortie, index) => {
      const entry = roster[index];
      return Boolean(
        entry &&
          sortie.rosterEntry.id === entry.id &&
          sortie.rosterEntry.airframeId === entry.airframeId &&
          sortie.rosterEntry.pilotId === entry.pilotId &&
          sortie.rosterEntry.candidate === entry.candidate &&
          sortie.rosterEntry.model === entry.model &&
          sortie.rosterEntry.effort === entry.effort &&
          sortie.rosterEntry.callsign === entry.callsign
      );
    })
  );
}

export function singleRunSnapshotMatchesConfiguration(
  snapshot: Readonly<RunSortieSnapshot> | undefined,
  taskId: string,
  rosterEntry: Readonly<HangarRosterEntry> | undefined
): boolean {
  return Boolean(
    snapshot && rosterEntry && snapshot.task.id === taskId && sameRosterConfiguration(snapshot.rosterEntry, rosterEntry)
  );
}

export function isSingleRunActive(
  stage: BenchRunStage = labState.run.stage,
  trackingStatus: SingleRunTrackingStatus | undefined = labState.run.trackingStatus
): boolean {
  return trackingStatus !== 'tracking_stopped' && stage !== 'idle' && stage !== 'completed' && stage !== 'failed';
}

export function isEvaluationActive(
  runStage: BenchRunStage,
  campaignStatus: BenchCampaignStatus,
  trackingStatus: SingleRunTrackingStatus | undefined = labState.run.trackingStatus
): boolean {
  return isSingleRunActive(runStage, trackingStatus) || isCampaignActive(campaignStatus);
}

export function isBenchRunActive(
  stage: BenchRunStage = labState.run.stage,
  trackingStatus: SingleRunTrackingStatus | undefined = labState.run.trackingStatus
): boolean {
  return isEvaluationActive(stage, labState.campaign.status, trackingStatus);
}

export function resetRunState(): void {
  labState.run = { stage: 'idle' };
}

function freezeTaskSnapshot(task: RunSortieTaskSnapshot): RunSortieTaskSnapshot {
  return Object.freeze({
    ...task,
    tags: task.tags ? Object.freeze([...task.tags]) : undefined,
  });
}

function sameRosterConfiguration(left: Readonly<HangarRosterEntry>, right: Readonly<HangarRosterEntry>): boolean {
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
