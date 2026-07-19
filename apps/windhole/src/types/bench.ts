export type BenchAvailability = 'ready' | 'blocked';
export type BenchAdmission = 'admitted' | 'quarantined';
export type BenchExecutionClass = 'conformance' | 'long_horizon';

export interface BenchTask {
  id: string;
  path: string;
  name: string;
  category: string;
  execution_class: BenchExecutionClass;
  availability: BenchAvailability;
  availability_reason: string;
  admission: BenchAdmission;
  admission_reason: string;
  provenance_ref: string;
  description?: string;
  tags?: string[];
}

export interface BenchHealth {
  connected: boolean;
  component: string;
  version: string;
  target: string;
  cliProtocol: string;
  workingDirectory: string;
}

export interface BenchRuntimeStatus {
  provider: string;
  ready: boolean;
  detail: string;
}

export interface BenchDoctorResult {
  config?: string | null;
  runtime: BenchRuntimeStatus;
  judge_model?: string | null;
}

export type BenchRunStage =
  | 'idle'
  | 'planned'
  | 'running'
  | 'runtime_ready'
  | 'inputs_resolved'
  | 'candidate_running'
  | 'candidate_completed'
  | 'judging'
  | 'completed'
  | 'failed';

export interface BenchRunResult {
  status: BenchRunStage;
  governance_status?: 'local_unofficial';
  run_id: string;
  task_reference?: string;
  task_id?: string;
  score?: string;
  result_path?: string;
  primary_metric?: string;
  runtime_provider?: string;
  model?: string | null;
  result_digest?: string;
  task_lock_digest?: string;
  candidate_lock_digest?: string;
  candidate_identity?: string;
  judge_identity?: string;
  model_usage?: {
    prompt_tokens: number;
    completion_tokens: number;
    total_tokens: number;
    cache_read_tokens?: number | null;
    cache_write_tokens?: number | null;
    tool_calls_count?: number | null;
  } | null;
}

export interface BenchRunJob {
  jobId: string;
  task: string;
  candidate: string;
  model?: string;
  locked: boolean;
  status: 'running' | 'completed' | 'failed';
  stage: BenchRunStage;
  runId?: string;
  startedAt: string;
  completedAt?: string;
  result?: BenchRunResult;
  error?: string;
}

export type BenchCampaignStatus =
  | 'idle'
  | 'running'
  | 'completed'
  | 'completed_with_failures'
  | 'failed'
  | 'tracking_stopped';

export type BenchDeploymentScope = 'single' | 'campaign';

export type BenchCampaignMemberStatus = 'queued' | 'starting' | 'running' | 'completed' | 'failed' | 'tracking_stopped';

export interface StartBenchRunInput {
  task: string;
  candidate: string;
  model?: string;
  locked: boolean;
}

export interface BenchOperationResult {
  message: string;
  outputPath?: string;
}

export interface TaskLockInput {
  source: string;
  outputPath: string;
}

export interface CandidateLockInput {
  candidate: string;
  model?: string;
  outputPath: string;
}

export type WorkspaceView = 'lab' | 'hangar' | 'results' | 'engineering';

export interface WindTunnelParameters {
  mach: number;
  angleOfAttack: number;
  airDensity: number;
  turbulence: number;
  smokeVisible: boolean;
  paused: boolean;
}

export interface AerodynamicTelemetry {
  velocity: number;
  dynamicPressure: number;
  liftCoefficient: number;
  dragCoefficient: number;
  lift: number;
  drag: number;
  reynolds: number;
  flowState: 'laminar' | 'transitional' | 'turbulent';
}
