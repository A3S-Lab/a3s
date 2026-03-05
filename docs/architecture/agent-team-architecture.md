# AgentTeam 架构设计

## 1. 整体架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Main Agent (主智能体)                        │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │                    AgentOrchestrator                          │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │  SubAgent Registry (子智能体注册表)                      │  │  │
│  │  │  - SubAgent ID → SubAgentHandle 映射                    │  │  │
│  │  │  - 生命周期管理                                          │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  │                                                                 │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │  Event Bus (事件总线) - tokio::broadcast               │  │  │
│  │  │  - 11 种事件类型                                         │  │  │
│  │  │  - 多订阅者支持                                          │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  │                                                                 │  │
│  │  ┌─────────────────────────────────────────────────────────┐  │  │
│  │  │  Control Channel (控制通道)                             │  │  │
│  │  │  - Pause / Resume / Cancel                              │  │  │
│  │  │  - AdjustParams / InjectPrompt                          │  │  │
│  │  └─────────────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    │               │               │
                    ▼               ▼               ▼
┌─────────────────────┐ ┌─────────────────────┐ ┌─────────────────────┐
│   SubAgent 1        │ │   SubAgent 2        │ │   SubAgent N        │
│  ┌───────────────┐  │ │  ┌───────────────┐  │ │  ┌───────────────┐  │
│  │SubAgentWrapper│  │ │  │SubAgentWrapper│  │ │  │SubAgentWrapper│  │
│  │               │  │ │  │               │  │ │  │               │  │
│  │ ┌───────────┐ │  │ │  │ ┌───────────┐ │  │ │  │ ┌───────────┐ │  │
│  │ │ State     │ │  │ │  │ │ State     │ │  │ │  │ │ State     │ │  │
│  │ │ Machine   │ │  │ │  │ │ Machine   │ │  │ │  │ │ Machine   │ │  │
│  │ └───────────┘ │  │ │  │ └───────────┘ │  │ │  │ └───────────┘ │  │
│  │               │  │ │  │               │  │ │  │               │  │
│  │ ┌───────────┐ │  │ │  │ ┌───────────┐ │  │ │  │ ┌───────────┐ │  │
│  │ │ Event     │ │  │ │  │ │ Event     │ │  │ │  │ │ Event     │ │  │
│  │ │ Emitter   │ │  │ │  │ │ Emitter   │ │  │ │  │ │ Emitter   │ │  │
│  │ └───────────┘ │  │ │  │ └───────────┘ │  │ │  │ └───────────┘ │  │
│  │               │  │ │  │               │  │ │  │               │  │
│  │ ┌───────────┐ │  │ │  │ ┌───────────┐ │  │ │  │ ┌───────────┐ │  │
│  │ │ Control   │ │  │ │  │ │ Control   │ │  │ │  │ │ Control   │ │  │
│  │ │ Receiver  │ │  │ │  │ │ Receiver  │ │  │ │  │ │ Receiver  │ │  │
│  │ └───────────┘ │  │ │  │ └───────────┘ │  │ │  │ └───────────┘ │  │
│  └───────────────┘  │ │  └───────────────┘  │ │  └───────────────┘  │
│                     │ │                     │ │                     │
│  ┌───────────────┐  │ │  ┌───────────────┐  │ │  ┌───────────────┐  │
│  │  AgentLoop    │  │ │  │  AgentLoop    │  │ │  │  AgentLoop    │  │
│  │  (执行引擎)   │  │ │  │  (执行引擎)   │  │ │  │  (执行引擎)   │  │
│  └───────────────┘  │ │  └───────────────┘  │ │  └───────────────┘  │
└─────────────────────┘ └─────────────────────┘ └─────────────────────┘
```

## 2. 核心组件说明

### 2.1 AgentOrchestrator (编排器)
- **职责**: 管理所有 SubAgent 的生命周期和通信
- **功能**:
  - 创建和启动 SubAgent
  - 分发控制信号
  - 聚合和转发事件
  - 维护 SubAgent 注册表

### 2.2 SubAgentWrapper (子智能体包装器)
- **职责**: 包装 AgentLoop 执行，提供事件和控制能力
- **功能**:
  - 状态机管理 (7 种状态)
  - 事件发射 (11 种事件类型)
  - 控制信号处理
  - 执行上下文隔离

### 2.3 SubAgentHandle (子智能体句柄)
- **职责**: 主智能体控制 SubAgent 的接口
- **功能**:
  - pause() / resume() / cancel()
  - adjust_params() / inject_prompt()
  - wait() / state() / result()

### 2.4 Event Bus (事件总线)
- **实现**: tokio::broadcast::channel
- **特性**:
  - 异步非阻塞
  - 多订阅者支持
  - 内存高效 (默认 1000 容量)

## 3. SubAgent 状态机

```
                    ┌──────────────┐
                    │ Initializing │
                    └──────┬───────┘
                           │
                           ▼
                    ┌──────────────┐
              ┌────▶│   Running    │◀────┐
              │     └──────┬───────┘     │
              │            │             │
              │            ▼             │
    Resume    │     ┌──────────────┐    │ Pause
              │     │    Paused    │────┘
              └─────┤              │
                    └──────┬───────┘
                           │
                           │ Cancel
                           ▼
              ┌────────────┴────────────┐
              │                         │
              ▼                         ▼
       ┌──────────────┐          ┌──────────────┐
       │  Completed   │          │  Cancelled   │
       └──────────────┘          └──────────────┘
              │                         │
              └────────┬────────────────┘
                       │
                       ▼
                ┌──────────────┐
                │    Error     │
                └──────────────┘
```

**状态说明**:
- **Initializing**: 初始化中，准备执行环境
- **Running**: 正在执行任务
- **Paused**: 已暂停，等待恢复
- **Completed**: 成功完成
- **Cancelled**: 被取消
- **Error**: 执行出错

## 4. 事件类型 (11 种)

| 事件类型 | 触发时机 | 数据内容 |
|---------|---------|---------|
| `SubAgentStarted` | SubAgent 开始执行 | id, config |
| `SubAgentCompleted` | SubAgent 成功完成 | id, result |
| `SubAgentFailed` | SubAgent 执行失败 | id, error |
| `SubAgentCancelled` | SubAgent 被取消 | id |
| `SubAgentStateChanged` | 状态变更 | id, old_state, new_state |
| `SubAgentProgress` | 执行进度更新 | id, progress, message |
| `SubAgentToolCalled` | 调用工具 | id, tool_name, args |
| `SubAgentToolResult` | 工具返回结果 | id, tool_name, result |
| `SubAgentLlmRequest` | LLM 请求 | id, messages |
| `SubAgentLlmResponse` | LLM 响应 | id, response |
| `SubAgentLog` | 日志消息 | id, level, message |

## 5. 控制信号 (5 种)

| 信号类型 | 作用 | 参数 |
|---------|------|------|
| `Pause` | 暂停执行 | - |
| `Resume` | 恢复执行 | - |
| `Cancel` | 取消执行 | - |
| `AdjustParams` | 调整参数 | key-value map |
| `InjectPrompt` | 注入提示词 | prompt string |

## 6. SubAgent 配置方法

### 6.1 Rust API

```rust
use a3s_code_core::orchestrator::{AgentOrchestrator, SubAgentConfig};

// 1. 创建编排器
let orchestrator = AgentOrchestrator::new();

// 2. 配置 SubAgent
let config = SubAgentConfig {
    name: "data-analyzer".to_string(),
    prompt: "Analyze the dataset and extract insights".to_string(),
    workspace: "/path/to/workspace".into(),
    max_turns: Some(10),
    tools: vec!["read".to_string(), "grep".to_string()],
    llm_config: None, // 使用默认配置
};

// 3. 启动 SubAgent
let handle = orchestrator.spawn_subagent(config).await?;

// 4. 订阅事件
let mut events = orchestrator.subscribe_all();
tokio::spawn(async move {
    while let Ok(event) = events.recv().await {
        println!("Event: {:?}", event);
    }
});

// 5. 控制 SubAgent
handle.pause().await?;
handle.resume().await?;
handle.adjust_params(HashMap::from([
    ("temperature".to_string(), "0.5".to_string())
])).await?;

// 6. 等待完成
let result = handle.wait().await?;

// 7. 实时查询任务列表和活动
let subagents = orchestrator.list_subagents().await;
for info in subagents {
    println!("SubAgent: {}", info.id);
    println!("  类型: {}", info.agent_type);
    println!("  状态: {}", info.state);
    println!("  当前活动: {:?}", info.current_activity);
}

// 8. 查询特定 SubAgent 详情
let info = orchestrator.get_subagent_info("subagent-1").await?;
println!("当前活动: {:?}", info.current_activity);

// 9. 获取所有活跃 SubAgent 的实时活动
let activities = orchestrator.get_active_activities().await;
for (id, activity) in activities {
    match activity {
        SubAgentActivity::CallingTool { tool_name, args } => {
            println!("{} 正在调用工具: {}", id, tool_name);
        }
        SubAgentActivity::RequestingLlm { message_count } => {
            println!("{} 正在请求 LLM ({} 条消息)", id, message_count);
        }
        SubAgentActivity::WaitingForControl { reason } => {
            println!("{} 等待控制: {}", id, reason);
        }
        SubAgentActivity::Idle => {
            println!("{} 空闲中", id);
        }
    }
}
```

### 6.2 Python API

```python
from a3s_code import Orchestrator, SubAgentConfig

# 1. 创建编排器
orchestrator = Orchestrator()

# 2. 配置 SubAgent
config = SubAgentConfig(
    name="data-analyzer",
    prompt="Analyze the dataset and extract insights",
    workspace="/path/to/workspace",
    max_turns=10,
    tools=["read", "grep"]
)

# 3. 启动 SubAgent
handle = await orchestrator.spawn_subagent(config)

# 4. 订阅事件
async for event in orchestrator.subscribe_all():
    print(f"Event: {event}")

# 5. 控制 SubAgent
await handle.pause()
await handle.resume()
await handle.adjust_params({"temperature": "0.5"})

# 6. 等待完成
result = await handle.wait()
```

### 6.3 Node.js API

```javascript
import { Orchestrator, SubAgentConfig } from '@a3s-lab/code';

// 1. 创建编排器
const orchestrator = new Orchestrator();

// 2. 配置 SubAgent
const config = new SubAgentConfig({
  name: 'data-analyzer',
  prompt: 'Analyze the dataset and extract insights',
  workspace: '/path/to/workspace',
  maxTurns: 10,
  tools: ['read', 'grep']
});

// 3. 启动 SubAgent
const handle = await orchestrator.spawnSubagent(config);

// 4. 订阅事件
for await (const event of orchestrator.subscribeAll()) {
  console.log('Event:', event);
}

// 5. 控制 SubAgent
await handle.pause();
await handle.resume();
await handle.adjustParams({ temperature: '0.5' });

// 6. 等待完成
const result = await handle.wait();
```

## 7. 执行流程

### 7.1 完整生命周期

```
1. 配置阶段
   └─ 创建 SubAgentConfig
   └─ 设置 name, prompt, workspace, tools, max_turns

2. 启动阶段
   └─ orchestrator.spawn_subagent(config)
   └─ 创建 SubAgentWrapper
   └─ 创建 control_channel (mpsc)
   └─ 创建 SubAgentHandle
   └─ 发送 SubAgentStarted 事件
   └─ 状态: Initializing → Running

3. 执行阶段
   └─ SubAgentWrapper.execute()
   └─ 循环执行任务 (max_turns 次)
   └─ 每次迭代:
      ├─ 检查 control_channel (Pause/Resume/Cancel)
      ├─ 发送 SubAgentProgress 事件
      ├─ 调用工具 → SubAgentToolCalled 事件
      ├─ 工具返回 → SubAgentToolResult 事件
      ├─ LLM 请求 → SubAgentLlmRequest 事件
      └─ LLM 响应 → SubAgentLlmResponse 事件

4. 控制阶段 (可选)
   └─ handle.pause() → 发送 Pause 信号
   └─ handle.resume() → 发送 Resume 信号
   └─ handle.cancel() → 发送 Cancel 信号
   └─ handle.adjust_params() → 发送 AdjustParams 信号
   └─ handle.inject_prompt() → 发送 InjectPrompt 信号

5. 完成阶段
   └─ 任务完成 → SubAgentCompleted 事件
   └─ 任务失败 → SubAgentFailed 事件
   └─ 任务取消 → SubAgentCancelled 事件
   └─ 状态: Running → Completed/Error/Cancelled
   └─ handle.wait() 返回结果
```

### 7.2 事件流示例

```
Time  │ Event
──────┼────────────────────────────────────────────────────
0ms   │ SubAgentStarted { id: "sub-1", config: {...} }
10ms  │ SubAgentStateChanged { id: "sub-1", old: Initializing, new: Running }
50ms  │ SubAgentProgress { id: "sub-1", progress: 0.1, message: "Reading file..." }
100ms │ SubAgentToolCalled { id: "sub-1", tool: "read", args: {...} }
150ms │ SubAgentToolResult { id: "sub-1", tool: "read", result: "..." }
200ms │ SubAgentLlmRequest { id: "sub-1", messages: [...] }
500ms │ SubAgentLlmResponse { id: "sub-1", response: "..." }
550ms │ SubAgentProgress { id: "sub-1", progress: 0.5, message: "Analyzing..." }
...
2000ms│ SubAgentCompleted { id: "sub-1", result: "..." }
```

## 8. 高级用法

### 8.0 实时监控任务列表和活动

主智能体可以实时查看所有子智能体的任务列表和当前活动：

```rust
// 获取所有子智能体列表（包含元信息和当前活动）
let subagents = orchestrator.list_subagents().await;
for info in subagents {
    println!("SubAgent: {}", info.id);
    println!("  类型: {}", info.agent_type);
    println!("  描述: {}", info.description);
    println!("  状态: {}", info.state);
    println!("  创建时间: {}", info.created_at);
    println!("  当前活动: {:?}", info.current_activity);
}

// 查询特定子智能体的详细信息
if let Some(info) = orchestrator.get_subagent_info("subagent-1").await {
    match info.current_activity {
        Some(SubAgentActivity::CallingTool { tool_name, args }) => {
            println!("正在调用工具: {}", tool_name);
            println!("参数: {}", args);
        }
        Some(SubAgentActivity::RequestingLlm { message_count }) => {
            println!("正在请求 LLM，消息数: {}", message_count);
        }
        Some(SubAgentActivity::WaitingForControl { reason }) => {
            println!("等待控制信号: {}", reason);
        }
        Some(SubAgentActivity::Idle) => {
            println!("空闲中");
        }
        None => {
            println!("无活动信息");
        }
    }
}

// 获取所有活跃子智能体的当前活动（过滤掉已完成的）
let activities = orchestrator.get_active_activities().await;
println!("当前有 {} 个活跃子智能体", activities.len());
for (id, activity) in activities {
    println!("{}: {:?}", id, activity);
}

// 获取活跃子智能体数量
let active_count = orchestrator.active_count().await;
println!("活跃子智能体: {}", active_count);

// 获取所有子智能体的状态
let states = orchestrator.get_all_states().await;
for (id, state) in states {
    println!("{}: {:?}", id, state);
}
```

**实时监控的典型场景**：

1. **仪表盘展示**: 在 UI 中实时显示所有子智能体的状态和活动
2. **负载均衡**: 根据活跃子智能体数量动态调整任务分配
3. **异常检测**: 监控子智能体活动，发现异常行为（如长时间卡在某个工具调用）
4. **性能分析**: 统计各类活动的时间分布，优化执行策略
5. **人工干预**: 当子智能体执行危险操作时，主智能体可以实时发现并暂停

### 8.1 多 SubAgent 并行执行

```rust
let handles = vec![
    orchestrator.spawn_subagent(config1).await?,
    orchestrator.spawn_subagent(config2).await?,
    orchestrator.spawn_subagent(config3).await?,
];

// 等待所有完成
let results = futures::future::join_all(
    handles.into_iter().map(|h| h.wait())
).await;
```

### 8.2 动态调整执行策略

```rust
// 监听进度事件，动态调整
let mut events = orchestrator.subscribe_all();
while let Ok(event) = events.recv().await {
    match event {
        OrchestratorEvent::SubAgentProgress { id, progress, .. } => {
            if progress < 0.3 {
                // 进度慢，降低温度提高准确性
                handle.adjust_params(HashMap::from([
                    ("temperature".to_string(), "0.3".to_string())
                ])).await?;
            }
        }
        OrchestratorEvent::SubAgentFailed { id, error } => {
            // 失败后重试
            let new_handle = orchestrator.spawn_subagent(config.clone()).await?;
        }
        _ => {}
    }
}
```

### 8.3 人工干预

```rust
// 监听工具调用，需要时暂停并注入指令
let mut events = orchestrator.subscribe_all();
while let Ok(event) = events.recv().await {
    match event {
        OrchestratorEvent::SubAgentToolCalled { id, tool_name, .. } => {
            if tool_name == "bash" {
                // 执行危险命令前暂停
                handle.pause().await?;

                // 等待人工审核
                let approved = ask_user_approval().await;

                if approved {
                    handle.resume().await?;
                } else {
                    handle.inject_prompt(
                        "Use a safer alternative instead of bash".to_string()
                    ).await?;
                    handle.resume().await?;
                }
            }
        }
        _ => {}
    }
}
```

## 9. 架构优势

### 9.1 松耦合
- 主智能体和子智能体通过事件总线通信
- 无需直接依赖，易于扩展

### 9.2 实时监控
- 11 种事件类型覆盖所有关键节点
- 主智能体可实时了解子智能体状态

### 9.3 动态控制
- 5 种控制信号支持运行时调整
- 支持暂停、恢复、取消、参数调整、提示词注入

### 9.4 可扩展性
- 支持自定义事件处理器
- 支持自定义传输层 (内存 → NATS)
- 支持自定义状态机扩展

### 9.5 高性能
- 基于 tokio 异步运行时
- 事件总线零拷贝广播
- 控制信号低延迟传递

## 10. 未来扩展

### 10.1 分布式支持
- 替换 tokio::broadcast 为 NATS
- SubAgent 可运行在不同节点
- 事件和控制信号通过网络传输

### 10.2 持久化
- 事件流持久化到数据库
- 支持断点续传
- 支持历史回放

### 10.3 可视化
- 实时监控面板
- 事件流可视化
- 状态机图形化展示

### 10.4 智能调度
- 基于负载的 SubAgent 分配
- 自动故障转移
- 资源配额管理
