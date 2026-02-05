# AgenticLoop 实现快速指南

## 概述

本文档说明如何为 A3S Code 实现类似 OpenCode 的 AgenticLoop 机制，使其具备自主规划、反思和学习能力。

## 核心增强功能

### 1. 规划阶段 (Planning)

**目标**: 在执行前创建详细的执行计划

```rust
// 使用方法
let result = agent.execute_with_planning(
    &history,
    "创建一个带认证的 REST API",
    Some(event_tx),
).await?;

// 代理会:
// 1. 分析任务复杂度
// 2. 分解为多个步骤
// 3. 确定所需工具
// 4. 按步骤执行
// 5. 跟踪进度
```

**实现位置**: `crates/code/src/agent.rs`

**新增结构**:
- `ExecutionPlan` - 执行计划
- `PlanStep` - 计划步骤
- `StepStatus` - 步骤状态

### 2. 自我反思 (Self-Reflection)

**目标**: 在每次工具执行后进行反思和学习

```rust
// 工具执行后自动反思
let (output, exit_code, reflection) = agent.execute_tool_with_reflection(&tool_call).await?;

if !reflection.success && reflection.should_retry {
    // 使用替代方案重试
    if let Some(alternative) = reflection.alternative {
        // 执行替代方案
    }
}
```

**新增结构**:
- `ToolReflection` - 工具反思结果
- 包含: 成功与否、学到什么、是否重试、替代方案

### 3. 目标跟踪 (Goal Tracking)

**目标**: 明确跟踪任务目标和完成进度

```rust
let config = AgentConfig {
    goal_tracking: true,
    ..Default::default()
};

// 代理会:
// 1. 从提示词中提取目标
// 2. 定义成功标准
// 3. 跟踪进度 (0.0 - 1.0)
// 4. 判断目标是否达成
```

**新增结构**:
- `AgentGoal` - 代理目标
- 包含: 描述、成功标准、进度、是否达成

### 4. 自适应策略 (Adaptive Strategy)

**目标**: 根据任务复杂度自动选择执行策略

```rust
// 自动选择最佳策略
let result = agent.execute_adaptive(
    &history,
    prompt,
    Some(event_tx),
).await?;

// 策略类型:
// - Direct: 简单任务直接执行
// - Planned: 中等任务先规划再执行
// - Iterative: 复杂任务迭代优化
// - Parallel: 超复杂任务并行执行
```

**新增枚举**:
- `ExecutionStrategy` - 执行策略
- `Complexity` - 任务复杂度

### 5. 记忆与学习 (Memory & Learning)

**目标**: 从过去的经验中学习，避免重复错误

```rust
let memory = AgentMemory::new(memory_store);
let config = AgentConfig {
    memory: Some(memory),
    ..Default::default()
};

// 代理会:
// 1. 回忆类似的过去任务
// 2. 使用成功的模式
// 3. 避免已知的失败
// 4. 存储新的学习成果
```

**新增结构**:
- `AgentMemory` - 代理记忆
- `MemoryItem` - 记忆项
- 包含: 短期记忆、长期记忆、工作记忆

## 实现步骤

### 第一阶段: 规划与目标跟踪 (1-2周)

**文件**: `crates/code/src/agent.rs`

1. **添加规划结构**
```rust
pub struct ExecutionPlan {
    pub goal: String,
    pub steps: Vec<PlanStep>,
    pub complexity: Complexity,
    pub required_tools: Vec<String>,
}

pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub tool: Option<String>,
    pub dependencies: Vec<String>,
    pub status: StepStatus,
}
```

2. **实现规划方法**
```rust
impl AgentLoop {
    pub async fn plan(&self, prompt: &str, context: Option<&str>) -> Result<ExecutionPlan> {
        // 使用 LLM 创建计划
        let planning_prompt = format!(
            "Create a detailed execution plan for: {}\nBreak it down into steps.",
            prompt
        );

        let response = self.llm_client.complete(
            &[Message::user(&planning_prompt)],
            Some("You are a planning assistant."),
            &[],
        ).await?;

        // 解析计划
        let plan = self.parse_plan(&response.text())?;
        Ok(plan)
    }
}
```

3. **添加目标跟踪**
```rust
pub struct AgentGoal {
    pub description: String,
    pub success_criteria: Vec<String>,
    pub progress: f32,
    pub achieved: bool,
}

impl AgentLoop {
    async fn extract_goal(&self, prompt: &str) -> Result<AgentGoal> {
        // 从提示词中提取目标
    }

    async fn check_goal_achievement(&self, goal: &AgentGoal, state: &str) -> Result<bool> {
        // 检查目标是否达成
    }
}
```

### 第二阶段: 反思与自适应 (3-4周)

**文件**: `crates/code/src/agent.rs`

4. **添加反思机制**
```rust
pub struct ToolReflection {
    pub success: bool,
    pub insights: Vec<String>,
    pub should_retry: bool,
    pub alternative: Option<String>,
}

impl AgentLoop {
    async fn reflect_on_tool_result(
        &self,
        tool_name: &str,
        tool_args: &serde_json::Value,
        tool_output: &str,
        exit_code: i32,
    ) -> Result<ToolReflection> {
        // 使用 LLM 分析工具执行结果
        let reflection_prompt = format!(
            "Reflect on this tool execution:\nTool: {}\nArgs: {}\nOutput: {}\nExit code: {}\n\nWas it successful? What did we learn?",
            tool_name, tool_args, tool_output, exit_code
        );

        // 调用 LLM 获取反思
        // 解析反思结果
    }
}
```

5. **实现自适应策略**
```rust
pub enum ExecutionStrategy {
    Direct,
    Planned,
    Iterative,
    Parallel,
}

impl AgentLoop {
    async fn analyze_complexity(&self, prompt: &str) -> Result<Complexity> {
        // 分析任务复杂度
    }

    pub async fn execute_adaptive(
        &self,
        history: &[Message],
        prompt: &str,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<AgentResult> {
        let complexity = self.analyze_complexity(prompt).await?;
        let strategy = self.select_strategy(complexity);

        match strategy {
            ExecutionStrategy::Direct => self.execute(history, prompt, event_tx).await,
            ExecutionStrategy::Planned => self.execute_with_planning(history, prompt, event_tx).await,
            // ... 其他策略
        }
    }
}
```

### 第三阶段: 记忆与学习 (5-6周)

**新文件**: `crates/code/src/memory.rs`

6. **实现记忆系统**
```rust
pub struct AgentMemory {
    pub short_term: Vec<MemoryItem>,
    pub long_term: Arc<dyn MemoryStore>,
    pub working: Vec<MemoryItem>,
}

pub struct MemoryItem {
    pub id: String,
    pub content: String,
    pub timestamp: i64,
    pub importance: f32,
    pub tags: Vec<String>,
}

impl AgentMemory {
    pub async fn remember(&mut self, item: MemoryItem) -> Result<()> {
        // 存储记忆
    }

    pub async fn recall(&self, query: &str) -> Result<Vec<MemoryItem>> {
        // 回忆相关记忆
    }
}
```

## 新增事件类型

**文件**: `crates/code/src/agent.rs`

```rust
pub enum AgentEvent {
    // ... 现有事件 ...

    /// 规划开始
    #[serde(rename = "planning_start")]
    PlanningStart { prompt: String },

    /// 规划完成
    #[serde(rename = "planning_end")]
    PlanningEnd { plan: ExecutionPlan },

    /// 步骤开始
    #[serde(rename = "step_start")]
    StepStart { step_id: String, description: String },

    /// 步骤完成
    #[serde(rename = "step_end")]
    StepEnd { step_id: String, status: StepStatus },

    /// 工具反思
    #[serde(rename = "tool_reflection")]
    ToolReflection { tool_name: String, reflection: ToolReflection },

    /// 目标进度
    #[serde(rename = "goal_progress")]
    GoalProgress { goal: String, progress: f32 },

    /// 目标达成
    #[serde(rename = "goal_achieved")]
    GoalAchieved { goal: String },

    /// 策略选择
    #[serde(rename = "strategy_selected")]
    StrategySelected { strategy: ExecutionStrategy },
}
```

## 配置选项

**文件**: `crates/code/src/agent.rs`

```rust
pub struct AgentConfig {
    // ... 现有字段 ...

    /// 启用规划
    pub planning_enabled: bool,

    /// 启用目标跟踪
    pub goal_tracking: bool,

    /// 启用反思
    pub reflection_enabled: bool,

    /// 启用自适应策略
    pub adaptive_strategy: bool,

    /// 记忆系统
    pub memory: Option<Arc<AgentMemory>>,
}
```

## 使用示例

### 示例 1: 基本规划执行

```rust
use a3s_code::agent::{AgentLoop, AgentConfig};

let config = AgentConfig {
    planning_enabled: true,
    ..Default::default()
};

let agent = AgentLoop::new(llm_client, tool_executor, config);

let result = agent.execute_with_planning(
    &[],
    "创建一个用户认证系统",
    Some(event_tx),
).await?;
```

### 示例 2: 完整自主模式

```rust
let memory = AgentMemory::new(memory_store);

let config = AgentConfig {
    planning_enabled: true,
    goal_tracking: true,
    reflection_enabled: true,
    adaptive_strategy: true,
    memory: Some(Arc::new(memory)),
    ..Default::default()
};

let agent = AgentLoop::new(llm_client, tool_executor, config);

// 自动选择最佳策略执行
let result = agent.execute_adaptive(
    &[],
    "修复所有认证模块的 bug",
    Some(event_tx),
).await?;
```

### 示例 3: 监听事件

```rust
let (tx, mut rx) = mpsc::channel(100);

tokio::spawn(async move {
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::PlanningEnd { plan } => {
                println!("计划创建完成: {} 个步骤", plan.steps.len());
            }
            AgentEvent::StepStart { step_id, description } => {
                println!("开始执行步骤 {}: {}", step_id, description);
            }
            AgentEvent::GoalProgress { goal, progress } => {
                println!("目标进度: {} - {:.1}%", goal, progress * 100.0);
            }
            AgentEvent::GoalAchieved { goal } => {
                println!("目标达成: {}", goal);
            }
            _ => {}
        }
    }
});

let result = agent.execute_adaptive(&[], prompt, Some(tx)).await?;
```

## 测试

### 单元测试

```rust
#[tokio::test]
async fn test_planning() {
    let agent = create_test_agent();
    let plan = agent.plan("创建 web 服务器", None).await.unwrap();

    assert!(!plan.steps.is_empty());
    assert!(plan.required_tools.contains(&"write".to_string()));
}

#[tokio::test]
async fn test_goal_tracking() {
    let config = AgentConfig {
        goal_tracking: true,
        ..Default::default()
    };

    let agent = create_test_agent_with_config(config);
    let result = agent.execute_with_planning(&[], "修复 bug", None).await.unwrap();

    // 验证目标被跟踪和达成
}

#[tokio::test]
async fn test_reflection() {
    let agent = create_test_agent();
    let reflection = agent.reflect_on_tool_result(
        "bash",
        &json!({"command": "invalid"}),
        "command not found",
        127,
    ).await.unwrap();

    assert!(!reflection.success);
    assert!(reflection.should_retry);
}
```

## 性能考虑

1. **规划开销**: 规划阶段会增加一次 LLM 调用，约增加 1-2 秒延迟
2. **反思开销**: 每次工具执行后的反思会增加 LLM 调用，可配置关闭
3. **记忆查询**: 记忆查询应该使用向量数据库优化
4. **并行执行**: 复杂任务可以并行执行多个步骤

## 向后兼容

所有新功能都是可选的，通过 `AgentConfig` 控制：

```rust
// 保持现有行为
let config = AgentConfig::default();

// 逐步启用新功能
let config = AgentConfig {
    planning_enabled: true,  // 只启用规划
    ..Default::default()
};

// 完全启用
let config = AgentConfig {
    planning_enabled: true,
    goal_tracking: true,
    reflection_enabled: true,
    adaptive_strategy: true,
    memory: Some(memory),
    ..Default::default()
};
```

## 下一步

1. ✅ 系统提示词已创建 (`crates/code/prompts/default_system_prompt.md`)
2. ✅ 设计文档已完成 (`docs/agentic-loop-enhancements.md`)
3. ⏳ 实现第一阶段: 规划与目标跟踪
4. ⏳ 实现第二阶段: 反思与自适应
5. ⏳ 实现第三阶段: 记忆与学习
6. ⏳ 集成测试与文档

## 参考资料

- **详细设计**: `docs/agentic-loop-enhancements.md`
- **系统提示词**: `crates/code/prompts/default_system_prompt.md`
- **当前实现**: `crates/code/src/agent.rs`
- **OpenCode**: https://opencode.ai/

---

**状态**: 📋 设计完成，待实现
**目标版本**: v0.2.0
**预计时间**: 6 周
