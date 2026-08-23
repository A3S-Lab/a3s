import type { Edge, Node, XYPosition } from '@xyflow/react';

export type PlaygroundLang = 'cn' | 'en';

export type WorkflowStepKind =
  | 'input'
  | 'output'
  | 'transform'
  | 'branch'
  | 'human_decision'
  | 'execution'
  | 'agent'
  | 'mcp'
  | 'model'
  | 'tool'
  | 'service'
  | 'memory'
  | 'subworkflow';

export type WorkflowNodeProfile =
  | 'agent'
  | 'answer'
  | 'code'
  | 'document-extractor'
  | 'http-request'
  | 'human-input'
  | 'if-else'
  | 'integration-trigger'
  | 'iteration'
  | 'knowledge-retrieval'
  | 'list-operator'
  | 'llm'
  | 'loop'
  | 'output'
  | 'parameter-extractor'
  | 'question-classifier'
  | 'schedule-trigger'
  | 'template'
  | 'tool'
  | 'user-input'
  | 'variable-aggregator'
  | 'variable-assigner'
  | 'webhook-trigger';

export type WorkflowNodeGroup =
  | 'core'
  | 'intelligence'
  | 'logic'
  | 'transform'
  | 'integration'
  | 'trigger'
  | 'human';
export type WorkflowNodeStatus = 'idle' | 'queued' | 'running' | 'succeeded' | 'failed';
export type WorkflowCanvasMode = 'select' | 'pan';
export type WorkflowExecutionClass =
  | 'workflow_local'
  | 'owning_application_port'
  | 'composite_region'
  | 'invocation_only';

export interface WorkflowRoute {
  handle: string;
  equals: string;
}

export interface WorkflowNodeConfiguration {
  template: string;
  selector: string;
  routes: WorkflowRoute[];
  defaultHandle: string;
  message: string;
  details: string;
  expiresAfterSeconds: number;
  capability: string;
  retryAttempts: number;
  failureMode: 'stop' | 'default_output' | 'route';
  defaultOutput: string;
  model: string;
  prompt: string;
  code: string;
  method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  url: string;
  query: string;
  variableSelectors: string[];
  assignmentMode: 'overwrite' | 'append' | 'clear';
  listFilterField: string;
  listFilterValue: string;
  inputVariable: string;
  maxIterations: number;
  loopCondition: string;
}

export interface WorkflowNodeData extends Record<string, unknown> {
  profile: WorkflowNodeProfile;
  kind: WorkflowStepKind;
  label: string;
  description: string;
  configuration: WorkflowNodeConfiguration;
  lang?: PlaygroundLang;
  runtimeStatus?: WorkflowNodeStatus;
  relationState?: 'active' | 'dimmed';
  lastRun?: WorkflowRunStep;
  onRun?: (nodeId: string) => void;
  onDuplicate?: (nodeId: string) => void;
  onDelete?: (nodeId: string) => void;
}

export interface WorkflowEdgeData extends Record<string, unknown> {
  route?: string;
  insertLabel?: string;
  relationState?: 'active' | 'dimmed';
  onInsert?: (edgeId: string) => void;
}

export type WorkflowNode = Node<WorkflowNodeData, 'workflow'>;
export type WorkflowEdge = Edge<WorkflowEdgeData, 'workflow'>;

export interface WorkflowGraph {
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
}

export interface WorkflowCatalogItem {
  profile: WorkflowNodeProfile;
  kind: WorkflowStepKind;
  group: WorkflowNodeGroup;
  executionClass: WorkflowExecutionClass;
  semanticProfile: string;
  name: Record<PlaygroundLang, string>;
  description: Record<PlaygroundLang, string>;
}

export type WorkflowValidationCode =
  | 'input_count'
  | 'output_missing'
  | 'missing_source'
  | 'missing_target'
  | 'missing_incoming'
  | 'missing_outgoing'
  | 'branch_routes'
  | 'cycle'
  | 'configuration';

export interface WorkflowValidationIssue {
  code: WorkflowValidationCode;
  nodeId?: string;
}

export interface WorkflowRunStep {
  nodeId: string;
  label: string;
  profile: WorkflowNodeProfile;
  kind: WorkflowStepKind;
  status: Exclude<WorkflowNodeStatus, 'idle' | 'queued' | 'running'>;
  durationMs: number;
  input: Record<string, unknown>;
  output: Record<string, unknown>;
}

const bilingual = (en: string, cn: string): Record<PlaygroundLang, string> => ({ en, cn });

export const workflowCatalog: WorkflowCatalogItem[] = [
  { profile: 'agent', kind: 'agent', group: 'intelligence', executionClass: 'owning_application_port', semanticProfile: 'agent.release', name: bilingual('Agent', 'Agent'), description: bilingual('Run an admitted Agent release', '运行已准入的 Agent 版本') },
  { profile: 'answer', kind: 'output', group: 'core', executionClass: 'owning_application_port', semanticProfile: 'application.answer', name: bilingual('Answer', '回答'), description: bilingual('Emit an application answer frame', '发送应用回答帧') },
  { profile: 'code', kind: 'execution', group: 'transform', executionClass: 'owning_application_port', semanticProfile: 'execution.code', name: bilingual('Code', '代码'), description: bilingual('Run a finite code task', '运行有限代码任务') },
  { profile: 'document-extractor', kind: 'service', group: 'transform', executionClass: 'owning_application_port', semanticProfile: 'knowledge.document-extract', name: bilingual('Document Extractor', '文档提取器'), description: bilingual('Extract bounded document content', '提取有界文档内容') },
  { profile: 'http-request', kind: 'service', group: 'integration', executionClass: 'owning_application_port', semanticProfile: 'connector.http', name: bilingual('HTTP Request', 'HTTP 请求'), description: bilingual('Call a governed HTTP connector', '调用受治理的 HTTP 连接器') },
  { profile: 'human-input', kind: 'human_decision', group: 'human', executionClass: 'workflow_local', semanticProfile: 'workflow.human-input', name: bilingual('Human Input', '人工输入'), description: bilingual('Pause for an authorized response', '暂停并等待授权响应') },
  { profile: 'if-else', kind: 'branch', group: 'logic', executionClass: 'workflow_local', semanticProfile: 'workflow.if-else', name: bilingual('If / Else', '条件分支'), description: bilingual('Route by an exact selector', '按精确选择器路由') },
  { profile: 'integration-trigger', kind: 'input', group: 'trigger', executionClass: 'invocation_only', semanticProfile: 'automation.plugin-trigger', name: bilingual('Integration Trigger', '集成触发器'), description: bilingual('Start from an admitted integration event', '由已准入的集成事件启动') },
  { profile: 'iteration', kind: 'subworkflow', group: 'logic', executionClass: 'composite_region', semanticProfile: 'workflow.iteration', name: bilingual('Iteration', '迭代'), description: bilingual('Process each item in a bounded region', '在有界区域内逐项处理') },
  { profile: 'knowledge-retrieval', kind: 'service', group: 'intelligence', executionClass: 'owning_application_port', semanticProfile: 'knowledge.retrieve', name: bilingual('Knowledge Retrieval', '知识检索'), description: bilingual('Retrieve governed knowledge records', '检索受治理的知识记录') },
  { profile: 'list-operator', kind: 'transform', group: 'transform', executionClass: 'workflow_local', semanticProfile: 'workflow.list-operator', name: bilingual('List Operator', '列表操作'), description: bilingual('Filter a typed list deterministically', '确定性筛选类型化列表') },
  { profile: 'llm', kind: 'model', group: 'intelligence', executionClass: 'owning_application_port', semanticProfile: 'model.llm', name: bilingual('LLM', '大语言模型'), description: bilingual('Use an exact model route', '使用精确模型路由') },
  { profile: 'loop', kind: 'subworkflow', group: 'logic', executionClass: 'composite_region', semanticProfile: 'workflow.loop', name: bilingual('Loop', '循环'), description: bilingual('Repeat a bounded composite region', '重复有界组合区域') },
  { profile: 'output', kind: 'output', group: 'core', executionClass: 'workflow_local', semanticProfile: 'workflow.output', name: bilingual('Output', '输出'), description: bilingual('Return typed workflow results', '返回类型化工作流结果') },
  { profile: 'parameter-extractor', kind: 'model', group: 'intelligence', executionClass: 'owning_application_port', semanticProfile: 'model.parameter-extract', name: bilingual('Parameter Extractor', '参数提取器'), description: bilingual('Extract typed structured parameters', '提取类型化结构参数') },
  { profile: 'question-classifier', kind: 'model', group: 'intelligence', executionClass: 'owning_application_port', semanticProfile: 'model.question-classifier', name: bilingual('Question Classifier', '问题分类器'), description: bilingual('Classify input into exact routes', '将输入分类到精确路由') },
  { profile: 'schedule-trigger', kind: 'input', group: 'trigger', executionClass: 'invocation_only', semanticProfile: 'automation.schedule', name: bilingual('Schedule Trigger', '定时触发器'), description: bilingual('Start from an admitted schedule', '由已准入的计划启动') },
  { profile: 'template', kind: 'transform', group: 'transform', executionClass: 'workflow_local', semanticProfile: 'workflow.template', name: bilingual('Template', '模板'), description: bilingual('Render a deterministic template', '渲染确定性模板') },
  { profile: 'tool', kind: 'tool', group: 'integration', executionClass: 'owning_application_port', semanticProfile: 'use.tool', name: bilingual('Tool', '工具'), description: bilingual('Apply an admitted A3S Use capability', '应用已准入的 A3S Use 能力') },
  { profile: 'user-input', kind: 'input', group: 'core', executionClass: 'workflow_local', semanticProfile: 'workflow.user-input', name: bilingual('User Input', '用户输入'), description: bilingual('Declare typed workflow inputs', '声明类型化工作流输入') },
  { profile: 'variable-aggregator', kind: 'transform', group: 'transform', executionClass: 'workflow_local', semanticProfile: 'workflow.variable-aggregate', name: bilingual('Variable Aggregator', '变量聚合器'), description: bilingual('Converge exclusive branch values', '汇聚互斥分支值') },
  { profile: 'variable-assigner', kind: 'service', group: 'transform', executionClass: 'owning_application_port', semanticProfile: 'application.conversation-variable-assign', name: bilingual('Variable Assigner', '变量赋值器'), description: bilingual('Update an application variable', '更新应用变量') },
  { profile: 'webhook-trigger', kind: 'input', group: 'trigger', executionClass: 'invocation_only', semanticProfile: 'automation.webhook', name: bilingual('Webhook Trigger', 'Webhook 触发器'), description: bilingual('Start from a verified webhook', '由已验证的 Webhook 启动') },
];

const triggerProfiles = new Set<WorkflowNodeProfile>([
  'user-input', 'schedule-trigger', 'webhook-trigger', 'integration-trigger',
]);
const terminalProfiles = new Set<WorkflowNodeProfile>(['output', 'answer']);
const routingProfiles = new Set<WorkflowNodeProfile>(['if-else', 'question-classifier']);

export function isTriggerProfile(profile: WorkflowNodeProfile): boolean {
  return triggerProfiles.has(profile);
}

export function isTerminalProfile(profile: WorkflowNodeProfile): boolean {
  return terminalProfiles.has(profile);
}

export function isRoutingProfile(profile: WorkflowNodeProfile): boolean {
  return routingProfiles.has(profile);
}

export function getCatalogItem(profile: WorkflowNodeProfile): WorkflowCatalogItem {
  const item = workflowCatalog.find((candidate) => candidate.profile === profile);
  if (!item) throw new Error(`Unsupported workflow node profile: ${profile}`);
  return item;
}

export function defaultConfiguration(profile: WorkflowNodeProfile, lang: PlaygroundLang = 'en'): WorkflowNodeConfiguration {
  const base: WorkflowNodeConfiguration = {
    template: '', selector: '', routes: [], defaultHandle: 'default', message: '', details: '',
    expiresAfterSeconds: 3600, capability: '', retryAttempts: 3, failureMode: 'stop',
    defaultOutput: '{}', model: '', prompt: '', code: '', method: 'GET', url: '', query: '',
    variableSelectors: [], assignmentMode: 'overwrite', listFilterField: 'type',
    listFilterValue: 'image', inputVariable: '{{input.items}}', maxIterations: 20,
    loopCondition: '{{current.should_continue}}',
  };

  if (profile === 'template') base.template = '{{current.result}}';
  if (profile === 'output' || profile === 'answer') base.template = '{{current}}';
  if (routingProfiles.has(profile)) {
    base.selector = '{{input.priority}}';
    base.routes = [{ handle: 'priority', equals: 'high' }, { handle: 'standard', equals: 'normal' }];
    base.defaultHandle = 'standard';
  }
  if (profile === 'human-input') {
    base.message = lang === 'cn' ? '批准高优先级响应' : 'Approve the priority response';
    base.details = lang === 'cn' ? '请在工作流继续前审核建议操作。' : 'Review the proposed action before the workflow continues.';
  }
  if (profile === 'llm') {
    base.model = 'inference/support-classifier@3';
    base.prompt = 'Classify the support request and return a typed result.';
  }
  if (profile === 'parameter-extractor') {
    base.model = 'inference/parameter-extractor@2';
    base.prompt = 'Extract customer_id and request_type.';
  }
  if (profile === 'question-classifier') base.model = 'inference/support-classifier@3';
  if (profile === 'agent') base.capability = 'agent/support-response@5';
  if (profile === 'knowledge-retrieval') { base.capability = 'knowledge/support@2'; base.query = '{{input.customer_message}}'; }
  if (profile === 'document-extractor') base.capability = 'knowledge/document-extract@1';
  if (profile === 'http-request') { base.capability = 'connector/support-api@4'; base.url = 'https://api.example.test/tickets'; }
  if (profile === 'tool') base.capability = 'tool/ticket-priority@1';
  if (profile === 'code') base.code = 'return { result: input };';
  if (profile === 'variable-aggregator') base.variableSelectors = ['{{steps.priority.output}}', '{{steps.standard.output}}'];
  if (profile === 'variable-assigner') { base.capability = 'application/customer-context@2'; base.variableSelectors = ['{{current}}']; }
  if (profile === 'schedule-trigger') base.capability = 'automation/daily-support@1';
  if (profile === 'webhook-trigger') base.capability = 'automation/support-webhook@1';
  if (profile === 'integration-trigger') base.capability = 'automation/support-event@1';
  if (profile === 'loop') base.maxIterations = 8;
  return base;
}

export function createWorkflowNode(
  profile: WorkflowNodeProfile,
  position: XYPosition,
  lang: PlaygroundLang,
  id: string,
  label?: string,
): WorkflowNode {
  const item = getCatalogItem(profile);
  return {
    id,
    type: 'workflow',
    position,
    data: {
      profile,
      kind: item.kind,
      label: label ?? item.name[lang],
      description: item.description[lang],
      configuration: defaultConfiguration(profile, lang),
    },
  };
}

function edge(id: string, source: string, target: string, route?: string): WorkflowEdge {
  return { id, source, target, type: 'workflow', data: route ? { route } : {}, label: route };
}

export function createInitialWorkflow(lang: PlaygroundLang): WorkflowGraph {
  const labels = lang === 'cn'
    ? ['接收请求', '识别意图', '优先级判断', '人工确认', '生成回复', '返回结果']
    : ['Receive request', 'Classify intent', 'Priority check', 'Human review', 'Compose response', 'Return result'];

  return {
    nodes: [
      createWorkflowNode('user-input', { x: 60, y: 220 }, lang, 'input', labels[0]),
      createWorkflowNode('llm', { x: 350, y: 220 }, lang, 'classify', labels[1]),
      createWorkflowNode('if-else', { x: 650, y: 220 }, lang, 'priority', labels[2]),
      createWorkflowNode('human-input', { x: 960, y: 70 }, lang, 'review', labels[3]),
      createWorkflowNode('agent', { x: 960, y: 370 }, lang, 'compose', labels[4]),
      createWorkflowNode('output', { x: 1280, y: 370 }, lang, 'output', labels[5]),
    ],
    edges: [
      edge('input-classify', 'input', 'classify'),
      edge('classify-priority', 'classify', 'priority'),
      edge('priority-review', 'priority', 'review', 'priority'),
      edge('priority-compose', 'priority', 'compose', 'standard'),
      edge('review-compose', 'review', 'compose'),
      edge('compose-output', 'compose', 'output'),
    ],
  };
}

export function cloneWorkflowGraph(graph: WorkflowGraph): WorkflowGraph {
  return {
    nodes: graph.nodes.map((node) => ({
      ...node,
      position: { ...node.position },
      data: {
        ...node.data,
        configuration: {
          ...node.data.configuration,
          routes: node.data.configuration.routes.map((route) => ({ ...route })),
          variableSelectors: [...node.data.configuration.variableSelectors],
        },
      },
    })),
    edges: graph.edges.map((item) => ({ ...item, data: { ...item.data } })),
  };
}

const legacyProfileByKind: Record<WorkflowStepKind, WorkflowNodeProfile> = {
  input: 'user-input', output: 'output', transform: 'template', branch: 'if-else',
  human_decision: 'human-input', execution: 'code', agent: 'agent', mcp: 'tool', model: 'llm',
  tool: 'tool', service: 'http-request', memory: 'knowledge-retrieval', subworkflow: 'iteration',
};

export function normalizePersistedWorkflow(graph: WorkflowGraph): WorkflowGraph {
  const nodes = graph.nodes.map((node) => {
    const profile = node.data.profile ?? legacyProfileByKind[node.data.kind];
    const defaults = defaultConfiguration(profile, node.data.lang ?? 'en');
    return {
      ...node,
      data: {
        ...node.data,
        profile,
        kind: getCatalogItem(profile).kind,
        configuration: { ...defaults, ...node.data.configuration },
      },
    };
  });
  return cloneWorkflowGraph({ nodes, edges: graph.edges });
}

function configurationIsValid(node: WorkflowNode): boolean {
  const config = node.data.configuration;
  const profile = node.data.profile;
  if (profile === 'template' || terminalProfiles.has(profile)) return config.template.trim().length > 0;
  if (routingProfiles.has(profile)) return config.selector.trim().length > 0 && config.routes.length > 0;
  if (profile === 'human-input') return config.message.trim().length > 0 && config.expiresAfterSeconds > 0;
  if (profile === 'llm' || profile === 'parameter-extractor') return config.model.trim().length > 0 && config.prompt.trim().length > 0;
  if (profile === 'question-classifier') return config.model.trim().length > 0;
  if (profile === 'code') return config.code.trim().length > 0;
  if (profile === 'http-request') return config.capability.trim().length > 0 && /^https?:\/\//.test(config.url);
  if (profile === 'list-operator') return config.listFilterField.trim().length > 0;
  if (profile === 'variable-aggregator' || profile === 'variable-assigner') return config.variableSelectors.length > 0;
  if (profile === 'iteration' || profile === 'loop') return config.maxIterations > 0 && config.maxIterations <= 100;
  if (['agent', 'document-extractor', 'knowledge-retrieval', 'tool', 'schedule-trigger', 'webhook-trigger', 'integration-trigger'].includes(profile)) {
    return config.capability.trim().length > 0;
  }
  return true;
}

export function validateWorkflow(graph: WorkflowGraph): WorkflowValidationIssue[] {
  const issues: WorkflowValidationIssue[] = [];
  const nodeIds = new Set(graph.nodes.map((node) => node.id));
  const inputs = graph.nodes.filter((node) => triggerProfiles.has(node.data.profile));
  const outputs = graph.nodes.filter((node) => terminalProfiles.has(node.data.profile));

  if (inputs.length !== 1) issues.push({ code: 'input_count' });
  if (outputs.length === 0) issues.push({ code: 'output_missing' });

  for (const item of graph.edges) {
    if (!nodeIds.has(item.source)) issues.push({ code: 'missing_source' });
    if (!nodeIds.has(item.target)) issues.push({ code: 'missing_target' });
  }

  for (const node of graph.nodes) {
    const incoming = graph.edges.filter((item) => item.target === node.id);
    const outgoing = graph.edges.filter((item) => item.source === node.id);
    if (!triggerProfiles.has(node.data.profile) && incoming.length === 0) issues.push({ code: 'missing_incoming', nodeId: node.id });
    if (!terminalProfiles.has(node.data.profile) && outgoing.length === 0) issues.push({ code: 'missing_outgoing', nodeId: node.id });
    if (routingProfiles.has(node.data.profile) && outgoing.length < 2) issues.push({ code: 'branch_routes', nodeId: node.id });
    if (!configurationIsValid(node)) issues.push({ code: 'configuration', nodeId: node.id });
  }

  if (topologicalNodes(graph).length !== graph.nodes.length) issues.push({ code: 'cycle' });
  return issues;
}

export function topologicalNodes(graph: WorkflowGraph): WorkflowNode[] {
  const nodesById = new Map(graph.nodes.map((node) => [node.id, node]));
  const indegree = new Map(graph.nodes.map((node) => [node.id, 0]));
  const outgoing = new Map(graph.nodes.map((node) => [node.id, [] as string[]]));

  for (const item of graph.edges) {
    if (!nodesById.has(item.source) || !nodesById.has(item.target)) continue;
    indegree.set(item.target, (indegree.get(item.target) ?? 0) + 1);
    outgoing.get(item.source)?.push(item.target);
  }

  const queue = graph.nodes.filter((node) => indegree.get(node.id) === 0);
  const ordered: WorkflowNode[] = [];
  while (queue.length > 0) {
    const node = queue.shift();
    if (!node) break;
    ordered.push(node);
    for (const target of outgoing.get(node.id) ?? []) {
      const next = (indegree.get(target) ?? 0) - 1;
      indegree.set(target, next);
      if (next === 0) {
        const targetNode = nodesById.get(target);
        if (targetNode) queue.push(targetNode);
      }
    }
  }
  return ordered;
}

export function relatedNodeIds(graph: WorkflowGraph, nodeId: string): Set<string> {
  const related = new Set<string>([nodeId]);
  const queue = [nodeId];
  while (queue.length > 0) {
    const current = queue.shift();
    if (!current) break;
    for (const item of graph.edges) {
      const adjacent = item.source === current ? item.target : item.target === current ? item.source : undefined;
      if (adjacent && !related.has(adjacent)) {
        related.add(adjacent);
        queue.push(adjacent);
      }
    }
  }
  return related;
}

export function insertNodeOnEdge(graph: WorkflowGraph, edgeId: string, node: WorkflowNode): WorkflowGraph {
  const targetEdge = graph.edges.find((item) => item.id === edgeId);
  if (!targetEdge) return { nodes: [...graph.nodes, node], edges: [...graph.edges] };

  return {
    nodes: [...graph.nodes, node],
    edges: [
      ...graph.edges.filter((item) => item.id !== edgeId),
      edge(`${targetEdge.source}-${node.id}`, targetEdge.source, node.id, targetEdge.data?.route),
      edge(`${node.id}-${targetEdge.target}`, node.id, targetEdge.target),
    ],
  };
}
