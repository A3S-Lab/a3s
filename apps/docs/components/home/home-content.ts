export type Lang = 'en' | 'cn';

export interface PrincipleCopy {
  index: string;
  title: string;
  description: string;
}

export const homeContent = {
  en: {
    nav: {
      ecosystem: 'Ecosystem',
      intelligence: 'Intelligence',
      principles: 'Principles',
      download: 'Desktop',
      language: '中文',
      menu: 'Open navigation',
      primaryNavigation: 'Primary navigation',
      homeLabel: 'A3S home',
      githubLabel: 'A3S on GitHub',
    },
    hero: {
      eyebrow: 'A3S OS / COLLECTIVE INTELLIGENCE',
      lineOne: 'An AI-native operating',
      lineTwo: 'system for fluid',
      accent: 'and crystal intelligence.',
      description:
        'A3S OS connects sessions, models, tools, memory, workflows, isolated execution, and Cloud operations through explicit contracts. Fluid intelligence helps teams form new solutions in context; crystal intelligence turns verified judgment into reusable organizational capability.',
      primaryAction: 'See the intelligence loop',
    },
    signal: ['Fluid → crystal intelligence', '35 focused projects', 'Local-first / progressive', 'Context travels with contribution'],
    vision: {
      eyebrow: 'FLUID → CRYSTAL / ORGANIZATIONAL INTELLIGENCE',
      title: 'Let fluid intelligence become a shared operating memory.',
      description:
        'Fluid intelligence helps a team combine people, agents, data, tools, and compute around a new problem. Crystal intelligence preserves the judgment that worked, with its context, evidence, and ownership, so the next team can call it again.',
      fluid: {
        label: '01 / FLUID INTELLIGENCE',
        title: 'Discover in context.',
        description:
          'Start from the task and its constraints. Compose the right people, agents, tools, data, and compute, then test a new path inside explicit policy boundaries.',
        steps: 'Sense · Compose · Test · Return',
      },
      crystal: {
        label: '02 / CRYSTAL INTELLIGENCE',
        title: 'Make good judgment reusable.',
        description:
          'Keep the context and evidence attached to a decision. Turn what worked into searchable memory, a capability, or a durable workflow with version, ownership, and a clear way to reuse it.',
        steps: 'Context · Evidence · Version · Reuse',
      },
      enterprise: {
        label: 'WHY A3S OS',
        title: 'Build the crystal intelligence brain your team can reuse.',
        description:
          'The advantage of an enterprise lives in expert context, trade-offs, and tacit workflows. A3S OS gives every contribution a safe path from local work to shared organizational memory while keeping authority and responsibility explicit.',
        points: ['Context stays attached', 'Evidence gates reuse', 'Permission and ownership stay clear'],
      },
    },
    principles: {
      eyebrow: 'SYSTEM RULES / 05',
      title: 'Five rules that let fluid intelligence crystallize safely.',
      description: 'They define where decisions are made, what survives a restart, and which component is responsible when work fails or a contribution is reused.',
      items: [
        { index: '01', title: 'Policy stays with the host', description: 'CLI, Bench, and Cloud choose the active models, tools, permissions, and workflows.' },
        { index: '02', title: 'Backends stay replaceable', description: 'Drivers, providers, stores, executors, and adapters meet through explicit interfaces.' },
        { index: '03', title: 'State survives restarts', description: 'Sessions, workflow runs, runtime units, and evaluation results are written beyond process memory.' },
        { index: '04', title: 'Requirements are named', description: 'Accounts, browsers, brokers, hypervisors, databases, and models are configured rather than assumed.' },
        { index: '05', title: 'Policy is not enforcement', description: 'The host decides what is allowed; the sandbox or infrastructure driver enforces that decision.' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: 'ECOSYSTEM DIRECTORY / 35 PROJECTS',
      title: 'Build the layer your work needs.',
      description: 'A3S OS keeps each capability in an owned project with a clear contract. Search by responsibility or filter by layer, then continue to its source, current version, and delivery stage.',
    },
    quickstart: {
      eyebrow: 'QUICKSTART / LOCAL-FIRST',
      title: 'Start with one local Code session.',
      description: 'A3S OS starts locally. Add memory, capabilities, orchestration, isolation, inference, or Cloud only when the work needs them.',
      copy: 'Copy install command',
      copied: 'Copied',
      docs: 'See all install options',
      note: 'macOS · Linux · Windows',
      installers: [
        {
          id: 'unix',
          label: 'macOS / Linux',
          shell: 'zsh / bash',
          command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
        },
        {
          id: 'windows',
          label: 'Windows',
          shell: 'PowerShell 5.1+',
          command: 'irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex\n\nSet-Location C:\\path\\to\\project\na3s code',
        },
        {
          id: 'homebrew',
          label: 'Homebrew',
          shell: 'macOS / Linux',
          command: 'brew install a3s-lab/tap/a3s\n\ncd /path/to/project\na3s code',
        },
      ],
    },
    cta: {
      eyebrow: 'USE ONLY WHAT YOU NEED',
      title: 'Every AI-native organization needs to build its own crystal intelligence brain.',
      description: 'A3S OS keeps authority boundaries explicit while turning verified contributions into context-rich, reusable team capability.',
      primary: 'View source',
    },
    footer: {
      description: 'Open tools for agent products, local work, and infrastructure.',
      resources: 'Resources',
      community: 'Community',
      ecosystem: 'Project directory',
      quickstart: 'Quick start',
      flow: 'A3S Flow',
      download: 'Desktop download',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT licensed · Source available on GitHub',
    },
  },
  cn: {
    nav: {
      ecosystem: '生态',
      intelligence: '智力循环',
      principles: '原则',
      download: '桌面端',
      language: 'EN',
      menu: '打开导航',
      primaryNavigation: '主导航',
      homeLabel: 'A3S 首页',
      githubLabel: 'A3S GitHub',
    },
    hero: {
      eyebrow: 'A3S OS / 集体智力',
      lineOne: '为 Agent 时代',
      lineTwo: '构建 AI 原生操作系统',
      accent: '让贡献沉淀为晶体智力',
      description:
        'A3S OS 把会话、模型、工具、记忆、工作流、隔离执行和 Cloud 控制平面连接成明确契约。流体智力帮助团队在新问题中即时组合资源，晶体智力让经过验证的判断成为可复用的组织能力。',
      primaryAction: '理解智力循环',
    },
    signal: ['流体智力 → 晶体智力', '35 个职责明确的项目', '本地优先 / 可渐进部署', '贡献带着上下文流动'],
    vision: {
      eyebrow: '流体智力 → 晶体智力 / 组织能力',
      title: '让团队的即时判断，成为可复用的组织记忆。',
      description:
        '流体智力帮助团队围绕新问题即时组合人、Agent、数据、工具和算力。晶体智力把有效判断连同上下文、证据和归属保存下来，让下一次工作可以直接调用。',
      fluid: {
        label: '01 / 流体智力',
        title: '在上下文中发现新解。',
        description:
          '从任务目标和约束出发，动态组合成员、Agent、工具、数据与算力，在清晰的策略边界内试错并快速形成判断。',
        steps: '感知 · 组合 · 试错 · 回流',
      },
      crystal: {
        label: '02 / 晶体智力',
        title: '把有效判断变成共享能力。',
        description:
          '让结论带着上下文和证据被记录，沉淀为可检索的记忆、可组合的能力或可恢复的工作流，并保留版本、归属和复用路径。',
        steps: '上下文 · 证据 · 版本 · 复用',
      },
      enterprise: {
        label: '为什么需要 A3S OS',
        title: '把团队贡献建设成持续生长的晶体智力大脑。',
        description:
          '企业真正独有的是专家判断中的上下文、取舍和隐性流程。A3S OS 为每次贡献提供从本地工作到组织记忆的安全路径，同时保留清晰的权限、责任和归属。',
        points: ['上下文随贡献保留', '证据决定是否复用', '权限与归属边界清晰'],
      },
    },
    principles: {
      eyebrow: '系统规则 / 05',
      title: '让流体智力安全结晶的五条系统规则。',
      description: '这些规则说明由谁做决定、哪些状态需要跨重启保留，以及任务失败或贡献复用时该由哪个组件负责。',
      items: [
        { index: '01', title: '策略留在宿主', description: 'CLI、Bench 与 Cloud 选择启用哪些模型、工具、权限和工作流。' },
        { index: '02', title: '后端可以替换', description: '驱动、Provider、Store、Executor 与 Adapter 通过明确接口协作。' },
        { index: '03', title: '状态跨重启保留', description: 'Session、工作流、运行时单元和评测结果都会写入进程之外。' },
        { index: '04', title: '依赖明确写出', description: '账户、浏览器、Broker、Hypervisor、数据库和模型都需要配置，而不是默认存在。' },
        { index: '05', title: '策略不等于执行', description: '宿主决定允许什么，沙箱或基础设施驱动负责真正执行限制。' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '生态目录 / 35 个项目',
      title: '按工作需要建设对应层级。',
      description: 'A3S OS 让每项能力都归属于一个有明确契约的项目。按职责搜索或按层级筛选，再进入代码、当前版本和交付阶段。',
    },
    quickstart: {
      eyebrow: '快速开始 / 本地优先',
      title: '从一个本地 Code 会话开始。',
      description: 'A3S OS 先在本地运行，再按任务需要加入记忆、能力编排、隔离执行、推理服务或 Cloud。',
      copy: '复制安装命令',
      copied: '已复制',
      docs: '查看全部安装方式',
      note: '支持 macOS、Linux 和 Windows',
      installers: [
        {
          id: 'unix',
          label: 'macOS / Linux',
          shell: 'zsh / bash',
          command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
        },
        {
          id: 'windows',
          label: 'Windows',
          shell: 'PowerShell 5.1+',
          command: 'irm https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.ps1 | iex\n\nSet-Location C:\\path\\to\\project\na3s code',
        },
        {
          id: 'homebrew',
          label: 'Homebrew',
          shell: 'macOS / Linux',
          command: 'brew install a3s-lab/tap/a3s\n\ncd /path/to/project\na3s code',
        },
      ],
    },
    cta: {
      eyebrow: '只用需要的部分',
      title: '每个 AI 原生组织都需要构建自己的晶体智力大脑',
      description: 'A3S OS 保留清晰的权限边界，把经过验证的贡献沉淀为带上下文、可复用的团队能力。',
      primary: '查看代码',
    },
    footer: {
      description: '面向 Agent 产品、本地工作与基础设施的开放工具。',
      resources: '资源',
      community: '社区',
      ecosystem: '项目目录',
      quickstart: '快速开始',
      flow: 'A3S Flow',
      download: '下载桌面端',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT 协议 · 代码公开在 GitHub',
    },
  },
} as const;
