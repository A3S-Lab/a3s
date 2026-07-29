export type CliTerminalLocale = 'zh' | 'en';

type Localized = Record<CliTerminalLocale, string>;
type OutputTone = 'accent' | 'muted' | 'success';

interface TerminalOutputLine {
  label: Localized;
  value: Localized;
  tone: OutputTone;
}

interface TerminalScenario {
  id: string;
  label: string;
  command: string;
  summary: Localized;
  output: readonly TerminalOutputLine[];
}

export const cliTerminalScenarios: readonly TerminalScenario[] = [
  {
    id: 'code',
    label: 'CODE',
    command: 'a3s code',
    summary: {
      zh: 'Code 终端工作区已准备就绪',
      en: 'The Code terminal workspace is ready',
    },
    output: [
      {
        label: { zh: '解析', en: 'resolve' },
        value: { zh: '内置宿主 · Code', en: 'bundled host · Code' },
        tone: 'accent',
      },
      {
        label: { zh: '加载', en: 'load' },
        value: {
          zh: '当前工作区与 ACL 配置',
          en: 'workspace and ACL configuration',
        },
        tone: 'muted',
      },
      {
        label: { zh: '结果', en: 'result' },
        value: {
          zh: '交互式 Agent Session',
          en: 'interactive agent session',
        },
        tone: 'success',
      },
    ],
  },
  {
    id: 'web',
    label: 'WEB',
    command: 'a3s web',
    summary: {
      zh: '本地浏览器工作区已启动',
      en: 'The local browser workspace is running',
    },
    output: [
      {
        label: { zh: '解析', en: 'resolve' },
        value: { zh: '内置宿主 · Web', en: 'bundled host · Web' },
        tone: 'accent',
      },
      {
        label: { zh: '绑定', en: 'bind' },
        value: { zh: '仅本机回环地址', en: 'loopback interface only' },
        tone: 'muted',
      },
      {
        label: { zh: '结果', en: 'result' },
        value: {
          zh: 'Work #home 与 Knowledge',
          en: 'Work #home and Knowledge',
        },
        tone: 'success',
      },
    ],
  },
  {
    id: 'box',
    label: 'BOX',
    command: 'a3s box ps',
    summary: {
      zh: 'Box 返回当前 Sandbox 状态',
      en: 'Box reports the current sandbox state',
    },
    output: [
      {
        label: { zh: '解析', en: 'resolve' },
        value: { zh: '受管组件 · Box', en: 'managed component · Box' },
        tone: 'accent',
      },
      {
        label: { zh: '检查', en: 'check' },
        value: {
          zh: '安装状态与宿主机能力',
          en: 'install state and host capabilities',
        },
        tone: 'muted',
      },
      {
        label: { zh: '结果', en: 'result' },
        value: {
          zh: 'MicroVM Sandbox 列表',
          en: 'MicroVM sandbox inventory',
        },
        tone: 'success',
      },
    ],
  },
  {
    id: 'use',
    label: 'USE',
    command: 'a3s use capabilities --json',
    summary: {
      zh: '类型化能力目录已输出为 JSON',
      en: 'The typed capability catalog is emitted as JSON',
    },
    output: [
      {
        label: { zh: '解析', en: 'resolve' },
        value: { zh: '受管组件 · Use', en: 'managed component · Use' },
        tone: 'accent',
      },
      {
        label: { zh: '路由', en: 'route' },
        value: {
          zh: 'Browser · OCR · Box · 外部包',
          en: 'Browser · OCR · Box · external packages',
        },
        tone: 'muted',
      },
      {
        label: { zh: '结果', en: 'result' },
        value: {
          zh: '带来源信息的能力目录',
          en: 'source-qualified capability catalog',
        },
        tone: 'success',
      },
    ],
  },
  {
    id: 'doctor',
    label: 'DOCTOR',
    command: 'a3s doctor',
    summary: {
      zh: '只读环境诊断已完成',
      en: 'The read-only environment diagnosis is complete',
    },
    output: [
      {
        label: { zh: '模式', en: 'mode' },
        value: { zh: '只读检查', en: 'read-only inspection' },
        tone: 'accent',
      },
      {
        label: { zh: '检查', en: 'check' },
        value: {
          zh: '平台 · 依赖 · 已安装组件',
          en: 'platform · dependencies · components',
        },
        tone: 'muted',
      },
      {
        label: { zh: '结果', en: 'result' },
        value: { zh: '可操作的诊断报告', en: 'actionable diagnostic report' },
        tone: 'success',
      },
    ],
  },
  {
    id: 'upgrade',
    label: 'UPGRADE',
    command: 'a3s upgrade',
    summary: {
      zh: '可用升级已列出，尚未修改系统',
      en: 'Available upgrades are listed without changing the system',
    },
    output: [
      {
        label: { zh: '模式', en: 'mode' },
        value: { zh: '无组件参数 · 只读', en: 'no component IDs · read-only' },
        tone: 'accent',
      },
      {
        label: { zh: '检查', en: 'check' },
        value: {
          zh: 'CLI 与已安装组件',
          en: 'CLI and installed components',
        },
        tone: 'muted',
      },
      {
        label: { zh: '结果', en: 'result' },
        value: { zh: '可用升级列表', en: 'available upgrade list' },
        tone: 'success',
      },
    ],
  },
];

export const cliTerminalInterfaceCopy = {
  zh: {
    region: 'A3S CLI 命令执行演示',
    pause: '暂停命令演示',
    play: '继续命令演示',
    replay: '重播',
    replayLabel: '重新播放当前命令',
    reduced: '系统已启用减弱动画',
    ready: '完成',
    running: '执行中',
    paused: '已暂停',
    scenario: '选择命令演示',
    progress: '演示进度',
  },
  en: {
    region: 'A3S CLI command playback',
    pause: 'Pause command playback',
    play: 'Resume command playback',
    replay: 'Replay',
    replayLabel: 'Replay the current command',
    reduced: 'Reduced motion is enabled',
    ready: 'DONE',
    running: 'RUNNING',
    paused: 'PAUSED',
    scenario: 'Select a command demonstration',
    progress: 'Playback progress',
  },
} as const;
