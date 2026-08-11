export type OrganizationRegion = 'americas' | 'europe' | 'africa' | 'asia' | 'oceania';

export interface OrganizationNode {
  code: string;
  latitude: number;
  longitude: number;
  phase: number;
  region: OrganizationRegion;
}

export const organizationNodes: OrganizationNode[] = [
  { code: 'SFO', latitude: 37.77, longitude: -122.42, phase: 0.04, region: 'americas' },
  { code: 'NYC', latitude: 40.71, longitude: -74.01, phase: 0.41, region: 'americas' },
  { code: 'GRU', latitude: -23.55, longitude: -46.63, phase: 0.76, region: 'americas' },
  { code: 'LDN', latitude: 51.51, longitude: -0.13, phase: 0.19, region: 'europe' },
  { code: 'NBO', latitude: -1.29, longitude: 36.82, phase: 0.58, region: 'africa' },
  { code: 'BLR', latitude: 12.97, longitude: 77.59, phase: 0.88, region: 'asia' },
  { code: 'SIN', latitude: 1.35, longitude: 103.82, phase: 0.31, region: 'asia' },
  { code: 'SHA', latitude: 31.23, longitude: 121.47, phase: 0.67, region: 'asia' },
  { code: 'SYD', latitude: -33.87, longitude: 151.21, phase: 0.52, region: 'oceania' },
];

export const workspaceSceneCopy = {
  cn: {
    title: 'A3S Workspace',
    mode: 'A3S OS',
    human: 'Human',
    agent: 'Agent',
    workspace: 'Shared Workspace',
    edgeCloud: 'Edge + Cloud',
    cloud: 'Cloud',
    edge: 'Edge',
    sync: '状态同步',
    search: '搜索 Workspace',
    global: '全球协作',
    teams: '成员与 Agent',
    workspaceNav: 'Workspace',
    syncNav: '本地与云端',
    regions: '在线地点',
    handoffTitle: '任务已交接',
    handoffMeta: '任务、上下文和状态已同步',
    handoffStatus: '已同步',
    handoffSteps: ['Human 创建任务', 'Agent 继续执行', 'Edge 与 Cloud 同步'],
    accessibleLabel: '分布在全球的成员和 Agent 共用一个 Workspace',
    accessibleDescription:
      '动画展示分布在全球的 Human 和 Agent 在 Shared Workspace 里协作。A3S OS 在 Edge 与 Cloud 之间同步任务、上下文和状态。',
  },
  en: {
    title: 'A3S Workspace',
    mode: 'A3S OS',
    human: 'Human',
    agent: 'Agent',
    workspace: 'Shared Workspace',
    edgeCloud: 'Edge + Cloud',
    cloud: 'Cloud',
    edge: 'Edge',
    sync: 'State sync',
    search: 'Search Workspace',
    global: 'Global collaboration',
    teams: 'Human + Agent',
    workspaceNav: 'Shared Workspace',
    syncNav: 'Edge-cloud sync',
    regions: 'ACTIVE REGIONS',
    handoffTitle: 'Task handed off',
    handoffMeta: 'Task, context, and state are synchronized',
    handoffStatus: 'SYNCED',
    handoffSteps: ['Human starts the task', 'Agent continues the work', 'Edge + Cloud share state'],
    accessibleLabel: 'A three-dimensional workspace shared by globally distributed humans and agents',
    accessibleDescription:
      'The animation shows globally distributed Human and Agent collaborators working through A3S OS in one Shared Workspace, with tasks, context, and state synchronized across Edge and Cloud.',
  },
} as const;
