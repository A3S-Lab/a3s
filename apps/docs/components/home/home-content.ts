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
      power: 'Power',
      principles: 'Principles',
      language: '中文',
      menu: 'Open navigation',
    },
    hero: {
      eyebrow: 'OPEN AGENT TOOLING',
      lineOne: 'An AI operating system',
      lineTwo: 'ecosystem for AI Native',
      accent: 'organizations.',
      description:
        'A3S brings together 34 focused projects for coding, browser work, durable execution, data, evaluation, and infrastructure. Start locally, then add only the components the job needs.',
      primaryAction: 'Browse 34 projects',
    },
    signal: ['34 focused projects', '8 public project sites', '3 system layers', 'Status and source in one place'],
    principles: {
      eyebrow: 'SYSTEM RULES / 05',
      title: 'Five rules that keep the parts understandable.',
      description: 'They define where decisions are made, what survives a restart, and which component is responsible when work fails.',
      items: [
        { index: '01', title: 'Policy stays with the host', description: 'CLI, Bench, and Cloud choose the active models, tools, permissions, and workflows.' },
        { index: '02', title: 'Backends stay replaceable', description: 'Drivers, providers, stores, executors, and adapters meet through explicit interfaces.' },
        { index: '03', title: 'State survives restarts', description: 'Sessions, workflow runs, runtime units, and evaluation results are written beyond process memory.' },
        { index: '04', title: 'Requirements are named', description: 'Accounts, browsers, brokers, hypervisors, databases, and models are configured rather than assumed.' },
        { index: '05', title: 'Policy is not enforcement', description: 'The host decides what is allowed; the sandbox or infrastructure driver enforces that decision.' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: 'ECOSYSTEM DIRECTORY / 34 PROJECTS',
      title: 'Find the project that owns the job.',
      description: 'Search by project or filter by layer. Every entry links to its source and shows its current version or channel and delivery stage.',
    },
    quickstart: {
      eyebrow: 'QUICKSTART / LOCAL',
      title: 'Install A3S and open a workspace.',
      description: 'The CLI runs locally. Configure a supported model provider before starting Code.',
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
      title: 'Every AI Native organization needs to build its own AI operating system.',
      description: 'Each project has its own repository, release cadence, and operating limits. The directory shows where to begin.',
      primary: 'View source',
    },
    footer: {
      description: 'Open tools for agent products, local work, and infrastructure.',
      resources: 'Resources',
      community: 'Community',
      ecosystem: 'Project directory',
      quickstart: 'Quick start',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT licensed · Source available on GitHub',
    },
  },
  cn: {
    nav: {
      ecosystem: '生态',
      power: 'Power',
      principles: '原则',
      language: 'EN',
      menu: '打开导航',
    },
    hero: {
      eyebrow: '开放的 Agent 工具链',
      lineOne: '为AI Native组织',
      lineTwo: '构建的AI操作系统',
      accent: '生态',
      description:
        'A3S 包含 34 个职责明确的项目，覆盖编码、浏览器操作、持久执行、数据、评测与基础设施。先在本地开始，再按任务需要添加组件。',
      primaryAction: '浏览 34 个项目',
    },
    signal: ['34 个职责明确的项目', '8 个公开项目页面', '3 个系统层级', '状态与代码统一索引'],
    principles: {
      eyebrow: '系统规则 / 05',
      title: '让各个组件保持清晰的五条规则。',
      description: '这些规则说明由谁做决定、哪些状态需要跨重启保留，以及任务失败时该由哪个组件负责。',
      items: [
        { index: '01', title: '策略留在宿主', description: 'CLI、Bench 与 Cloud 选择启用哪些模型、工具、权限和工作流。' },
        { index: '02', title: '后端可以替换', description: '驱动、Provider、Store、Executor 与 Adapter 通过明确接口协作。' },
        { index: '03', title: '状态跨重启保留', description: 'Session、工作流、运行时单元和评测结果都会写入进程之外。' },
        { index: '04', title: '依赖明确写出', description: '账户、浏览器、Broker、Hypervisor、数据库和模型都需要配置，而不是默认存在。' },
        { index: '05', title: '策略不等于执行', description: '宿主决定允许什么，沙箱或基础设施驱动负责真正执行限制。' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '生态目录 / 34 个项目',
      title: '按职责找到对应项目。',
      description: '按项目搜索或按层级筛选。每个条目都提供代码入口、当前版本或通道和交付阶段。',
    },
    quickstart: {
      eyebrow: '快速开始 / 本地',
      title: '安装 A3S，打开一个工作区。',
      description: 'CLI 在本地运行。启动 Code 前，请先配置受支持的模型 Provider。',
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
      title: '每个AI Native组织都需要构建专属的AI操作系统',
      description: '每个项目都有独立仓库、发布节奏和使用边界。项目目录会告诉你该从哪里开始。',
      primary: '查看代码',
    },
    footer: {
      description: '面向 Agent 产品、本地工作与基础设施的开放工具。',
      resources: '资源',
      community: '社区',
      ecosystem: '项目目录',
      quickstart: '快速开始',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT 协议 · 代码公开在 GitHub',
    },
  },
} as const;
