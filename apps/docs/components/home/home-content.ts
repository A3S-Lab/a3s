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
      eyebrow: 'Agent sessions and tools',
      description:
        'Run a coding agent in the terminal or embed the same Rust core from Rust, Node.js, Python, or Go.',
      command: 'a3s code',
      href: '/docs/code',
    },
    {
      id: 'web',
      index: '02',
      name: 'Web + Work',
      eyebrow: 'Tasks, files, and editors',
      description:
        'Use one local workspace for tasks, real files, Monaco, Git, knowledge bases, Office documents, and PDF.',
      command: 'a3s web',
      href: '/docs/cli/commands',
    },
    {
      id: 'research',
      index: '03',
      name: 'Research',
      eyebrow: 'Web research workflow',
      description:
        'Search the web, keep source records, and produce editable Markdown and HTML reports from Code.',
      command: 'a3s code research --web',
      href: '/tutorials/deep-research',
    },
    {
      id: 'use',
      index: '04',
      name: 'Use',
      eyebrow: 'Browser, OCR, and packages',
      description:
        'Install Browser and OCR routes, then add Office, Science, or another reviewed package without rebuilding Code.',
      command: 'a3s install use',
      href: 'https://github.com/A3S-Lab/a3s#typed-capabilities-and-components',
      external: true,
    },
    {
      id: 'box',
      index: '05',
      name: 'Box',
      eyebrow: 'OCI workloads in MicroVMs',
      description:
        'Run OCI images in a Linux MicroVM, or choose the shared-kernel Sandbox explicitly on supported hosts.',
      command: 'a3s box ps',
      href: '/docs/box',
    },
    {
      id: 'bench',
      index: '06',
      name: 'Bench',
      eyebrow: 'Task, Candidate, Judge',
      description:
        'Freeze a Task and Candidate, run the Task-owned Judge, and keep the exact inputs, result, and runtime records.',
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
      eyebrow: 'Agent 会话与工具',
      description:
        '在终端运行编程 Agent，也可以从 Rust、Node.js、Python 或 Go 嵌入同一个 Rust Core。',
      command: 'a3s code',
      href: '/docs/code',
    },
    {
      id: 'web',
      index: '02',
      name: 'Web + Work',
      eyebrow: '任务、文件与编辑器',
      description:
        '在一个本地工作台中处理任务、真实文件、Monaco、Git、知识库、Office 文档和 PDF。',
      command: 'a3s web',
      href: '/docs/cli/commands',
    },
    {
      id: 'research',
      index: '03',
      name: 'Research',
      eyebrow: '网页研究工作流',
      description:
        '用 Code 搜索网页、保存来源记录，并生成可继续编辑的 Markdown 和 HTML 报告。',
      command: 'a3s code research --web',
      href: '/tutorials/deep-research',
    },
    {
      id: 'use',
      index: '04',
      name: 'Use',
      eyebrow: 'Browser、OCR 与扩展包',
      description:
        '安装 Browser 和 OCR 路由，再按需加入 Office、Science 或其他经过检查的包。',
      command: 'a3s install use',
      href: 'https://github.com/A3S-Lab/a3s#typed-capabilities-and-components',
      external: true,
    },
    {
      id: 'box',
      index: '05',
      name: 'Box',
      eyebrow: '在 MicroVM 中运行 OCI',
      description:
        '在受支持的宿主机上用 Linux MicroVM 运行 OCI 镜像，也可以明确选择共享内核 Sandbox。',
      command: 'a3s box ps',
      href: '/docs/box',
    },
    {
      id: 'bench',
      index: '06',
      name: 'Bench',
      eyebrow: 'Task、Candidate、Judge',
      description:
        '冻结 Task 和 Candidate，运行 Task 自带的 Judge，并保存确切输入、结果和运行记录。',
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
      lineOne: 'Run Code locally.',
      lineTwo: 'Add Browser, Box, or Cloud',
      accent: 'when you need them.',
      description:
        'Install the A3S CLI and start Code in a project directory. Browser automation, OCR, Office editing, evaluation, MicroVMs, and a self-hosted control plane are separate projects you can add later.',
      primaryAction: 'Install A3S',
      secondaryAction: 'See every project',
      terminalTitle: 'a3s / local session',
      terminalReady: 'READY',
      terminalRows: [
        ['session.events', 'ordered'],
        ['workspace.path', 'bounded'],
        ['tool.approval', 'on_request'],
        ['state.persist', 'configured'],
      ],
      intent: 'a3s code',
      policy: 'AgentSession',
      runtime: 'Tool runtime',
      status: 'session running',
    },
    signal: [
      'Runs on your machine',
      'Rust core + four SDKs',
      'Browser, OCR, and Office packages',
      'Box and Cloud stay optional',
    ],
    products: {
      eyebrow: 'WHERE TO START / 01—06',
      title: 'Pick the part you need first.',
      description:
        'Code, Web, Use, Box, and Bench are separate programs. Install one, then add another only when your workflow calls for it.',
      action: 'View details',
      items: sharedProducts.en,
    },
    architecture: {
      eyebrow: '37 PROJECTS / SOURCE-BASED DIAGRAMS',
      title: 'See what each project is actually made of.',
      description:
        'Each diagram uses names and relationships taken from that project’s README, source tree, manifest, and architecture notes. Select a node to see its direct connections.',
    },
    principles: {
      eyebrow: 'OWNERSHIP / 05',
      title: 'Which project owns which job.',
      description:
        'These boundaries explain where configuration, execution, storage, and enforcement live in the repository.',
      items: [
        {
          index: '01',
          title: 'CLI owns the local entrypoint',
          description:
            'It parses commands, loads configuration, hosts the Code TUI, and starts independently installed components.',
        },
        {
          index: '02',
          title: 'Code owns agent sessions',
          description:
            'AgentSession orders events and runs; the host supplies models, tools, stores, permissions, and presentation.',
        },
        {
          index: '03',
          title: 'Use owns capability packages',
          description:
            'Built-in Browser and OCR routes sit beside receipt-backed Office, Science, and other external packages.',
        },
        {
          index: '04',
          title: 'Runtime owns unit lifecycle',
          description:
            'Tasks and Services use one apply, inspect, logs, exec, stop, and remove contract; providers create the resources.',
        },
        {
          index: '05',
          title: 'Observer and Sentry split signal from judgment',
          description:
            'Observer collects and enforces at the kernel boundary; Sentry evaluates events and produces deny actions.',
        },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: 'ALL 37 PROJECTS',
      title: 'Open any repository from the map.',
    },
    quickstart: {
      eyebrow: 'INSTALL / LOCAL',
      title: 'Install the CLI, then run Code in a project.',
      description:
        'The installer adds the a3s command. Code uses the model provider or compatible local account configured on your machine.',
      copy: 'Copy install command',
      copied: 'Copied',
      docs: 'Read installation options',
      note: 'macOS · glibc Linux · Windows installer available',
      command:
        "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: 'DOCUMENTATION AND SOURCE',
      title: 'Read the setup guide or inspect the code.',
      description:
        'Every project in the architecture map links to its documentation or repository.',
      primary: 'Open documentation',
      secondary: 'Browse GitHub',
    },
    footer: {
      description:
        'Local agent tools, runtimes, and infrastructure from A3S Lab.',
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
      lineOne: '先在本地运行。',
      lineTwo: '需要时，再接入',
      accent: '浏览器、文档、评测与云节点。',
      description:
        '安装 A3S CLI，在项目目录中启动 Code。浏览器自动化、OCR、Office 编辑、评测、MicroVM 和自托管控制平面都是独立项目，可以以后再装。',
      primaryAction: '安装 A3S',
      secondaryAction: '查看全部项目',
      terminalTitle: 'a3s / 本地会话',
      terminalReady: '就绪',
      terminalRows: [
        ['session.events', 'ordered'],
        ['workspace.path', 'bounded'],
        ['tool.approval', 'on_request'],
        ['state.persist', 'configured'],
      ],
      intent: 'a3s code',
      policy: 'AgentSession',
      runtime: '工具运行时',
      status: '会话运行中',
    },
    signal: [
      '直接运行在本机',
      'Rust Core + 四种 SDK',
      'Browser、OCR、Office 可选安装',
      'Box 与 Cloud 不强制依赖',
    ],
    products: {
      eyebrow: '从哪里开始 / 01—06',
      title: '先选现在需要的那一部分。',
      description:
        'Code、Web、Use、Box 和 Bench 是不同程序。先装一个，遇到具体需求时再加入其他项目。',
      action: '查看详情',
      items: sharedProducts.cn,
    },
    architecture: {
      eyebrow: '37 个项目 / 按源码绘制',
      title: '逐个查看每个项目到底由什么组成。',
      description:
        '节点和连线来自对应项目的 README、源码目录、manifest 与架构文档。选择节点可以查看它的直接连接。',
    },
    principles: {
      eyebrow: '职责归属 / 05',
      title: '每一项工作由哪个项目负责。',
      description:
        '下面列出配置、会话、扩展包、运行时和安全判断在仓库中的实际归属。',
      items: [
        {
          index: '01',
          title: 'CLI 负责本地入口',
          description:
            '它解析命令、读取配置、承载 Code TUI，并启动单独安装的组件。',
        },
        {
          index: '02',
          title: 'Code 负责 Agent 会话',
          description:
            'AgentSession 排序事件和运行；模型、工具、存储、权限与界面由宿主提供。',
        },
        {
          index: '03',
          title: 'Use 负责能力包',
          description:
            '内置 Browser、OCR 路由与基于 receipt 的 Office、Science 等扩展包使用同一入口。',
        },
        {
          index: '04',
          title: 'Runtime 负责执行单元生命周期',
          description:
            'Task 和 Service 使用同一套 apply、inspect、logs、exec、stop、remove 接口，Provider 负责创建资源。',
        },
        {
          index: '05',
          title: 'Observer 与 Sentry 分开采集和判断',
          description:
            'Observer 从内核采集并执行拦截；Sentry 评估事件并生成 deny action。',
        },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '全部 37 个项目',
      title: '架构图直达各个仓库。',
    },
    quickstart: {
      eyebrow: '安装 / 本地',
      title: '装好 CLI，在项目里运行 Code。',
      description:
        '安装器会加入 a3s 命令。Code 使用你在本机配置的模型 Provider 或兼容账户。',
      copy: '复制安装命令',
      copied: '已复制',
      docs: '查看完整安装选项',
      note: 'macOS · glibc Linux · 提供 Windows 安装器',
      command:
        "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: '文档与源码',
      title: '先看安装说明，也可以看源码。',
      description: '架构图中的每个项目都链接到对应文档或仓库。',
      primary: '打开文档',
      secondary: '浏览 GitHub',
    },
    footer: {
      description: 'A3S Lab 开发的本地 Agent 工具、运行时与基础设施。',
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
