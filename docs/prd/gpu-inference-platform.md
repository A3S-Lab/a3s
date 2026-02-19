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
| 5 | 异构 GPU 混合推理 | 自动处理不同型号 GPU 的调度、张量并行适配、算力加权路由，用户完全不感知 |
| 6 | 负载均衡 | 多副本场景下，按算力加权智能路由推理请求 |
| 7 | 弹性扩缩容 | 根据用户实际访问流量（队列深度、延迟、QPS）自动扩缩容，无需人工干预 |
| 8 | 部署生命周期管理 | 查询、销毁、扩缩容 |
| 9 | GPU 集群可观测性 | 全局算力概览、节点拓扑、每张卡状态、容量分布、已部署模型汇总、历史指标 |

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

### 3.6 能力 5：异构 GPU 混合推理

集群中可能同时存在多种 GPU 型号（A100-80GB、A100-40GB、L40S-48GB、T4-16GB 等）。平台内部自动处理异构差异，用户完全不感知。

**平台内部自动决策**：

| 决策项 | 平台行为 | 用户感知 |
|--------|---------|---------|
| GPU 型号选择 | 根据模型显存需求，从可用 GPU 中选择最优型号 | 无 |
| 跨型号副本分布 | 同一模型的多副本可分布在不同型号 GPU 上 | 无 |
| 算力差异补偿 | 不同型号 GPU 算力不同，负载均衡按算力加权分配请求 | 无 |
| 自动降级 | 首选 GPU 不可用时，自动选择次优型号（如 A100 满 → 用 L40S） | 无 |
| 张量并行适配 | 不同型号显存不同，TP 策略自动调整（A100-80GB TP=1 vs A100-40GB TP=2） | 无 |

**异构调度示例**：

```
用户请求: model = "Qwen2.5-72B", quantization = "awq" (需要 ~40GB 显存)

集群状态:
  gpu-node-01: A100-80GB × 8 (可用 0 张) ← 满了
  gpu-node-02: A100-80GB × 8 (可用 2 张)
  gpu-node-03: L40S-48GB × 4  (可用 4 张)

平台决策:
  → 首选 A100-80GB (80GB > 40GB, 单卡即可, TP=1)
  → gpu-node-01 满了，跳过
  → gpu-node-02 可用，调度到 gpu-node-02 ✓
```

```
用户请求: model = "Qwen2.5-72B", quantization = "none" (需要 ~144GB 显存)

集群状态:
  gpu-node-01: A100-80GB × 8 (可用 0 张) ← 满了
  gpu-node-02: A100-80GB × 8 (可用 2 张)
  gpu-node-03: L40S-48GB × 4  (可用 4 张)

平台决策:
  → 需要 144GB，单卡不够
  → 方案 A: 2× A100-80GB (160GB, TP=2) → gpu-node-02 可用 ✓
  → 方案 B: 3× L40S-48GB (144GB, TP=3) → gpu-node-03 可用
  → 选择方案 A（A100 算力更强，TP=2 效率更高）
```

**异构负载均衡示例**：

```
同一模型 "qwen-72b" 有 2 个副本:
  副本 1: gpu-node-02 (A100-80GB) → 算力权重 1.0
  副本 2: gpu-node-03 (L40S-48GB) → 算力权重 0.6

请求分配比例:
  副本 1 承担 62.5% 的请求
  副本 2 承担 37.5% 的请求
```

用户看到的只是一个统一的端点 `https://qwen-72b.a3s.internal/v1`，背后的异构调度和加权路由完全透明。

### 3.7 能力 6：负载均衡

同一模型多副本时，平台自动路由推理请求：

| 策略 | 说明 |
|------|------|
| 最短队列 (least-queue) | 路由到推理队列最短的实例（默认） |
| 最低负载 (least-load) | 路由到 GPU 利用率最低的实例 |
| 算力加权 (weighted) | 按 GPU 算力加权轮询（A100 权重 > L40S），异构集群默认启用 |

### 3.8 能力 7：弹性扩缩容

平台根据用户实际访问流量自动扩缩容推理服务副本，无需任何人工干预：

- 流量增大 → 请求排队变长、延迟升高 → 自动扩容新副本
- 流量减少 → GPU 空闲、队列为空 → 自动缩容释放资源

**触发指标**（均从用户访问流量衍生）：

| 指标 | 含义 | 扩容条件 | 缩容条件 |
|------|------|---------|---------|
| 推理队列深度 | 用户请求排队数量 | > 10 | = 0 持续 5 分钟 |
| 请求延迟 P99 | 用户感知到的响应时间 | > 2s | < 500ms |
| GPU 利用率 | 用户请求导致的 GPU 负载 | > 80% | < 20% |
| QPS | 每秒用户请求数 | 超过单副本处理能力 | 远低于单副本处理能力 |

**扩缩容规则**：
- 扩容冷却: 60 秒
- 缩容冷却: 5 分钟（GPU 实例启动慢，缩容要保守）
- 下限: 1 副本（GPU 冷启动太慢，不缩到零）
- 扩容优先级: 推理队列深度 > 请求延迟 > GPU 利用率（用户体验优先）

## 4. 接口定义

### 4.1 HTTP API 总览

| 方法 | 路径 | 说明 | 优先级 |
|------|------|------|--------|
| `POST` | `/api/gpu/deploy` | 提交部署请求 | P0 |
| `GET` | `/api/gpu/deployments/:id` | 查询部署详情 | P0 |
| `GET` | `/api/gpu/deployments` | 列出所有部署 | P0 |
| `DELETE` | `/api/gpu/deployments/:id` | 销毁部署 | P0 |
| `POST` | `/api/gpu/deployments/:id/scale` | 手动扩缩容 | P1 |
| `GET` | `/api/gpu/deployments/:id/logs` | 获取历史日志 | P1 |
| `GET` | `/api/gpu/deployments/:id/metrics` | 获取运行时指标 | P1 |
| `GET` | `/api/gpu/dashboard` | 平台全局概览（算力池、负载、利用率） | P0 |
| `GET` | `/api/gpu/nodes` | 查询 GPU 节点列表（含每张卡详情） | P0 |
| `GET` | `/api/gpu/nodes/:nodeName` | 查询单个节点详情 | P0 |
| `GET` | `/api/gpu/capacity` | 查询集群 GPU 容量与型号分布 | P0 |
| `GET` | `/api/gpu/models` | 查询所有已部署模型汇总 | P0 |
| `POST` | `/api/gpu/deployments/:id/apikey/rotate` | 轮换 API Key | P2 |

---

### 4.2 提交部署

```
POST /api/gpu/deploy
```

**请求体**：见第 2 章输入规范。

**响应 202 Accepted**：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "status": "queued",
  "wsRoom": "gpu:gpu-a1b2c3d4",
  "createdAt": "2026-02-19T10:00:00Z"
}
```

A3S OS 拿到 `wsRoom` 后，通过 WebSocket 订阅该房间，实时接收部署事件。

---

### 4.3 查询部署详情

```
GET /api/gpu/deployments/:id
```

**响应 200**：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "status": "running",
  "namespace": "a3s-gpu-a1b2c3d4",
  "createdAt": "2026-02-19T10:00:00Z",
  "readyAt": "2026-02-19T10:03:45Z",
  "models": [
    {
      "name": "qwen-72b",
      "model": "Qwen/Qwen2.5-72B-Instruct",
      "engine": "vllm",
      "quantization": "awq",
      "status": "running",
      "endpoint": "https://qwen-72b.a3s.internal/v1",
      "apiKey": "sk-a3s-xxxxxxxxxxxx",
      "replicas": { "desired": 1, "ready": 1 },
      "gpu": {
        "type": "nvidia-a100-80gb",
        "count": 1,
        "tensorParallel": 1,
        "vramUsed": "40GB",
        "node": "gpu-node-03"
      }
    }
  ]
}
```

---

### 4.4 列出所有部署

```
GET /api/gpu/deployments?page=1&limit=20&status=running
```

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `page` | number | 页码，默认 1 |
| `limit` | number | 每页数量，默认 20 |
| `status` | string | 按状态过滤（可选） |
| `userId` | string | 按用户过滤（可选） |

**响应 200**：
```json
{
  "data": [ /* GpuDeployment[] */ ],
  "total": 42,
  "page": 1,
  "limit": 20
}
```

---

### 4.5 销毁部署

```
DELETE /api/gpu/deployments/:id
```

释放所有 GPU 资源，清理 K8s 中的 Pod、Service、Secret 等。

**响应 202 Accepted**：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "status": "destroying"
}
```

销毁过程通过 WebSocket 推送事件，销毁完成后状态变为 `destroyed`。

---

### 4.6 手动扩缩容

```
POST /api/gpu/deployments/:id/scale
```

**请求体**：
```json
{
  "modelName": "qwen-72b",
  "replicas": 3
}
```

**响应 202 Accepted**：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "modelName": "qwen-72b",
  "status": "scaling",
  "replicas": { "desired": 3, "ready": 1 }
}
```

---

### 4.7 获取历史日志

```
GET /api/gpu/deployments/:id/logs?limit=500&modelName=qwen-72b
```

**响应 200**：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "logs": [
    {
      "timestamp": "2026-02-19T10:02:30Z",
      "level": "INFO",
      "message": "Downloading model weights...",
      "source": "pod/qwen-72b-0"
    }
  ]
}
```

---

### 4.8 获取运行时指标

```
GET /api/gpu/deployments/:id/metrics?modelName=qwen-72b
```

**响应 200**：
```json
{
  "modelName": "qwen-72b",
  "gpu": {
    "utilization": 65,
    "vramUtilization": 50,
    "temperature": 72,
    "powerUsage": "250W"
  },
  "inference": {
    "qps": 12.5,
    "pendingRequests": 3,
    "latencyP50Ms": 120,
    "latencyP99Ms": 450
  },
  "replicas": { "desired": 1, "ready": 1 },
  "timestamp": "2026-02-19T10:15:00Z"
}
```

---

### 4.9 平台全局概览

```
GET /api/gpu/dashboard
```

A3S OS 通过此接口获取平台全局状态，用于仪表盘展示。

**响应 200**：
```json
{
  "cluster": {
    "totalNodes": 5,
    "activeNodes": 4,
    "faultNodes": 0,
    "maintenanceNodes": 1
  },
  "gpu": {
    "totalCards": 36,
    "availableCards": 14,
    "usedCards": 22,
    "utilizationPercent": 61.1,
    "totalVramGb": 2240,
    "usedVramGb": 1380,
    "availableVramGb": 860
  },
  "compute": {
    "totalTflopsFp16": 4480,
    "usedTflopsFp16": 2780,
    "availableTflopsFp16": 1700,
    "utilizationPercent": 62.1
  },
  "deployments": {
    "total": 12,
    "running": 8,
    "scaling": 1,
    "loading": 2,
    "failed": 1,
    "queued": 0
  },
  "models": {
    "totalDeployed": 10,
    "totalReplicas": 15,
    "totalInferenceQps": 156.3,
    "avgLatencyP99Ms": 380
  },
  "gpuTypeDistribution": [
    {
      "type": "nvidia-a100-80gb",
      "total": 16,
      "available": 4,
      "used": 12,
      "memoryPerCardGb": 80,
      "fp16Tflops": 312,
      "utilizationPercent": 75.0
    },
    {
      "type": "nvidia-a100-40gb",
      "total": 8,
      "available": 2,
      "used": 6,
      "memoryPerCardGb": 40,
      "fp16Tflops": 312,
      "utilizationPercent": 75.0
    },
    {
      "type": "nvidia-l40s-48gb",
      "total": 8,
      "available": 4,
      "used": 4,
      "memoryPerCardGb": 48,
      "fp16Tflops": 362,
      "utilizationPercent": 50.0
    },
    {
      "type": "nvidia-t4-16gb",
      "total": 4,
      "available": 4,
      "used": 0,
      "memoryPerCardGb": 16,
      "fp16Tflops": 65,
      "utilizationPercent": 0
    }
  ],
  "timestamp": "2026-02-19T10:15:00Z"
}
```

---

### 4.10 查询 GPU 节点列表

```
GET /api/gpu/nodes?status=active&gpuType=nvidia-a100-80gb
```

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `status` | string | 按状态过滤：`active` / `idle` / `fault` / `maintenance`（可选） |
| `gpuType` | string | 按 GPU 型号过滤（可选） |

**响应 200**：
```json
{
  "nodes": [
    {
      "nodeName": "gpu-node-01",
      "status": "active",
      "labels": {
        "topology.kubernetes.io/zone": "zone-a",
        "node.kubernetes.io/instance-type": "p4d.24xlarge"
      },
      "gpu": {
        "type": "nvidia-a100-80gb",
        "driver": "535.129.03",
        "cudaVersion": "12.2",
        "totalCards": 8,
        "availableCards": 2,
        "usedCards": 6,
        "memoryPerCardGb": 80,
        "totalMemoryGb": 640,
        "usedMemoryGb": 480,
        "availableMemoryGb": 160,
        "cards": [
          {
            "index": 0,
            "uuid": "GPU-a1b2c3d4-...",
            "status": "in_use",
            "temperature": 72,
            "powerUsageW": 280,
            "powerLimitW": 400,
            "gpuUtilizationPercent": 85,
            "memoryUtilizationPercent": 75,
            "usedMemoryGb": 60,
            "deployment": {
              "deploymentId": "gpu-a1b2c3d4",
              "modelName": "qwen-72b",
              "replicaIndex": 0
            }
          },
          {
            "index": 1,
            "uuid": "GPU-e5f6g7h8-...",
            "status": "in_use",
            "temperature": 68,
            "powerUsageW": 250,
            "powerLimitW": 400,
            "gpuUtilizationPercent": 72,
            "memoryUtilizationPercent": 62,
            "usedMemoryGb": 50,
            "deployment": {
              "deploymentId": "gpu-a1b2c3d4",
              "modelName": "qwen-72b",
              "replicaIndex": 0
            }
          },
          {
            "index": 6,
            "uuid": "GPU-m9n0o1p2-...",
            "status": "available",
            "temperature": 35,
            "powerUsageW": 25,
            "powerLimitW": 400,
            "gpuUtilizationPercent": 0,
            "memoryUtilizationPercent": 0,
            "usedMemoryGb": 0,
            "deployment": null
          }
        ]
      },
      "system": {
        "cpuCores": 96,
        "cpuUtilizationPercent": 35,
        "totalMemoryGb": 768,
        "usedMemoryGb": 320,
        "networkBandwidthGbps": 100,
        "nvlinkAvailable": true,
        "pcieGeneration": 4
      },
      "deployments": [
        {
          "deploymentId": "gpu-a1b2c3d4",
          "modelName": "qwen-72b",
          "gpuIndices": [0, 1],
          "tensorParallel": 2,
          "status": "running"
        },
        {
          "deploymentId": "gpu-e5f6g7h8",
          "modelName": "llama-8b",
          "gpuIndices": [2, 3, 4, 5],
          "tensorParallel": 1,
          "status": "running"
        }
      ],
      "uptime": "15d 6h 32m",
      "lastHeartbeat": "2026-02-19T10:14:55Z"
    }
  ],
  "total": 5
}
```

**GPU 卡状态说明**：

| 状态 | 说明 |
|------|------|
| `available` | 空闲可用 |
| `in_use` | 已分配给部署 |
| `fault` | 硬件故障（ECC 错误、温度过高等） |
| `reserved` | 已预留（扩容中，尚未绑定 Pod） |

---

### 4.11 查询单个节点详情

```
GET /api/gpu/nodes/:nodeName
```

返回与 4.10 列表中单个节点相同的数据结构，但额外包含历史指标：

**响应 200**（在 4.10 单节点基础上增加）：
```json
{
  "nodeName": "gpu-node-01",
  "...": "（同 4.10 节点完整字段）",
  "history": {
    "gpuUtilization": [
      { "timestamp": "2026-02-19T09:00:00Z", "value": 45 },
      { "timestamp": "2026-02-19T09:15:00Z", "value": 62 },
      { "timestamp": "2026-02-19T09:30:00Z", "value": 78 },
      { "timestamp": "2026-02-19T09:45:00Z", "value": 85 },
      { "timestamp": "2026-02-19T10:00:00Z", "value": 72 }
    ],
    "memoryUtilization": [
      { "timestamp": "2026-02-19T09:00:00Z", "value": 50 },
      { "timestamp": "2026-02-19T09:15:00Z", "value": 55 },
      { "timestamp": "2026-02-19T09:30:00Z", "value": 68 },
      { "timestamp": "2026-02-19T09:45:00Z", "value": 75 },
      { "timestamp": "2026-02-19T10:00:00Z", "value": 62 }
    ],
    "powerUsage": [
      { "timestamp": "2026-02-19T09:00:00Z", "value": 1200 },
      { "timestamp": "2026-02-19T10:00:00Z", "value": 1850 }
    ],
    "temperature": [
      { "timestamp": "2026-02-19T09:00:00Z", "avg": 45, "max": 52 },
      { "timestamp": "2026-02-19T10:00:00Z", "avg": 68, "max": 75 }
    ],
    "period": "1h",
    "interval": "15m"
  }
}
```

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `historyPeriod` | string | 历史数据时间范围：`1h` / `6h` / `24h` / `7d`，默认 `1h` |
| `historyInterval` | string | 数据点间隔：`5m` / `15m` / `1h`，默认自动 |

---

### 4.12 查询集群 GPU 容量与型号分布

```
GET /api/gpu/capacity
```

**响应 200**：
```json
{
  "summary": {
    "totalGpus": 36,
    "availableGpus": 14,
    "usedGpus": 22,
    "faultGpus": 0,
    "reservedGpus": 0,
    "utilizationPercent": 61.1,
    "totalVramGb": 2240,
    "usedVramGb": 1380,
    "availableVramGb": 860
  },
  "byType": [
    {
      "type": "nvidia-a100-80gb",
      "architecture": "Ampere",
      "memoryPerCardGb": 80,
      "fp16Tflops": 312,
      "int8Tops": 624,
      "nvlinkSupport": true,
      "total": 16,
      "available": 4,
      "used": 12,
      "fault": 0,
      "reserved": 0,
      "totalVramGb": 1280,
      "usedVramGb": 960,
      "availableVramGb": 320,
      "utilizationPercent": 75.0,
      "nodes": ["gpu-node-01", "gpu-node-02"],
      "canFit": {
        "qwen-72b-awq": 4,
        "qwen-72b-fp16": 2,
        "llama-70b-awq": 4,
        "qwen-7b-fp16": 4
      }
    },
    {
      "type": "nvidia-l40s-48gb",
      "architecture": "Ada Lovelace",
      "memoryPerCardGb": 48,
      "fp16Tflops": 362,
      "int8Tops": 724,
      "nvlinkSupport": false,
      "total": 8,
      "available": 4,
      "used": 4,
      "fault": 0,
      "reserved": 0,
      "totalVramGb": 384,
      "usedVramGb": 192,
      "availableVramGb": 192,
      "utilizationPercent": 50.0,
      "nodes": ["gpu-node-03", "gpu-node-04"],
      "canFit": {
        "qwen-72b-awq": 4,
        "qwen-32b-fp16": 2,
        "qwen-7b-fp16": 4
      }
    },
    {
      "type": "nvidia-t4-16gb",
      "architecture": "Turing",
      "memoryPerCardGb": 16,
      "fp16Tflops": 65,
      "int8Tops": 130,
      "nvlinkSupport": false,
      "total": 4,
      "available": 4,
      "used": 0,
      "fault": 0,
      "reserved": 0,
      "totalVramGb": 64,
      "usedVramGb": 0,
      "availableVramGb": 64,
      "utilizationPercent": 0,
      "nodes": ["gpu-node-05"],
      "canFit": {
        "qwen-7b-awq": 4,
        "llama-8b-awq": 4,
        "bge-large": 4
      }
    }
  ],
  "totalDeployments": 12,
  "totalRunningModels": 10,
  "totalReplicas": 15,
  "timestamp": "2026-02-19T10:15:00Z"
}
```

`canFit` 字段表示该型号剩余可用 GPU 还能部署多少个指定模型实例（基于当前可用资源的估算），帮助 A3S OS 在提交部署前预判资源是否充足。

---

### 4.13 查询所有已部署模型汇总

```
GET /api/gpu/models?status=running&page=1&limit=20
```

**查询参数**：

| 参数 | 类型 | 说明 |
|------|------|------|
| `status` | string | 按状态过滤：`running` / `loading` / `failed`（可选） |
| `engine` | string | 按引擎过滤：`vllm` / `tgi` / `sglang` / `ollama`（可选） |
| `page` | number | 页码，默认 1 |
| `limit` | number | 每页数量，默认 20 |

**响应 200**：
```json
{
  "data": [
    {
      "deploymentId": "gpu-a1b2c3d4",
      "modelName": "qwen-72b",
      "modelId": "Qwen/Qwen2.5-72B-Instruct",
      "engine": "vllm",
      "quantization": "awq",
      "status": "running",
      "endpoint": "https://qwen-72b.a3s.internal/v1",
      "createdAt": "2026-02-19T08:00:00Z",
      "readyAt": "2026-02-19T08:03:45Z",
      "replicas": {
        "desired": 2,
        "ready": 2,
        "details": [
          {
            "replicaIndex": 0,
            "node": "gpu-node-01",
            "gpuType": "nvidia-a100-80gb",
            "gpuCount": 1,
            "gpuIndices": [2],
            "tensorParallel": 1,
            "vramUsedGb": 42,
            "status": "running",
            "uptime": "2h 15m"
          },
          {
            "replicaIndex": 1,
            "node": "gpu-node-03",
            "gpuType": "nvidia-l40s-48gb",
            "gpuCount": 1,
            "gpuIndices": [0],
            "tensorParallel": 1,
            "vramUsedGb": 42,
            "status": "running",
            "uptime": "1h 30m"
          }
        ]
      },
      "inference": {
        "qps": 25.3,
        "pendingRequests": 2,
        "latencyP50Ms": 95,
        "latencyP99Ms": 320,
        "totalRequests": 18542,
        "totalTokensGenerated": 2850000
      },
      "scaling": {
        "mode": "auto",
        "minReplicas": 1,
        "maxReplicas": 5,
        "currentReplicas": 2,
        "lastScaleEvent": {
          "type": "scale_up",
          "from": 1,
          "to": 2,
          "reason": "推理队列深度 > 10",
          "timestamp": "2026-02-19T09:30:00Z"
        }
      },
      "loadBalancing": {
        "strategy": "weighted",
        "distribution": [
          { "replicaIndex": 0, "weight": 1.0, "requestPercent": 62.5 },
          { "replicaIndex": 1, "weight": 0.6, "requestPercent": 37.5 }
        ]
      },
      "metadata": {
        "userId": "user-001",
        "teamId": "team-001"
      }
    },
    {
      "deploymentId": "gpu-e5f6g7h8",
      "modelName": "embedding",
      "modelId": "BAAI/bge-large-zh-v1.5",
      "engine": "tgi",
      "quantization": "none",
      "status": "running",
      "endpoint": "https://embedding.a3s.internal/v1",
      "createdAt": "2026-02-19T07:00:00Z",
      "readyAt": "2026-02-19T07:01:20Z",
      "replicas": {
        "desired": 1,
        "ready": 1,
        "details": [
          {
            "replicaIndex": 0,
            "node": "gpu-node-05",
            "gpuType": "nvidia-t4-16gb",
            "gpuCount": 1,
            "gpuIndices": [0],
            "tensorParallel": 1,
            "vramUsedGb": 1.2,
            "status": "running",
            "uptime": "3h 15m"
          }
        ]
      },
      "inference": {
        "qps": 85.7,
        "pendingRequests": 0,
        "latencyP50Ms": 12,
        "latencyP99Ms": 45,
        "totalRequests": 125000,
        "totalTokensGenerated": 0
      },
      "scaling": {
        "mode": "auto",
        "minReplicas": 1,
        "maxReplicas": 3,
        "currentReplicas": 1,
        "lastScaleEvent": null
      },
      "loadBalancing": {
        "strategy": "least-queue",
        "distribution": [
          { "replicaIndex": 0, "weight": 1.0, "requestPercent": 100 }
        ]
      },
      "metadata": {
        "userId": "user-001",
        "teamId": "team-001"
      }
    }
  ],
  "total": 10,
  "page": 1,
  "limit": 20
}
```

## 5. WebSocket 实时推送协议

### 5.1 连接与订阅

```
连接地址:  ws://platform.a3s.internal/gpu
订阅部署:  emit('subscribe', { deploymentId: 'gpu-a1b2c3d4' })
取消订阅:  emit('unsubscribe', { deploymentId: 'gpu-a1b2c3d4' })
```

A3S OS 在调用 `POST /api/gpu/deploy` 后，使用返回的 `wsRoom` 订阅对应房间。

### 5.2 事件类型

| 事件名 | 说明 | 触发时机 |
|--------|------|---------|
| `deployment:phase` | 阶段转换 | 每个流水线阶段开始时 |
| `deployment:log` | Pod 日志 | 模型加载阶段，Pod stdout/stderr |
| `deployment:progress` | 进度更新 | 模型下载/加载过程中 |
| `deployment:complete` | 部署成功 | 所有模型就绪 |
| `deployment:failed` | 部署失败 | 任一阶段失败 |
| `deployment:scaling` | 扩缩容事件 | 自动或手动扩缩容触发 |
| `deployment:destroying` | 销毁进度 | 销毁过程中 |
| `deployment:destroyed` | 销毁完成 | 所有资源清理完毕 |
| `gpu:node:status` | 节点状态变化 | GPU 节点上线/下线/故障 |

### 5.3 事件载荷格式

**阶段转换** (`deployment:phase`)：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "phase": "resolving",
  "message": "Qwen2.5-72B (AWQ): 约 40GB 显存, 需要 1× A100-80GB",
  "timestamp": "2026-02-19T10:00:05Z"
}
```

**Pod 日志** (`deployment:log`)：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "level": "INFO",
  "message": "Downloading model weights: 45% (18.2GB / 40.5GB)",
  "source": "pod/qwen-72b-0",
  "timestamp": "2026-02-19T10:01:30Z"
}
```

**进度更新** (`deployment:progress`)：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "phase": "loading",
  "percent": 45,
  "eta": "2m30s",
  "timestamp": "2026-02-19T10:01:30Z"
}
```

**部署成功** (`deployment:complete`)：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "status": "running",
  "models": [
    {
      "name": "qwen-72b",
      "endpoint": "https://qwen-72b.a3s.internal/v1",
      "apiKey": "sk-a3s-xxxxxxxxxxxx",
      "gpu": {
        "type": "nvidia-a100-80gb",
        "count": 1,
        "tensorParallel": 1,
        "node": "gpu-node-03"
      }
    }
  ],
  "timestamp": "2026-02-19T10:03:45Z"
}
```

**部署失败** (`deployment:failed`)：
```json
{
  "deploymentId": "gpu-a1b2c3d4",
  "phase": "scheduling",
  "error": "GPU 资源不足",
  "reason": "需要 2× A100-80GB，但集群所有 A100 节点 GPU 已满",
  "suggestion": "等待其他部署释放资源，或使用 AWQ 量化减少 GPU 需求",
  "timestamp": "2026-02-19T10:00:15Z"
}
```

### 5.4 完整事件流示例

A3S OS 订阅后收到的完整事件流：

```
1. { event: "deployment:phase",    phase: "validating",   message: "校验请求参数..." }
2. { event: "deployment:phase",    phase: "resolving",    message: "推算 Qwen2.5-72B-Instruct 资源需求..." }
3. { event: "deployment:phase",    phase: "resolving",    message: "Qwen2.5-72B (AWQ): 约 40GB 显存, 需要 1× A100-80GB, TP=1" }
4. { event: "deployment:phase",    phase: "scheduling",   message: "匹配 GPU 节点..." }
5. { event: "deployment:phase",    phase: "scheduling",   message: "调度到 gpu-node-03: A100-80GB × 8, 可用 6 张" }
6. { event: "deployment:phase",    phase: "provisioning", message: "创建命名空间和密钥..." }
7. { event: "deployment:phase",    phase: "deploying",    message: "创建 vLLM 推理服务..." }
8. { event: "deployment:phase",    phase: "loading",      message: "等待模型加载..." }
9. { event: "deployment:log",      level: "INFO",         message: "Downloading model weights..." }
10.{ event: "deployment:progress", phase: "loading",      percent: 25, eta: "3m" }
11.{ event: "deployment:log",      level: "INFO",         message: "Loading model weights to GPU..." }
12.{ event: "deployment:progress", phase: "loading",      percent: 80, eta: "30s" }
13.{ event: "deployment:log",      level: "INFO",         message: "Model loaded, warming up..." }
14.{ event: "deployment:phase",    phase: "ready",        message: "部署完成!" }
15.{ event: "deployment:complete", endpoint: "https://...", apiKey: "sk-a3s-..." }
```

### 5.5 断线重连

- A3S OS 断线后重新连接并订阅同一 `deploymentId`
- 平台推送该部署的所有历史事件（从事件日志中读取）
- A3S OS 根据事件时间戳去重

## 6. 部署状态机

### 6.1 状态定义

| 状态 | 说明 |
|------|------|
| `queued` | 已入队，等待处理 |
| `validating` | 校验请求参数 |
| `resolving` | 推算模型资源需求 |
| `scheduling` | 匹配 GPU 节点 |
| `provisioning` | 创建 K8s 命名空间和密钥 |
| `deploying` | 创建推理服务 |
| `loading` | 模型权重加载中 |
| `ready` | 部署就绪（短暂状态，立即转为 running） |
| `running` | 正常运行中 |
| `scaling` | 扩缩容进行中 |
| `failed` | 部署失败 |
| `destroying` | 销毁进行中 |
| `destroyed` | 已销毁 |

### 6.2 状态转换

```
queued → validating → resolving → scheduling → provisioning → deploying → loading → ready → running
                                                                                              │
                                                                                              ├→ scaling → running
                                                                                              │
                                                                                              └→ destroying → destroyed

任一阶段均可 → failed
failed → destroying → destroyed（手动清理）
```

## 7. 错误处理

### 7.1 同步错误（校验阶段，直接返回 HTTP 错误）

| 错误 | HTTP 状态码 | 示例 |
|------|------------|------|
| JSON 格式错误 | 400 | `"models 字段不能为空"` |
| 模型名为空 | 400 | `"models[0].model 字段不能为空"` |
| 引擎不支持 | 400 | `"不支持的引擎: xxx，可选: vllm/tgi/sglang/ollama"` |
| 部署不存在 | 404 | `"部署 gpu-xxxx 不存在"` |
| 认证失败 | 401 | `"无效的 Bearer Token"` |

### 7.2 异步错误（通过 WebSocket `deployment:failed` 推送）

| 阶段 | 错误 | 建议 |
|------|------|------|
| resolving | 无法获取模型参数量 | "请检查模型 ID 是否正确" |
| scheduling | 集群无满足条件的 GPU | "当前无可用 A100-80GB，建议使用 AWQ 量化" |
| scheduling | GPU 资源不足 | "所有 GPU 已满，请等待其他部署释放" |
| deploying | K8s 配额不足 | "命名空间 GPU 配额已满" |
| loading | 模型下载超时 | "模型下载超时，请检查网络或使用本地模型" |
| loading | 模型加载 OOM | "显存不足，建议使用量化版本" |
| running | Pod CrashLoopBackOff | "推理服务崩溃，请查看日志" |

### 7.3 回滚

部署失败时，平台自动清理已创建的所有 K8s 资源（Namespace、Secret、Deployment、Service），清理完成后推送 `deployment:cleanup` 事件。

## 8. 非功能需求

| 类别 | 指标 | 目标 |
|------|------|------|
| 性能 | 提交部署 API 响应时间 | < 500ms |
| 性能 | 部署完成时间（不含模型下载） | < 5 分钟 |
| 性能 | WebSocket 事件延迟 | < 1 秒 |
| 性能 | 并发部署数 | ≥ 10 |
| 可靠性 | 部署成功率（资源充足时） | > 99% |
| 可靠性 | 断线重连历史事件补发 | 支持 |
| 可靠性 | 失败自动回滚 | 100% |
| 可靠性 | 平台故障不影响已部署服务 | 是 |
| 安全 | API Key 存储 | 加密存储，接口只返回一次 |
| 安全 | 接口认证 | Bearer Token |
| 安全 | 部署间隔离 | 独立 Namespace + NetworkPolicy |

## 9. 术语表

| 术语 | 说明 |
|------|------|
| A3S OS | A3S 操作系统平台，本平台的上游调用方 |
| A3sfile | HCL 格式的声明式配置文件（由 A3S OS 解析，本平台不感知） |
| ModelSpec | A3sfile 解析后的 JSON 模型声明（本平台的输入） |
| 张量并行 (TP) | 将模型权重切分到多张 GPU 卡上并行计算 |
| vLLM | 高性能 LLM 推理引擎，支持 PagedAttention |
| TGI | HuggingFace Text Generation Inference |
| SGLang | 高性能推理引擎，支持 RadixAttention |
| DCGM | NVIDIA Data Center GPU Manager，GPU 监控工具 |
| Bin-packing | 装箱算法，优先填满已有节点以减少碎片 |
| KV Cache | 推理时缓存的 Key-Value 张量，占用额外显存 |

---

> 本文档定义了 A3S GPU 推理平台的产品需求和接口规范。平台作为独立服务，接收 A3S OS 发送的 JSON 格式模型声明，自动完成资源推算、GPU 调度、推理服务部署、负载均衡和弹性扩缩容，并通过 HTTP API 和 WebSocket 向 A3S OS 提供部署管理和实时监控能力。
