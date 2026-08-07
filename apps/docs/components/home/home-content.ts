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
        'People and agents work in one Workspace. A3S OS syncs their tasks, context, and state between local machines, edge nodes, and the cloud.',
      primaryAction: 'View 36 projects',
      secondaryAction: 'Read the blog',
    },
    signal: ['Teams across time zones', 'People + Agent', 'One shared Workspace', 'Local and cloud stay in sync'],
    principles: {
      eyebrow: 'HOW WE BUILD / 05',
      title: 'Five rules we build by.',
      description: 'The same rules apply across all 36 projects.',
      items: [
        { index: '01', title: 'The host chooses policy', description: 'CLI, Web, Bench, and Cloud choose their own models, tools, permissions, and workflows.' },
        { index: '02', title: 'Interfaces can be replaced', description: 'Drivers, providers, stores, executors, and adapters connect through explicit interfaces.' },
        { index: '03', title: 'State can be restored', description: 'Sessions, workflow runs, runtime units, and evaluation results are stored outside the process.' },
        { index: '04', title: 'Dependencies are named', description: 'Accounts, browsers, brokers, hypervisors, databases, and models are configured directly.' },
        { index: '05', title: 'Decide, then enforce', description: 'Permission decisions stay separate from sandbox and infrastructure enforcement.' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '36 OPEN PROJECTS',
      title: 'Browse the A3S projects.',
      description: 'Open a site to see the product, or a repository to read the code. Filter the list by area and release stage.',
    },
    quickstart: {
      eyebrow: 'INSTALL A3S CLI',
      title: 'Install the CLI. Run Code.',
      description: 'Choose your platform, then run a3s code from your project directory.',
      copy: 'Copy install command',
      copied: 'Copied',
      docs: 'Read installation instructions',
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
      eyebrow: 'A3S OS',
      title: 'Every AI Native organization should have its own AI operating system.',
      description: 'A3S ships that operating system as open projects you can inspect and run yourself.',
      primary: 'Read the blog',
      secondary: 'Open the repository',
    },
    footer: {
      description: 'The ecosystem built for AI Native organizations.',
      resources: 'Resources',
      community: 'Community',
      ecosystem: 'Project directory',
      blog: 'Blog',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT licensed · Source available on GitHub',
    },
  },
  cn: {
    nav: {
      ecosystem: '生态',
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
        '成员和 Agent 在同一个 Workspace 里协作，任务、上下文和状态由 A3S OS 在本地、边缘与云端同步。',
      primaryAction: '查看 36 个项目',
      secondaryAction: '阅读博客',
    },
    signal: ['成员分布在不同时区', '成员与 Agent 协作', '共用一个 Workspace', '本地与云端同步'],
    principles: {
      eyebrow: '我们怎么做 / 05',
      title: '写代码时遵守的五条原则。',
      description: '36 个项目都按这五条原则开发。',
      items: [
        { index: '01', title: '策略留给宿主', description: 'CLI、Web、Bench 和 Cloud 各自选择模型、工具、权限和工作流。' },
        { index: '02', title: '接口随时可换', description: 'Driver、Provider、Store、Executor 和 Adapter 都通过明确接口连接。' },
        { index: '03', title: '状态可以恢复', description: 'Session、工作流、运行时单元和评测结果不会只留在进程内存里。' },
        { index: '04', title: '依赖写清楚', description: '账户、浏览器、Broker、Hypervisor、数据库和模型都要明确配置。' },
        { index: '05', title: '决策和执行分开', description: '权限判断不和沙箱或基础设施的具体执行混在一起。' },
      ] satisfies PrincipleCopy[],
    },
    ecosystem: {
      eyebrow: '36 个开源项目',
      title: 'A3S 的项目都在这里。',
      description: '打开网站看产品，打开仓库看代码，也可以按分类和开发阶段筛选。',
    },
    quickstart: {
      eyebrow: '安装 A3S CLI',
      title: '装好 CLI，运行 Code。',
      description: '选择你的系统，然后在项目目录运行 a3s code。',
      copy: '复制安装命令',
      copied: '已复制',
      docs: '查看安装说明',
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
      eyebrow: 'A3S OS',
      title: '每个AI Native的组织都应该有自己的AI操作系统',
      description: 'A3S 把这套操作系统拆成开放项目，代码可以检查，也可以自行部署。',
      primary: '阅读博客',
      secondary: '打开仓库',
    },
    footer: {
      description: '为 AI Native 组织构建的生态系统。',
      resources: '资源',
      community: '社区',
      ecosystem: '项目目录',
      blog: '博客',
      github: 'GitHub',
      discord: 'Discord',
      license: 'MIT 协议 · 代码公开在 GitHub',
    },
  },
} as const;
