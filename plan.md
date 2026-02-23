# SafeClaw 后端完善：会话生命周期管理 + 多Agent协作

## 一、会话生命周期管理

### 1.1 活动追踪 (`engine.rs`)

在 `EngineSession` 中添加字段：
```rust
last_activity_at: u64,   // 最后活动时间戳（浏览器消息/generation）
```

每次 `handle_browser_message`、`spawn_generation`、`handle_browser_open` 时调用 `touch_activity()` 更新时间戳。

### 1.2 生命周期配置 (`config.rs`)

在 `SafeClawConfig` 中添加：
```hcl
session_lifecycle {
  idle_timeout_secs     = 1800    # 30 min 无活动自动归档
  purge_after_secs      = 604800  # 7 天归档后自动删除
  max_sessions          = 100     # 最大活跃会话数
  cleanup_interval_secs = 60      # 清理扫描间隔
}
```

### 1.3 后台清理任务 (`engine.rs`)

添加 `start_lifecycle_task(&self)` 方法，启动后台 tokio 任务：
- 每 `cleanup_interval_secs` 扫描所有 session
- 无浏览器连接 + 无活跃 generation + 超过 `idle_timeout_secs` → 自动归档
- 已归档超过 `purge_after_secs` → 硬删除
- 活跃会话数超过 `max_sessions` → 按 last_activity_at 排序，归档最旧的

### 1.4 优雅关闭 (`engine.rs`)

添加 `shutdown(&self)` 方法：
- 取消所有活跃 generation handle
- 持久化所有 session 状态到磁盘
- 在 `main.rs` 的 signal handler 中调用

### 1.5 会话生命周期 API (`handler.rs`)

- `POST /api/agent/sessions/:id/archive` — 归档
- `POST /api/agent/sessions/:id/unarchive` — 取消归档
- `GET /api/agent/stats` — 会话统计（活跃数、归档数、总 token 用量）

## 二、多Agent协作增强

### 2.1 消息协议增强 (`bus.rs` / `types.rs`)

扩展 `AgentMessagePayload`：
```rust
pub struct AgentMessagePayload {
    pub message_id: String,           // 唯一消息 ID
    pub from_session_id: String,
    pub topic: String,
    pub content: String,
    pub message_type: AgentMessageType,  // Chat / TaskRequest / TaskResponse
    pub reply_to: Option<String>,        // 回复哪条消息
    pub timestamp: u64,
}

pub enum AgentMessageType {
    Chat,          // 普通对话
    TaskRequest,   // 任务请求
    TaskResponse,  // 任务响应
}
```

### 2.2 投递可靠性 (`bus.rs`)

- 订阅循环断线自动重连（指数退避，最大 30s）
- auto-execute 速率限制：每 session 每分钟最多 30 条
- 投递失败日志 + 审计事件

### 2.3 Agent 发现 (`engine.rs` / `handler.rs`)

新增 `list_agent_directory()` 方法，返回所有可用 agent 信息：
```rust
pub struct AgentDirectoryEntry {
    pub session_id: String,
    pub persona_id: Option<String>,
    pub persona_name: Option<String>,
    pub status: String,           // active / busy / idle / archived
    pub auto_execute: bool,
}
```

API 端点：`GET /api/agent/directory`

## 三、实现顺序

1. `EngineSession` 添加 `last_activity_at` + `touch_activity()`
2. 生命周期配置结构体 (`SessionLifecycleConfig`)
3. 后台清理任务 (`start_lifecycle_task`)
4. 优雅关闭 (`shutdown`)
5. 会话生命周期 API（archive/unarchive/stats）
6. `AgentMessagePayload` 增强（message_id, type, reply_to, timestamp）
7. 订阅循环重连 + 速率限制
8. Agent 发现（directory）

## 四、涉及文件

- `crates/safeclaw/src/agent/engine.rs` — 生命周期任务、活动追踪、优雅关闭、agent 发现
- `crates/safeclaw/src/agent/bus.rs` — 消息协议增强、重连、速率限制
- `crates/safeclaw/src/agent/handler.rs` — 新 API 端点
- `crates/safeclaw/src/agent/types.rs` — 新类型
- `crates/safeclaw/src/agent/mod.rs` — 导出
- `crates/safeclaw/src/config.rs` — 生命周期配置
- `crates/safeclaw/src/main.rs` — 启动生命周期任务 + 优雅关闭
