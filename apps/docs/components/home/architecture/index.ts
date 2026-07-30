import { architectureNode as node, localized, type ArchitectureProject } from './architecture-types';
import { interfaceArchitectureProjects } from './interfaces';
import { productArchitectureProjects } from './products';
import { runtimeArchitectureProjects } from './runtime';

export type {
  ArchitectureCategory,
  ArchitectureNode,
  ArchitectureProject,
  ArchitectureTone,
  LocalizedArchitectureText,
} from './architecture-types';

export const systemArchitectureProject: ArchitectureProject = {
  id: 'a3s-system',
  name: 'A3S System',
  category: 'products',
  role: localized('34 个独立演进的项目，通过显式契约组成一套可治理 Agent 系统。', 'Thirty-four independently evolving projects composed through explicit contracts.'),
  href: 'https://github.com/A3S-Lab/a3s',
  nodes: [
    node('entrypoints', 'Entrypoints', 'surface', '终端、浏览器与 Rust、Node.js、Python、Go SDK。', 'Terminal, browser, and Rust, Node.js, Python, and Go SDKs.'),
    node('hosts', 'Product Hosts', 'core', 'CLI、Web、Bench 与 Cloud 拥有模型、工具和权限策略。', 'CLI, Web, Bench, and Cloud own model, tool, and permission policy.'),
    node('governed', 'Governed Core', 'contract', 'Code、Use、Flow、Event、Lane 与 Memory 承载治理和状态。', 'Code, Use, Flow, Event, Lane, and Memory carry governance and state.'),
    node('contracts', 'Runtime Contracts', 'runtime', 'Task、Service、Store、Provider 与 Driver 保持可替换。', 'Task, Service, Store, Provider, and Driver boundaries remain replaceable.'),
    node('drivers', 'Execution + Evidence', 'evidence', '进程、容器、MicroVM 与远程 Provider 负责执行并返回证据。', 'Processes, containers, MicroVMs, and remote providers execute and return evidence.'),
  ],
};

export const architectureProjects: readonly ArchitectureProject[] = [
  ...productArchitectureProjects,
  ...runtimeArchitectureProjects,
  ...interfaceArchitectureProjects,
];

export const architectureProjectCount = architectureProjects.length;
