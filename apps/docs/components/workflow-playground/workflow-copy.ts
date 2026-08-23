import type {
  PlaygroundLang,
  WorkflowNodeGroup,
  WorkflowNodeStatus,
  WorkflowValidationCode,
} from './workflow-model';

export interface WorkflowPlaygroundCopy {
  pageTitle: string;
  workflowName: string;
  simulation: string;
  backHome: string;
  saved: string;
  saving: string;
  undo: string;
  redo: string;
  reset: string;
  validate: string;
  valid: string;
  run: string;
  stop: string;
  addNode: string;
  selectMode: string;
  panMode: string;
  canvasTools: string;
  moreActions: string;
  variables: string;
  viewCachedVariables: string;
  history: string;
  fitView: string;
  nodeLibrary: string;
  nodeLibraryDescription: string;
  nodesTab: string;
  toolsTab: string;
  searchNodes: string;
  noNodes: string;
  close: string;
  inspector: string;
  nodeName: string;
  nodeDescription: string;
  configuration: string;
  inputVariables: string;
  outputVariables: string;
  inputPayload: string;
  outputPayload: string;
  steps: string;
  template: string;
  selector: string;
  capability: string;
  routes: string;
  routeHandle: string;
  routeEquals: string;
  defaultRoute: string;
  decisionMessage: string;
  decisionDetails: string;
  expiry: string;
  retryAttempts: string;
  failureHandling: string;
  failureStop: string;
  failureDefault: string;
  failureRoute: string;
  defaultOutput: string;
  runStep: string;
  duplicate: string;
  delete: string;
  selectNode: string;
  debug: string;
  settingsTab: string;
  lastRun: string;
  trace: string;
  cachedVariables: string;
  runHistory: string;
  editValue: string;
  noTrace: string;
  noHistory: string;
  noSelection: string;
  openLibrary: string;
  testRun: string;
  runDescription: string;
  customerMessage: string;
  priority: string;
  normal: string;
  high: string;
  startRun: string;
  running: string;
  runSucceeded: string;
  runFailed: string;
  simulatedNotice: string;
  validationTitle: string;
  validationPassed: string;
  validationFailed: string;
  resetTitle: string;
  resetDescription: string;
  cancel: string;
  confirmReset: string;
  restored: string;
  graphExported: string;
  exportGraph: string;
  addRoute: string;
  removeRoute: string;
  keyboardHint: string;
  canvasLabel: string;
  modelRoute: string;
  prompt: string;
  code: string;
  method: string;
  url: string;
  query: string;
  filterField: string;
  filterValue: string;
  collectionVariable: string;
  loopCondition: string;
  maxIterations: string;
  variableSelectors: string;
  addVariable: string;
  removeVariable: string;
  assignmentMode: string;
  assignmentOverwrite: string;
  assignmentAppend: string;
  assignmentClear: string;
  noNodeRun: string;
  groups: Record<WorkflowNodeGroup, string>;
  statuses: Record<WorkflowNodeStatus, string>;
  validation: Record<WorkflowValidationCode, string>;
}

export const workflowCopy: Record<PlaygroundLang, WorkflowPlaygroundCopy> = {
  en: {
    pageTitle: 'Workflow Designer Playground',
    workflowName: 'Customer support triage',
    simulation: 'Local simulation',
    backHome: 'Back to A3S',
    saved: 'Saved locally',
    saving: 'Saving',
    undo: 'Undo',
    redo: 'Redo',
    reset: 'Reset example',
    validate: 'Validate',
    valid: 'Graph valid',
    run: 'Test run',
    stop: 'Stop run',
    addNode: 'Add node',
    selectMode: 'Select and move nodes',
    panMode: 'Pan canvas',
    canvasTools: 'Canvas history',
    moreActions: 'More workflow actions',
    variables: 'Variables',
    viewCachedVariables: 'View cached variables',
    history: 'Run history',
    fitView: 'Fit workflow',
    nodeLibrary: 'Add a node',
    nodeLibraryDescription: 'Choose from the versioned node profiles defined by A3S Cloud.',
    nodesTab: 'Nodes',
    toolsTab: 'Tools',
    searchNodes: 'Search nodes',
    noNodes: 'No matching nodes',
    close: 'Close',
    inspector: 'Node configuration',
    nodeName: 'Name',
    nodeDescription: 'Description',
    configuration: 'Configuration',
    inputVariables: 'Input variables',
    outputVariables: 'Output variables',
    inputPayload: 'Input',
    outputPayload: 'Output',
    steps: 'steps',
    template: 'Template',
    selector: 'Selector',
    capability: 'Capability reference',
    routes: 'Routes',
    routeHandle: 'Handle',
    routeEquals: 'Equals',
    defaultRoute: 'Default handle',
    decisionMessage: 'Decision message',
    decisionDetails: 'Decision details',
    expiry: 'Expires after (seconds)',
    retryAttempts: 'Maximum attempts',
    failureHandling: 'Failure handling',
    failureStop: 'Stop workflow',
    failureDefault: 'Use default output',
    failureRoute: 'Route failure',
    defaultOutput: 'Default output',
    runStep: 'Run this step',
    duplicate: 'Duplicate node',
    delete: 'Delete node',
    selectNode: 'Select a node to configure it.',
    debug: 'Debug console',
    settingsTab: 'Settings',
    lastRun: 'Last run',
    trace: 'Trace',
    cachedVariables: 'Variables',
    runHistory: 'History',
    editValue: 'Edit cached value',
    noTrace: 'Run the workflow or one node to inspect its trace.',
    noHistory: 'Completed simulations appear here.',
    noSelection: 'No node selected',
    openLibrary: 'Open node library',
    testRun: 'Test the workflow',
    runDescription: 'Supply example inputs, then follow each deterministic step in the debug console.',
    customerMessage: 'Customer message',
    priority: 'Priority',
    normal: 'Normal',
    high: 'High',
    startRun: 'Start run',
    running: 'Running',
    runSucceeded: 'Succeeded',
    runFailed: 'Failed',
    simulatedNotice: 'This Playground simulates execution in your browser. It does not call a production control plane.',
    validationTitle: 'Workflow validation',
    validationPassed: 'The graph is ready for a simulated run.',
    validationFailed: 'Resolve the listed graph issues before running.',
    resetTitle: 'Reset the example?',
    resetDescription: 'This replaces local edits with the original customer support workflow.',
    cancel: 'Cancel',
    confirmReset: 'Reset workflow',
    restored: 'The example workflow was restored.',
    graphExported: 'Workflow JSON downloaded.',
    exportGraph: 'Export JSON',
    addRoute: 'Add route',
    removeRoute: 'Remove route',
    keyboardHint: 'Drag to move. Scroll to pan. Ctrl or Cmd + Z to undo.',
    canvasLabel: 'Interactive workflow canvas',
    modelRoute: 'Model route',
    prompt: 'Prompt',
    code: 'Code',
    method: 'Method',
    url: 'URL',
    query: 'Query',
    filterField: 'Filter field',
    filterValue: 'Equals',
    collectionVariable: 'Collection variable',
    loopCondition: 'Loop condition',
    maxIterations: 'Maximum iterations',
    variableSelectors: 'Variables',
    addVariable: 'Add variable',
    removeVariable: 'Remove variable',
    assignmentMode: 'Assignment mode',
    assignmentOverwrite: 'Overwrite',
    assignmentAppend: 'Append',
    assignmentClear: 'Clear',
    noNodeRun: 'Run this node to inspect its latest input and output.',
    groups: {
      core: 'Core',
      intelligence: 'Intelligence',
      logic: 'Logic',
      transform: 'Transform',
      integration: 'Integrations',
      trigger: 'Triggers',
      human: 'Human interaction',
    },
    statuses: {
      idle: 'Idle',
      queued: 'Queued',
      running: 'Running',
      succeeded: 'Succeeded',
      failed: 'Failed',
    },
    validation: {
      input_count: 'The graph must contain exactly one Input node.',
      output_missing: 'The graph must contain at least one Output node.',
      missing_source: 'An edge references a missing source node.',
      missing_target: 'An edge references a missing target node.',
      missing_incoming: 'This node has no incoming connection.',
      missing_outgoing: 'This node has no outgoing connection.',
      branch_routes: 'A Branch node needs at least two outgoing connections.',
      cycle: 'The graph contains a cycle. Published workflows must be acyclic.',
      configuration: 'This node is missing required configuration.',
    },
  },
  cn: {
    pageTitle: '工作流设计器 Playground',
    workflowName: '客户支持分流',
    simulation: '本地模拟',
    backHome: '返回 A3S',
    saved: '已保存到本地',
    saving: '正在保存',
    undo: '撤销',
    redo: '重做',
    reset: '重置示例',
    validate: '校验',
    valid: '图已通过校验',
    run: '测试运行',
    stop: '停止运行',
    addNode: '添加节点',
    selectMode: '选择并移动节点',
    panMode: '平移画布',
    canvasTools: '画布历史',
    moreActions: '更多工作流操作',
    variables: '变量',
    viewCachedVariables: '查看缓存变量',
    history: '运行历史',
    fitView: '适配画布',
    nodeLibrary: '添加节点',
    nodeLibraryDescription: '选择 A3S Cloud 定义的版本化节点 Profile。',
    nodesTab: '节点',
    toolsTab: '工具',
    searchNodes: '搜索节点',
    noNodes: '没有匹配的节点',
    close: '关闭',
    inspector: '节点配置',
    nodeName: '名称',
    nodeDescription: '说明',
    configuration: '配置',
    inputVariables: '输入变量',
    outputVariables: '输出变量',
    inputPayload: '输入',
    outputPayload: '输出',
    steps: '个步骤',
    template: '模板',
    selector: '选择器',
    capability: '能力引用',
    routes: '路由',
    routeHandle: 'Handle',
    routeEquals: '匹配值',
    defaultRoute: '默认 Handle',
    decisionMessage: '决策消息',
    decisionDetails: '决策详情',
    expiry: '过期时间（秒）',
    retryAttempts: '最大尝试次数',
    failureHandling: '失败处理',
    failureStop: '停止工作流',
    failureDefault: '使用默认输出',
    failureRoute: '路由失败',
    defaultOutput: '默认输出',
    runStep: '运行此步骤',
    duplicate: '复制节点',
    delete: '删除节点',
    selectNode: '选择一个节点进行配置。',
    debug: '调试控制台',
    settingsTab: '设置',
    lastRun: '上次运行',
    trace: '跟踪',
    cachedVariables: '变量',
    runHistory: '历史',
    editValue: '编辑缓存值',
    noTrace: '运行整个工作流或单个节点后，可在这里查看跟踪。',
    noHistory: '完成的模拟运行会显示在这里。',
    noSelection: '未选择节点',
    openLibrary: '打开节点库',
    testRun: '测试工作流',
    runDescription: '填写示例输入，然后在调试控制台中查看每个确定性步骤。',
    customerMessage: '客户消息',
    priority: '优先级',
    normal: '普通',
    high: '高',
    startRun: '开始运行',
    running: '运行中',
    runSucceeded: '成功',
    runFailed: '失败',
    simulatedNotice: 'Playground 仅在浏览器中模拟执行，不会调用生产控制面。',
    validationTitle: '工作流校验',
    validationPassed: '图已准备好进行模拟运行。',
    validationFailed: '请先解决列出的图问题。',
    resetTitle: '重置示例？',
    resetDescription: '这会使用初始客户支持工作流替换本地编辑。',
    cancel: '取消',
    confirmReset: '重置工作流',
    restored: '已恢复示例工作流。',
    graphExported: '已下载工作流 JSON。',
    exportGraph: '导出 JSON',
    addRoute: '添加路由',
    removeRoute: '删除路由',
    keyboardHint: '拖动节点，滚动平移，Ctrl 或 Cmd + Z 撤销。',
    canvasLabel: '交互式工作流画布',
    modelRoute: '模型路由',
    prompt: '提示词',
    code: '代码',
    method: '方法',
    url: 'URL',
    query: '查询',
    filterField: '筛选字段',
    filterValue: '等于',
    collectionVariable: '集合变量',
    loopCondition: '循环条件',
    maxIterations: '最大迭代次数',
    variableSelectors: '变量',
    addVariable: '添加变量',
    removeVariable: '移除变量',
    assignmentMode: '赋值模式',
    assignmentOverwrite: '覆盖',
    assignmentAppend: '追加',
    assignmentClear: '清除',
    noNodeRun: '运行此节点后，可在这里检查最近一次输入和输出。',
    groups: {
      core: '核心',
      intelligence: '智能能力',
      logic: '逻辑',
      transform: '转换',
      integration: '集成',
      trigger: '触发器',
      human: '人工交互',
    },
    statuses: {
      idle: '空闲',
      queued: '等待中',
      running: '运行中',
      succeeded: '成功',
      failed: '失败',
    },
    validation: {
      input_count: '图中必须且只能有一个输入节点。',
      output_missing: '图中至少需要一个输出节点。',
      missing_source: '有一条边引用了不存在的源节点。',
      missing_target: '有一条边引用了不存在的目标节点。',
      missing_incoming: '此节点没有入向连接。',
      missing_outgoing: '此节点没有出向连接。',
      branch_routes: '分支节点至少需要两条出向连接。',
      cycle: '图中存在循环，发布的工作流必须是无环图。',
      configuration: '此节点缺少必填配置。',
    },
  },
};
