import { cloudLifecycleContent } from "./cloud-lifecycle-content";

export type Lang = "en" | "cn";

export interface AiNativeStepCopy {
  id: string;
  index: string;
  title: string;
  summary: string;
  detail: string;
  path: readonly string[];
  projects: readonly string[];
}

export interface AiNativeReasonCopy {
  index: string;
  title: string;
  description: string;
}

export interface AiNativeCopy {
  eyebrow: string;
  title: string;
  description: string;
  interactionEyebrow: string;
  interactionTitle: string;
  interactionDescription: string;
  play: string;
  pause: string;
  stageProjects: string;
  openArchitecture: string;
  steps: readonly AiNativeStepCopy[];
  organizationTitle: string;
  organizationDescription: string;
  reasons: readonly AiNativeReasonCopy[];
}

export type CliTerminalLineTone =
  "default" | "muted" | "path" | "success" | "command";

export interface CliTerminalLineCopy {
  text: string;
  tone?: CliTerminalLineTone;
}

export interface CliTerminalCommandCopy {
  id: string;
  label: string;
  command: string;
  summary: string;
  output: readonly CliTerminalLineCopy[];
}

export interface CliTerminalCopy {
  ariaLabel: string;
  title: string;
  play: string;
  pause: string;
  typing: string;
  running: string;
  complete: string;
  paused: string;
  commands: readonly CliTerminalCommandCopy[];
}

export type CloudLifecycleSystemId =
  "code" | "os" | "cloud" | "runtime" | "gateway";

export type CloudLifecycleLineTone = "default" | "muted" | "accent" | "success";

export interface CloudLifecycleLineCopy {
  source: string;
  text: string;
  tone?: CloudLifecycleLineTone;
}

export interface CloudLifecycleStageCopy {
  id: string;
  index: string;
  phase: string;
  title: string;
  summary: string;
  result: string;
  prompt: string;
  command: string;
  lines: readonly CloudLifecycleLineCopy[];
  systems: readonly CloudLifecycleSystemId[];
}

export interface CloudLifecycleCopy {
  eyebrow: string;
  title: string;
  description: string;
  ariaLabel: string;
  terminalTitle: string;
  stageNavigation: string;
  systemPath: string;
  play: string;
  pause: string;
  typing: string;
  running: string;
  complete: string;
  paused: string;
  contract: readonly {
    label: string;
    value: string;
  }[];
  systems: readonly {
    id: CloudLifecycleSystemId;
    label: string;
    detail: string;
  }[];
  stages: readonly CloudLifecycleStageCopy[];
  verified: {
    label: string;
    value: string;
  };
  next: {
    label: string;
    value: string;
  };
}

export const lifecycleProjects = {
  foundation: ["homebrew", "docs", "cli", "acl", "common"],
  build: ["code", "tui", "web", "desktop", "gui", "webview"],
  capabilities: [
    "use",
    "browser",
    "search",
    "ocr",
    "parser",
    "office",
    "science",
    "memory",
  ],
  coordinate: ["flow", "event", "lane", "bench", "test", "windhole"],
  execute: ["runtime", "box", "oci-runtime", "power", "boot"],
  scale: ["gateway", "cloud", "orm"],
  govern: ["observer", "sentry", "updater"],
} as const;

export const homeContent = {
  en: {
    nav: {
      aiNative: "AI Native",
      cloudLifecycle: "Cloud lifecycle",
      architecture: "Architecture",
      docs: "Docs",
      blog: "Blog",
      language: "中文",
      menu: "Open navigation",
    },
    hero: {
      eyebrow: "OPEN SOURCE · AI NATIVE · LOCAL FIRST",
      lineOne: "A3S is",
      lineTwo: "the AI Native",
      accent: "operating system for agents.",
      description:
        "AI Native treats an agent as a unit that can be built, deployed, and operated. A3S gives its sessions, models, tools, state, workflows, execution, and permissions one interface—from a local machine to Box and Cloud.",
      primaryAction: "Install A3S CLI",
      secondaryAction: "Explore project architecture",
      terminal: {
        ariaLabel: "Interactive A3S CLI command demonstration",
        title: "a3s cli / ~/workspace",
        play: "Play commands",
        pause: "Pause commands",
        typing: "typing",
        running: "running",
        complete: "exit 0",
        paused: "paused",
        commands: [
          {
            id: "paths",
            label: "PATHS",
            command: "a3s config paths",
            summary: "Configuration, state, and workspace paths resolved",
            output: [
              {
                text: "config             ~/.config/a3s/config.acl",
                tone: "path",
              },
              {
                text: "workspace config   ./.a3s/config.acl",
                tone: "path",
              },
              {
                text: "state              ~/.local/state/a3s",
                tone: "path",
              },
              { text: "agent              ./.a3s/agents", tone: "path" },
              { text: "memory             ./.a3s/memory", tone: "path" },
            ],
          },
          {
            id: "model",
            label: "MODEL",
            command: "a3s model current",
            summary: "Effective model and its ACL source shown",
            output: [
              { text: "codex/gpt-5.2-codex", tone: "success" },
              {
                text: "config: ~/.config/a3s/config.acl",
                tone: "path",
              },
            ],
          },
          {
            id: "code",
            label: "CODE",
            command: 'a3s code exec "Check the API boundary"',
            summary: "One non-interactive Code session completed",
            output: [
              { text: "tool: rg", tone: "muted" },
              { text: "tool: read", tone: "muted" },
              { text: "Checked 12 API routes." },
              {
                text: "No boundary violations found.",
                tone: "success",
              },
            ],
          },
          {
            id: "web",
            label: "WEB",
            command: "a3s web -d",
            summary: "Managed Web instance started and verified",
            output: [
              {
                text: "A3S Web:       http://127.0.0.1:7331/",
                tone: "path",
              },
              {
                text: "A3S Code API:  http://127.0.0.1:7331/api/health",
                tone: "path",
              },
              { text: "Background PID: 84217", tone: "muted" },
              { text: "a3s web status", tone: "command" },
              { text: "running · pid 84217 · managed", tone: "success" },
            ],
          },
          {
            id: "doctor",
            label: "DOCTOR",
            command: "a3s doctor",
            summary: "Installed components passed their health checks",
            output: [
              {
                text: "ok code               Bundled / Ready",
                tone: "success",
              },
              {
                text: "ok box                Managed / Ready",
                tone: "success",
              },
              {
                text: "ok use                Managed / Ready",
                tone: "success",
              },
              {
                text: "ok use/browser        External / Ready",
                tone: "success",
              },
            ],
          },
        ],
      } satisfies CliTerminalCopy,
    },
    signal: [
      "Linux · macOS · Windows",
      "Agent sessions and durable state",
      "Tools, policy, and workflows",
      "Box and Cloud when needed",
    ],
    aiNative: {
      eyebrow: "AI NATIVE / HOW WORK RUNS",
      title: "AI Native is not a chat box added to old software.",
      description:
        "People set the goal, boundaries, and exceptions. Agents carry the work across tools, change real state, and leave a record that another person or process can inspect and continue.",
      interactionEyebrow: "CODE ↔ OPERATING SYSTEM",
      interactionTitle:
        "Build the agent in Code, then carry it through development, deployment, and operations.",
      interactionDescription:
        "The API grows with the job. Start with a local AgentSession; add durable state, placement, isolation, routing, and fleet control only when the agent needs them.",
      play: "Play lifecycle",
      pause: "Pause lifecycle",
      stageProjects: "Projects introduced at this stage",
      openArchitecture: "Open architecture",
      steps: [
        {
          id: "foundation",
          index: "01",
          title: "Set the boundary",
          summary: "Install · configure · contracts",
          detail:
            "Install the local entrypoint, read the system contracts, and describe the Agent in ACL. At this point A3S is still one local command with explicit model, tool, workspace, and permission boundaries.",
          path: ["Install", "ACL", "a3s CLI"],
          projects: lifecycleProjects.foundation,
        },
        {
          id: "build",
          index: "02",
          title: "Build the agent",
          summary: "Developer → A3S Code",
          detail:
            "A3S Code creates AgentSession, assembles context, calls the model, runs tools, and emits typed events. The same core can sit behind the terminal, Web browser workbench, a native desktop host, or an SDK.",
          path: ["Developer", "A3S Code", "AgentSession"],
          projects: lifecycleProjects.build,
        },
        {
          id: "capabilities",
          index: "03",
          title: "Add capabilities",
          summary: "Use · data · real tools",
          detail:
            "Use resolves typed capability packages without expanding Code itself. Add browser control, search, OCR, document parsing and editing, scientific tools, or durable memory only when the Agent needs them.",
          path: ["A3S Code", "A3S Use", "Capability packages"],
          projects: lifecycleProjects.capabilities,
        },
        {
          id: "coordinate",
          index: "04",
          title: "Orchestrate and verify",
          summary: "Workflow · queue · evidence",
          detail:
            "Flow records replay-safe workflows, Event carries facts, and Lane bounds concurrent work. Bench, Test, and Windhole turn tasks, candidates, drivers, judges, and run evidence into repeatable checks.",
          path: ["Flow / Lane", "Bench / Test", "Run evidence"],
          projects: lifecycleProjects.coordinate,
        },
        {
          id: "execute",
          index: "05",
          title: "Deploy and execute",
          summary: "Progressive API → A3S OS",
          detail:
            "Promote the same Agent definition through progressive APIs. Runtime owns the Task and Service contract, Box provides explicit isolation, Power serves models, and Boot hosts modular services.",
          path: ["Agent release", "Runtime", "Box / Power"],
          projects: lifecycleProjects.execute,
        },
        {
          id: "scale",
          index: "06",
          title: "Serve and scale",
          summary: "Gateway · Runtime · Cloud",
          detail:
            "Gateway accepts concurrent traffic and routes it to healthy revisions. Runtime executes independent units in parallel. Cloud stores desired state, schedules capacity, reconciles replicas, and coordinates scale-out or scale-in.",
          path: ["Gateway", "Runtime fan-out", "Cloud replicas"],
          projects: lifecycleProjects.scale,
        },
        {
          id: "govern",
          index: "07",
          title: "Operate and govern",
          summary: "Observe · decide · update",
          detail:
            "Observer records process, file, network, and model activity. Sentry evaluates those observations and can produce enforcement actions. Updater applies signed, health-gated changes across the running fleet.",
          path: ["Observer", "Sentry", "Update / rollback"],
          projects: lifecycleProjects.govern,
        },
      ],
      organizationTitle: "Why a new kind of organization needs this layer",
      organizationDescription:
        "Once agents can edit files, call services, and run jobs, sharing a model account is not an operating model. The organization needs common rules and durable records for work done by people and agents together.",
      reasons: [
        {
          index: "01",
          title: "Proven processes become reusable",
          description:
            "A proven process can be stored as an Agent, Skill, or Workflow, reviewed like code, and run again without rebuilding it from a chat transcript.",
        },
        {
          index: "02",
          title: "Everyone uses the same boundaries",
          description:
            "Workspace limits, tool permissions, approval points, and execution policy apply whether work starts in a terminal, Web, Desktop, or an API.",
        },
        {
          index: "03",
          title: "State survives a conversation",
          description:
            "Sessions, events, artifacts, memory, and workflow checkpoints make long-running work resumable and handoffs inspectable.",
        },
        {
          index: "04",
          title: "Execution grows without changing the job",
          description:
            "Keep ordinary work local, move risky workloads into Box, and use Cloud when a team needs remote machines or coordinated capacity.",
        },
      ],
    } satisfies AiNativeCopy,
    cloudLifecycle: cloudLifecycleContent.en,
    architecture: {
      eyebrow: "36 PROJECTS / INTERNAL ARCHITECTURE",
      title: "Inside every A3S project.",
      description:
        "Entrypoints, core modules, state, adapters, security boundaries, control flow, and data flow.",
    },
    quickstart: {
      eyebrow: "INSTALL / LOCAL",
      title: "Install the CLI, then run Code in a project.",
      description:
        "The installer adds the a3s command. Code uses the model provider or compatible local account configured on your machine.",
      copy: "Copy install command",
      copied: "Copied",
      docs: "Read installation options",
      note: "macOS · glibc Linux · Windows installer available",
      command:
        "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: "DOCUMENTATION AND SOURCE",
      title: "Read the setup guide or inspect the code.",
      description: "Start with setup, interfaces, or source.",
      primary: "Open documentation",
      secondary: "Browse GitHub",
    },
    footer: {
      description:
        "An open-source operating system for agents, built by A3S Lab.",
      resources: "Resources",
      community: "Community",
      docs: "Documentation",
      blog: "Blog",
      github: "GitHub",
      discord: "Discord",
      license: "MIT licensed · Built in the open",
    },
  },
  cn: {
    nav: {
      aiNative: "AI Native",
      cloudLifecycle: "云端生命周期",
      architecture: "架构",
      docs: "文档",
      blog: "博客",
      language: "EN",
      menu: "打开导航",
    },
    hero: {
      eyebrow: "开源 · AI NATIVE · 本地优先",
      lineOne: "A3S 是",
      lineTwo: "AI Native 的",
      accent: "智能体操作系统。",
      description:
        "AI Native 把智能体作为可开发、可部署、可运维的执行单元。A3S 统一它的会话、模型、工具、状态、工作流、运行环境和权限，从本机到 Box 与 Cloud 使用同一套接口。",
      primaryAction: "安装 A3S CLI",
      secondaryAction: "查看项目架构",
      terminal: {
        ariaLabel: "A3S CLI 命令交互演示",
        title: "a3s cli / ~/workspace",
        play: "播放命令",
        pause: "暂停命令",
        typing: "输入中",
        running: "执行中",
        complete: "退出码 0",
        paused: "已暂停",
        commands: [
          {
            id: "paths",
            label: "路径",
            command: "a3s config paths",
            summary: "已解析配置、状态与工作区路径",
            output: [
              {
                text: "config             ~/.config/a3s/config.acl",
                tone: "path",
              },
              {
                text: "workspace config   ./.a3s/config.acl",
                tone: "path",
              },
              {
                text: "state              ~/.local/state/a3s",
                tone: "path",
              },
              { text: "agent              ./.a3s/agents", tone: "path" },
              { text: "memory             ./.a3s/memory", tone: "path" },
            ],
          },
          {
            id: "model",
            label: "模型",
            command: "a3s model current",
            summary: "已显示当前模型与对应 ACL 配置",
            output: [
              { text: "codex/gpt-5.2-codex", tone: "success" },
              {
                text: "config: ~/.config/a3s/config.acl",
                tone: "path",
              },
            ],
          },
          {
            id: "code",
            label: "CODE",
            command: 'a3s code exec "Check the API boundary"',
            summary: "已完成一次非交互 Code 会话",
            output: [
              { text: "tool: rg", tone: "muted" },
              { text: "tool: read", tone: "muted" },
              { text: "Checked 12 API routes." },
              {
                text: "No boundary violations found.",
                tone: "success",
              },
            ],
          },
          {
            id: "web",
            label: "WEB",
            command: "a3s web -d",
            summary: "Web 后台实例已启动并通过状态检查",
            output: [
              {
                text: "A3S Web:       http://127.0.0.1:7331/",
                tone: "path",
              },
              {
                text: "A3S Code API:  http://127.0.0.1:7331/api/health",
                tone: "path",
              },
              { text: "Background PID: 84217", tone: "muted" },
              { text: "a3s web status", tone: "command" },
              { text: "running · pid 84217 · managed", tone: "success" },
            ],
          },
          {
            id: "doctor",
            label: "诊断",
            command: "a3s doctor",
            summary: "已安装组件全部通过健康检查",
            output: [
              {
                text: "ok code               Bundled / Ready",
                tone: "success",
              },
              {
                text: "ok box                Managed / Ready",
                tone: "success",
              },
              {
                text: "ok use                Managed / Ready",
                tone: "success",
              },
              {
                text: "ok use/browser        External / Ready",
                tone: "success",
              },
            ],
          },
        ],
      } satisfies CliTerminalCopy,
    },
    signal: [
      "Linux · macOS · Windows",
      "智能体会话与持久状态",
      "工具、权限与工作流",
      "Box、Cloud 按需接入",
    ],
    aiNative: {
      eyebrow: "AI NATIVE / 工作方式",
      title: "AI Native，不是给旧软件加一个聊天框。",
      description:
        "人给出目标、边界和例外，智能体在多个工具之间把工作做完，修改真实状态，并留下能被检查、接手和继续执行的记录。",
      interactionEyebrow: "CODE ↔ 操作系统",
      interactionTitle:
        "在 Code 中创建智能体，再沿同一套接口完成开发、部署和运维。",
      interactionDescription:
        "API 会随任务逐步展开：先用本地 AgentSession，需要时再加入持久状态、任务放置、隔离、流量入口和节点管理。",
      play: "播放生命周期",
      pause: "暂停生命周期",
      stageProjects: "这一阶段引入的项目",
      openArchitecture: "查看架构",
      steps: [
        {
          id: "foundation",
          index: "01",
          title: "定义边界",
          summary: "安装 · 配置 · 契约",
          detail:
            "先装好本地入口，读清系统契约，再用 ACL 描述 Agent。此时 A3S 仍只是一个本地命令，模型、工具、工作区和权限边界都明确写出。",
          path: ["Install", "ACL", "a3s CLI"],
          projects: lifecycleProjects.foundation,
        },
        {
          id: "build",
          index: "02",
          title: "开发智能体",
          summary: "开发者 → A3S Code",
          detail:
            "A3S Code 创建 AgentSession，组装上下文、调用模型、运行工具并发送类型化事件。同一个核心可以放在终端、Web 浏览器工作台、原生桌面宿主或 SDK 后面。",
          path: ["Developer", "A3S Code", "AgentSession"],
          projects: lifecycleProjects.build,
        },
        {
          id: "capabilities",
          index: "03",
          title: "加入能力",
          summary: "Use · 数据 · 真实工具",
          detail:
            "Use 解析类型化能力包，不必继续膨胀 Code。智能体需要时再加入浏览器、检索、OCR、文档解析与编辑、科研工具或持久记忆。",
          path: ["A3S Code", "A3S Use", "Capability packages"],
          projects: lifecycleProjects.capabilities,
        },
        {
          id: "coordinate",
          index: "04",
          title: "编排与验证",
          summary: "工作流 · 队列 · 证据",
          detail:
            "Flow 记录可重放工作流，Event 传递事实，Lane 限制并发工作。Bench、Test 与 Windhole 把 Task、Candidate、Driver、Judge 和运行证据变成可重复检查。",
          path: ["Flow / Lane", "Bench / Test", "Run evidence"],
          projects: lifecycleProjects.coordinate,
        },
        {
          id: "execute",
          index: "05",
          title: "部署与执行",
          summary: "渐进式 API → A3S OS",
          detail:
            "同一份 Agent 定义沿渐进式 API 进入系统。Runtime 负责 Task 与 Service 契约，Box 提供明确隔离，Power 提供模型服务，Boot 承载模块化服务。",
          path: ["Agent release", "Runtime", "Box / Power"],
          projects: lifecycleProjects.execute,
        },
        {
          id: "scale",
          index: "06",
          title: "流量与扩缩容",
          summary: "Gateway · Runtime · Cloud",
          detail:
            "Gateway 接收并发流量并路由到健康 revision；Runtime 并行执行独立单元；Cloud 保存期望状态、调度容量、收敛副本并协调扩容或缩容。",
          path: ["Gateway", "Runtime fan-out", "Cloud replicas"],
          projects: lifecycleProjects.scale,
        },
        {
          id: "govern",
          index: "07",
          title: "运维与治理",
          summary: "观察 · 判断 · 更新",
          detail:
            "Observer 记录进程、文件、网络与模型活动；Sentry 评估这些观察并生成可选的执行动作；Updater 对运行中的节点执行签名且带健康检查的更新。",
          path: ["Observer", "Sentry", "Update / rollback"],
          projects: lifecycleProjects.govern,
        },
      ],
      organizationTitle: "新型组织为什么需要这一层",
      organizationDescription:
        "当智能体开始改文件、调用服务和运行任务，共用一个模型账号并不能管理工作。人和智能体需要共用规则、状态和执行记录。",
      reasons: [
        {
          index: "01",
          title: "工作方法可以复用",
          description:
            "成熟流程可以保存成 Agent、Skill 或 Workflow，像代码一样评审和版本化，不必从聊天记录里重新拼装。",
        },
        {
          index: "02",
          title: "所有入口遵守同一套边界",
          description:
            "无论工作从终端、Web、Desktop 还是 API 开始，工作区限制、工具权限、审批点和执行策略保持一致。",
        },
        {
          index: "03",
          title: "状态不会随对话结束而消失",
          description:
            "Session、事件、Artifact、Memory 和 Workflow checkpoint 让长期任务能够恢复，也让交接有据可查。",
        },
        {
          index: "04",
          title: "执行规模可以逐步增加",
          description:
            "普通工作留在本机，风险任务放进 Box；团队需要远程机器或统一容量时，再接入 Cloud。",
        },
      ],
    } satisfies AiNativeCopy,
    cloudLifecycle: cloudLifecycleContent.cn,
    architecture: {
      eyebrow: "36 个项目 / 内部技术架构",
      title: "A3S 各项目的内部结构",
      description: "入口、核心模块、状态、适配器、安全边界、控制流与数据流。",
    },
    quickstart: {
      eyebrow: "安装 / 本地",
      title: "装好 CLI，在项目里运行 Code。",
      description:
        "安装器会加入 a3s 命令。Code 使用你在本机配置的模型 Provider 或兼容账户。",
      copy: "复制安装命令",
      copied: "已复制",
      docs: "查看完整安装选项",
      note: "macOS · glibc Linux · 提供 Windows 安装器",
      command:
        "curl --proto '=https' --tlsv1.2 -LsSf \\\n  https://raw.githubusercontent.com/A3S-Lab/a3s/main/install.sh | sh\n\ncd /path/to/project\na3s code",
    },
    cta: {
      eyebrow: "文档与源码",
      title: "先看安装说明，也可以看源码。",
      description: "安装、接口和源码都可以从这里开始。",
      primary: "打开文档",
      secondary: "浏览 GitHub",
    },
    footer: {
      description: "A3S Lab 开发的开源智能体操作系统。",
      resources: "资源",
      community: "社区",
      docs: "文档",
      blog: "博客",
      github: "GitHub",
      discord: "Discord",
      license: "MIT 协议 · 开放构建",
    },
  },
} as const;
