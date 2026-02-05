# A3S Code Agent - 使用示例

本文档提供 A3S Code Agent 各项功能的详细使用示例。

## 目录

1. [会话存储配置](#1-会话存储配置)
2. [HITL (Human-in-the-Loop) 确认](#2-hitl-human-in-the-loop-确认)
3. [外部任务处理](#3-外部任务处理)
4. [Provider 配置](#4-provider-配置)
5. [Todo/任务跟踪](#5-todo任务跟踪)
6. [上下文管理](#6-上下文管理)
7. [权限系统](#7-权限系统)

---

## 1. 会话存储配置

每个会话可以独立配置存储类型（内存或文件）。

### TypeScript SDK 示例

```typescript
import { CodeAgentClient, StorageType } from '@a3s/sdk';

const client = new CodeAgentClient('localhost:4088');

// 创建使用内存存储的临时会话（不持久化）
const tempSession = await client.createSession({
  name: "Temporary Analysis",
  workspace: "/tmp/workspace",
  storageType: StorageType.STORAGE_TYPE_MEMORY,
  systemPrompt: "You are a code analyzer."
});

// 创建使用文件存储的持久化会话
const persistentSession = await client.createSession({
  name: "Long-term Project",
  workspace: "/home/user/project",
  storageType: StorageType.STORAGE_TYPE_FILE,
  systemPrompt: "You are a helpful coding assistant."
});

// 会话会自动保存到 {workspace}/sessions/ 目录
// 重启 agent 后可以恢复
```

### Python SDK 示例

```python
from a3s_sdk import CodeAgentClient, StorageType

client = CodeAgentClient('localhost:4088')

# 内存存储会话
temp_session = client.create_session(
    name="Temporary Session",
    workspace="/tmp/workspace",
    storage_type=StorageType.STORAGE_TYPE_MEMORY
)

# 文件存储会话
persistent_session = client.create_session(
    name="Persistent Session",
    workspace="/home/user/project",
    storage_type=StorageType.STORAGE_TYPE_FILE
)
```

### CLI 配置

```bash
# 全局配置：所有会话默认使用内存存储
./a3s-code --storage-backend memory

# 全局配置：使用文件存储，指定自定义目录
./a3s-code --storage-backend file --sessions-dir /var/lib/a3s/sessions

# 通过环境变量配置
export A3S_STORAGE_BACKEND=file
export A3S_SESSIONS_DIR=/custom/path
./a3s-code
```

---

## 2. HITL (Human-in-the-Loop) 确认

HITL 系统允许在执行敏感工具前请求用户确认。

### 基本配置

```typescript
import { TimeoutAction, SessionLane } from '@a3s/sdk';

// 配置 HITL 策略
await client.setConfirmationPolicy(sessionId, {
  enabled: true,

  // 自动批准的工具（无需确认）
  autoApproveTools: ['Read', 'Glob', 'Grep'],

  // 需要确认的工具
  requireConfirmTools: ['Bash', 'Write', 'Edit'],

  // 默认超时时间（30秒）
  defaultTimeoutMs: 30000,

  // 超时后的行为：拒绝或自动批准
  timeoutAction: TimeoutAction.TIMEOUT_ACTION_REJECT,

  // YOLO 模式：这些 lane 中的任务自动批准
  yoloLanes: [SessionLane.SESSION_LANE_QUERY]
});
```

### 处理确认请求

```typescript
// 订阅事件流
const eventStream = client.subscribeEvents(sessionId, [
  'ToolExecutionPending'
]);

for await (const event of eventStream) {
  if (event.type === 'ToolExecutionPending') {
    const { toolName, args, confirmationId } = event.data;

    // 显示给用户
    console.log(`Tool: ${toolName}`);
    console.log(`Args: ${JSON.stringify(args, null, 2)}`);

    // 获取用户决策
    const approved = await askUser(`Approve ${toolName}?`);

    // 发送确认
    await client.confirmToolExecution(sessionId, confirmationId, {
      approved,
      reason: approved ? undefined : "User rejected"
    });
  }
}
```

### 高级场景：条件批准

```typescript
// 根据工具参数动态决策
const eventStream = client.subscribeEvents(sessionId);

for await (const event of eventStream) {
  if (event.type === 'ToolExecutionPending') {
    const { toolName, args, confirmationId } = event.data;

    let approved = false;

    // 自动批准安全的 bash 命令
    if (toolName === 'Bash') {
      const command = args.command;
      const safeCommands = ['ls', 'pwd', 'echo', 'cat'];
      approved = safeCommands.some(cmd => command.startsWith(cmd));
    }

    // 自动批准读取操作
    if (toolName === 'Read' || toolName === 'Glob') {
      approved = true;
    }

    // 其他情况询问用户
    if (!approved) {
      approved = await askUser(`Approve ${toolName}?`);
    }

    await client.confirmToolExecution(sessionId, confirmationId, {
      approved,
      reason: approved ? "Auto-approved" : "Rejected by policy"
    });
  }
}
```

---

## 3. 外部任务处理

外部任务处理允许将特定 lane 的任务委托给外部系统处理。

### 配置外部处理器

```typescript
import { SessionLane, TaskHandlerMode } from '@a3s/sdk';

// 将 Execute lane 的任务发送到外部系统
await client.setLaneHandler(sessionId, SessionLane.SESSION_LANE_EXECUTE, {
  mode: TaskHandlerMode.TASK_HANDLER_MODE_EXTERNAL,
  timeoutMs: 60000  // 60秒超时
});

// Hybrid 模式：内部处理 + 外部通知
await client.setLaneHandler(sessionId, SessionLane.SESSION_LANE_GENERATE, {
  mode: TaskHandlerMode.TASK_HANDLER_MODE_HYBRID,
  timeoutMs: 120000
});
```

### 处理外部任务

```typescript
// 轮询待处理的外部任务
setInterval(async () => {
  const tasks = await client.listPendingExternalTasks(sessionId);

  for (const task of tasks.tasks) {
    console.log(`Task ID: ${task.taskId}`);
    console.log(`Lane: ${task.lane}`);
    console.log(`Command: ${task.commandType}`);
    console.log(`Payload: ${task.payload}`);

    try {
      // 在外部系统中执行任务
      const result = await executeInExternalSystem(
        task.commandType,
        JSON.parse(task.payload)
      );

      // 返回结果
      await client.completeExternalTask(sessionId, task.taskId, {
        success: true,
        result: JSON.stringify(result)
      });
    } catch (error) {
      // 报告失败
      await client.completeExternalTask(sessionId, task.taskId, {
        success: false,
        error: error.message
      });
    }
  }
}, 1000);
```

### 使用场景示例

```typescript
// 场景：将 Bash 命令发送到安全沙箱执行
async function setupSecureBashExecution(sessionId: string) {
  // 配置 Execute lane 为外部处理
  await client.setLaneHandler(
    sessionId,
    SessionLane.SESSION_LANE_EXECUTE,
    {
      mode: TaskHandlerMode.TASK_HANDLER_MODE_EXTERNAL,
      timeoutMs: 30000
    }
  );

  // 处理外部任务
  const processExternalTasks = async () => {
    const tasks = await client.listPendingExternalTasks(sessionId);

    for (const task of tasks.tasks) {
      if (task.commandType === 'Bash') {
        const payload = JSON.parse(task.payload);

        // 在安全沙箱中执行
        const result = await secureSandbox.execute(payload.command);

        await client.completeExternalTask(sessionId, task.taskId, {
          success: true,
          result: JSON.stringify({
            stdout: result.stdout,
            stderr: result.stderr,
            exitCode: result.exitCode
          })
        });
      }
    }
  };

  // 定期处理
  setInterval(processExternalTasks, 500);
}
```

---

## 4. Provider 配置

动态配置 LLM provider 和模型。

### 添加 Provider

```typescript
// 添加 Anthropic provider
await client.addProvider({
  name: "anthropic",
  apiKey: "sk-ant-xxx",
  baseUrl: "https://api.anthropic.com",
  models: [
    {
      id: "claude-sonnet-4-20250514",
      name: "Claude Sonnet 4",
      family: "claude-sonnet",
      toolCall: true,
      temperature: true,
      attachment: true,
      reasoning: true,
      cost: {
        input: 3.0,
        output: 15.0,
        cacheRead: 0.3,
        cacheWrite: 3.75
      },
      limit: {
        context: 200000,
        output: 8192
      }
    }
  ]
});

// 添加 OpenAI provider
await client.addProvider({
  name: "openai",
  apiKey: "sk-xxx",
  baseUrl: "https://api.openai.com/v1",
  models: [
    {
      id: "gpt-4-turbo",
      name: "GPT-4 Turbo",
      family: "gpt-4",
      toolCall: true,
      temperature: true
    }
  ]
});
```

### 设置默认模型

```typescript
// 设置全局默认模型
await client.setDefaultModel("anthropic", "claude-sonnet-4-20250514");

// 获取当前默认模型
const defaultModel = await client.getDefaultModel();
console.log(`Provider: ${defaultModel.provider}`);
console.log(`Model: ${defaultModel.model}`);
```

### 为会话配置特定模型

```typescript
// 创建会话时指定 LLM
const session = await client.createSession({
  name: "GPT-4 Session",
  workspace: "/tmp/workspace",
  llm: {
    provider: "openai",
    model: "gpt-4-turbo",
    apiKey: "sk-xxx",  // 可选：覆盖 provider 的 API key
    baseUrl: "https://custom-endpoint.com"  // 可选：自定义端点
  }
});

// 运行时更新会话的 LLM 配置
await client.configureSession(sessionId, {
  llm: {
    provider: "anthropic",
    model: "claude-sonnet-4-20250514"
  }
});
```

### 列出可用的 Providers 和模型

```typescript
// 列出所有 providers
const providers = await client.listProviders();

for (const provider of providers.providers) {
  console.log(`\nProvider: ${provider.name}`);
  console.log(`Base URL: ${provider.baseUrl}`);
  console.log(`Models:`);

  for (const model of provider.models) {
    console.log(`  - ${model.id} (${model.name})`);
    console.log(`    Tool Call: ${model.toolCall}`);
    console.log(`    Context: ${model.limit.context} tokens`);
    console.log(`    Cost: $${model.cost.input}/M input, $${model.cost.output}/M output`);
  }
}
```

---

## 5. Todo/任务跟踪

Agent 可以维护任务列表，跟踪工作进度。

### 基本操作

```typescript
// 设置任务列表
await client.setTodos(sessionId, [
  {
    id: "1",
    content: "Implement user authentication",
    status: "in_progress",
    priority: "high"
  },
  {
    id: "2",
    content: "Write unit tests",
    status: "pending",
    priority: "medium"
  },
  {
    id: "3",
    content: "Update documentation",
    status: "pending",
    priority: "low"
  }
]);

// 获取任务列表
const todos = await client.getTodos(sessionId);
console.log(`Total tasks: ${todos.todos.length}`);

for (const todo of todos.todos) {
  console.log(`[${todo.status}] ${todo.content} (${todo.priority})`);
}
```

### 与 Agent 交互

```typescript
// Agent 可以在对话中引用和更新任务
const response = await client.generate(sessionId, {
  messages: [{
    role: "user",
    content: "What tasks are pending?"
  }]
});

// Agent 会自动访问任务列表并回复

// 更新任务状态
await client.generate(sessionId, {
  messages: [{
    role: "user",
    content: "Mark task 1 as completed"
  }]
});

// Agent 会更新任务状态
const updatedTodos = await client.getTodos(sessionId);
```

### 任务状态流转

```typescript
// 任务状态：pending -> in_progress -> completed/cancelled

// 开始任务
await client.setTodos(sessionId, [
  {
    id: "1",
    content: "Refactor authentication module",
    status: "in_progress",  // 从 pending 变为 in_progress
    priority: "high"
  }
]);

// 完成任务
await client.setTodos(sessionId, [
  {
    id: "1",
    content: "Refactor authentication module",
    status: "completed",  // 标记为完成
    priority: "high"
  }
]);

// 取消任务
await client.setTodos(sessionId, [
  {
    id: "2",
    content: "Deprecated feature",
    status: "cancelled",  // 取消任务
    priority: "low"
  }
]);
```

---

## 6. 上下文管理

管理会话的上下文使用情况，优化 token 消耗。

### 查看上下文使用情况

```typescript
// 获取上下文使用统计
const usage = await client.getContextUsage(sessionId);

console.log(`Total tokens: ${usage.totalTokens}`);
console.log(`Prompt tokens: ${usage.promptTokens}`);
console.log(`Completion tokens: ${usage.completionTokens}`);
console.log(`Message count: ${usage.messageCount}`);
console.log(`Usage: ${(usage.totalTokens / 200000 * 100).toFixed(2)}%`);
```

### 压缩上下文

```typescript
// 当上下文接近限制时，压缩历史消息
const usage = await client.getContextUsage(sessionId);

if (usage.totalTokens > 150000) {  // 超过 75% 限制
  console.log("Context is getting full, compacting...");

  const result = await client.compactContext(sessionId);

  console.log(`Before: ${result.before.totalTokens} tokens`);
  console.log(`After: ${result.after.totalTokens} tokens`);
  console.log(`Saved: ${result.before.totalTokens - result.after.totalTokens} tokens`);
}
```

### 清空上下文

```typescript
// 清空会话的所有历史消息（保留系统提示）
await client.clearContext(sessionId);

console.log("Context cleared. Starting fresh conversation.");

// 验证清空
const usage = await client.getContextUsage(sessionId);
console.log(`Current tokens: ${usage.totalTokens}`);  // 应该接近 0
```

### 自动上下文管理

```typescript
// 创建会话时启用自动压缩
const session = await client.createSession({
  name: "Auto-compact Session",
  workspace: "/tmp/workspace",
  maxContextLength: 200000,
  autoCompact: true  // 启用自动压缩
});

// Agent 会在上下文接近限制时自动压缩
// 无需手动干预
```

### 监控上下文使用

```typescript
// 定期监控上下文使用情况
setInterval(async () => {
  const usage = await client.getContextUsage(sessionId);
  const percentage = (usage.totalTokens / 200000 * 100).toFixed(2);

  console.log(`Context usage: ${usage.totalTokens} / 200000 (${percentage}%)`);

  if (usage.totalTokens > 180000) {  // 90% 限制
    console.warn("⚠️  Context is almost full!");

    // 自动压缩
    await client.compactContext(sessionId);
  }
}, 10000);  // 每 10 秒检查一次
```

---

## 7. 权限系统

声明式权限系统，控制工具执行权限。

### 基本权限规则

```typescript
// 设置权限策略
await client.setPermissionPolicy(sessionId, {
  // Allow 规则：自动批准
  allowRules: [
    "Read(*)",           // 允许所有读取操作
    "Glob(*)",           // 允许所有文件搜索
    "Grep(*)",           // 允许所有内容搜索
    "Bash(ls:*)",        // 允许 ls 命令
    "Bash(pwd:*)",       // 允许 pwd 命令
    "Bash(echo:*)"       // 允许 echo 命令
  ],

  // Deny 规则：始终拒绝
  denyRules: [
    "Bash(rm:*)",        // 禁止删除文件
    "Bash(sudo:*)",      // 禁止 sudo
    "Write(/etc/*)",     // 禁止写入系统目录
    "Edit(/etc/*)"       // 禁止编辑系统文件
  ],

  // Ask 规则：需要确认
  askRules: [
    "Bash(*)",           // 其他 bash 命令需要确认
    "Write(*)",          // 写入文件需要确认
    "Edit(*)"            // 编辑文件需要确认
  ]
});
```

### 规则评估顺序

```
Deny → Allow → Ask → Default (Ask)
```

```typescript
// 示例：评估 "Bash(rm -rf /tmp/test)"
// 1. 检查 Deny 规则：匹配 "Bash(rm:*)" → 拒绝 ✓
// 2. 不再检查后续规则

// 示例：评估 "Read(/home/user/file.txt)"
// 1. 检查 Deny 规则：无匹配
// 2. 检查 Allow 规则：匹配 "Read(*)" → 允许 ✓
// 3. 不再检查后续规则

// 示例：评估 "Bash(git commit)"
// 1. 检查 Deny 规则：无匹配
// 2. 检查 Allow 规则：无匹配
// 3. 检查 Ask 规则：匹配 "Bash(*)" → 需要确认 ✓
```

### 动态添加规则

```typescript
// 运行时添加规则
await client.addPermissionRule(sessionId, "allow", "Bash(cargo:*)");
await client.addPermissionRule(sessionId, "deny", "Bash(npm:install*)");
await client.addPermissionRule(sessionId, "ask", "Write(*.rs)");
```

### 检查权限

```typescript
// 检查特定工具调用的权限
const permission = await client.checkPermission(sessionId, {
  toolName: "Bash",
  args: { command: "rm -rf /tmp/test" }
});

console.log(`Decision: ${permission.decision}`);  // DENY
console.log(`Reason: ${permission.reason}`);      // "Matched deny rule: Bash(rm:*)"
```

### 高级场景：项目特定权限

```typescript
// 为不同项目配置不同的权限策略

// 开发环境：宽松权限
const devPolicy = {
  allowRules: [
    "Read(*)",
    "Write(src/**)",
    "Edit(src/**)",
    "Bash(npm:*)",
    "Bash(cargo:*)",
    "Bash(git:*)"
  ],
  denyRules: [
    "Bash(rm:*)",
    "Write(/etc/*)"
  ],
  askRules: []
};

// 生产环境：严格权限
const prodPolicy = {
  allowRules: [
    "Read(*)",
    "Glob(*)",
    "Grep(*)"
  ],
  denyRules: [
    "Write(*)",
    "Edit(*)",
    "Bash(*)"
  ],
  askRules: []
};

// 根据环境应用策略
const policy = process.env.NODE_ENV === 'production' ? prodPolicy : devPolicy;
await client.setPermissionPolicy(sessionId, policy);
```

---

## 完整示例：构建一个安全的代码审查 Agent

```typescript
import { CodeAgentClient, StorageType, TimeoutAction } from '@a3s/sdk';

async function createCodeReviewAgent() {
  const client = new CodeAgentClient('localhost:4088');

  // 1. 创建持久化会话
  const session = await client.createSession({
    name: "Code Review Agent",
    workspace: "/home/user/project",
    storageType: StorageType.STORAGE_TYPE_FILE,
    systemPrompt: "You are a code review assistant. Analyze code for bugs, security issues, and best practices.",
    maxContextLength: 200000,
    autoCompact: true
  });

  const sessionId = session.sessionId;

  // 2. 配置权限：只读访问
  await client.setPermissionPolicy(sessionId, {
    allowRules: [
      "Read(*)",
      "Glob(*)",
      "Grep(*)",
      "Bash(git:log*)",
      "Bash(git:diff*)",
      "Bash(git:show*)"
    ],
    denyRules: [
      "Write(*)",
      "Edit(*)",
      "Bash(git:push*)",
      "Bash(git:commit*)"
    ],
    askRules: []
  });

  // 3. 配置 HITL：自动批准读取操作
  await client.setConfirmationPolicy(sessionId, {
    enabled: true,
    autoApproveTools: ["Read", "Glob", "Grep"],
    requireConfirmTools: ["Bash"],
    defaultTimeoutMs: 30000,
    timeoutAction: TimeoutAction.TIMEOUT_ACTION_REJECT
  });

  // 4. 设置审查任务
  await client.setTodos(sessionId, [
    {
      id: "1",
      content: "Review authentication module",
      status: "pending",
      priority: "high"
    },
    {
      id: "2",
      content: "Check for SQL injection vulnerabilities",
      status: "pending",
      priority: "high"
    },
    {
      id: "3",
      content: "Verify error handling",
      status: "pending",
      priority: "medium"
    }
  ]);

  // 5. 开始审查
  const response = await client.generate(sessionId, {
    messages: [{
      role: "user",
      content: "Please review the code in src/auth/ directory. Check for security issues and best practices."
    }]
  });

  console.log(response.content);

  // 6. 监控上下文使用
  const usage = await client.getContextUsage(sessionId);
  console.log(`Context usage: ${usage.totalTokens} tokens`);

  return sessionId;
}

// 运行
createCodeReviewAgent().then(sessionId => {
  console.log(`Code review agent created: ${sessionId}`);
});
```

---

## 总结

A3S Code Agent 提供了丰富的功能来构建安全、可控的 AI 编程助手：

- **灵活的存储**: 每会话可配置内存或文件存储
- **HITL 确认**: 在执行敏感操作前请求用户批准
- **外部任务**: 将任务委托给外部系统处理
- **Provider 管理**: 动态配置多个 LLM provider
- **任务跟踪**: 维护和管理工作任务列表
- **上下文管理**: 优化 token 使用，避免超出限制
- **权限系统**: 声明式规则控制工具执行权限

这些功能可以组合使用，构建适合不同场景的 AI Agent。
