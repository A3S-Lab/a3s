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
      eyebrow: 'A3S PURPOSE',
      lineOne: 'An ecosystem built',
      lineTwo: 'for AI Native',
      accent: 'organizations.',
      description:
        'Thirty-five projects cover agent products, runtimes, data, protocols, and infrastructure, with clear ownership, delivery status, sites, and repositories.',
      primaryAction: 'View 35 projects',
      secondaryAction: 'Read the blog',
      terminalTitle: 'a3s / ecosystem index',
      terminalReady: 'DIRECTORY LIVE',
      terminalRows: [
        ['products.apps', '14'],
        ['runtime.data', '08'],
        ['services.interfaces', '13'],
        ['live.destinations', '07'],
      ],
      intent: 'PRODUCTS',
      policy: 'RUNTIMES',
      runtime: 'INTERFACES',
      status: '35 projects',
    },
    signal: ['35 projects', '7 project sites', '3 architecture layers', 'One index for sites and code'],
    architecture: {
      eyebrow: 'PROJECT RELATIONSHIPS / 35',
      title: 'How the projects work together.',
      description:
        'Select a project to inspect its entrypoints, core responsibility, contracts, execution boundary, and evidence path.',
    },
    principles: {
      eyebrow: 'ENGINEERING CONSTRAINTS / 05',
      title: 'The constraints A3S follows.',
      description:
        'These rules determine where policy lives, how state is recovered, and how execution boundaries are verified.',
      items: [
        { index: '01', title: 'Hosts own policy', description: 'CLI, Web, Bench, and Cloud decide which models, tools, permissions, and workflows are active.' },
        { index: '02', title: 'Interfaces stay replaceable', description: 'Runtime drivers, providers, stores, executors, and adapters use explicit interfaces.' },
        { index: '03', title: 'State is recoverable', description: 'Sessions, workflow runs, runtime units, and evaluation results survive process boundaries.' },
        { index: '04', title: 'Dependencies are explicit', description: 'Accounts, browsers, brokers, hypervisors, databases, and models are configured directly.' },
        { index: '05', title: 'Decisions and enforcement differ', description: 'Permission routing and concrete sandbox or infrastructure enforcement remain separate.' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: 'ECOSYSTEM DIRECTORY / 35 PROJECTS',
      title: 'Projects, sites, and delivery stages.',
      description:
        'Open a project site, or filter the complete directory by layer, responsibility, and delivery stage.',
    },
    quickstart: {
      eyebrow: 'QUICKSTART / LOCAL',
      title: 'Install the CLI and start Code.',
      description: 'Install the released CLI, enter a workspace, and run Code with a configured model provider.',
      copy: 'Copy install command',
      copied: 'Copied',
      docs: 'Read installation instructions',
      note: 'macOS · glibc Linux · Windows installer available',
      command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: 'A3S OS',
      title: 'Every AI Native organization should have its own AI operating system.',
      description: 'A3S provides the open projects that make that operating system inspectable and self-hostable.',
      primary: 'Read the blog',
      secondary: 'Open the repository',
    },
    footer: {
      description: 'The ecosystem built for AI Native organizations.',
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
      eyebrow: 'A3S 的宗旨',
      lineOne: '为 AI Native 组织',
      lineTwo: '构建的',
      accent: '生态系统。',
      description:
        '35 个项目覆盖 Agent 产品、运行时、数据、协议和基础设施。每个项目都有明确职责、开发阶段、网站和代码入口。',
      primaryAction: '查看 35 个项目',
      secondaryAction: '阅读博客',
      terminalTitle: 'a3s / 生态索引',
      terminalReady: '目录在线',
      terminalRows: [
        ['products.apps', '14'],
        ['runtime.data', '08'],
        ['services.interfaces', '13'],
        ['live.destinations', '07'],
      ],
      intent: '产品与应用',
      policy: '运行与数据',
      runtime: '服务与接口',
      status: '35 个项目',
    },
    signal: ['35 个项目', '7 个项目网站', '3 个架构层', '网站与代码统一索引'],
    architecture: {
      eyebrow: '项目关系 / 35',
      title: '项目之间如何协作。',
      description: '选择一个项目，查看它的入口、核心职责、契约、执行边界和证据路径。',
    },
    principles: {
      eyebrow: '工程约束 / 05',
      title: 'A3S 遵循的工程约束。',
      description: '这些约束决定策略放在哪里、状态如何恢复，以及执行边界如何验证。',
      items: [
        { index: '01', title: '策略由宿主定义', description: 'CLI、Web、Bench 与 Cloud 决定启用哪些模型、工具、权限和工作流。' },
        { index: '02', title: '接口可以替换', description: '运行时驱动、Provider、Store、Executor 与 Adapter 通过明确接口协作。' },
        { index: '03', title: '状态可以恢复', description: 'Session、工作流运行、运行时单元和评测结果不只存在于进程内存。' },
        { index: '04', title: '依赖显式配置', description: '账户、浏览器、Broker、Hypervisor、数据库和模型都需要明确配置。' },
        { index: '05', title: '决策与执行分离', description: '权限路由与具体沙箱或基础设施执行保持分离。' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '生态目录 / 35 个项目',
      title: '项目、网站和开发阶段。',
      description: '打开项目网站，或按层级、职责和开发阶段筛选完整目录。',
    },
    quickstart: {
      eyebrow: '快速开始 / 本地',
      title: '安装 CLI，启动 Code。',
      description: '安装已发布的 CLI，进入工作区，并使用配置好的模型 Provider 运行 Code。',
      copy: '复制安装命令',
      copied: '已复制',
      docs: '查看安装说明',
      note: 'macOS · glibc Linux · 提供 Windows 安装器',
      command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: 'A3S OS',
      title: '每个AI Native的组织都应该有自己的AI操作系统',
      description: 'A3S 提供构建这套操作系统所需的开源项目，并保留可检查、自托管的实现边界。',
      primary: '阅读博客',
      secondary: '打开仓库',
    },
    footer: {
      description: '为 AI Native 组织构建的生态系统。',
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
