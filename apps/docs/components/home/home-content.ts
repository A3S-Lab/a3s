export type Lang = 'en' | 'cn';

export type ProductId = 'code' | 'web' | 'research' | 'use' | 'box' | 'bench';

export interface ProductCopy {
  id: ProductId;
  index: string;
  name: string;
  eyebrow: string;
  description: string;
  command: string;
  href: string;
  external?: boolean;
}

export interface PrincipleCopy {
  index: string;
  title: string;
  description: string;
}

const sharedProducts = {
  en: [
    {
      id: 'code',
      index: '01',
      name: 'Code',
      eyebrow: 'Bundled agent runtime',
      description:
        'Governed coding sessions with explicit permissions, persistence, memory, delegation, verification, and dynamic workflows.',
      command: 'a3s code',
      href: '/docs/code',
    },
    {
      id: 'web',
      index: '02',
      name: 'Web + Work',
      eyebrow: 'Local browser workspace',
      description:
        'Tasks, Monaco editing, Git review, knowledge compilation, and native document work in one loopback-first surface.',
      command: 'a3s web',
      href: '/docs/cli/commands',
    },
    {
      id: 'research',
      index: '03',
      name: 'Research',
      eyebrow: 'Evidence-first synthesis',
      description:
        'Typed research runs with admitted sources, a bounded event journal, and editable Markdown and HTML artifacts.',
      command: 'a3s code research --web',
      href: '/tutorials/deep-research',
    },
    {
      id: 'use',
      index: '04',
      name: 'Use',
      eyebrow: 'Typed capability facade',
      description:
        'Browser and OCR routes plus an explicit lifecycle for external capabilities such as Office and scientific tooling.',
      command: 'a3s install use',
      href: 'https://github.com/A3S-Lab/a3s#typed-capabilities-and-components',
      external: true,
    },
    {
      id: 'box',
      index: '05',
      name: 'Box',
      eyebrow: 'MicroVM isolation',
      description:
        'Docker-like OCI workflows inside Linux MicroVMs on supported virtualization hosts, with isolation kept explicit.',
      command: 'a3s box ps',
      href: '/docs/box',
    },
    {
      id: 'bench',
      index: '06',
      name: 'Bench',
      eyebrow: 'Reproducible evaluation',
      description:
        'Identity-bound Task, Candidate, and Judge evaluation with visible validation and runtime evidence boundaries.',
      command: 'a3s install bench',
      href: 'https://github.com/A3S-Lab/Bench',
      external: true,
    },
  ] satisfies ProductCopy[],
  cn: [
    {
      id: 'code',
      index: '01',
      name: 'Code',
      eyebrow: '内置 Agent 运行时',
      description: '用清晰权限、持久会话、记忆、委派、验证和动态工作流，承载可治理的编程 Agent。',
      command: 'a3s code',
      href: '/cn/docs/code',
    },
    {
      id: 'web',
      index: '02',
      name: 'Web + Work',
      eyebrow: '本地浏览器工作台',
      description: '把任务、Monaco 编辑、Git 审查、知识编译与原生文档工作放进本地优先的统一界面。',
      command: 'a3s web',
      href: '/cn/docs/cli/commands',
    },
    {
      id: 'research',
      index: '03',
      name: 'Research',
      eyebrow: '证据优先的研究',
      description: '只接纳可验证来源，产出有界事件日志，以及可继续编辑的 Markdown 与 HTML 成果。',
      command: 'a3s code research --web',
      href: '/cn/tutorials/deep-research',
    },
    {
      id: 'use',
      index: '04',
      name: 'Use',
      eyebrow: '类型化能力门面',
      description: '统一 Browser、OCR 路由，并为 Office、科学工具等外部能力提供显式生命周期。',
      command: 'a3s install use',
      href: 'https://github.com/A3S-Lab/a3s#typed-capabilities-and-components',
      external: true,
    },
    {
      id: 'box',
      index: '05',
      name: 'Box',
      eyebrow: 'MicroVM 隔离',
      description: '在受支持的虚拟化宿主机上，用类 Docker 的 OCI 工作流运行 Linux MicroVM。',
      command: 'a3s box ps',
      href: '/cn/docs/box',
    },
    {
      id: 'bench',
      index: '06',
      name: 'Bench',
      eyebrow: '可复现评测',
      description: '将 Task、Candidate 与 Judge 绑定到同一身份链，明确呈现验证与运行时证据边界。',
      command: 'a3s install bench',
      href: 'https://github.com/A3S-Lab/Bench',
      external: true,
    },
  ] satisfies ProductCopy[],
};

export const homeContent = {
  en: {
    nav: {
      products: 'Products',
      architecture: 'Architecture',
      principles: 'Principles',
      docs: 'Docs',
      blog: 'Blog',
      language: '中文',
      menu: 'Open navigation',
    },
    hero: {
      eyebrow: 'OPEN SOURCE · RUST NATIVE · LOCAL FIRST',
      lineOne: 'One command.',
      lineTwo: 'Every agent boundary,',
      accent: 'explicit.',
      description:
        'A3S is a Rust-native platform for governed agents, local AI work, and composable infrastructure. Start with Code, then add capabilities, research, isolation, and services only when the workflow needs them.',
      primaryAction: 'Start building',
      secondaryAction: 'Explore the system',
      terminalTitle: 'a3s / control plane',
      terminalReady: 'SYSTEM READY',
      terminalRows: [
        ['host.policy', 'explicit'],
        ['session.state', 'durable'],
        ['runtime.driver', 'replaceable'],
        ['network.scope', 'loopback'],
      ],
      intent: 'USER INTENT',
      policy: 'HOST POLICY',
      runtime: 'RUNTIME',
      status: 'governed execution',
    },
    signal: ['One entry point', 'Explicit product boundaries', 'Composable Rust contracts', 'No hidden infrastructure'],
    products: {
      eyebrow: 'PRODUCT SURFACES / 01—06',
      title: 'Start small. Compose only what the work demands.',
      description:
        'A3S is not a mandatory vertical stack. Each surface owns a clear job and can remain independently useful.',
      action: 'Open guide',
      items: sharedProducts.en,
    },
    architecture: {
      eyebrow: 'SYSTEM ARCHITECTURE / LIVE MAP',
      title: 'Policy at the top. Replaceable machinery underneath.',
      description:
        'The product host decides what is active. Durable contracts carry identity and state. Concrete drivers do the infrastructure work.',
      layers: [
        { index: 'L0', label: 'Entrypoints', items: ['Terminal', 'Browser', 'Rust SDK', 'Node.js', 'Python'] },
        { index: 'L1', label: 'Product hosts', items: ['CLI', 'Code Web', 'Bench', 'Cloud', 'Services'] },
        { index: 'L2', label: 'Governed core', items: ['Code', 'Use', 'Flow', 'Event', 'Lane', 'Memory'] },
        { index: 'L3', label: 'Runtime contracts', items: ['Task', 'Service', 'Store', 'Provider', 'Driver'] },
        { index: 'L4', label: 'Execution', items: ['Process', 'Container', 'MicroVM', 'Remote'] },
      ],
      railTop: 'HOST OWNS POLICY',
      railBottom: 'DRIVER OWNS ENFORCEMENT',
    },
    principles: {
      eyebrow: 'DESIGN PRINCIPLES / 05',
      title: 'Autonomy without invisible assumptions.',
      description:
        'The hard operational boundaries are part of the product model—not footnotes discovered after deployment.',
      items: [
        { index: '01', title: 'Hosts own policy', description: 'CLI, Web, Bench, and Cloud decide which models, tools, permissions, and workflows are active.' },
        { index: '02', title: 'Contracts stay replaceable', description: 'Runtime drivers, providers, stores, executors, and adapters use explicit interfaces.' },
        { index: '03', title: 'Identity is durable', description: 'Sessions, workflow runs, runtime units, and evaluation results survive process boundaries.' },
        { index: '04', title: 'Dependencies stay visible', description: 'Accounts, browsers, brokers, hypervisors, databases, and models are never hidden defaults.' },
        { index: '05', title: 'Policy is not enforcement', description: 'Permission routing and concrete sandbox or infrastructure enforcement remain separate.' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: 'COMPOSABLE LAYER',
      title: 'A system of focused parts.',
      modules: ['Runtime', 'Flow', 'Event', 'Lane', 'Memory', 'ORM', 'Boot', 'Gateway', 'Power', 'ACL', 'AHP', 'Observer', 'Sentry', 'Search', 'Browser', 'OCR', 'Office'],
    },
    quickstart: {
      eyebrow: 'QUICKSTART / LOCAL',
      title: 'From zero to a governed agent.',
      description: 'Install the stable CLI, enter a workspace, and launch Code. Model-backed sessions use your configured provider or compatible local account.',
      copy: 'Copy install command',
      copied: 'Copied',
      docs: 'Read installation options',
      note: 'macOS · glibc Linux · Windows installer available',
      command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: 'READY WHEN YOU ARE',
      title: 'Give your agents a system worth trusting.',
      description: 'Open source, locally operable, and explicit all the way down.',
      primary: 'Read the docs',
      secondary: 'View on GitHub',
    },
    footer: {
      description: 'Governed agents, local AI work, and composable infrastructure.',
      resources: 'Resources',
      community: 'Community',
      docs: 'Documentation',
      tutorials: 'Tutorials',
      blog: 'Blog',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT licensed · Built in the open',
    },
  },
  cn: {
    nav: {
      products: '产品',
      architecture: '架构',
      principles: '原则',
      docs: '文档',
      blog: '博客',
      language: 'EN',
      menu: '打开导航',
    },
    hero: {
      eyebrow: '开源 · RUST 原生 · 本地优先',
      lineOne: '一个命令。',
      lineTwo: '每一道 Agent 边界，',
      accent: '都清晰可控。',
      description:
        'A3S 是面向可治理 Agent、本地 AI 工作与可组合基础设施的 Rust 原生平台。从 Code 开始，只在任务真正需要时加入能力、研究、隔离与服务。',
      primaryAction: '开始构建',
      secondaryAction: '探索系统',
      terminalTitle: 'a3s / 控制平面',
      terminalReady: '系统就绪',
      terminalRows: [
        ['host.policy', 'explicit'],
        ['session.state', 'durable'],
        ['runtime.driver', 'replaceable'],
        ['network.scope', 'loopback'],
      ],
      intent: '用户意图',
      policy: '宿主策略',
      runtime: '运行时',
      status: '受治理执行',
    },
    signal: ['一个统一入口', '清晰产品边界', '可组合 Rust 契约', '没有隐藏基础设施'],
    products: {
      eyebrow: '产品界面 / 01—06',
      title: '从最小闭环开始，只组合任务真正需要的部分。',
      description: 'A3S 不是强制垂直技术栈。每个产品界面职责清晰，也可以独立发挥价值。',
      action: '打开指南',
      items: sharedProducts.cn,
    },
    architecture: {
      eyebrow: '系统架构 / 实时拓扑',
      title: '策略在上，底层实现随时可替换。',
      description: '产品宿主决定启用什么，持久契约承载身份与状态，具体驱动负责基础设施执行。',
      layers: [
        { index: 'L0', label: '入口', items: ['终端', '浏览器', 'Rust SDK', 'Node.js', 'Python'] },
        { index: 'L1', label: '产品宿主', items: ['CLI', 'Code Web', 'Bench', 'Cloud', '服务'] },
        { index: 'L2', label: '治理核心', items: ['Code', 'Use', 'Flow', 'Event', 'Lane', 'Memory'] },
        { index: 'L3', label: '运行时契约', items: ['Task', 'Service', 'Store', 'Provider', 'Driver'] },
        { index: 'L4', label: '执行', items: ['进程', '容器', 'MicroVM', '远程 Provider'] },
      ],
      railTop: '宿主拥有策略',
      railBottom: '驱动负责执行',
    },
    principles: {
      eyebrow: '设计原则 / 05',
      title: '让 Agent 自主，但不允许隐形假设。',
      description: '困难的运行边界属于产品模型本身，而不是部署之后才被发现的脚注。',
      items: [
        { index: '01', title: '策略归宿主所有', description: 'CLI、Web、Bench 与 Cloud 决定启用哪些模型、工具、权限和工作流。' },
        { index: '02', title: '核心契约可替换', description: '运行时驱动、Provider、Store、Executor 与 Adapter 都通过明确接口协作。' },
        { index: '03', title: '身份与状态持久化', description: 'Session、工作流运行、运行时单元和评测结果不会只存在于进程内存。' },
        { index: '04', title: '外部依赖始终可见', description: '账户、浏览器、Broker、Hypervisor、数据库和模型从不被当成隐藏默认值。' },
        { index: '05', title: '策略不等于执行', description: '权限路由与具体沙箱或基础设施执行保持清晰分离。' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '可组合层',
      title: '由专注的部件，组成完整系统。',
      modules: ['Runtime', 'Flow', 'Event', 'Lane', 'Memory', 'ORM', 'Boot', 'Gateway', 'Power', 'ACL', 'AHP', 'Observer', 'Sentry', 'Search', 'Browser', 'OCR', 'Office'],
    },
    quickstart: {
      eyebrow: '快速开始 / 本地',
      title: '从零启动一个可治理 Agent。',
      description: '安装稳定版 CLI，进入工作区，然后启动 Code。模型会话使用你已配置的 Provider 或兼容的本地账户。',
      copy: '复制安装命令',
      copied: '已复制',
      docs: '查看完整安装选项',
      note: 'macOS · glibc Linux · 提供 Windows 安装器',
      command: "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: '一切就绪',
      title: '为你的 Agent，构建一套值得信任的系统。',
      description: '开源、本地可运行，并且每一层边界都清晰可见。',
      primary: '阅读文档',
      secondary: '前往 GitHub',
    },
    footer: {
      description: '可治理 Agent、本地 AI 工作与可组合基础设施。',
      resources: '资源',
      community: '社区',
      docs: '文档',
      tutorials: '教程',
      blog: '博客',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT 协议 · 开放构建',
    },
  },
} as const;
