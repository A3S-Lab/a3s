# SafeClaw + Power 本地隐私推理集成方案

## 需求分析

**目标**：在资源有限的用户机器上，通过 tee-minimal 进行本地数据隐私处理（至少 10B 模型）

**约束**：
- 用户机器资源有限（可能 8-16GB RAM，无独显或仅集显）
- 需要支持 10B 模型（Q4_K_M 量化后约 6-7GB）
- 不能依赖云端推理（数据不离开本地）
- 大部分用户机器无 TEE 硬件（AMD SEV-SNP / Intel TDX）

**Power tee-minimal 特性**：
- **picolm backend**：纯 Rust layer-streaming GGUF 推理
- **峰值内存 = O(layer_size)**：逐层加载，不是 O(model_size)
- **512MB EPC 可运行 7B 模型**：适合 TEE 内存受限环境
- **零 C 依赖**：完全可审计的供应链

---

## 方案设计：三层渐进式隐私保护

### Level 1：本地推理（基础隐私）
**适用场景**：所有用户机器（无 TEE 硬件要求）
**隐私保证**：数据不离开本地，无网络传输

```
SafeClaw (Tauri app)
  ├── Frontend (React)
  ├── Gateway (port 18790) ──┐
  └── Power (embedded, port 11435) ←┘
       └── picolm backend (layer-streaming)
```

**实现**：
1. SafeClaw Tauri app 嵌入 Power server（进程内）
2. Power 配置 `tee_mode = false`，使用 picolm backend
3. 模型存储在 `~/.a3s/power/models/`（用户本地）
4. SafeClaw agent 配置 `base_url = "http://127.0.0.1:11435/v1"`

**内存优化**：
- picolm layer-streaming：10B Q4_K_M 模型峰值内存 ~3GB（vs 全加载 7GB）
- 适合 8GB RAM 机器（系统 2GB + 浏览器 2GB + Power 3GB + 余量 1GB）

---

### Level 2：内存加密 + 日志脱敏（中等隐私）
**适用场景**：用户机器 + 本地推理
**隐私保证**：Level 1 + 推理后内存清零 + 日志不记录敏感内容

```
SafeClaw (Tauri app)
  └── Power (embedded)
       ├── tee_mode = true (软件模拟)
       ├── redact_logs = true
       ├── in_memory_decrypt = true
       └── zeroize after inference
```

**实现**：
1. Power 配置 `tee_mode = true`（即使无硬件 TEE）
2. 启用 `redact_logs`：日志中脱敏 prompt/response
3. 启用 `in_memory_decrypt`：模型加密存储，推理时内存解密
4. 推理后自动 `zeroize` 内存（防止内存 dump 泄露）

**模型加密**：
```bash
# 用户首次下载模型时，Power 自动加密存储
# AES-256-GCM，密钥存储在系统 keychain（macOS Keychain / Linux Secret Service）
```

---

### Level 3：TEE 硬件隔离（最高隐私）
**适用场景**：支持 AMD SEV-SNP / Intel TDX 的机器
**隐私保证**：Level 2 + 硬件加密内存 + 远程证明

```
SafeClaw (Tauri app, host)
  └── Gateway (port 18790)
       └── HTTP ──→ vsock ──→ Power (Box MicroVM, TEE)
                                └── picolm (512MB EPC)
```

**实现**：
1. Power 运行在 a3s-box MicroVM 内（AMD SEV-SNP / Intel TDX）
2. SafeClaw 通过 vsock 连接 Power（`vsock://3:11435`）
3. Power 配置 `tee_mode = true` + `vsock_port = 11435`
4. 远程证明：客户端验证 Power 运行在真实 TEE 内

**EPC 内存路由**（AMD SEV-SNP）：
- 检测 EPC 大小（通常 512MB - 8GB）
- 小于 1GB：强制 picolm（layer-streaming）
- 大于 1GB：可选 mistralrs（全加载，更快）

---

## 实现步骤

### Step 1：嵌入 Power（Level 1）

#### 1.1 添加依赖

```toml
# apps/safeclaw/src-tauri/Cargo.toml
[dependencies]
a3s-power = { path = "../../../../crates/power", default-features = false, features = ["picolm", "tls"] }
```

**feature 选择**：
- `picolm`：纯 Rust layer-streaming（必选，资源受限环境）
- `tls`：HTTPS 支持（可选，本地推理不需要）
- **不选** `mistralrs`：避免大依赖（candle + safetensors）
- **不选** `vsock`：Level 1 不需要 MicroVM

#### 1.2 启动 Power server

```rust
// apps/safeclaw/src-tauri/src/power.rs
use a3s_power::{config::PowerConfig, server};
use std::path::PathBuf;

pub async fn start_embedded_power() -> anyhow::Result<()> {
    let data_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("safeclaw")
        .join("power");

    let config = PowerConfig {
        host: "127.0.0.1".to_string(),
        port: 11435,  // 避免和 Ollama (11434) 冲突
        data_dir,
        max_loaded_models: 1,  // 资源受限，只保留 1 个模型
        keep_alive: "5m".to_string(),
        use_mlock: false,  // 不锁定内存（避免 OOM）
        num_parallel: 1,   // 单并发（资源受限）
        tee_mode: false,   // Level 1：无 TEE
        redact_logs: false,
        ..Default::default()
    };

    tracing::info!("Starting embedded Power server on 127.0.0.1:11435");
    server::start(config).await?;
    Ok(())
}
```

#### 1.3 Tauri setup 集成

```rust
// apps/safeclaw/src-tauri/src/lib.rs
mod power;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // ... existing setup ...

    tauri::Builder::default()
        .setup(|app| {
            // ... existing setup ...

            // Spawn embedded Power server
            tauri::async_runtime::spawn(async {
                if let Err(e) = power::start_embedded_power().await {
                    tracing::error!("Embedded Power server failed: {e:#}");
                }
            });

            // Spawn embedded SafeClaw gateway
            tauri::async_runtime::spawn(async {
                if let Err(e) = server::start_embedded_gateway().await {
                    tracing::error!("Embedded gateway failed: {e:#}");
                }
            });

            Ok(())
        })
        // ...
}
```

#### 1.4 前端配置

```typescript
// apps/safeclaw/src/models/settings.model.ts
export const DEFAULT_PROVIDERS = [
  {
    name: "local-power",
    displayName: "本地推理 (Power)",
    baseUrl: "http://127.0.0.1:11435/v1",
    models: ["qwen2.5:7b-q4", "llama3.2:3b-q4"],  // 预设模型
  },
  // ... existing providers
];
```

---

### Step 2：模型下载 UI（Level 1）

#### 2.1 Power 模型管理 API

Power 已提供 OpenAI 兼容 API + 模型管理：
- `GET /v1/models` — 列出已下载模型
- `POST /api/pull` — 下载模型（HuggingFace Hub）
- `DELETE /api/models/:name` — 删除模型

#### 2.2 SafeClaw 前端集成

```tsx
// apps/safeclaw/src/pages/settings/components/power-models-section.tsx
import { useState, useEffect } from "react";

interface PowerModel {
  name: string;
  size: number;
  modified_at: string;
}

export function PowerModelsSection() {
  const [models, setModels] = useState<PowerModel[]>([]);
  const [pulling, setPulling] = useState<string | null>(null);

  useEffect(() => {
    fetch("http://127.0.0.1:11435/v1/models")
      .then(r => r.json())
      .then(data => setModels(data.data || []));
  }, []);

  const pullModel = async (name: string) => {
    setPulling(name);
    try {
      const res = await fetch("http://127.0.0.1:11435/api/pull", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      // SSE stream: {"status": "downloading", "completed": 1024, "total": 4096}
      const reader = res.body?.getReader();
      // ... handle progress
    } finally {
      setPulling(null);
    }
  };

  return (
    <div>
      <h3>本地模型</h3>
      <ul>
        {models.map(m => (
          <li key={m.name}>{m.name} ({(m.size / 1e9).toFixed(1)} GB)</li>
        ))}
      </ul>
      <button onClick={() => pullModel("qwen2.5:7b-q4")}>
        下载 Qwen2.5 7B (Q4_K_M, 4.2GB)
      </button>
    </div>
  );
}
```

---

### Step 3：内存加密（Level 2）

#### 3.1 配置更新

```rust
// apps/safeclaw/src-tauri/src/power.rs
pub async fn start_embedded_power() -> anyhow::Result<()> {
    let config = PowerConfig {
        // ... Level 1 config ...
        tee_mode: true,           // 启用 TEE 模式（软件模拟）
        redact_logs: true,        // 日志脱敏
        in_memory_decrypt: true,  // 内存解密
        suppress_token_metrics: true,  // 不记录 token 统计
        ..Default::default()
    };
    // ...
}
```

#### 3.2 模型加密存储

Power 自动处理：
- 首次下载模型时，自动 AES-256-GCM 加密
- 密钥存储在系统 keychain（macOS Keychain / Linux Secret Service）
- 推理时内存解密，推理后 `zeroize`

---

### Step 4：TEE 硬件隔离（Level 3）

#### 4.1 检测 TEE 硬件

```rust
// apps/safeclaw/src-tauri/src/power.rs
use a3s_power::tee::attestation::{DefaultTeeProvider, TeeProvider};

fn detect_tee() -> Option<String> {
    let provider = DefaultTeeProvider::detect();
    let tee_type = provider.tee_type();
    if tee_type == "none" {
        None
    } else {
        Some(tee_type.to_string())
    }
}

pub async fn start_embedded_power() -> anyhow::Result<()> {
    let tee_available = detect_tee();
    tracing::info!("TEE detection: {:?}", tee_available);

    let config = if tee_available.is_some() {
        // Level 3: TEE 硬件隔离
        PowerConfig {
            tee_mode: true,
            vsock_port: Some(11435),  // vsock 监听
            // ... other config
        }
    } else {
        // Level 2: 软件模拟
        PowerConfig {
            tee_mode: true,
            port: 11435,  // TCP 监听
            // ... other config
        }
    };
    // ...
}
```

#### 4.2 Box MicroVM 集成（可选）

如果用户机器支持 TEE，SafeClaw 可以：
1. 通过 `a3s-box` SDK 启动 MicroVM
2. Power 运行在 MicroVM 内（AMD SEV-SNP / Intel TDX）
3. SafeClaw 通过 vsock 连接（`vsock://3:11435`）

```rust
// apps/safeclaw/src-tauri/src/power.rs (Level 3)
use a3s_box_sdk::BoxClient;

async fn start_power_in_microvm() -> anyhow::Result<()> {
    let client = BoxClient::new("http://127.0.0.1:8080")?;

    // 创建 MicroVM
    let vm = client.create_box(CreateBoxRequest {
        image: "a3s-power:latest".to_string(),
        memory_mb: 1024,  // 1GB RAM（picolm layer-streaming）
        vcpus: 2,
        tee_type: Some("sev-snp".to_string()),
        ..Default::default()
    }).await?;

    tracing::info!("Power running in MicroVM: {}", vm.id);
    Ok(())
}
```

---

## 资源需求对比

| 配置 | 模型 | 峰值内存 | 推理速度 | 隐私级别 |
|------|------|----------|----------|----------|
| **Level 1 (picolm)** | Qwen2.5 7B Q4 | ~3GB | 15 tok/s | 本地推理 |
| **Level 2 (picolm + 加密)** | Qwen2.5 7B Q4 | ~3GB | 14 tok/s | + 内存加密 |
| **Level 3 (MicroVM + TEE)** | Qwen2.5 7B Q4 | ~3GB | 12 tok/s | + 硬件隔离 |
| **对比：mistralrs 全加载** | Qwen2.5 7B Q4 | ~7GB | 25 tok/s | - |

**10B 模型支持**：
- Qwen2.5 10B Q4_K_M：~6GB 文件，picolm 峰值内存 ~4GB
- 适合 12GB+ RAM 机器

---

## 推荐实施路径

### Phase 1（MVP）：Level 1 本地推理
**时间**：1-2 天
**交付**：
- ✅ SafeClaw 嵌入 Power (picolm)
- ✅ 前端模型下载 UI
- ✅ 支持 Qwen2.5 7B / Llama3.2 3B

### Phase 2：Level 2 内存加密
**时间**：1 天
**交付**：
- ✅ 启用 `tee_mode` + `redact_logs`
- ✅ 模型加密存储
- ✅ 推理后内存清零

### Phase 3（可选）：Level 3 TEE 硬件
**时间**：3-5 天
**交付**：
- ✅ TEE 硬件检测
- ✅ Box MicroVM 集成
- ✅ vsock 通信
- ✅ 远程证明 UI

---

## 用户体验流程

### 首次使用
1. 用户打开 SafeClaw → Settings → 本地推理
2. 点击"下载 Qwen2.5 7B (4.2GB)"
3. 进度条显示下载进度（HuggingFace Hub）
4. 下载完成后自动加密存储
5. 创建 agent session，选择 `local-power/qwen2.5:7b-q4`
6. 开始对话（本地推理，数据不离开机器）

### 隐私提示
```
🛡️ 本地推理模式
✓ 数据不离开您的设备
✓ 推理后自动清除内存
✓ 日志不记录敏感内容
[检测到 AMD SEV-SNP] 升级到硬件隔离模式？
```

---

## 技术风险与缓解

### 风险 1：picolm 推理速度慢
**影响**：15 tok/s vs mistralrs 25 tok/s
**缓解**：
- 用户可选：资源充足时切换到 mistralrs
- 优化 picolm matmul（SIMD / Rayon 并行）

### 风险 2：模型下载失败（HuggingFace 被墙）
**影响**：国内用户无法下载模型
**缓解**：
- 提供国内镜像（ModelScope / HF Mirror）
- 支持本地导入 GGUF 文件

### 风险 3：Tauri 沙箱权限
**影响**：Power 无法访问 `~/.a3s/power/models/`
**缓解**：
- `capabilities/default.json` 添加文件系统权限
- 或使用 Tauri 的 `fs` API 代理访问

---

## 当前进展（已开工）

已落地（MVP 起步）：
1. **Tauri 侧嵌入 Power 服务**：启动时后台拉起本地 `127.0.0.1:11435`。
2. **资源分档自动配置**：按内存探测选择 `Minimal / Balanced / HighMemory`，自动调节并发、线程与 keep-alive。
3. **默认隐私基线**：开启 `tee_mode + redact_logs + in_memory_decrypt + streaming_decrypt`。
4. **前端默认 provider**：内置 `local-power`，默认 `Qwen2.5 7B Q3_K_M`（ModelScope 可直拉，兼容大多数机器），可切换到 Q4。
5. **TEE 状态可视化**：设置页显示运行档位、内存档位与 `SEV-SNP/TDX/模拟` 状态，并自动对齐内置 Power URL。
6. **本地模型导入**：设置页支持本地 `GGUF` 文件选择并注册到 Power（`POST /v1/models`）。
7. **默认 layer-stream 后端**：SafeClaw 默认使用 `picolm`（layer-streaming）作为本地推理后端。
8. **本地模型诊断按钮**：设置页支持“一键诊断”本地模型健康与速度（延迟 + tok/s）。

会话策略：
- 常规 Session 默认优先使用用户配置的云端 Provider。
- 当出现权限确认（敏感工具调用）时，Session 自动切换到 `local-power`；完成后恢复常规 Provider。
- 敏感工具策略支持在设置页选择预设（开发机/生产机/严格模式）并可自定义规则。
- 会话状态栏支持一键开关“本地隐私 ON/OFF”，可对当前 Session 强制启用本地模型。

后续待办：
1. 完成 8GB/12GB/16GB 三档基准测试并固化阈值。
2. 增加硬件 TEE 远程证明校验与可视化详情（Level 3）。
