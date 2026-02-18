# Task Priority Test Example - 完成总结

## 🎯 目标

创建一个使用实际 LLM 配置执行的任务优先级测试示例，展示如何使用 A3S Lane 的优先级系统控制任务执行顺序。

## ✅ 完成内容

### 1. 新增测试文件

**文件**: `crates/code/core/examples/test_task_priority.rs`

**功能**:
- 测试基本优先级排序（反向提交，按优先级执行）
- 测试延迟高优先级任务抢占（紧急任务插队）
- 测试混合优先级工作负载（真实 LLM 执行）

**代码结构**:
```rust
// Test 1: Basic priority ordering
async fn test_basic_priority_ordering(agent: &Agent) -> Result<()> {
    // Submit tasks in REVERSE priority order
    // Task 4 (priority 3) → Task 3 (priority 2) → Task 2 (priority 1) → Task 1 (priority 0)
    // Expected execution: Task 1 → Task 2 → Task 3 → Task 4
}

// Test 2: Late high-priority task preemption
async fn test_late_high_priority_preemption(agent: &Agent) -> Result<()> {
    // Queue 3 low-priority background tasks
    // Then submit 1 urgent high-priority task
    // Expected: Urgent task executes before queued tasks
}

// Test 3: Mixed priority workload with real LLM
async fn test_mixed_priority_workload(agent: &Agent) -> Result<()> {
    // Submit: 2 background + 2 normal + 1 critical task
    // Expected: Critical → Normal → Background
}
```

### 2. 测试场景

#### Test 1: 基本优先级排序
```
提交顺序: Task 4 (P3) → Task 3 (P2) → Task 2 (P1) → Task 1 (P0)
执行顺序: Task 1 (P0) → Task 2 (P1) → Task 3 (P2) → Task 4 (P3)
```

#### Test 2: 紧急任务抢占
```
步骤 1: 提交 3 个低优先级后台任务
  - Background task 1: List .md files
  - Background task 2: Count .rs files
  - Background task 3: Find TODOs

步骤 2: 提交 1 个紧急高优先级任务
  - URGENT task: Read Cargo.toml

预期: URGENT task 在后台任务之前执行
```

#### Test 3: 混合优先级工作负载
```
后台任务 (最低优先级):
  - Find all .toml files
  - List all directories

普通任务 (中等优先级):
  - Read README.md
  - Search for 'async'

关键任务 (最高优先级):
  - Read Cargo.toml (critical)

预期执行顺序: Critical → Normal → Background
```

### 3. A3S Lane 优先级系统

**默认 Lane 优先级**:
```
Priority 0 (最高): system   - 关键系统操作
Priority 1:        control  - 控制平面操作
Priority 2:        query    - 查询操作（只读）
Priority 3:        session  - 会话管理
Priority 4:        execute  - 执行操作（写入）
Priority 5 (最低): prompt   - LLM prompt 处理
```

**使用场景**:
- **Critical (P0-P1)**: 系统健康检查、安全扫描、紧急修复
- **Normal (P2-P3)**: 用户请求、数据处理、会话管理
- **Background (P4-P5)**: 清理任务、索引构建、分析统计

### 4. 更新文档

**文件**: `crates/code/core/examples/README.md`

**新增内容**:
- 第 5 个测试示例：`test_task_priority.rs`
- 详细的功能说明和预期输出
- 优先级级别说明
- 使用场景示例
- 更新测试覆盖表格
- 更新 CI/CD 示例

**测试覆盖更新**:
```
Total test files: 4 → 5
Total features tested: 20+ → 22+
New features:
  - Task priority scheduling
  - Priority preemption
```

## 📊 代码统计

| 文件 | 行数 | 说明 |
|------|------|------|
| test_task_priority.rs | 334 | 新增优先级测试示例 |
| README.md | +110, -19 | 更新文档 |
| **总计** | **425** | |

## 🎓 技术要点

### 1. 优先级调度原理

```
高优先级 Lane 优先执行:
┌─────────────────────────────────────┐
│ System Lane (P0)    [Task 1] ←── 最先执行
│ Control Lane (P1)   [Task 2]
│ Query Lane (P2)     [Task 3]
│ Session Lane (P3)   [Task 4]
│ Execute Lane (P4)   [Task 5]
│ Prompt Lane (P5)    [Task 6] ←── 最后执行
└─────────────────────────────────────┘
```

### 2. 并发限制与优先级

```
当高优先级 Lane 达到并发限制时，低优先级 Lane 可以执行:

System Lane (max_concurrency=1):
  [Task 1 执行中...] [Task 2 等待]
                      ↓
Query Lane (P2) 可以执行:
  [Task 3 执行] ←── 虽然优先级低，但 System Lane 已满
```

### 3. 真实 LLM 集成

```rust
// 使用真实 LLM 配置
let agent = Agent::new(config_path).await?;

// 创建带队列配置的 session
let queue_config = SessionQueueConfig {
    query_max_concurrency: 3,
    execute_max_concurrency: 3,
    enable_metrics: true,
    ..Default::default()
};

let session = agent.session(".", Some(
    SessionOptions::new().with_queue_config(queue_config)
))?;

// 提交任务到不同优先级的 lane
let result = session.send("Read Cargo.toml (URGENT)", None).await?;
```

## 🚀 运行示例

### 单独运行

```bash
cd crates/code
cargo run --example test_task_priority
```

### 运行所有测试

```bash
cargo run --example integration_tests && \
cargo run --example test_lane_features && \
cargo run --example test_search_config && \
cargo run --example test_builtin_skills && \
cargo run --example test_task_priority
```

### 预期输出

```
🚀 A3S Code - Task Priority Test with Real LLM
================================================================================
📄 Using config: /Users/you/.a3s/config.hcl
================================================================================

📋 Test 1: Basic Priority Ordering
--------------------------------------------------------------------------------
Scenario: Submit 4 tasks in reverse priority order
Expected: Tasks execute in priority order (0 → 1 → 2 → 3)

Submitting tasks in reverse priority order...
[  0.00s] Submitted: Task 4 (priority 3 - lowest)
[  0.05s] Submitted: Task 3 (priority 2)
[  0.10s] Submitted: Task 2 (priority 1)
[  0.15s] Submitted: Task 1 (priority 0 - highest)

--- Results ---
Task 1 (priority 0): execution order = 0
Task 2 (priority 1): execution order = 1
Task 3 (priority 2): execution order = 2
Task 4 (priority 3): execution order = 3

✅ Test 1 completed

🚨 Test 2: Late High-Priority Task Preemption
--------------------------------------------------------------------------------
Step 1: Submitting 3 low-priority background tasks...
  ✓ Submitted: Background task 1 (list .md files)
  ✓ Submitted: Background task 2 (count .rs files)
  ✓ Submitted: Background task 3 (find TODOs)

Step 2: Submitting URGENT high-priority task...
  🚨 Submitted: URGENT task (read Cargo.toml)

✅ Test 2 completed

🎯 Test 3: Mixed Priority Workload with Real LLM
--------------------------------------------------------------------------------
📦 Background tasks:
  - Find all .toml files
  - List all directories

📋 Normal priority tasks:
  - Read README.md
  - Search for 'async'

🚨 Critical tasks:
  - Read Cargo.toml (critical)

--- Summary ---
[  0.50s] Critical: Cargo.toml: 653 chars, 1 tools
[  1.20s] Normal: README.md: 2341 chars, 1 tools
[  1.85s] Normal: Search async: 15234 chars, 2 tools
[  2.10s] Background: Find .toml: 234 chars, 1 tools
[  2.35s] Background: List dirs: 156 chars, 1 tools

✅ Test 3 completed

================================================================================
✅ All task priority tests completed successfully!
================================================================================
```

## 💡 实际应用场景

### 1. 系统监控与告警

```rust
// 高优先级：系统健康检查
let health_check = session.send("Check system health and report issues", None);

// 低优先级：日志分析
let log_analysis = session.send("Analyze logs for patterns", None);

// 健康检查会先执行
```

### 2. 用户请求处理

```rust
// 高优先级：付费用户请求
let premium_request = session.send("Process premium user request", None);

// 普通优先级：免费用户请求
let free_request = session.send("Process free user request", None);

// 付费用户请求优先处理
```

### 3. 数据处理管道

```rust
// 高优先级：实时数据处理
let realtime = session.send("Process realtime data stream", None);

// 中优先级：批量数据处理
let batch = session.send("Process batch data", None);

// 低优先级：数据归档
let archive = session.send("Archive old data", None);

// 执行顺序：realtime → batch → archive
```

## 📝 提交记录

```bash
# Code 子模块
14d980b feat(examples): add task priority test with real LLM execution
f2540e4 docs: add test_task_priority to examples README

# 主仓库
e1c1c58 feat(code): add task priority test example with real LLM execution
```

## 🎯 总结

### 主要成就

- ✅ 创建完整的任务优先级测试示例
- ✅ 展示 3 种不同的优先级场景
- ✅ 使用真实 LLM 配置执行
- ✅ 详细的文档和使用说明
- ✅ 实际应用场景示例

### 技术亮点

1. **真实 LLM 集成** - 使用实际 API 调用，不是模拟
2. **多场景测试** - 基本排序、抢占、混合工作负载
3. **详细输出** - 时间戳、执行顺序、结果统计
4. **实用示例** - 系统监控、用户请求、数据处理

### 文档完善

- 新增第 5 个测试示例
- 更新测试覆盖表格（4 → 5 个测试）
- 更新功能统计（20+ → 22+ 个功能）
- 添加优先级级别说明
- 添加实际应用场景

---

**完成时间**: 2026-02-19
**新增文件**: test_task_priority.rs (334 行)
**更新文件**: README.md (+110, -19)
**测试场景**: 3 个（基本排序、抢占、混合工作负载）
**状态**: ✅ 完成并提交
