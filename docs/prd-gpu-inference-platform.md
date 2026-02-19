# A3S GPU 推理平台 — 产品需求文档

> 版本: 1.0 | 日期: 2026-02-19 | 作者: A3S Team

---

## 1. 概述

### 1.1 问题陈述

AI 团队部署 GPU 推理服务时，需要手动选择 GPU 硬件型号、计算显存需求、配置张量并行、编写 K8s 清单、搭建负载均衡、制定弹性扩缩容策略。这些基础设施工作与他们的核心目标——跑一个模型、拿到一个 API 端点——毫无关系。

### 1.2 产品愿景

构建一个 GPU 推理平台，A3S OS 发送一个 JSON 格式的模型声明，平台自动完成一切基础设施工作，返回可用的推理端点和 API Key，并允许 A3S OS 实时监控部署过程。

```
A3S OS                                   本平台交付:

A3sfile → 解析 → JSON ──POST──→         endpoint: https://qwen-72b.a3s.internal/v1
{                                        apiKey:   sk-a3s-xxxx
  "name": "qwen-72b",                   status:   running
  "model": "Qwen/Qwen2.5-72B-Instruct", 日志/事件: WebSocket 实时推送
  "engine": "vllm"
}
```

### 1.3 设计原则

**原则一：用户声明要什么模型，平台决定怎么跑。**

- 用户不指定 GPU 型号、数量、显存、节点
- 用户不编写 K8s Deployment、Service、HPA
- 用户不配置张量并行、显存分配
- 用户不搭建负载均衡和扩缩容规则

**原则二：平台不感知 A3sfile。**

- 平台不解析 A3sfile（HCL 格式）
- 平台通过 HTTP API 接收预解析的 JSON 数据
- A3sfile 解析是 A3S OS 的职责
- 平台开发团队不需要了解 HCL 语法

### 1.4 系统边界

```
┌──────────────────────────┐      ┌──────────────────────────────────┐
│   A3S OS (上游调用方)      │      │   GPU 推理平台 (本文档范围)        │
│                           │      │                                  │
│  A3sfile 解析 → JSON      │      │  接收: JSON                      │
│  CLI / UI / SDK           │      │  返回: endpoint + apiKey         │
│                           │      │  推送: WebSocket 事件/日志        │
│  产出 JSON ──────────────────→   │                                  │
│                           │      │  职责:                           │
│  监听 WebSocket ←════════════    │  - 模型资源自动推算               │
│                           │      │  - GPU 调度                      │
│  查询/销毁/扩缩 ─────────────→   │  - 自动部署                      │
│                           │      │  - 负载均衡                      │
│                           │      │  - 弹性扩缩容                    │
│                           │      │  - API Key 管理                  │
│                           │      │  - 实时事件/日志推送              │
└──────────────────────────┘      └──────────────────────────────────┘
```

## 2. 输入规范

### 2.1 模型部署请求 (JSON)

```json
{
  "models": [
    {
      "name": "qwen-72b",
      "model": "Qwen/Qwen2.5-72B-Instruct",
      "engine": "vllm",
      "quantization": "awq",
      "max_model_len": 32768,
      "replicas": 1,
      "extra_args": {}
    }
  ],
  "namespace": "team-ai",
  "metadata": {
    "userId": "user-001",
    "teamId": "team-001",
    "source": "cli"
  }
}
```

### 2.2 字段说明

| 字段 | 类型 | 必填 | 默认值 | 说明 |
|------|------|------|--------|------|
| `models` | 数组 | 是 | — | 模型声明列表 |
| `models[].name` | string | 是 | — | 模型名称（本次请求内唯一） |
| `models[].model` | string | 是 | — | HuggingFace 模型 ID 或模型仓库路径 |
| `models[].engine` | enum | 否 | `"vllm"` | 推理引擎：`vllm` / `tgi` / `sglang` / `ollama` |
| `models[].quantization` | enum | 否 | `"none"` | 量化方式：`awq` / `gptq` / `fp8` / `none` |
| `models[].max_model_len` | number | 否 | 自动检测 | 最大序列长度 |
| `models[].replicas` | number | 否 | `1` | 初始副本数 |
| `models[].extra_args` | object | 否 | `{}` | 引擎特定参数（高级用户逃生舱） |
| `namespace` | string | 否 | 自动生成 | 目标命名空间 |
| `metadata` | object | 否 | `{}` | 调用方元数据 |

### 2.3 请求示例

**最简请求**：
```json
{
  "models": [
    { "name": "qwen-72b", "model": "Qwen/Qwen2.5-72B-Instruct" }
  ]
}
```

**多模型请求**：
```json
{
  "models": [
    { "name": "qwen-72b", "model": "Qwen/Qwen2.5-72B-Instruct", "engine": "vllm", "quantization": "awq" },
    { "name": "embedding", "model": "BAAI/bge-large-zh-v1.5", "engine": "tgi" }
  ]
}
```

## 3. 平台能力

### 3.1 核心能力总览

| # | 能力 | 说明 |
|---|------|------|
| 1 | 模型资源自动推算 | 根据模型 ID + 量化方式，自动推算显存、GPU 数量、张量并行度 |
| 2 | GPU 智能调度 | 根据推算结果，自动匹配集群中最优的 GPU 节点 |
| 3 | 推理服务自动部署 | 根据引擎类型，自动生成并部署推理服务，返回端点和 API Key |
| 4 | 实时事件与日志推送 | 部署全过程通过 WebSocket 实时推送给 A3S OS |
| 5 | 部署生命周期管理 | 查询、销毁、扩缩容 |
| 6 | 负载均衡 | 多副本场景下，智能路由推理请求 |
| 7 | 弹性扩缩容 | 根据 GPU 利用率和推理队列深度自动扩缩容 |
| 8 | GPU 集群可观测性 | 查询集群 GPU 拓扑、容量、利用率 |

### 3.2 能力 1：模型资源自动推算

A3S OS 只告诉平台"我要 Qwen-72B，AWQ 量化"。平台自动推算：

```
输入: model = "Qwen/Qwen2.5-72B-Instruct", quantization = "awq"

平台推算:
  → 参数量: 72B
  → AWQ 4-bit 量化后显存: 约 40GB
  → 加上 KV Cache + overhead: 约 50GB
  → 需要: 1× A100-80GB, 张量并行 = 1
  → 引擎镜像: vllm/vllm-openai:latest
  → 启动参数: --model Qwen/Qwen2.5-72B-Instruct --quantization awq
```

**推算策略**：
1. 查内置知识库（常见模型硬编码资源画像）
2. 未命中 → 从 HuggingFace 获取参数量，按公式推算
3. 用户 `extra_args` 可覆盖任何自动推算值

**首批支持的模型**：

| 模型 | 参数量 | FP16 显存 | AWQ 显存 | 推荐配置 |
|------|--------|----------|---------|---------|
| Qwen2.5-72B | 72B | ~144GB | ~40GB | 2×A100-80(FP16) / 1×A100-80(AWQ) |
| Qwen2.5-32B | 32B | ~64GB | ~18GB | 1×A100-80(FP16) / 1×L40S(AWQ) |
| Qwen2.5-7B | 7B | ~14GB | ~4GB | 1×L40S(FP16) / 1×T4(AWQ) |
| Llama-3.1-70B | 70B | ~140GB | ~38GB | 2×A100-80(FP16) / 1×A100-80(AWQ) |
| Llama-3.1-8B | 8B | ~16GB | ~5GB | 1×L40S(FP16) / 1×T4(AWQ) |
| DeepSeek-V3 | 671B(MoE) | ~400GB | ~120GB | 8×A100-80(FP16) / 4×A100-80(AWQ) |
| BGE-large-zh | 0.3B | ~1GB | — | 1×T4 |

### 3.3 能力 2：GPU 智能调度

平台根据推算结果，自动在集群中找到最优节点：

**调度流程**：
1. 发现集群所有 GPU 节点及其型号、显存、可用数量
2. 过滤：型号匹配、显存满足、可用数量 ≥ 需求
3. 打分：优先填满已有节点（减少碎片）
4. 绑定：输出调度结果

**不可调度处理**：
- 返回明确原因，例如："需要 2× A100-80GB，但集群所有 A100 节点 GPU 已满"
- 通过 WebSocket 推送失败事件，附带建议（如"使用 AWQ 量化可减少 GPU 需求"）

### 3.4 能力 3：推理服务自动部署

平台根据引擎类型自动部署推理服务：

| 引擎 | 说明 | 对外接口 |
|------|------|---------|
| vLLM | 高性能推理，支持 PagedAttention | OpenAI 兼容 API |
| TGI | HuggingFace 推理引擎 | OpenAI 兼容 API |
| SGLang | 高性能推理，支持 RadixAttention | OpenAI 兼容 API |
| Ollama | 轻量级推理，适合小模型 | OpenAI 兼容 API |

所有引擎对外暴露统一的 OpenAI 兼容 API（`/v1/chat/completions`、`/v1/models`）。

### 3.5 能力 4：实时事件与日志推送

部署全过程通过 WebSocket 实时推送给 A3S OS（详见第 5 章）。

### 3.6 能力 5：负载均衡

同一模型多副本时，平台自动路由推理请求：

| 策略 | 说明 |
|------|------|
| 最短队列 (least-queue) | 路由到推理队列最短的实例（默认） |
| 最低负载 (least-load) | 路由到 GPU 利用率最低的实例 |
| 算力加权 (weighted) | 按 GPU 算力加权轮询（A100 权重 > L40S） |

### 3.7 能力 6：弹性扩缩容

平台根据运行时指标自动扩缩容：

| 指标 | 扩容条件 | 缩容条件 |
|------|---------|---------|
| GPU 利用率 | > 80% | < 20% |
| 推理队列深度 | > 10 | = 0 持续 5 分钟 |
| 请求延迟 P99 | > 2s | < 500ms |

扩缩容规则：
- 扩容冷却: 60 秒
- 缩容冷却: 5 分钟（GPU 实例启动慢，缩容要保守）
- 下限: 1 副本（GPU 冷启动太慢，不缩到零）
