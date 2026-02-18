# Session 内部并行任务执行速度优化方案

## 问题分析

### 当前性能表现

测试结果显示，启用队列的并行执行反而比顺序执行慢：

```
测试 1 (简单 glob):
  顺序执行: 14.70s (11 tools)
  并行执行: 24.36s (11 tools)
  结果: 1.66x 变慢

测试 2 (重型 grep):
  顺序执行: 11.86s (6 tools)
  并行执行: 20.50s (10 tools)
  结果: 1.73x 变慢

测试 3 (内部并行):
  顺序执行: 8.52s (6 tools)
  并行执行: 14.48s (8 tools)
  结果: 1.70x 变慢
```

### 根本原因

1. **队列开销过大**
   - Channel 通信延迟
   - 任务提交和结果收集的协调开销
   - SessionCommandAdapter 包装层开销
   - 每个任务都要经过多层异步转换

2. **默认并发度过低**
   ```rust
   SessionLane::Query => LaneConfig::new(1, 4)  // 最大只有 4 并发
   ```
   - Query lane 默认最大并发度只有 4
   - 即使用户设置 `set_query_concurrency(5)`，底层 LaneConfig 仍然限制为 4
   - 实际并发度 = min(用户设置, LaneConfig.max_concurrency)

3. **操作速度太快**
   - glob 和 grep 操作通常在毫秒级完成
   - 队列开销 (5-10s) >> 操作本身时间 (< 100ms)

4. **LLM 决策不一致**
   - 不同运行中 LLM 做出不同决策
   - 工具调用次数不同 (6 vs 10, 11 vs 11)
   - 难以公平比较

## 优化方案

### 方案 1: 提高默认并发度 ⭐ 推荐

**问题**: Query lane 默认最大并发度只有 4，限制了并行能力

**解决方案**: 提高 LaneConfig 的默认并发度

```rust
// 当前配置 (session_lane_queue.rs:49-56)
fn lane_config(self) -> LaneConfig {
    match self {
        SessionLane::Control => LaneConfig::new(1, 2),
        SessionLane::Query => LaneConfig::new(1, 4),      // ❌ 太低
        SessionLane::Execute => LaneConfig::new(1, 2),
        SessionLane::Generate => LaneConfig::new(1, 1),
    }
}

// 优化后配置
fn lane_config(self) -> LaneConfig {
    match self {
        SessionLane::Control => LaneConfig::new(1, 4),
        SessionLane::Query => LaneConfig::new(2, 16),     // ✅ 提高到 16
        SessionLane::Execute => LaneConfig::new(1, 4),    // ✅ 提高到 4
        SessionLane::Generate => LaneConfig::new(1, 2),   // ✅ 提高到 2
    }
}
```

**优点**:
- 简单直接，只需修改一处代码
- 允许更高的并发度
- 用户设置的 `query_concurrency` 可以生效

**缺点**:
- 可能增加资源消耗
- 需要测试验证最佳值

### 方案 2: 应用用户配置到 LaneConfig

**问题**: 用户通过 `SessionQueueConfig` 设置的并发度没有应用到底层 LaneConfig

**解决方案**: 在构建 QueueManager 时使用用户配置覆盖默认值

```rust
// 修改 session_lane_queue.rs:305-333
async fn build_queue_manager(
    config: &SessionQueueConfig,
) -> Result<(QueueManager, Option<QueueMetrics>)> {
    let emitter = EventEmitter::new(100);
    let mut builder = QueueManagerBuilder::new(emitter);
    let default_timeout = config.default_timeout_ms.map(Duration::from_millis);
    let default_retry = Some(RetryPolicy::exponential(3));

    for lane in [
        SessionLane::Control,
        SessionLane::Query,
        SessionLane::Execute,
        SessionLane::Generate,
    ] {
        let mut cfg = lane.lane_config();

        // ✅ 应用用户配置的并发度
        let max_concurrency = match lane {
            SessionLane::Control => config.control_max_concurrency,
            SessionLane::Query => config.query_max_concurrency,
            SessionLane::Execute => config.execute_max_concurrency,
            SessionLane::Generate => config.generate_max_concurrency,
        };

        // ✅ 重新创建 LaneConfig 使用用户设置的并发度
        cfg = LaneConfig::new(1, max_concurrency);

        if let Some(timeout) = default_timeout {
            cfg = cfg.with_timeout(timeout);
        }
        if let Some(ref retry) = default_retry {
            cfg = cfg.with_retry_policy(retry.clone());
        }
        if lane == SessionLane::Generate {
            cfg = cfg.with_rate_limit(RateLimitConfig::per_minute(60));
            cfg = cfg
                .with_priority_boost(PriorityBoostConfig::standard(Duration::from_secs(300)));
        }
        builder = builder.with_lane(lane.lane_id(), cfg, lane.lane_priority());
    }

    // ... rest of the code
}
```

**优点**:
- 用户配置真正生效
- 灵活性高，用户可以根据需求调整
- 向后兼容

**缺点**:
- 需要修改构建逻辑
- 稍微复杂一些

### 方案 3: 减少队列开销

**问题**: 每个任务都要经过多层包装和异步转换

**解决方案**: 优化 SessionCommandAdapter 和任务提交流程

```rust
// 当前流程 (session_lane_queue.rs:382-421)
pub async fn submit(
    &self,
    lane: SessionLane,
    command: Box<dyn SessionCommand>,
) -> oneshot::Receiver<Result<Value>> {
    let (result_tx, result_rx) = oneshot::channel();
    let handler_config = self.get_lane_handler(lane).await;  // ❌ 异步读锁
    let task_id = uuid::Uuid::new_v4().to_string();          // ❌ UUID 生成开销
    let adapter = SessionCommandAdapter::new(/* ... */);      // ❌ 包装层

    match self.manager.submit(lane.lane_id(), Box::new(adapter)).await {
        Ok(lane_rx) => {
            tokio::spawn(async move {  // ❌ 额外的 spawn
                match lane_rx.await {
                    Ok(Ok(value)) => {
                        let _ = result_tx.send(Ok(value));
                    }
                    // ...
                }
            });
        }
        // ...
    }
    result_rx
}

// 优化后流程
pub async fn submit(
    &self,
    lane: SessionLane,
    command: Box<dyn SessionCommand>,
) -> oneshot::Receiver<Result<Value>> {
    let (result_tx, result_rx) = oneshot::channel();

    // ✅ 缓存 handler_config，避免每次读锁
    let handler_config = self.lane_handlers_cache.get(&lane).cloned()
        .unwrap_or_default();

    // ✅ 使用更快的 ID 生成
    let task_id = self.next_task_id.fetch_add(1, Ordering::Relaxed).to_string();

    // ✅ 对于 Internal 模式，直接执行，跳过包装
    if handler_config.mode == TaskHandlerMode::Internal {
        let manager = Arc::clone(&self.manager);
        let lane_id = lane.lane_id();
        tokio::spawn(async move {
            match manager.submit(lane_id, command).await {
                Ok(lane_rx) => {
                    if let Ok(Ok(value)) = lane_rx.await {
                        let _ = result_tx.send(Ok(value));
                    }
                }
                Err(e) => {
                    let _ = result_tx.send(Err(e.into()));
                }
            }
        });
    } else {
        // External/Hybrid 模式才使用 adapter
        let adapter = SessionCommandAdapter::new(/* ... */);
        // ...
    }

    result_rx
}
```

**优点**:
- 减少不必要的开销
- 对于 Internal 模式（最常见）优化最明显
- 保持功能完整性

**缺点**:
- 代码复杂度增加
- 需要仔细测试确保正确性

### 方案 4: 智能队列启用策略

**问题**: 对于快速操作，队列开销大于收益

**解决方案**: 根据操作类型和数量智能决定是否使用队列

```rust
// 在 agent.rs 中添加智能判断
async fn should_use_parallel_execution(&self, query_tools: &[ToolCall]) -> bool {
    // 如果没有队列，直接返回 false
    if self.command_queue.is_none() {
        return false;
    }

    // 如果工具数量太少（< 3），顺序执行更快
    if query_tools.len() < 3 {
        return false;
    }

    // 如果都是快速操作（glob, ls），顺序执行更快
    let all_fast_ops = query_tools.iter().all(|t| {
        matches!(t.name.as_str(), "glob" | "ls" | "list_files")
    });
    if all_fast_ops {
        return false;
    }

    // 其他情况使用并行执行
    true
}

// 修改 agent.rs:1090-1117
let (query_tools, sequential_tools) = if self.command_queue.is_some() {
    partition_by_lane(&tool_calls)
} else {
    (Vec::new(), tool_calls.clone())
};

// ✅ 智能判断是否使用并行执行
if !query_tools.is_empty() && self.should_use_parallel_execution(&query_tools).await {
    if let Some(queue) = &self.command_queue {
        let parallel_count = self
            .execute_query_tools_parallel(
                &query_tools,
                queue,
                &mut messages,
                &event_tx,
                &mut augmented_system,
                session_id,
            )
            .await;
        tool_calls_count += parallel_count;
    }
} else {
    // 顺序执行所有工具
    for tool_call in tool_calls {
        // ...
    }
}
```

**优点**:
- 自动选择最优执行策略
- 避免不必要的队列开销
- 用户无需手动配置

**缺点**:
- 增加判断逻辑复杂度
- 可能需要调优阈值

### 方案 5: 批量提交优化

**问题**: 每个工具单独提交到队列，增加开销

**解决方案**: 批量提交多个工具到队列

```rust
// 在 SessionLaneQueue 中添加批量提交方法
pub async fn submit_batch(
    &self,
    lane: SessionLane,
    commands: Vec<Box<dyn SessionCommand>>,
) -> Vec<oneshot::Receiver<Result<Value>>> {
    let handler_config = self.get_lane_handler(lane).await;
    let mut receivers = Vec::with_capacity(commands.len());

    // ✅ 批量创建 adapter 和提交
    for command in commands {
        let (result_tx, result_rx) = oneshot::channel();
        let task_id = uuid::Uuid::new_v4().to_string();
        let adapter = SessionCommandAdapter::new(
            command,
            task_id,
            handler_config.mode,
            self.session_id.clone(),
            lane,
            handler_config.timeout_ms,
            Arc::clone(&self.external_tasks),
            self.event_tx.clone(),
        );

        // 立即提交，不等待
        if let Ok(lane_rx) = self.manager.submit(lane.lane_id(), Box::new(adapter)).await {
            let tx = result_tx;
            tokio::spawn(async move {
                if let Ok(Ok(value)) = lane_rx.await {
                    let _ = tx.send(Ok(value));
                }
            });
        }

        receivers.push(result_rx);
    }

    receivers
}

// 在 agent.rs 中使用批量提交
async fn execute_query_tools_parallel(
    &self,
    query_tools: &[ToolCall],
    queue: &SessionLaneQueue,
    // ...
) -> usize {
    // ✅ 按 lane 分组
    let mut tools_by_lane: HashMap<SessionLane, Vec<ToolCall>> = HashMap::new();
    for tool in query_tools {
        let lane = SessionLane::from_tool_name(&tool.name);
        tools_by_lane.entry(lane).or_default().push(tool.clone());
    }

    // ✅ 批量提交每个 lane 的工具
    let mut all_receivers = Vec::new();
    for (lane, tools) in tools_by_lane {
        let commands: Vec<Box<dyn SessionCommand>> = tools
            .iter()
            .map(|t| create_command(t))
            .collect();
        let receivers = queue.submit_batch(lane, commands).await;
        all_receivers.extend(receivers);
    }

    // 等待所有结果
    let results = join_all(all_receivers).await;
    // ...
}
```

**优点**:
- 减少重复的锁获取和配置读取
- 更高效的批量处理
- 可以进一步优化为真正的批量 API

**缺点**:
- 需要重构现有代码
- 增加 API 复杂度

## 推荐实施顺序

### 阶段 1: 快速修复（立即实施）⭐

**方案 1 + 方案 2**: 提高默认并发度并应用用户配置

```rust
// 1. 修改 session_lane_queue.rs:49-56
fn lane_config(self) -> LaneConfig {
    match self {
        SessionLane::Control => LaneConfig::new(1, 4),
        SessionLane::Query => LaneConfig::new(2, 16),     // 4 → 16
        SessionLane::Execute => LaneConfig::new(1, 4),    // 2 → 4
        SessionLane::Generate => LaneConfig::new(1, 2),   // 1 → 2
    }
}

// 2. 修改 session_lane_queue.rs:305-333
async fn build_queue_manager(config: &SessionQueueConfig) -> Result<...> {
    // ...
    for lane in [...] {
        let max_concurrency = match lane {
            SessionLane::Control => config.control_max_concurrency,
            SessionLane::Query => config.query_max_concurrency,
            SessionLane::Execute => config.execute_max_concurrency,
            SessionLane::Generate => config.generate_max_concurrency,
        };
        let mut cfg = LaneConfig::new(1, max_concurrency);
        // ...
    }
}
```

**预期效果**:
- 用户设置 `query_concurrency=10` 时，真正能有 10 个并发
- 默认并发度提高，更好地利用并行能力

### 阶段 2: 性能优化（短期）

**方案 3**: 减少队列开销

- 缓存 handler_config
- 优化 task_id 生成
- Internal 模式跳过不必要的包装

**预期效果**:
- 减少 20-30% 的队列开销
- 提高整体吞吐量

### 阶段 3: 智能优化（中期）

**方案 4**: 智能队列启用策略

- 根据工具类型和数量自动选择执行策略
- 避免不必要的队列开销

**预期效果**:
- 快速操作自动使用顺序执行
- 重型操作自动使用并行执行
- 用户无需手动配置

### 阶段 4: 架构优化（长期）

**方案 5**: 批量提交优化

- 重构为批量 API
- 进一步减少开销

**预期效果**:
- 最大化并行效率
- 最小化协调开销

## 测试验证

### 测试场景

1. **轻量级操作** (glob, ls)
   - 工具数量: 3, 5, 10
   - 预期: 3 个以下顺序更快，5 个以上并行更快

2. **中等操作** (grep 小文件)
   - 工具数量: 3, 5, 10
   - 预期: 3 个以上并行明显更快

3. **重型操作** (read 大文件, grep 大代码库)
   - 工具数量: 3, 5, 10
   - 预期: 并行显著更快 (2-3x)

### 性能指标

- **Speedup**: 并行执行时间 / 顺序执行时间
- **Efficiency**: Speedup / 并发度
- **Overhead**: 队列开销占总时间的百分比

### 目标

- 轻量级操作 (3+ 工具): Speedup > 1.2x
- 中等操作 (3+ 工具): Speedup > 1.5x
- 重型操作 (3+ 工具): Speedup > 2.0x
- 队列开销: < 10% 总时间

## 总结

### 核心问题

1. **默认并发度过低**: Query lane 只有 4 并发
2. **用户配置未生效**: SessionQueueConfig 的设置没有应用到 LaneConfig
3. **队列开销过大**: 对于快速操作，开销大于收益

### 推荐方案

**立即实施**: 方案 1 + 方案 2（提高并发度 + 应用用户配置）
- 简单直接，影响最大
- 预期可以看到明显改善

**后续优化**: 方案 3 → 方案 4 → 方案 5
- 逐步减少开销
- 提高智能化程度
- 最大化性能

### 预期效果

实施方案 1 + 2 后：
- 轻量级操作: 可能仍然略慢，但差距缩小
- 中等操作: 接近持平或略快
- 重型操作: 明显加速 (1.5-2x)

实施所有方案后：
- 轻量级操作: 自动选择顺序执行，无性能损失
- 中等操作: 1.5-2x 加速
- 重型操作: 2-3x 加速
