# A3S 下一步开发计划 — 基于第一性原理

## 核心使命回顾

A3S 是一个 **Agent Operating System** — 为 AI Agent 提供声明、打包、部署、安全和演进的全栈平台。核心差异化：**隐私保护执行**（TEE 硬件加密 + VM 隔离）。

## 当前状态评估

### 已完成（成熟）
| 组件 | 版本 | 测试 | 状态 |
|------|------|------|------|
| a3s-code | 0.8.0 | 1,214 | ✅ Agent 框架，14 工具，技能系统，并行规划 |
| a3s-box | 0.5.2 | 1,466+ | ✅ MicroVM 运行时，52 命令 CLI，CRI，TEE 代码 |
| a3s-gateway | 0.1.0 | 872 | ✅ K8s Ingress，15 中间件，TLS/ACME |
| a3s-power | 0.2.0 | 554+ | ✅ LLM 推理，mistralrs + llama.cpp，TEE 层 |
| a3s-lane | 0.4.0 | 246 | ✅ 优先级队列，6 lanes，重试/DLQ |
| a3s-search | 0.8.0 | 401 | ✅ 元搜索引擎，9 引擎，共识排名 |
| a3s-event | 0.3.0 | 176+ | ✅ 事件系统，NATS JetStream，加密 |

### 未完成（关键缺口）
| 缺口 | 影响 | 紧迫性 |
|------|------|--------|
| SafeClaw 未集成测试 | 安全代理是核心差异化，但没有端到端验证 | 🔴 高 |
| apps/os 早期阶段 | 用户无法通过 UI 管理 Agent | 🟡 中 |
| TEE 真机验证 | 所有 TEE 代码是单元测试，未在 SEV-SNP 硬件上验证 | 🟡 中 |
| 组件间集成 | 各组件独立成熟，但全链路未验证 | 🔴 高 |

---

## 第一性原理分析

### 问题：A3S 的用户是谁？他们需要什么？

1. **开发者** — 想用 A3S 构建和部署 AI Agent
2. **企业** — 想在隐私保护环境中运行 AI Agent

### 问题：什么阻止了用户今天使用 A3S？

答案很清楚：**各组件独立存在，但没有一条完整的端到端路径让用户从零到运行一个 Agent**。

就像我们刚完成的 Box + Power 集成测试一样 — 在此之前，没有人验证过 Power 能在 Box VM 里真正运行推理。同样的问题存在于整个系统。

### 核心原则：先打通，再打磨

> 一条完整的路径 > 十个独立完美的组件

---

## 开发计划：三个阶段

### Phase 1: 全链路打通（4-6 周）

**目标：一个开发者可以在本地机器上，从零开始，声明并运行一个 AI Agent**

#### 1.1 SafeClaw + Code + Power 集成（最高优先级）

这是 A3S 的核心价值链：

```
用户消息 → SafeClaw（安全分类）→ Code（Agent 执行）→ Power（LLM 推理）→ 响应
```

具体任务：
- [ ] SafeClaw 在 Box VM 内启动，通过 vsock 接收请求
- [ ] SafeClaw 调用本地 a3s-code Agent 处理消息
- [ ] Code Agent 调用 Power 进行 LLM 推理
- [ ] 端到端集成测试：发送消息 → 收到 AI 回复

#### 1.2 A3sfile — 声明式 Agent 拓扑

用户需要一个简单的方式声明 Agent：

```hcl
# agent.a3s
agent "my-assistant" {
  model  = "qwen2.5:0.5b"

  channel "webchat" {
    port = 8080
  }

  security {
    privacy_level = "sensitive"
  }
}
```

具体任务：
- [ ] 定义 A3sfile HCL schema
- [ ] 实现 A3sfile 解析器（SafeClaw 或独立 crate）
- [ ] `a3s-box run --a3sfile agent.a3s` 一键启动

#### 1.3 最小可用 CLI 体验

```bash
# 开发者的完整体验
a3s init my-agent          # 生成 A3sfile 模板
a3s up                     # 启动 VM + SafeClaw + Code + Power
a3s chat "Hello"           # 发送消息，收到回复
a3s logs                   # 查看 Agent 日志
a3s down                   # 停止
```

### Phase 2: 安全与可观测性（3-4 周）

**目标：证明隐私保护真正工作**

#### 2.1 隐私分级端到端验证

```
Normal 消息 → 普通处理
Sensitive 消息 → 日志脱敏 + 内存清零
Critical 消息 → TEE 模式（如果硬件可用）
```

- [ ] SafeClaw 隐私分类器集成测试
- [ ] Power 日志脱敏验证（敏感数据不出现在日志中）
- [ ] Box TEE 模拟模式端到端测试

#### 2.2 可观测性链路

- [ ] OpenTelemetry trace 从 Gateway → SafeClaw → Code → Power 全链路
- [ ] Prometheus metrics 聚合（每个组件已有 metrics，需要统一 dashboard）
- [ ] 审计日志：谁在什么时候问了什么，Agent 做了什么

#### 2.3 TEE 真机验证（如果有硬件）

- [ ] 在 AMD SEV-SNP 机器上运行 Box + SafeClaw + Power
- [ ] 验证 attestation report 生成
- [ ] 验证内存加密确实生效

### Phase 3: 平台化（4-6 周）

**目标：从单机开发到多机部署**

#### 3.1 apps/os — 管理平台

- [ ] Agent 生命周期管理 API（创建、启动、停止、删除）
- [ ] 基础 Web UI（Agent 列表、日志查看、聊天测试）
- [ ] 多 Agent 编排（A3sfile 中声明多个 Agent 协作）

#### 3.2 K8s 部署

- [ ] Helm chart 整合（Gateway + Box CRI + Power）
- [ ] Knative-style 自动扩缩容
- [ ] 多租户隔离

#### 3.3 多渠道接入

- [ ] WebChat 前端（apps/safeclaw-ui）
- [ ] Telegram Bot 集成验证
- [ ] Feishu/DingTalk 企业场景验证

---

## 建议的下一步行动

**立即开始 Phase 1.1** — SafeClaw + Code + Power 集成。

理由：
1. 这是 A3S 的核心价值主张（隐私保护的 AI Agent）
2. 我们刚验证了 Box + Power 可以工作，下一步自然是加入 SafeClaw 和 Code
3. 没有这条链路，其他一切都是空中楼阁
4. 这也是最能吸引早期用户的 demo

具体第一步：创建 `crates/safeclaw/examples/box_integration.rs`，类似我们刚完成的 Power 集成测试，但这次是完整链路：Box VM 内运行 SafeClaw → Code → Power。
