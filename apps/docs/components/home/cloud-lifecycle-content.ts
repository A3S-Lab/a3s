import type { CloudLifecycleCopy } from "./home-content";

export const cloudLifecycleContent = {
  en: {
    eyebrow: "A3S CODE / PROGRESSIVE API / A3S CLOUD",
    title: "The same Agent, from local development to a running service.",
    description:
      "A3S Code creates and checks the Agent locally, then discovers the A3S OS capability it needs and reads that operation's input contract before execution. Cloud commits deployment intent as desired state and a durable operation. Runtime and Box converge the Service; Gateway publishes only a healthy revision. Status, logs, and rollback continue from Code.",
    ariaLabel:
      "Interactive terminal showing A3S Code using the progressive API and A3S Cloud",
    terminalTitle: "a3s code / cloud lifecycle",
    stageNavigation: "Development, deployment, and operations stages",
    systemPath: "Active A3S execution path",
    play: "Play lifecycle",
    pause: "Pause lifecycle",
    typing: "typing",
    running: "running",
    complete: "stage complete",
    paused: "paused",
    contract: [
      {
        label: "SINGLE ENDPOINT",
        value: "POST /api/v1/kernel/capabilities",
      },
      {
        label: "PROGRESSION",
        value: "search → describe → execute",
      },
      {
        label: "DURABLE RETURN",
        value: "workloadId · revisionId · operationId",
      },
      {
        label: "RUNNING PATH",
        value: "Cloud · Flow · Runtime · Box · Gateway",
      },
    ],
    systems: [
      {
        id: "code",
        label: "A3S Code",
        detail: "Agent project and developer session",
      },
      {
        id: "os",
        label: "Progressive API",
        detail: "Permission-filtered capability contract",
      },
      {
        id: "cloud",
        label: "Cloud + Flow",
        detail: "Desired state and durable operation",
      },
      {
        id: "runtime",
        label: "Runtime + Box",
        detail: "Service lifecycle and isolation",
      },
      {
        id: "gateway",
        label: "Gateway",
        detail: "Healthy target and request path",
      },
    ],
    stages: [
      {
        id: "develop",
        index: "01",
        phase: "DEVELOP",
        title: "Build the Agent locally",
        summary: "Create a readable package with explicit tools and limits.",
        result: "The local Agent package is ready to publish.",
        prompt: "$",
        command: "a3s code",
        lines: [
          {
            source: "YOU",
            text: "Create an application agent for release checks.",
            tone: "accent",
          },
          {
            source: "CODE",
            text: "created agents/release-guard/agent.md",
          },
          {
            source: "CODE",
            text: "created prompts/system.md · workflows/operating-procedure.md",
          },
          {
            source: "CODE",
            text: "wrote .a3s/asset.acl · agentKind application",
          },
          {
            source: "CHECK",
            text: "definition · tools · success criteria / ready",
            tone: "success",
          },
        ],
        systems: ["code"],
      },
      {
        id: "discover",
        index: "02",
        phase: "DISCOVER",
        title: "Resolve one OS capability",
        summary:
          "Search broadly, then read the exact deploy operation contract.",
        result:
          "Code resolved the application-Agent build operation and its schema.",
        prompt: "›",
        command: "Find the A3S OS capability that deploys an application agent",
        lines: [
          {
            source: "CODE",
            text: "POST /api/v1/kernel/capabilities",
            tone: "accent",
          },
          {
            source: "CODE",
            text: '{"action":"search","query":"Agent as a Service application agent build launch deploy asset"}',
          },
          {
            source: "OS",
            text: "assets / AgentBuildController_triggerAgentBuild",
            tone: "success",
          },
          {
            source: "CODE",
            text: '{"action":"describe","module":"assets","operation":"AgentBuildController_triggerAgentBuild"}',
          },
          {
            source: "OS",
            text: "POST · TriggerAgentBuildRequestDto",
            tone: "success",
          },
        ],
        systems: ["code", "os"],
      },
      {
        id: "deploy",
        index: "03",
        phase: "DEPLOY",
        title: "Commit deployment intent",
        summary:
          "Execute the described operation and return durable Cloud identities.",
        result:
          "Cloud accepted the desired state without holding the terminal open.",
        prompt: "›",
        command:
          "Deploy release-guard through the A3S OS capability and keep the operation trace",
        lines: [
          {
            source: "CODE",
            text: "resolved assets / AgentBuildController_triggerAgentBuild",
            tone: "accent",
          },
          {
            source: "API",
            text: '{"action":"execute","module":"assets","operation":"AgentBuildController_triggerAgentBuild","shaped":true}',
          },
          {
            source: "CLOUD",
            text: "desired state committed in PostgreSQL",
          },
          {
            source: "CLOUD",
            text: "workloadId 8d53…9bc1 · revisionId 4f91…e821 · generation 18",
          },
          {
            source: "FLOW",
            text: "operationId c7a4…f103 · pending",
            tone: "success",
          },
        ],
        systems: ["code", "os", "cloud"],
      },
      {
        id: "converge",
        index: "04",
        phase: "CONVERGE",
        title: "Converge the Service",
        summary:
          "Flow resumes the operation while Runtime and Box apply the revision.",
        result: "Revision 18 became active only after Runtime health evidence.",
        prompt: "→",
        command: "watch operation c7a4…f103",
        lines: [
          {
            source: "FLOW",
            text: "pending → running",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "node command lease issued",
          },
          {
            source: "RUNTIME",
            text: "Service generation 18 applied",
          },
          {
            source: "BOX",
            text: "isolation microvm · health healthy",
            tone: "success",
          },
          {
            source: "CLOUD",
            text: "activeRevision generation 18",
          },
          {
            source: "FLOW",
            text: "operation succeeded",
            tone: "success",
          },
        ],
        systems: ["cloud", "runtime"],
      },
      {
        id: "serve",
        index: "05",
        phase: "SERVE",
        title: "Publish a healthy target",
        summary:
          "Read workload, Runtime health, and route state before traffic moves.",
        result:
          "Gateway is serving the healthy active revision through one route.",
        prompt: "›",
        command:
          "Check release-guard workload, Runtime health, and route state",
        lines: [
          {
            source: "CODE",
            text: "capability a3s_cloud_workloads_get",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "desired running · active revision 18",
          },
          {
            source: "RUNTIME",
            text: "state running · health healthy",
            tone: "success",
          },
          {
            source: "CODE",
            text: "capability a3s_cloud_routes_get",
            tone: "accent",
          },
          {
            source: "GATEWAY",
            text: "release.example.com/ · route active · revision 18",
            tone: "success",
          },
        ],
        systems: ["code", "os", "cloud", "runtime", "gateway"],
      },
      {
        id: "observe",
        index: "06",
        phase: "OPERATE",
        title: "Read bounded logs",
        summary:
          "Fetch an ordered page with an opaque cursor and explicit gaps.",
        result:
          "Code received one bounded log page and its next durable cursor.",
        prompt: "›",
        command: "Read the latest release-guard workload logs",
        lines: [
          {
            source: "CODE",
            text: "capability a3s_cloud_workload_logs_get",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "cursor v1:1842 · stream stdout",
          },
          {
            source: "RUNTIME",
            text: "12:41:08 request accepted",
          },
          {
            source: "RUNTIME",
            text: "12:41:08 release check completed in 384ms",
            tone: "success",
          },
          {
            source: "CLOUD",
            text: "nextCursor v1:1845 · gap records 0",
          },
        ],
        systems: ["code", "os", "cloud", "runtime"],
      },
      {
        id: "recover",
        index: "07",
        phase: "RECOVER",
        title: "Roll back without losing the trace",
        summary:
          "Clone a proven revision into a new generation with one replay-safe key.",
        result:
          "Rollback created a new durable operation and kept the healthy target until cutover.",
        prompt: "›",
        command: "Roll release-guard back to the last healthy revision",
        lines: [
          {
            source: "CODE",
            text: "describe cloud / a3s_cloud_workloads_rollback",
            tone: "accent",
          },
          {
            source: "OS",
            text: "params workloadId · sourceRevisionId · idempotencyKey",
          },
          {
            source: "CODE",
            text: "execute · shaped true",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "new generation cloned from a proven revision",
          },
          {
            source: "FLOW",
            text: "operation accepted · replay-safe",
            tone: "success",
          },
          {
            source: "GATEWAY",
            text: "healthy target retained until cutover",
            tone: "success",
          },
        ],
        systems: ["code", "os", "cloud", "runtime", "gateway"],
      },
    ],
    verified: {
      label: "VERIFIED CLOUD FOUNDATION",
      value:
        "H0.1 durable replica identity and H0.2 health-bound Gateway target projection.",
    },
    next: {
      label: "H0 DELIVERY",
      value:
        "H0.3 multi-node replicas and H0.5 measured autoscaling remain planned work.",
    },
  } satisfies CloudLifecycleCopy,
  cn: {
    eyebrow: "A3S CODE / 渐进式 API / A3S CLOUD",
    title: "同一个 Agent，从本地开发到云端运行。",
    description:
      "A3S Code 先在本地创建并检查 Agent，再按需从 A3S OS 发现能力，读取该操作的输入契约后执行。Cloud 把部署意图写成期望状态和可恢复的 operation；Runtime 与 Box 负责收敛 Service，Gateway 只发布通过健康检查的 revision。查看状态、日志和回滚仍从 Code 发起。",
    ariaLabel: "A3S Code 使用渐进式 API 与 A3S Cloud 的交互终端动画",
    terminalTitle: "a3s code / cloud lifecycle",
    stageNavigation: "开发、部署与运维阶段",
    systemPath: "当前经过的 A3S 运行链路",
    play: "播放生命周期",
    pause: "暂停生命周期",
    typing: "输入中",
    running: "执行中",
    complete: "本阶段完成",
    paused: "已暂停",
    contract: [
      {
        label: "单一端点",
        value: "POST /api/v1/kernel/capabilities",
      },
      {
        label: "逐步调用",
        value: "search → describe → execute",
      },
      {
        label: "持久返回",
        value: "workloadId · revisionId · operationId",
      },
      {
        label: "运行链路",
        value: "Cloud · Flow · Runtime · Box · Gateway",
      },
    ],
    systems: [
      {
        id: "code",
        label: "A3S Code",
        detail: "Agent 项目与开发会话",
      },
      {
        id: "os",
        label: "渐进式 API",
        detail: "按权限返回能力契约",
      },
      {
        id: "cloud",
        label: "Cloud + Flow",
        detail: "期望状态与持久 operation",
      },
      {
        id: "runtime",
        label: "Runtime + Box",
        detail: "Service 生命周期与隔离",
      },
      {
        id: "gateway",
        label: "Gateway",
        detail: "健康 target 与请求入口",
      },
    ],
    stages: [
      {
        id: "develop",
        index: "01",
        phase: "开发",
        title: "在本地完成 Agent",
        summary: "创建可读的项目，写清工具、流程和限制。",
        result: "本地 Agent 项目已具备发布条件。",
        prompt: "$",
        command: "a3s code",
        lines: [
          {
            source: "YOU",
            text: "创建一个用于版本发布检查的 application agent。",
            tone: "accent",
          },
          {
            source: "CODE",
            text: "created agents/release-guard/agent.md",
          },
          {
            source: "CODE",
            text: "created prompts/system.md · workflows/operating-procedure.md",
          },
          {
            source: "CODE",
            text: "wrote .a3s/asset.acl · agentKind application",
          },
          {
            source: "CHECK",
            text: "definition · tools · success criteria / ready",
            tone: "success",
          },
        ],
        systems: ["code"],
      },
      {
        id: "discover",
        index: "02",
        phase: "发现",
        title: "只展开需要的系统能力",
        summary: "先搜索，再读取部署操作的准确输入契约。",
        result: "Code 已找到 application Agent 的构建操作与输入 schema。",
        prompt: "›",
        command: "查找 A3S OS 中部署 application agent 的能力",
        lines: [
          {
            source: "CODE",
            text: "POST /api/v1/kernel/capabilities",
            tone: "accent",
          },
          {
            source: "CODE",
            text: '{"action":"search","query":"Agent as a Service application agent build launch deploy asset"}',
          },
          {
            source: "OS",
            text: "assets / AgentBuildController_triggerAgentBuild",
            tone: "success",
          },
          {
            source: "CODE",
            text: '{"action":"describe","module":"assets","operation":"AgentBuildController_triggerAgentBuild"}',
          },
          {
            source: "OS",
            text: "POST · TriggerAgentBuildRequestDto",
            tone: "success",
          },
        ],
        systems: ["code", "os"],
      },
      {
        id: "deploy",
        index: "03",
        phase: "部署",
        title: "提交部署意图",
        summary: "执行已确认的操作，拿到 Cloud 的持久标识。",
        result: "Cloud 已保存期望状态，终端无需等待整个部署过程。",
        prompt: "›",
        command:
          "通过 A3S OS 能力部署 release-guard，并保留 operation 追踪信息",
        lines: [
          {
            source: "CODE",
            text: "resolved assets / AgentBuildController_triggerAgentBuild",
            tone: "accent",
          },
          {
            source: "API",
            text: '{"action":"execute","module":"assets","operation":"AgentBuildController_triggerAgentBuild","shaped":true}',
          },
          {
            source: "CLOUD",
            text: "desired state committed in PostgreSQL",
          },
          {
            source: "CLOUD",
            text: "workloadId 8d53…9bc1 · revisionId 4f91…e821 · generation 18",
          },
          {
            source: "FLOW",
            text: "operationId c7a4…f103 · pending",
            tone: "success",
          },
        ],
        systems: ["code", "os", "cloud"],
      },
      {
        id: "converge",
        index: "04",
        phase: "收敛",
        title: "让 Service 收敛",
        summary: "Flow 继续 operation，Runtime 与 Box 应用目标 revision。",
        result: "Runtime 给出健康证据后，revision 18 才成为 active。",
        prompt: "→",
        command: "watch operation c7a4…f103",
        lines: [
          {
            source: "FLOW",
            text: "pending → running",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "node command lease issued",
          },
          {
            source: "RUNTIME",
            text: "Service generation 18 applied",
          },
          {
            source: "BOX",
            text: "isolation microvm · health healthy",
            tone: "success",
          },
          {
            source: "CLOUD",
            text: "activeRevision generation 18",
          },
          {
            source: "FLOW",
            text: "operation succeeded",
            tone: "success",
          },
        ],
        systems: ["cloud", "runtime"],
      },
      {
        id: "serve",
        index: "05",
        phase: "接入",
        title: "发布健康 target",
        summary: "流量切换前读取 Workload、Runtime 健康和 Route 状态。",
        result: "Gateway 已通过一条 Route 接入健康的 active revision。",
        prompt: "›",
        command: "检查 release-guard 的 Workload、Runtime 健康与 Route 状态",
        lines: [
          {
            source: "CODE",
            text: "capability a3s_cloud_workloads_get",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "desired running · active revision 18",
          },
          {
            source: "RUNTIME",
            text: "state running · health healthy",
            tone: "success",
          },
          {
            source: "CODE",
            text: "capability a3s_cloud_routes_get",
            tone: "accent",
          },
          {
            source: "GATEWAY",
            text: "release.example.com/ · route active · revision 18",
            tone: "success",
          },
        ],
        systems: ["code", "os", "cloud", "runtime", "gateway"],
      },
      {
        id: "observe",
        index: "06",
        phase: "运维",
        title: "读取有边界的日志",
        summary: "按 opaque cursor 读取有序日志页，缺口也会明确记录。",
        result: "Code 已拿到一页日志和下一次读取使用的持久 cursor。",
        prompt: "›",
        command: "读取 release-guard 最近的 Workload 日志",
        lines: [
          {
            source: "CODE",
            text: "capability a3s_cloud_workload_logs_get",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "cursor v1:1842 · stream stdout",
          },
          {
            source: "RUNTIME",
            text: "12:41:08 request accepted",
          },
          {
            source: "RUNTIME",
            text: "12:41:08 release check completed in 384ms",
            tone: "success",
          },
          {
            source: "CLOUD",
            text: "nextCursor v1:1845 · gap records 0",
          },
        ],
        systems: ["code", "os", "cloud", "runtime"],
      },
      {
        id: "recover",
        index: "07",
        phase: "恢复",
        title: "回滚也保留完整记录",
        summary: "用一个可重放的 key，把已验证 revision 克隆成新 generation。",
        result: "回滚已创建新的持久 operation，切换前继续保留健康 target。",
        prompt: "›",
        command: "把 release-guard 回滚到上一个健康 revision",
        lines: [
          {
            source: "CODE",
            text: "describe cloud / a3s_cloud_workloads_rollback",
            tone: "accent",
          },
          {
            source: "OS",
            text: "params workloadId · sourceRevisionId · idempotencyKey",
          },
          {
            source: "CODE",
            text: "execute · shaped true",
            tone: "accent",
          },
          {
            source: "CLOUD",
            text: "new generation cloned from a proven revision",
          },
          {
            source: "FLOW",
            text: "operation accepted · replay-safe",
            tone: "success",
          },
          {
            source: "GATEWAY",
            text: "healthy target retained until cutover",
            tone: "success",
          },
        ],
        systems: ["code", "os", "cloud", "runtime", "gateway"],
      },
    ],
    verified: {
      label: "已验证的 CLOUD 基线",
      value:
        "H0.1 持久副本身份，以及 H0.2 与健康状态绑定的 Gateway target 投影。",
    },
    next: {
      label: "H0 后续交付",
      value: "H0.3 多节点副本与 H0.5 指标驱动的自动扩缩容仍在计划中。",
    },
  } satisfies CloudLifecycleCopy,
} as const;
