export interface HealthResponse {
  ok: boolean;
  app: string;
  version: string;
  configPath: string;
  workspace: string;
  model?: string | null;
}

export type MemoryTier = 'short' | 'mid' | 'long';
export type MemoryForgetSignal = 'keep' | 'cooling' | 'candidate' | 'protected';

export interface MemoryEntry {
  id: string;
  content: string;
  preview: string;
  tags: string[];
  importance: number;
  timestamp: string;
  memoryType: string;
  metadata?: Record<string, string> | null;
  accessCount: number;
  lastAccessed?: string | null;
}

export interface MemoryStats {
  entries: number;
  types: Record<string, number>;
  tags: number;
  important: number;
  ctxSources: number;
}

export interface MemoryGraphStats {
  events: number;
  entities: number;
  relations: number;
  aliases: number;
  short: number;
  mid: number;
  long: number;
  forgetCandidates: number;
  llmExtracted: number;
  consolidated: number;
  conflicts: number;
}

export interface MemoryGraphEvent {
  id: string;
  memoryId: string;
  label: string;
  source: string;
  tier: MemoryTier;
  forget: MemoryForgetSignal;
  retentionScore: number;
  timestamp: string;
  entityIds: string[];
}

export interface MemoryGraphEntity {
  id: string;
  kind: string;
  name: string;
  aliases: string[];
  mentions: number;
  importance: number;
  firstSeen?: string | null;
  lastSeen?: string | null;
  memoryIds: string[];
}

export interface MemoryGraphRelation {
  id: number;
  from: string;
  to: string;
  kind: string;
  memoryId: string;
  weight: number;
}

export interface MemoryGraphFacet {
  eventId: string;
  tier: MemoryTier;
  forget: MemoryForgetSignal;
  retentionScore: number;
  llmExtracted: boolean;
  consolidated: boolean;
  conflicts: boolean;
  entityIds: string[];
  relationIds: number[];
}

export interface MemoryGraph {
  stats: MemoryGraphStats;
  events: MemoryGraphEvent[];
  entities: MemoryGraphEntity[];
  relations: MemoryGraphRelation[];
  facets: Record<string, MemoryGraphFacet>;
}

export interface MemoryPagination {
  offset: number;
  limit: number;
  returned: number;
  total: number;
  hasMore: boolean;
}

export interface MemoryOverview {
  root: string;
  entries: MemoryEntry[];
  stats: MemoryStats;
  graph: MemoryGraph;
  pagination?: MemoryPagination;
}

export interface OsAccount {
  configured: boolean;
  address?: string | null;
  origin?: string | null;
  signedIn: boolean;
  label?: string | null;
  loginAtMs?: number | null;
  expiresAtMs?: number | null;
  tokenType?: string | null;
  needsRefresh: boolean;
  capabilitySkillActive: boolean;
  builtinSkillActive: boolean;
  runtimeToolActive: boolean;
  refreshError?: string | null;
}

export interface ModelInfo {
  id: string;
  name?: string;
  family?: string;
  apiKey?: string | null;
  baseUrl?: string | null;
  headers?: Record<string, string>;
  sessionIdHeader?: string | null;
  attachment?: boolean;
  reasoning?: boolean;
  toolCall?: boolean;
  temperature?: boolean;
  releaseDate?: string | null;
  modalities?: {
    input: string[];
    output: string[];
  };
  cost?: {
    input: number;
    output: number;
    cacheRead: number;
    cacheWrite: number;
  };
  limit?: {
    context?: number;
    output?: number;
  };
}

export interface ProviderInfo {
  name: string;
  apiKey?: string | null;
  baseUrl?: string | null;
  headers?: Record<string, string>;
  sessionIdHeader?: string | null;
  models: ModelInfo[];
}

export interface LlmSettings {
  category?: 'llm';
  effect?: ConfigEffect;
  configPath?: string;
  defaultModel: string;
  providers: ProviderInfo[];
  maxToolRounds?: number | null;
  maxParallelTasks?: number | null;
  autoParallel?: boolean | null;
  thinkingBudget?: number | null;
  llmApiTimeoutMs?: number | null;
}

export interface ConfigEffect {
  scope: 'immediate' | 'newTasks' | 'restartRequired';
  label: string;
  description: string;
}

export interface CatalogModel {
  id: string;
  name: string;
  source: string;
  contextWindow?: number | null;
  reasoning: boolean;
  toolCall: boolean;
}

export interface ModelCatalog {
  items: CatalogModel[];
  warnings: string[];
  defaultModel?: string | null;
}

export interface CodeSession {
  sessionId: string;
  workspace: string;
  cwd: string;
  model?: string | null;
  followDefaultModel: boolean;
  permissionMode: string;
  state: string;
  title?: string | null;
  agentId?: string | null;
  createdAt: number;
  parentSessionId?: string | null;
  forkFocus?: string | null;
}

export interface SessionList {
  items: CodeSession[];
  total: number;
}

export interface ContentBlock {
  type: string;
  id?: string;
  name?: string;
  input?: Record<string, unknown>;
  toolUseId?: string;
  content?: string;
  isError?: boolean;
  exitCode?: number | null;
  durationMs?: number | null;
}

export interface ChatMessage {
  id: string;
  sessionId: string;
  role: 'user' | 'assistant' | 'system';
  content: string;
  createdAt: string;
  model?: string;
  source?: string;
  contentBlocks?: ContentBlock[];
  reasoning?: string;
  pending?: boolean;
  events?: AgentEvent[];
}

export interface MessageList {
  items: ChatMessage[];
  total: number;
  page: number;
  limit: number;
}

export interface WorkspaceEntry {
  name: string;
  path: string;
  isDirectory: boolean;
  isFile: boolean;
  size: number;
  mtimeMs?: number | null;
  extension?: string | null;
  isBinary: boolean;
}

export interface WorkspaceDirectorySelection {
  cancelled: boolean;
  path: string | null;
}

export interface WorkspaceFileCatalogItem {
  path: string;
  relativePath: string;
  name: string;
  isBinary: boolean;
}

export interface WorkspaceFileCatalog {
  workspaceRoot: string;
  items: WorkspaceFileCatalogItem[];
  total: number;
  truncated: boolean;
}

export interface SkillCatalogItem {
  name: string;
  command: string;
  description: string;
  enabled: boolean;
  sources: Array<{ path: string }>;
}

export interface SkillCatalog {
  workspaceRoot: string;
  items: SkillCatalogItem[];
  total: number;
  enabledCount: number;
  disabledCount: number;
}

export interface PluginActivityItem {
  key: string;
  packageId: string;
  route: string;
  version: string;
  enabled: boolean;
  id: string;
  title: string;
  description: string;
  icon: string;
  skill: string;
  order: number;
  sha256: string;
  mediaType: 'text/html';
}

export interface PluginActivityCatalog {
  schemaVersion: number;
  available: boolean;
  generation: number;
  revision: string;
  items: PluginActivityItem[];
}

export interface PluginActivityContent {
  key: string;
  packageId: string;
  skill: string;
  registryRevision: string;
  sha256: string;
  mediaType: 'text/html';
  html: string;
}

export interface PluginMarketplaceRegistry {
  name: string;
  url: string;
  configured: boolean;
  verified: boolean;
  error?: string;
  hostTarget?: string;
  metadata?: {
    rootVersion: number;
    timestampVersion: number;
    snapshotVersion: number;
    targetsVersion: number;
    packageTargets: number;
  };
}

export interface PluginMarketplaceItem {
  componentId: string;
  packageId: string;
  displayName: string;
  registryName: string;
  registryUrl: string;
  version: string;
  channel: 'stable' | 'beta' | 'nightly';
  target: string;
  archiveName: string;
  length: number;
  sha256: string;
  signedPlanDigest: string;
  installed: boolean;
  enabled: boolean;
}

export interface PluginMarketplaceCatalog {
  schemaVersion: number;
  verifiedAt: string;
  registries: PluginMarketplaceRegistry[];
  items: PluginMarketplaceItem[];
}

export type PluginOperationAction = 'install' | 'upgrade' | 'uninstall';

export interface PluginOperationRequest {
  action: PluginOperationAction;
  componentId: string;
  version?: string;
  channel?: 'stable' | 'beta' | 'nightly';
}

export interface PluginOperationPlan {
  dryRun: true;
  planSchemaVersion: number;
  planCommand: string;
  planDigest: string;
  plans: Array<{
    component: string;
    action: string;
    source: string;
    mutates: boolean;
    message: string;
    resolvedRegistryPackages?: Record<string, unknown>;
  }>;
}

export interface PluginOperationResult {
  planDigest: string;
  operations: Array<{
    component: string;
    changed: boolean;
    message: string;
  }>;
}

export interface WorkspaceSearchMatch {
  line: number;
  /** One-based UTF-16 column, matching Monaco's position contract. */
  column: number;
  /** Bounded context around this match rather than an unbounded source line. */
  text: string;
  /** Zero-based UTF-16 offsets inside `text`. */
  matchStart: number;
  matchEnd: number;
}

export interface WorkspaceSearchFile {
  path: string;
  matches: WorkspaceSearchMatch[];
}

export interface CodePosition {
  line: number;
  character: number;
}

export interface CodeRange {
  start: CodePosition;
  end: CodePosition;
}

export interface CodeLocation {
  path: string;
  range: CodeRange;
}

export interface CodeDocumentSnapshot {
  revision: number;
  contentHash: string;
  stale: boolean;
}

export interface CodeIntelligenceCapabilities {
  documentSymbols: boolean;
  workspaceSymbols: boolean;
  definition: boolean;
  declaration: boolean;
  references: boolean;
  implementations: boolean;
  diagnostics: boolean;
}

export type CodeIntelligenceState = 'starting' | 'ready' | 'degraded' | 'unavailable';

export interface CodeLanguageStatus {
  language: string;
  state: CodeIntelligenceState;
  capabilities: CodeIntelligenceCapabilities;
  message: string | null;
}

export interface CodeIntelligenceStatus {
  state: CodeIntelligenceState;
  capabilities: CodeIntelligenceCapabilities;
  languages: CodeLanguageStatus[];
  message: string | null;
}

export interface CodeOutlineSymbol {
  name: string;
  detail: string | null;
  kind: string;
  range: CodeRange;
  selectionRange: CodeRange;
  children: CodeOutlineSymbol[];
}

export interface CodeWorkspaceSymbol {
  name: string;
  kind: string;
  location: CodeLocation;
  containerName: string | null;
}

export interface CodeIntelligenceResult<T> {
  items: T[];
  truncated: boolean;
  workspaceRevision: number;
  document: CodeDocumentSnapshot | null;
}

export type CodeOutlineResult = CodeIntelligenceResult<CodeOutlineSymbol>;

export type CodeWorkspaceSymbolResult = CodeIntelligenceResult<CodeWorkspaceSymbol>;

export type CodeNavigationKind = 'definition' | 'declaration' | 'references' | 'implementations';

export type CodeNavigationResult = CodeIntelligenceResult<CodeLocation>;

export type CodeDiagnosticSeverity = 'error' | 'warning' | 'information' | 'hint';

export interface CodeDiagnostic {
  location: CodeLocation;
  severity: CodeDiagnosticSeverity | null;
  code: string | null;
  source: string | null;
  message: string;
}

export type CodeDiagnosticsResult = CodeIntelligenceResult<CodeDiagnostic>;

export interface GitStatusFile {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
  status: string;
}

export interface GitStatus {
  isGitRepo: boolean;
  branch?: string | null;
  files: GitStatusFile[];
}

export interface GitDiff {
  path?: string | null;
  staged: boolean;
  content: string;
  original: string;
  modified: string;
  isBinary: boolean;
}

export interface ConfigValidation {
  valid: boolean;
  issues: string[];
  summary?: { defaultModel?: string | null; providers: number; models: number; mcpServers: number } | null;
}

export interface VerificationSummary {
  status: 'passed' | 'failed' | 'needs_review' | 'skipped';
  report_count: number;
  required_check_count: number;
  pending_required_check_count: number;
  failed_check_count: number;
  residual_risk_count: number;
  pending_subjects?: string[];
  failed_subjects?: string[];
}

export interface ExecutionPlanTask {
  id: string;
  content: string;
  status: string;
  priority?: string;
  tool?: string;
  success_criteria?: string;
}
export interface ExecutionPlan {
  goal: string;
  steps: ExecutionPlanTask[];
  complexity: string;
  required_tools: string[];
  estimated_steps: number;
}

export type AgentEvent = {
  type: string;
  text?: string;
  message?: string;
  id?: string;
  name?: string;
  args?: Record<string, unknown>;
  delta?: string;
  output?: string;
  exit_code?: number;
  duration_ms?: number;
  metadata?: Record<string, unknown>;
  error_kind?: string;
  is_error?: boolean;
  tool_id?: string;
  tool_name?: string;
  timeout_ms?: number;
  approved?: boolean;
  reason?: string;
  scope?: string;
  risk?: string;
  action_taken?: string;
  verification_summary?: VerificationSummary;
  plan?: ExecutionPlan;
  tasks?: ExecutionPlanTask[];
  step_id?: string;
  description?: string;
  step_number?: number;
  total_steps?: number;
  status?: string;
  task_id?: string;
  session_id?: string;
  parent_session_id?: string;
  agent?: string;
  started_ms?: number;
  finished_ms?: number;
  success?: boolean;
  progress?: number;
  usage?: {
    prompt_tokens?: number;
    completion_tokens?: number;
    total_tokens?: number;
  };
  [key: string]: unknown;
};

export interface SessionControls {
  sessionId: string;
  effort: string;
  goal?: string | null;
  goalState?: GoalState | null;
  planningMode: string;
  goalTracking: boolean;
  context?: {
    estimatedTokens: number;
    limitTokens: number;
    percent: number;
    historyMessages: number;
    compacted: boolean;
    compactSummary?: string | null;
  };
}

export type GoalStatus = 'active' | 'paused' | 'retrying' | 'achieved';

export interface GoalState {
  status: GoalStatus;
  startedAt: number;
  updatedAt: number;
  completedAt?: number | null;
  attempts: number;
  progressPercent: number;
  completedSteps: number;
  totalSteps: number;
  lastError?: string | null;
  extractedGoal?: string | null;
}

export type QueuedTurnKind = 'user' | 'goalContinuation';

export interface QueuedTurn {
  id: string;
  kind: QueuedTurnKind;
  content: string;
  contextFiles: string[];
  skillNames: string[];
  priority: number;
  enqueuedAt: number;
}

export interface ActiveTurn {
  turn: QueuedTurn;
  startedAt: number;
}

export interface TurnQueue {
  sessionId: string;
  status: 'idle' | 'pending' | 'running' | 'paused';
  paused: boolean;
  active?: ActiveTurn | null;
  items: QueuedTurn[];
  total: number;
  nextItemId?: string | null;
  acceptedItemId?: string;
}

export interface SessionCompaction {
  sessionId: string;
  compacted: boolean;
  summary: string;
  historyMessages: number;
  completedAt: string;
}

export interface ToolOutputRecord {
  id: string;
  index: number;
  toolUseId: string;
  toolName: string;
  input: string;
  output: string;
  isError: boolean;
  exitCode?: number | string | null;
  durationMs?: number | null;
  createdAt?: string | null;
  completedAt?: string | null;
  filePath?: string | null;
  cwd?: string | null;
}

export interface SessionOutput {
  sessionId: string;
  items: ToolOutputRecord[];
  total: number;
  format: string;
}

export interface EffortLevel {
  id: string;
  label: string;
  description?: string;
}
