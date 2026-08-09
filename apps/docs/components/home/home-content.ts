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
      architecture: 'Architecture',
      principles: 'Principles',
      blog: 'Blog',
      language: '中文',
      menu: 'Open navigation',
    },
    hero: {
      eyebrow: 'OPEN AGENT TOOLING',
      lineOne: 'An AI operating system',
      lineTwo: 'ecosystem for AI Native',
      accent: 'organizations.',
      description:
        'A3S brings together 35 focused projects for coding, browser work, durable execution, data, evaluation, and local infrastructure. Use the full CLI or only the pieces you need.',
      primaryAction: 'Browse 35 projects',
      secondaryAction: 'Read engineering notes',
      terminalTitle: 'a3s / project index',
      terminalReady: 'INDEX READY',
      terminalRows: [
        ['products.apps', '14'],
        ['runtime.data', '08'],
        ['services.interfaces', '13'],
        ['live.destinations', '07'],
      ],
      intent: 'PRODUCTS',
      policy: 'RUNTIME + DATA',
      runtime: 'SERVICES + UI',
      status: '35 projects',
    },
    signal: ['35 focused projects', '7 live project sites', '3 system layers', 'Status and source in one place'],
    architecture: {
      eyebrow: 'PROJECT RELATIONSHIPS / 35',
      title: 'See where each project fits.',
      description:
        'Choose a project to see what calls it, which contract it owns, what it runs, and which evidence it returns.',
    },
    principles: {
      eyebrow: 'SYSTEM RULES / 05',
      title: 'Five rules that keep the parts understandable.',
      description:
        'They define where decisions are made, what survives a restart, and which component is responsible when work fails.',
      items: [
        { index: '01', title: 'Policy stays with the host', description: 'CLI, Web, Bench, and Cloud choose the active models, tools, permissions, and workflows.' },
        { index: '02', title: 'Backends stay replaceable', description: 'Drivers, providers, stores, executors, and adapters meet through explicit interfaces.' },
        { index: '03', title: 'State survives restarts', description: 'Sessions, workflow runs, runtime units, and evaluation results are written beyond process memory.' },
        { index: '04', title: 'Requirements are named', description: 'Accounts, browsers, brokers, hypervisors, databases, and models are configured rather than assumed.' },
        { index: '05', title: 'Policy is not enforcement', description: 'The host decides what is allowed; the sandbox or infrastructure driver enforces that decision.' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: 'ECOSYSTEM DIRECTORY / 35 PROJECTS',
      title: 'Find the project that owns the job.',
      description:
        'Search by project or filter by layer. Every entry links to its source and shows its current release channel and delivery stage.',
    },
    quickstart: {
      eyebrow: 'QUICKSTART / LOCAL',
      title: 'Install A3S and open a workspace.',
      description: 'The CLI runs locally. Configure a supported model provider before starting Code.',
      copy: 'Copy install command',
      copied: 'Copied',
      docs: 'See all install options',
      note: 'macOS · glibc Linux · Windows installer available',
      command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: 'USE ONLY WHAT YOU NEED',
      title: 'Every AI Native organization needs to build its own AI operating system.',
      description: 'Each project has its own repository, release cadence, and operating limits. The directory shows where to begin.',
      primary: 'Read engineering notes',
      secondary: 'View source',
    },
    footer: {
      description: 'Open tools for agent products, local work, and infrastructure.',
      resources: 'Resources',
      community: 'Community',
      ecosystem: 'Project directory',
      architecture: 'Architecture',
      blog: 'Blog',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT licensed · Built in the open',
    },
  },
  cn: {
    nav: {
      ecosystem: '生态',
      architecture: '架构',
      principles: '原则',
      blog: '博客',
      language: 'EN',
      menu: '打开导航',
    },
    hero: {
      eyebrow: '开放的 Agent 工具链',
      lineOne: '为AI Native组织',
      lineTwo: '构建的AI操作系统',
      accent: '生态',
      description:
        'A3S 包含 35 个职责明确的项目，覆盖编码、浏览器操作、持久执行、数据、评测与本地基础设施。你可以使用完整 CLI，也可以只取需要的组件。',
      primaryAction: '浏览 35 个项目',
      secondaryAction: '阅读工程文章',
      terminalTitle: 'a3s / 项目索引',
      terminalReady: '索引就绪',
      terminalRows: [
        ['products.apps', '14'],
        ['runtime.data', '08'],
        ['services.interfaces', '13'],
        ['live.destinations', '07'],
      ],
      intent: '产品与应用',
      policy: '运行时与数据',
      runtime: '服务与界面',
      status: '35 个项目',
    },
    signal: ['35 个职责明确的项目', '7 个公开项目页面', '3 个系统层级', '状态与代码统一索引'],
    architecture: {
      eyebrow: '项目关系 / 35',
      title: '看清每个项目处在什么位置。',
      description: '选择一个项目，查看谁调用它、它负责哪份契约、运行什么，以及返回什么证据。',
    },
    principles: {
      eyebrow: '系统规则 / 05',
      title: '让各个组件保持清晰的五条规则。',
      description: '这些规则说明由谁做决定、哪些状态需要跨重启保留，以及任务失败时该由哪个组件负责。',
      items: [
        { index: '01', title: '策略留在宿主', description: 'CLI、Web、Bench 与 Cloud 选择启用哪些模型、工具、权限和工作流。' },
        { index: '02', title: '后端可以替换', description: '驱动、Provider、Store、Executor 与 Adapter 通过明确接口协作。' },
        { index: '03', title: '状态跨重启保留', description: 'Session、工作流运行、运行时单元和评测结果都会写入进程之外。' },
        { index: '04', title: '依赖明确写出', description: '账户、浏览器、Broker、Hypervisor、数据库和模型都需要配置，而不是默认存在。' },
        { index: '05', title: '策略不等于执行', description: '宿主决定允许什么，沙箱或基础设施驱动负责真正执行限制。' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '生态目录 / 35 个项目',
      title: '按职责找到对应项目。',
      description: '按项目搜索或按层级筛选。每个条目都提供代码入口、当前发布通道和交付阶段。',
    },
    quickstart: {
      eyebrow: '快速开始 / 本地',
      title: '安装 A3S，打开一个工作区。',
      description: 'CLI 在本地运行。启动 Code 前，请先配置受支持的模型 Provider。',
      copy: '复制安装命令',
      copied: '已复制',
      docs: '查看全部安装方式',
      note: 'macOS · glibc Linux · 提供 Windows 安装器',
      command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: '只用需要的部分',
      title: '每个AI Native组织都需要构建专属的AI操作系统',
      description: '每个项目都有独立仓库、发布节奏和使用边界。项目目录会告诉你该从哪里开始。',
      primary: '阅读工程文章',
      secondary: '查看代码',
    },
    footer: {
      description: '面向 Agent 产品、本地工作与基础设施的开放工具。',
      resources: '资源',
      community: '社区',
      ecosystem: '项目目录',
      architecture: '架构',
      blog: '博客',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT 协议 · 开放构建',
    },
  },
} as const;
