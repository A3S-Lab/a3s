import type { Lang } from '@/components/home/home-content';

type LocalizedText = Record<Lang, string>;

export type CliCapabilityTone = 'context' | 'action' | 'success';

export interface CliCapabilityScene {
  id: 'code' | 'web' | 'box' | 'bench' | 'search' | 'use' | 'observe' | 'manage';
  label: string;
  command: LocalizedText;
  output: Array<{
    key: string;
    value: LocalizedText;
    tone: CliCapabilityTone;
  }>;
}

export const cliCapabilityScenes: CliCapabilityScene[] = [
  {
    id: 'code',
    label: 'CODE',
    command: {
      cn: 'a3s code exec "修复失败的认证测试" --mode auto',
      en: 'a3s code exec "fix the failing auth test" --mode auto',
    },
    output: [
      { key: 'workspace', value: { cn: '/repo · 策略已加载', en: '/repo · policy loaded' }, tone: 'context' },
      { key: 'workflow', value: { cn: '检查 → 修改 → 测试', en: 'inspect → edit → test' }, tone: 'action' },
      { key: 'result', value: { cn: '42 项测试通过', en: '42 tests passed' }, tone: 'success' },
    ],
  },
  {
    id: 'web',
    label: 'WEB',
    command: { cn: 'a3s web -d', en: 'a3s web -d' },
    output: [
      { key: 'surface', value: { cn: 'Web + 本地 API', en: 'Web + local API' }, tone: 'context' },
      { key: 'endpoint', value: { cn: 'http://127.0.0.1:3000', en: 'http://127.0.0.1:3000' }, tone: 'action' },
      { key: 'status', value: { cn: '后台实例已就绪', en: 'managed instance ready' }, tone: 'success' },
    ],
  },
  {
    id: 'box',
    label: 'BOX',
    command: { cn: 'a3s up -d', en: 'a3s up -d' },
    output: [
      { key: 'compose', value: { cn: '3 个服务已收敛', en: '3 services converged' }, tone: 'context' },
      { key: 'runtime', value: { cn: 'MicroVM / Sandbox', en: 'MicroVM / Sandbox' }, tone: 'action' },
      { key: 'health', value: { cn: '全部服务健康', en: 'all services healthy' }, tone: 'success' },
    ],
  },
  {
    id: 'bench',
    label: 'BENCH',
    command: {
      cn: 'a3s bench run ./tasks/smoke --agent codex',
      en: 'a3s bench run ./tasks/smoke --agent codex',
    },
    output: [
      { key: 'inputs', value: { cn: '任务与候选已锁定', en: 'task and candidate locked' }, tone: 'context' },
      { key: 'trial', value: { cn: '隔离运行，可重放', en: 'isolated and replayable' }, tone: 'action' },
      { key: 'report', value: { cn: '评分与证据已保存', en: 'score and evidence saved' }, tone: 'success' },
    ],
  },
  {
    id: 'search',
    label: 'SEARCH',
    command: { cn: 'a3s search doctor', en: 'a3s search doctor' },
    output: [
      { key: 'engines', value: { cn: 'DuckDuckGo · Wikipedia', en: 'DuckDuckGo · Wikipedia' }, tone: 'context' },
      { key: 'browser', value: { cn: 'Chrome 已就绪', en: 'Chrome ready' }, tone: 'action' },
      { key: 'fallback', value: { cn: '失败边界可见', en: 'failure boundary visible' }, tone: 'success' },
    ],
  },
  {
    id: 'use',
    label: 'USE',
    command: { cn: 'a3s use capabilities --json', en: 'a3s use capabilities --json' },
    output: [
      { key: 'browser', value: { cn: '渲染 · 自动化', en: 'render · automate' }, tone: 'context' },
      { key: 'office', value: { cn: '编辑 · 导出', en: 'edit · export' }, tone: 'action' },
      { key: 'ocr', value: { cn: '识别 · 来源证据', en: 'extract · source evidence' }, tone: 'success' },
    ],
  },
  {
    id: 'observe',
    label: 'TOP',
    command: { cn: 'a3s top --view agents', en: 'a3s top --view agents' },
    output: [
      { key: 'agents', value: { cn: '4 个运行中', en: '4 running' }, tone: 'context' },
      { key: 'runtime', value: { cn: '容器 · Session', en: 'containers · sessions' }, tone: 'action' },
      { key: 'events', value: { cn: '工具 · 安全 · 出站', en: 'tool · security · egress' }, tone: 'success' },
    ],
  },
  {
    id: 'manage',
    label: 'OPS',
    command: { cn: 'a3s doctor', en: 'a3s doctor' },
    output: [
      { key: 'components', value: { cn: 'Code · Box · Use', en: 'Code · Box · Use' }, tone: 'context' },
      { key: 'lifecycle', value: { cn: '安装 · 升级 · 卸载', en: 'install · upgrade · uninstall' }, tone: 'action' },
      { key: 'status', value: { cn: '系统健康', en: 'system healthy' }, tone: 'success' },
    ],
  },
];

export const cliTerminalCopy = {
  cn: {
    accessibleLabel: 'A3S CLI 能力演示：编码、本地 Web、隔离运行、评测、搜索、Browser、Office、OCR、运行观测与组件管理。',
    chromeTitle: 'a3s · ~/workspace',
    ready: 'CLI 在线',
    motd: 'A3S CLI v2.1 · 已载入 8 组能力',
    hint: '输入 a3s help 查看完整命令',
    capability: '能力',
  },
  en: {
    accessibleLabel: 'A3S CLI capability demo covering coding, local Web, isolated execution, evaluation, search, Browser, Office, OCR, runtime observation, and component management.',
    chromeTitle: 'a3s · ~/workspace',
    ready: 'CLI READY',
    motd: 'A3S CLI v2.1 · 8 capability groups loaded',
    hint: 'Type a3s help to inspect every command',
    capability: 'CAPABILITY',
  },
} as const;
