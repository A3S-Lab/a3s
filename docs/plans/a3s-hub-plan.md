# A3S Hub — 设计规划

## 项目信息

| 项 | 值 |
|----|-----|
| GitHub 仓库 | `git@github.com:A3S-Lab/Hub.git` |
| Submodule 路径 | `crates/hub` |
| Crate 名称 | `a3s-hub` |
| Lib 名称 | `a3s_hub` |

## 定位

`a3s-hub` 是 A3S 生态的统一内容仓库层，基于 S3 兼容存储引擎（默认 Garage），提供内容寻址存储 + 元数据索引，支撑四类仓库：GEP 仓库、Skills 仓库、文件仓库、代码仓库。

在 A3S 架构中的位置：

```
a3s-gateway (Ingress)
  └── a3s-box (VM Runtime)
      ├── SafeClaw (Security)
      └── Your Agent (a3s-code + a3s-lane)
              │
              ▼
          a3s-hub ◄── NEW: 统一内容仓库
              │
              ▼
          S3 Backend (Garage / MinIO / AWS S3 / LocalFS)
```

---

## 第一性原理设计：极简内核 + 外部扩展

### 内核（4 个组件）

| 组件 | 职责 | 为什么必须是内核 |
|------|------|-----------------|
| `ObjectStore` | 内容寻址存储（SHA-256 → blob），put/get/delete/exists | 所有仓库的底层基础，不可替换 |
| `MetadataStore` | 元数据 CRUD + 查询（标签、类型、关系、评分） | 所有仓库都需要元数据，不可替换 |
| `VersionChain` | 版本链管理（parent → child 演化关系） | GEP 和 Code 仓库的核心需求，跨仓库共用 |
| `Registry` | 仓库注册表，管理不同 Repository 扩展的生命周期 | 统一入口，协调多仓库 |

### 扩展点（trait，可替换）

| 扩展 trait | 默认实现 | 职责 |
|-----------|---------|------|
| `StorageBackend` | `GarageBackend` | S3 兼容存储引擎抽象，可切换为 MinIO / AWS S3 / LocalFS |
| `MetadataBackend` | `SqliteMetadata` | 元数据持久化，可切换为 PostgreSQL / 内存 |
| `Repository` | — | 仓库扩展基础 trait |
| `GepRepository` | `DefaultGepRepository` | Gene/Capsule 发布、获取、GDI 评分 |
| `SkillRepository` | `DefaultSkillRepository` | 技能注册、依赖解析、安装 |
| `FileRepository` | `DefaultFileRepository` | 通用文件存储、流式上传下载 |
| `CodeRepository` | `DefaultCodeRepository` | 代码片段存储、版本管理 |
| `Scorer` | `GdiScorer` | 资产评分策略（GDI 或自定义） |
| `AccessControl` | `DefaultAccessControl` | 访问控制策略 |

---

## 项目结构

```
crates/hub/
├── Cargo.toml
├── README.md
├── justfile
├── src/
│   ├── lib.rs                    # 公开 API 入口
│   ├── error.rs                  # HubError 统一错误类型
│   │
│   ├── core/                     # 内核（4 组件）
│   │   ├── mod.rs
│   │   ├── object_store.rs       # ObjectStore — 内容寻址存储
│   │   ├── metadata_store.rs     # MetadataStore — 元数据管理
│   │   ├── version_chain.rs      # VersionChain — 版本链
│   │   └── registry.rs           # Registry — 仓库注册表
│   │
│   ├── backend/                  # 存储后端扩展
│   │   ├── mod.rs
│   │   ├── traits.rs             # StorageBackend + MetadataBackend trait
│   │   ├── garage.rs             # GarageBackend（默认 S3 后端）
│   │   ├── local.rs              # LocalBackend（本地文件系统，开发用）
│   │   └── sqlite.rs             # SqliteMetadata（默认元数据后端）
│   │
│   ├── repository/               # 仓库扩展
│   │   ├── mod.rs
│   │   ├── traits.rs             # Repository 基础 trait
│   │   ├── gep.rs                # GepRepository — Gene/Capsule
│   │   ├── skill.rs              # SkillRepository — 技能管理
│   │   ├── file.rs               # FileRepository — 通用文件
│   │   └── code.rs               # CodeRepository — 代码片段
│   │
│   ├── scoring/                  # 评分扩展
│   │   ├── mod.rs
│   │   └── gdi.rs                # GDI 评分实现
│   │
│   └── access/                   # 访问控制扩展
│       ├── mod.rs
│       └── default.rs            # 默认访问控制
│
└── tests/
    ├── object_store_test.rs
    ├── metadata_test.rs
    ├── version_chain_test.rs
    ├── gep_repository_test.rs
    ├── skill_repository_test.rs
    └── integration_test.rs
```

---

## 核心 Trait 设计

### StorageBackend（S3 引擎抽象）

```rust
/// S3 兼容存储引擎抽象
/// 默认实现：GarageBackend
/// 可替换为：MinIO / AWS S3 / LocalFS / RustFS
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn put(&self, bucket: &str, key: &str, data: Bytes) -> Result<()>;
    async fn get(&self, bucket: &str, key: &str) -> Result<Bytes>;
    async fn delete(&self, bucket: &str, key: &str) -> Result<()>;
    async fn exists(&self, bucket: &str, key: &str) -> Result<bool>;
    async fn list(&self, bucket: &str, prefix: &str) -> Result<Vec<String>>;
    async fn head(&self, bucket: &str, key: &str) -> Result<ObjectMeta>;

    /// 流式上传（大文件）
    async fn put_stream(
        &self, bucket: &str, key: &str, stream: BoxStream<Bytes>,
    ) -> Result<()>;

    /// 流式下载（大文件）
    async fn get_stream(
        &self, bucket: &str, key: &str,
    ) -> Result<BoxStream<Bytes>>;

    /// 创建 bucket
    async fn create_bucket(&self, bucket: &str) -> Result<()>;

    /// 健康检查
    async fn health(&self) -> Result<()>;
}
```

### MetadataBackend（元数据持久化抽象）

```rust
#[async_trait]
pub trait MetadataBackend: Send + Sync {
    async fn put_metadata(&self, id: &ContentId, meta: &Metadata) -> Result<()>;
    async fn get_metadata(&self, id: &ContentId) -> Result<Option<Metadata>>;
    async fn delete_metadata(&self, id: &ContentId) -> Result<()>;
    async fn query(&self, filter: &MetadataFilter) -> Result<Vec<(ContentId, Metadata)>>;
    async fn health(&self) -> Result<()>;
}
```

### Repository（仓库扩展基础）

```rust
#[async_trait]
pub trait Repository: Send + Sync {
    /// 仓库类型标识
    fn kind(&self) -> &str;

    /// 仓库使用的 bucket 名称
    fn bucket(&self) -> &str;

    /// 初始化（创建 bucket 等）
    async fn init(&self) -> Result<()>;

    /// 健康检查
    async fn health(&self) -> Result<()>;
}
```

---

## 核心数据模型

```rust
/// 内容寻址 ID（SHA-256）
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentId(pub String);  // hex-encoded SHA-256

/// 对象元数据
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub content_id: ContentId,
    pub kind: AssetKind,          // Gene / Capsule / Skill / File / Code
    pub name: String,
    pub version: String,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub parent: Option<ContentId>, // 版本链：从哪个版本演化来
    pub size_bytes: u64,
    pub content_type: String,
    pub score: Option<f64>,        // GDI 评分
    pub extra: HashMap<String, Value>, // 仓库特定的扩展字段
}

/// 资产类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AssetKind {
    Gene,       // GEP 策略模板
    Capsule,    // GEP 验证方案
    Skill,      // Agent 技能
    File,       // 通用文件
    Code,       // 代码片段
}

/// 元数据查询过滤器
#[derive(Clone, Debug, Default)]
pub struct MetadataFilter {
    pub kind: Option<AssetKind>,
    pub tags: Option<Vec<String>>,
    pub author: Option<String>,
    pub name_prefix: Option<String>,
    pub min_score: Option<f64>,
    pub parent: Option<ContentId>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
```

---

## GEP 仓库特定模型

```rust
/// Gene — 可复用策略模板
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Gene {
    pub content_id: ContentId,
    pub name: String,
    pub description: String,
    pub preconditions: Vec<String>,     // 前置条件
    pub validation_commands: Vec<String>, // 验证命令
    pub strategy: String,               // 策略内容
    pub tags: Vec<String>,
    pub author: String,
}

/// Capsule — 验证过的修复方案
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Capsule {
    pub content_id: ContentId,
    pub gene_id: ContentId,             // 来源 Gene
    pub confidence: f64,                // 置信度
    pub environment: EnvironmentFingerprint, // 环境指纹
    pub patch: String,                  // 修复内容
    pub validation_result: ValidationResult,
}

/// GDI 评分（Global Desirability Index）
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GdiScore {
    pub intrinsic_quality: f64,   // 35%
    pub usage_metrics: f64,       // 30%
    pub social_signals: f64,      // 20%
    pub freshness: f64,           // 15%
    pub composite: f64,           // 加权总分
}
```

---

## Hub 入口（Builder 模式）

```rust
/// Hub 是统一入口，通过 Builder 组装内核 + 扩展
pub struct Hub {
    object_store: ObjectStore,
    metadata_store: MetadataStore,
    version_chain: VersionChain,
    registry: Registry,
}

impl Hub {
    pub fn builder() -> HubBuilder {
        HubBuilder::default()
    }

    /// 获取指定类型的仓库
    pub fn repository<T: Repository>(&self) -> Option<&T> { ... }

    /// 健康检查
    pub async fn health(&self) -> Result<HealthReport> { ... }
}

/// Builder — 组装 Hub
pub struct HubBuilder {
    storage_backend: Option<Box<dyn StorageBackend>>,
    metadata_backend: Option<Box<dyn MetadataBackend>>,
    repositories: Vec<Box<dyn Repository>>,
    scorer: Option<Box<dyn Scorer>>,
    access_control: Option<Box<dyn AccessControl>>,
}

impl HubBuilder {
    /// 设置 S3 存储后端（默认 Garage）
    pub fn storage_backend(mut self, backend: impl StorageBackend + 'static) -> Self;

    /// 设置元数据后端（默认 SQLite）
    pub fn metadata_backend(mut self, backend: impl MetadataBackend + 'static) -> Self;

    /// 注册仓库扩展
    pub fn repository(mut self, repo: impl Repository + 'static) -> Self;

    /// 构建 Hub（使用默认值填充未设置的组件）
    pub async fn build(self) -> Result<Hub>;
}
```

### 使用示例

```rust
// 最简用法 — 全部使用默认值（Garage + SQLite + 四个仓库）
let hub = Hub::builder()
    .build()
    .await?;

// 自定义存储后端 — 切换到 MinIO
let hub = Hub::builder()
    .storage_backend(S3Backend::new("http://minio:9000", "key", "secret"))
    .build()
    .await?;

// 开发模式 — 本地文件系统，零外部依赖
let hub = Hub::builder()
    .storage_backend(LocalBackend::new("/tmp/hub"))
    .metadata_backend(MemoryMetadata::new())
    .build()
    .await?;

// 只注册需要的仓库
let hub = Hub::builder()
    .repository(DefaultGepRepository::new())
    .repository(DefaultSkillRepository::new())
    .build()
    .await?;
```

---

## HCL 配置

```hcl
hub {
  storage {
    backend = "garage"  # garage | s3 | local

    garage {
      endpoint = "http://127.0.0.1:3900"
      access_key = "GK..."
      secret_key = "..."
      region = "garage"
    }
  }

  metadata {
    backend = "sqlite"  # sqlite | memory
    sqlite {
      path = "hub.db"
    }
  }

  repositories = ["gep", "skill", "file", "code"]

  scoring {
    algorithm = "gdi"
    weights {
      intrinsic_quality = 0.35
      usage_metrics     = 0.30
      social_signals    = 0.20
      freshness         = 0.15
    }
  }
}
```

---

## 关键依赖

```toml
[dependencies]
aws-sdk-s3 = "1"          # S3 客户端（对接 Garage/MinIO/AWS）
rusqlite = "0.31"          # SQLite 元数据
sha2 = "0.10"              # SHA-256 内容寻址
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
hcl-rs = "0.18"            # HCL 配置解析
thiserror = "2"
bytes = "1"
futures = "0.3"
tracing = "0.1"
```

---

## 实现阶段

### Phase 1：内核 + 存储后端
- [ ] 项目脚手架（Cargo.toml、目录结构、error.rs）
- [ ] `StorageBackend` trait + `LocalBackend` 实现
- [ ] `MetadataBackend` trait + `SqliteMetadata` 实现
- [ ] `ObjectStore`（内容寻址：SHA-256 计算 → StorageBackend 存取）
- [ ] `MetadataStore`（元数据 CRUD → MetadataBackend）
- [ ] `VersionChain`（parent/child 关系管理）
- [ ] `Registry` + `Hub` builder
- [ ] 单元测试（基于 LocalBackend）

### Phase 2：Garage 后端 + GEP 仓库
- [ ] `GarageBackend`（aws-sdk-s3 对接 Garage）
- [ ] `GepRepository`（Gene/Capsule CRUD）
- [ ] `GdiScorer`（GDI 评分计算）
- [ ] HCL 配置解析
- [ ] 集成测试（需要 Garage 实例）

### Phase 3：Skills + File + Code 仓库
- [ ] `SkillRepository`（技能注册、依赖解析）
- [ ] `FileRepository`（通用文件、流式上传下载）
- [ ] `CodeRepository`（代码片段、版本管理）
- [ ] `AccessControl` 默认实现

### Phase 4：与 A3S 生态集成
- [ ] a3s-code 的 `SkillRegistry` 对接 Hub
- [ ] a3s-code 的 `MemoryStore` 对接 Hub
- [ ] a3s-event 事件通知（资产发布/更新时触发事件）

---

## 与现有 A3S 生态的关系

| 现有组件 | 与 Hub 的关系 |
|---------|-------------|
| a3s-code `SkillRegistry` | 可委托给 Hub 的 `SkillRepository` 做持久化和发现 |
| a3s-code `MemoryStore` | 可委托给 Hub 的 `FileRepository` 做持久化 |
| a3s-code `SessionStore` | 可委托给 Hub 的 `FileRepository` 做持久化 |
| a3s-lane `Storage` | 保持独立（队列存储和内容仓库是不同场景） |
| a3s-event | Hub 发布/更新资产时通过 Event 发送通知 |
| a3s-common | Hub 的共享类型（ContentId、AssetKind）未来可提升到 common |
